//! Message queue module for asynchronous task processing
//!
//! Provides pub/sub messaging and task queue functionality using Redis

use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use redis::aio::Connection;
use redis::{AsyncCommands, Client};
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub mod handlers;
pub mod worker;

/// Queue configuration
#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub redis_url: String,
    pub worker_threads: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub task_timeout_seconds: u64,
    pub dead_letter_queue_enabled: bool,
    pub enabled: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            worker_threads: 4,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            task_timeout_seconds: 300, // 5 minutes
            dead_letter_queue_enabled: true,
            enabled: true,
        }
    }
}

/// Message types that can be published
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QueueMessage {
    /// Trade execution completed
    TradeExecuted {
        trade_id: String,
        wallet: String,
        market_id: String,
        amount: u64,
        outcome: u8,
        timestamp: DateTime<Utc>,
    },
    
    /// Market created
    MarketCreated {
        market_id: String,
        title: String,
        creator: String,
        timestamp: DateTime<Utc>,
    },
    
    /// Position closed
    PositionClosed {
        position_id: String,
        wallet: String,
        market_id: String,
        pnl: i64,
        timestamp: DateTime<Utc>,
    },
    
    /// Settlement completed
    SettlementCompleted {
        market_id: String,
        winning_outcome: u8,
        total_payout: u64,
        timestamp: DateTime<Utc>,
    },
    
    /// Risk alert
    RiskAlert {
        wallet: String,
        alert_type: String,
        severity: String,
        details: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    
    /// Cache invalidation request
    CacheInvalidation {
        patterns: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    
    /// Email notification
    EmailNotification {
        to: String,
        subject: String,
        body: String,
        priority: String,
    },
    
    /// Webhook delivery
    WebhookDelivery {
        url: String,
        payload: serde_json::Value,
        headers: std::collections::HashMap<String, String>,
        retry_count: u32,
    },
}

/// Task that can be queued for processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub id: String,
    pub message: QueueMessage,
    pub created_at: DateTime<Utc>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub execute_after: Option<DateTime<Utc>>,
}

impl QueueTask {
    pub fn new(message: QueueMessage) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            message,
            created_at: Utc::now(),
            attempts: 0,
            last_error: None,
            execute_after: None,
        }
    }
    
    pub fn with_delay(message: QueueMessage, delay_seconds: i64) -> Self {
        let mut task = Self::new(message);
        task.execute_after = Some(Utc::now() + chrono::Duration::seconds(delay_seconds));
        task
    }
}

/// Queue channels
pub struct QueueChannels;

impl QueueChannels {
    pub const TRADES: &'static str = "queue:trades";
    pub const MARKETS: &'static str = "queue:markets";
    pub const SETTLEMENTS: &'static str = "queue:settlements";
    pub const RISK_ALERTS: &'static str = "queue:risk_alerts";
    pub const NOTIFICATIONS: &'static str = "queue:notifications";
    pub const GENERAL: &'static str = "queue:general";
    pub const DEAD_LETTER: &'static str = "queue:dead_letter";
}

/// Message queue service
pub struct QueueService {
    client: Option<Client>,
    config: QueueConfig,
    stats: Arc<RwLock<QueueStats>>,
}

/// Queue statistics
#[derive(Debug, Default, Clone, Serialize)]
pub struct QueueStats {
    pub messages_published: u64,
    pub messages_consumed: u64,
    pub messages_failed: u64,
    pub messages_retried: u64,
    pub messages_dead_lettered: u64,
    pub active_workers: u32,
}

