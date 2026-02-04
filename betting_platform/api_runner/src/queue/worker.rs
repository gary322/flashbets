//! Queue worker for processing background tasks

use anyhow::Result;
use crate::AppState;
use super::{QueueService, QueueChannels, QueueTask, handlers};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, error};

/// Queue worker that processes messages from different queues
pub struct QueueWorker {
    queue_service: Arc<QueueService>,
    state: AppState,
    handles: Vec<JoinHandle<()>>,
}

impl QueueWorker {
    /// Create a new queue worker
    pub fn new(queue_service: Arc<QueueService>, state: AppState) -> Self {
        Self {
            queue_service,
            state,
            handles: Vec::new(),
        }
    }
    
    /// Start the queue worker
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting queue workers...");
        
        // Start trade processing worker
        let worker_handle = self.spawn_worker(QueueChannels::TRADES, "trade");
        self.handles.push(worker_handle);
        
        // Start market processing worker
        let worker_handle = self.spawn_worker(QueueChannels::MARKETS, "market");
        self.handles.push(worker_handle);
        
        // Start settlement processing worker
        let worker_handle = self.spawn_worker(QueueChannels::SETTLEMENTS, "settlement");
        self.handles.push(worker_handle);
        
        // Start risk alert processing worker
        let worker_handle = self.spawn_worker(QueueChannels::RISK_ALERTS, "risk");
        self.handles.push(worker_handle);
        
        // Start notification processing worker
        let worker_handle = self.spawn_worker(QueueChannels::NOTIFICATIONS, "notification");
        self.handles.push(worker_handle);
        
        // Start general queue worker
        let worker_handle = self.spawn_worker(QueueChannels::GENERAL, "general");
        self.handles.push(worker_handle);
        
        info!("Queue workers started: {} workers active", self.handles.len());
        
        Ok(())
    }
    
    /// Spawn a worker for a specific queue
    fn spawn_worker(&self, queue: &'static str, name: &str) -> JoinHandle<()> {
        let queue_service = self.queue_service.clone();
        let state = self.state.clone();
        let worker_name = format!("{}_worker", name);
        
        tokio::spawn(async move {
            info!("{}: Starting for queue {}", worker_name, queue);
            
            loop {
                let state_for_closure = state.clone();
                match queue_service.consume(queue, move |task| {
                    // Process task synchronously within the closure
                    let state_clone = state_for_closure.clone();
                    let task_clone = task.clone();
                    
                    // Use tokio's block_in_place to run async code in sync context
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            handlers::process_queue_task(&state_clone, task_clone).await
                        })
                    })
                }).await {
                    Ok(_) => {
                        // Consumer exited normally (shouldn't happen)
                        error!("{}: Consumer exited unexpectedly", worker_name);
                        break;
                    }
                    Err(e) => {
                        error!("{}: Consumer error: {}. Restarting in 5 seconds...", worker_name, e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        })
    }
    
    /// Stop all workers
    pub async fn stop(&mut self) {
        info!("Stopping queue workers...");
        
        for handle in &self.handles {
            handle.abort();
        }
        
        self.handles.clear();
        
        info!("Queue workers stopped");
    }
}

/// Start queue workers for the application
pub async fn start_queue_workers(state: AppState) -> Result<()> {
    let queue_config = super::QueueConfig {
        redis_url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        enabled: std::env::var("QUEUE_ENABLED").unwrap_or_else(|_| "true".to_string()) == "true",
        worker_threads: std::env::var("QUEUE_WORKERS")
            .unwrap_or_else(|_| "4".to_string())
            .parse()
            .unwrap_or(4),
        retry_attempts: 3,
        retry_delay_ms: 1000,
        task_timeout_seconds: 300,
        dead_letter_queue_enabled: true,
    };
    
    if !queue_config.enabled {
        info!("Queue workers disabled");
        return Ok(());
    }
    
    let queue_service = Arc::new(QueueService::new(queue_config).await?);
    // Queue service is already initialized in main.rs, no need to update it here
    
    let mut worker = QueueWorker::new(queue_service, state);
    worker.start().await?;
    
    // Keep worker handle in a static location or return it
    // For now, we'll just let it run in the background
    std::mem::forget(worker);
    
    Ok(())
}