impl QueueService {
    /// Create new queue service
    pub async fn new(config: QueueConfig) -> Result<Self> {
        if !config.enabled {
            info!("Queue service disabled");
            return Ok(Self {
                client: None,
                config,
                stats: Arc::new(RwLock::new(QueueStats::default())),
            });
        }
        
        info!("Connecting to Redis queue at: {}", config.redis_url);
        
        let client = match Client::open(config.redis_url.clone()) {
            Ok(client) => {
                // Test connection
                match client.get_async_connection().await {
                    Ok(_) => {
                        info!("Successfully connected to Redis queue");
                        Some(client)
                    }
                    Err(e) => {
                        warn!("Failed to connect to Redis queue: {}. Queue disabled.", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create Redis client for queue: {}. Queue disabled.", e);
                None
            }
        };
        
        Ok(Self {
            client,
            config,
            stats: Arc::new(RwLock::new(QueueStats::default())),
        })
    }
    
    /// Publish a message to a channel
    pub async fn publish(&self, channel: &str, message: QueueMessage) -> Result<()> {
        if self.client.is_none() {
            return Ok(());
        }
        
        debug!("Publishing message to channel: {}", channel);
        
        let task = QueueTask::new(message);
        let serialized = serde_json::to_string(&task)?;
        
        let client = self.client.as_ref().unwrap();
        let mut conn = client.get_async_connection().await
            .context("Failed to get Redis connection")?;
        
        // Push to queue
        conn.lpush::<_, _, ()>(channel, &serialized).await
            .context("Failed to publish message")?;
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.messages_published += 1;
        
        debug!("Message published successfully to {}", channel);
        Ok(())
    }
    
    /// Publish a delayed message
    pub async fn publish_delayed(&self, channel: &str, message: QueueMessage, delay_seconds: i64) -> Result<()> {
        if self.client.is_none() {
            return Ok(());
        }
        
        debug!("Publishing delayed message to channel: {} (delay: {}s)", channel, delay_seconds);
        
        let task = QueueTask::with_delay(message, delay_seconds);
        let serialized = serde_json::to_string(&task)?;
        
        let client = self.client.as_ref().unwrap();
        let mut conn = client.get_async_connection().await
            .context("Failed to get Redis connection")?;
        
        // Add to sorted set with score as execute time
        let score = task.execute_after.unwrap().timestamp();
        conn.zadd::<_, _, _, ()>("queue:delayed", &serialized, score).await
            .context("Failed to publish delayed message")?;
        
        Ok(())
    }
    
    /// Subscribe to a channel for pub/sub
    pub async fn subscribe<F>(&self, channels: Vec<&str>, handler: F) -> Result<()>
    where
        F: Fn(String, QueueMessage) + Send + Sync + 'static,
    {
        if self.client.is_none() {
            return Ok(());
        }
        
        let client = self.client.as_ref().unwrap();
        let mut pubsub = client.get_async_connection().await
            .context("Failed to get Redis connection")?
            .into_pubsub();
        
        // Subscribe to channels
        for channel in &channels {
            pubsub.subscribe(channel).await
                .context(format!("Failed to subscribe to {}", channel))?;
        }
        
        info!("Subscribed to channels: {:?}", channels);
        
        // Listen for messages
        let mut pubsub_stream = pubsub.on_message();
        while let Some(msg) = pubsub_stream.next().await {
            let channel = msg.get_channel_name().to_string();
            let payload: String = msg.get_payload()?;
            
            match serde_json::from_str::<QueueTask>(&payload) {
                Ok(task) => {
                    handler(channel, task.message);
                }
                Err(e) => {
                    error!("Failed to deserialize message: {}", e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Consume tasks from a queue (blocking)
    pub async fn consume<F>(&self, queue: &str, handler: F) -> Result<()>
    where
        F: Fn(QueueTask) -> Result<()> + Send + Sync + 'static,
    {
        if self.client.is_none() {
            return Ok(());
        }
        
        let client = self.client.as_ref().unwrap();
        let stats = self.stats.clone();
        let config = self.config.clone();
        
        info!("Starting queue consumer for: {}", queue);
        
        loop {
            let mut conn = client.get_async_connection().await
                .context("Failed to get Redis connection")?;
            
            // Check delayed messages first
            self.process_delayed_messages(&mut conn).await?;
            
            // Block and wait for message
            let result: Option<(String, String)> = conn.brpop(queue, 5.0).await?;
            
            if let Some((_, payload)) = result {
                match serde_json::from_str::<QueueTask>(&payload) {
                    Ok(mut task) => {
                        // Check if task should be executed
                        if let Some(execute_after) = task.execute_after {
                            if execute_after > Utc::now() {
                                // Re-queue with delay
                                let serialized = serde_json::to_string(&task)?;
                                let score = execute_after.timestamp();
                                conn.zadd::<_, _, _, ()>("queue:delayed", &serialized, score).await?;
                                continue;
                            }
                        }
                        
                        // Process task
                        task.attempts += 1;
                        
                        match handler(task.clone()) {
                            Ok(_) => {
                                let mut s = stats.write().await;
                                s.messages_consumed += 1;
                                debug!("Task {} processed successfully", task.id);
                            }
                            Err(e) => {
                                error!("Task {} failed: {}", task.id, e);
                                task.last_error = Some(e.to_string());
                                
                                let mut s = stats.write().await;
                                s.messages_failed += 1;
                                
                                // Retry logic
                                if task.attempts < config.retry_attempts {
                                    s.messages_retried += 1;
                                    
                                    // Re-queue with delay
                                    task.execute_after = Some(
                                        Utc::now() + chrono::Duration::milliseconds(
                                            config.retry_delay_ms as i64 * task.attempts as i64
                                        )
                                    );
                                    
                                    let serialized = serde_json::to_string(&task)?;
                                    conn.lpush::<_, _, ()>(queue, &serialized).await?;
                                    
                                    info!("Task {} re-queued for retry (attempt {})", task.id, task.attempts);
                                } else if config.dead_letter_queue_enabled {
                                    // Move to dead letter queue
                                    s.messages_dead_lettered += 1;
                                    
                                    let serialized = serde_json::to_string(&task)?;
                                    conn.lpush::<_, _, ()>(QueueChannels::DEAD_LETTER, &serialized).await?;
                                    
                                    warn!("Task {} moved to dead letter queue after {} attempts", task.id, task.attempts);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize task: {}", e);
                    }
                }
            }
        }
    }
    
    /// Process delayed messages
    async fn process_delayed_messages(&self, conn: &mut Connection) -> Result<()> {
        let now = Utc::now().timestamp();
        
        // Get messages that should be executed
        let messages: Vec<String> = conn.zrangebyscore_limit(
            "queue:delayed",
            0,
            now,
            0,
            100 // Process up to 100 at a time
        ).await?;
        
        for msg in messages {
            if let Ok(task) = serde_json::from_str::<QueueTask>(&msg) {
                // Determine target queue based on message type
                let queue = match &task.message {
                    QueueMessage::TradeExecuted { .. } => QueueChannels::TRADES,
                    QueueMessage::MarketCreated { .. } => QueueChannels::MARKETS,
                    QueueMessage::SettlementCompleted { .. } => QueueChannels::SETTLEMENTS,
                    QueueMessage::RiskAlert { .. } => QueueChannels::RISK_ALERTS,
                    QueueMessage::EmailNotification { .. } |
                    QueueMessage::WebhookDelivery { .. } => QueueChannels::NOTIFICATIONS,
                    _ => QueueChannels::GENERAL,
                };
                
                // Move to appropriate queue
                conn.lpush::<_, _, ()>(queue, &msg).await?;
                
                // Remove from delayed queue
                conn.zrem::<_, _, ()>("queue:delayed", &msg).await?;
            }
        }
        
        Ok(())
    }
    
    /// Get queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        self.stats.read().await.clone()
    }
    
    /// Get queue length
    pub async fn get_queue_length(&self, queue: &str) -> Result<u64> {
        if self.client.is_none() {
            return Ok(0);
        }
        
        let client = self.client.as_ref().unwrap();
        let mut conn = client.get_async_connection().await?;
        
        let length: u64 = conn.llen(queue).await?;
        Ok(length)
    }
    
    /// Clear a queue (dangerous!)
    pub async fn clear_queue(&self, queue: &str) -> Result<()> {
        if self.client.is_none() {
            return Ok(());
        }
        
        let client = self.client.as_ref().unwrap();
        let mut conn = client.get_async_connection().await?;
        
        conn.del::<_, ()>(queue).await?;
        info!("Cleared queue: {}", queue);
        
        Ok(())
    }
}

use futures_util::StreamExt;