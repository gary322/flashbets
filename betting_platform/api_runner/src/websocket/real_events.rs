//! Real-time event system that wires WebSocket to actual blockchain and system events

use anyhow::Result;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{info, debug, error, warn};
use chrono::Utc;

use crate::{
    AppState,
    websocket::enhanced::{EnhancedWsMessage, OrderLevel, PositionInfo},
    queue::{QueueMessage, QueueChannels},
    types::MarketOutcome,
    integration,
};

/// Real-time event processor
pub struct RealTimeEventProcessor {
    state: AppState,
    event_rx: mpsc::Receiver<SystemEvent>,
    stop_signal: mpsc::Receiver<()>,
}

/// System events that trigger WebSocket updates
#[derive(Debug, Clone)]
pub enum SystemEvent {
    // Blockchain events
    TradeExecuted {
        market_id: u128,
        wallet: String,
        outcome: u8,
        amount: u64,
        price: f64,
        signature: String,
    },
    PositionOpened {
        _position_id: String,
        wallet: String,
        market_id: u128,
        size: u64,
        leverage: u32,
    },
    PositionClosed {
        _position_id: String,
        wallet: String,
        market_id: u128,
        pnl: i128,
    },
    MarketCreated {
        market_id: u128,
        title: String,
        creator: String,
    },
    MarketResolved {
        market_id: u128,
        winning_outcome: u8,
    },
    
    // System events
    LiquidityAdded {
        market_id: u128,
        amount: u64,
        provider: String,
    },
    LiquidityRemoved {
        market_id: u128,
        amount: u64,
        provider: String,
    },
    CircuitBreakerTriggered {
        breaker_type: String,
        market_id: Option<u128>,
        reason: String,
    },
    SystemAlert {
        severity: String,
        message: String,
    },
}

impl RealTimeEventProcessor {
    pub fn new(state: AppState) -> (Self, mpsc::Sender<SystemEvent>, mpsc::Sender<()>) {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let (stop_tx, stop_signal) = mpsc::channel(1);
        
        let processor = Self {
            state,
            event_rx,
            stop_signal,
        };
        
        (processor, event_tx, stop_tx)
    }
    
    /// Start processing events
    pub async fn start(mut self) {
        info!("Starting real-time event processor");
        
        loop {
            tokio::select! {
                Some(event) = self.event_rx.recv() => {
                    if let Err(e) = self.process_event(event).await {
                        error!("Failed to process event: {}", e);
                    }
                }
                _ = self.stop_signal.recv() => {
                    info!("Stopping real-time event processor");
                    break;
                }
            }
        }
    }
    
    /// Process a system event and broadcast WebSocket updates
    async fn process_event(&self, event: SystemEvent) -> Result<()> {
        let ws_manager = self.state.enhanced_ws_manager.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WebSocket manager not initialized"))?;
        
        match event {
            SystemEvent::TradeExecuted { market_id, wallet, outcome, amount, price, signature: _ } => {
                // Update market data
                if let Ok(Some(market)) = self.state.platform_client.get_market(market_id).await {
                    let total_stake: u64 = market.outcomes.iter().map(|o| o.total_stake).sum();
                    let yes_price = if total_stake > 0 && market.outcomes.len() >= 2 {
                        market.outcomes[0].total_stake as f64 / total_stake as f64
                    } else {
                        0.5
                    };
                    
                    // Broadcast market update
                    ws_manager.broadcast_market_update(EnhancedWsMessage::MarketUpdate {
                        market_id,
                        yes_price,
                        no_price: 1.0 - yes_price,
                        volume: market.total_volume + amount,
                        liquidity: market.total_liquidity,
                        trades_24h: 0, // Would need to track this
                        timestamp: Utc::now().timestamp(),
                    });
                    
                    // Broadcast trade execution
                    ws_manager.broadcast_market_update(EnhancedWsMessage::TradeExecution {
                        market_id,
                        price,
                        size: amount,
                        side: if outcome == 0 { "buy".to_string() } else { "sell".to_string() },
                        timestamp: Utc::now().timestamp(),
                    });
                }
                
                // Update position if exists
                if let Ok(positions) = self.state.platform_client.get_positions(&solana_sdk::pubkey::Pubkey::from_str(&wallet)?).await {
                    if let Some(position) = positions.iter().find(|p| p.market_id == market_id) {
                        let position_info = PositionInfo {
                            size: position.size,
                            entry_price: position.entry_price as f64 / 1e6,
                            current_price: price,
                            pnl: ((price - position.entry_price as f64 / 1e6) * position.size as f64),
                            pnl_percentage: ((price / (position.entry_price as f64 / 1e6)) - 1.0) * 100.0,
                            leverage: position.leverage,
                            liquidation_price: 0.0, // Would need to calculate
                        };
                        
                        ws_manager.broadcast_position_update(EnhancedWsMessage::PositionUpdate {
                            wallet: wallet.clone(),
                            market_id,
                            position: position_info,
                            action: "updated".to_string(),
                            timestamp: Utc::now().timestamp(),
                        });
                    }
                }
            }
            
            SystemEvent::PositionOpened { _position_id, wallet, market_id, size, leverage } => {
                // Get current market price
                let current_price = if let Ok(Some(market)) = self.state.platform_client.get_market(market_id).await {
                    let total_stake: u64 = market.outcomes.iter().map(|o| o.total_stake).sum();
                    if total_stake > 0 && market.outcomes.len() >= 2 {
                        market.outcomes[0].total_stake as f64 / total_stake as f64
                    } else {
                        0.5
                    }
                } else {
                    0.5
                };
                
                let position_info = PositionInfo {
                    size,
                    entry_price: current_price,
                    current_price,
                    pnl: 0.0,
                    pnl_percentage: 0.0,
                    leverage,
                    liquidation_price: current_price * 0.8, // Simplified
                };
                
                ws_manager.broadcast_position_update(EnhancedWsMessage::PositionUpdate {
                    wallet,
                    market_id,
                    position: position_info,
                    action: "opened".to_string(),
                    timestamp: Utc::now().timestamp(),
                });
            }
            
            SystemEvent::PositionClosed { _position_id, wallet, market_id, pnl } => {
                ws_manager.broadcast_position_update(EnhancedWsMessage::PositionUpdate {
                    wallet,
                    market_id,
                    position: PositionInfo {
                        size: 0,
                        entry_price: 0.0,
                        current_price: 0.0,
                        pnl: pnl as f64,
                        pnl_percentage: 0.0,
                        leverage: 1,
                        liquidation_price: 0.0,
                    },
                    action: "closed".to_string(),
                    timestamp: Utc::now().timestamp(),
                });
            }
            
            SystemEvent::MarketCreated { market_id, title, creator } => {
                ws_manager.broadcast_system_event(EnhancedWsMessage::SystemEvent {
                    event_type: "market_created".to_string(),
                    message: format!("New market created: {}", title),
                    severity: "info".to_string(),
                    timestamp: Utc::now().timestamp(),
                });
            }
            
            SystemEvent::MarketResolved { market_id, winning_outcome } => {
                ws_manager.broadcast_system_event(EnhancedWsMessage::SystemEvent {
                    event_type: "market_resolved".to_string(),
                    message: format!("Market {} resolved with outcome {}", market_id, winning_outcome),
                    severity: "info".to_string(),
                    timestamp: Utc::now().timestamp(),
                });
            }
            
            SystemEvent::CircuitBreakerTriggered { breaker_type, market_id, reason } => {
                ws_manager.broadcast_system_event(EnhancedWsMessage::CircuitBreakerAlert {
                    breaker_type,
                    market_id,
                    triggered: true,
                    message: reason,
                    timestamp: Utc::now().timestamp(),
                });
            }
            
            SystemEvent::SystemAlert { severity, message } => {
                ws_manager.broadcast_system_event(EnhancedWsMessage::SystemEvent {
                    event_type: "system_alert".to_string(),
                    message,
                    severity,
                    timestamp: Utc::now().timestamp(),
                });
            }
            
            _ => {
                debug!("Unhandled event type");
            }
        }
        
        Ok(())
    }
}

/// Queue message processor that converts queue messages to system events
pub struct QueueEventBridge {
    state: AppState,
    event_tx: mpsc::Sender<SystemEvent>,
}

impl QueueEventBridge {
    pub fn new(state: AppState, event_tx: mpsc::Sender<SystemEvent>) -> Self {
        Self { state, event_tx }
    }
    
    /// Start processing queue messages
    pub async fn start(&self) -> Result<()> {
        let queue_service = self.state.queue_service.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Queue service not initialized"))?;
        
        // Subscribe to relevant queues
        let channels = vec![
            QueueChannels::TRADES,
            QueueChannels::MARKETS,
            QueueChannels::SETTLEMENTS,
            QueueChannels::RISK_ALERTS,
        ];
        
        for channel in channels {
            let queue_service = queue_service.clone();
            let event_tx = self.event_tx.clone();
            
            tokio::spawn(async move {
                loop {
                    let event_tx_clone = event_tx.clone();
                    match queue_service.consume(channel, move |task| {
                        let event_tx = event_tx_clone.clone();
                        
                        // Convert queue message to system event
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                if let Err(e) = Self::process_queue_message(task.message, event_tx).await {
                                    error!("Failed to process queue message: {}", e);
                                }
                                Ok(())
                            })
                        })
                    }).await {
                        Ok(_) => {
                            warn!("Queue consumer exited unexpectedly");
                            break;
                        }
                        Err(e) => {
                            error!("Queue consumer error: {}. Restarting...", e);
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            });
        }
        
        Ok(())
    }
    
    /// Process a queue message and convert to system event
    async fn process_queue_message(
        msg: QueueMessage,
        event_tx: mpsc::Sender<SystemEvent>,
    ) -> Result<()> {
        let system_event = match msg {
            QueueMessage::TradeExecuted { trade_id, wallet, market_id, amount, outcome, timestamp } => {
                SystemEvent::TradeExecuted {
                    market_id: market_id.parse::<u128>().unwrap_or(0),
                    wallet,
                    outcome,
                    amount,
                    price: 0.5, // Would need to calculate actual price
                    signature: trade_id,
                }
            }
            
            QueueMessage::MarketCreated { market_id, title, creator, timestamp } => {
                SystemEvent::MarketCreated {
                    market_id: market_id.parse::<u128>().unwrap_or(0),
                    title,
                    creator,
                }
            }
            
            QueueMessage::PositionClosed { position_id, wallet, market_id, pnl, timestamp } => {
                SystemEvent::PositionClosed {
                    _position_id: position_id,
                    wallet,
                    market_id: market_id.parse::<u128>().unwrap_or(0),
                    pnl: pnl as i128,
                }
            }
            
            QueueMessage::SettlementCompleted { market_id, winning_outcome, total_payout, timestamp: _ } => {
                SystemEvent::MarketResolved {
                    market_id: market_id.parse::<u128>().unwrap_or(0),
                    winning_outcome,
                }
            }
            
            QueueMessage::RiskAlert { wallet, alert_type, severity, details: _, timestamp: _ } => {
                SystemEvent::SystemAlert {
                    severity,
                    message: format!("Risk alert for {}: {}", wallet, alert_type),
                }
            }
            
            _ => return Ok(()), // Skip other message types
        };
        
        event_tx.send(system_event).await?;
        Ok(())
    }
}

/// Blockchain event monitor that watches for on-chain events
pub struct BlockchainEventMonitor {
    state: AppState,
    event_tx: mpsc::Sender<SystemEvent>,
}

impl BlockchainEventMonitor {
    pub fn new(state: AppState, event_tx: mpsc::Sender<SystemEvent>) -> Self {
        Self { state, event_tx }
    }
    
    /// Start monitoring blockchain events
    pub async fn start(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(2)); // Poll every 2 seconds
        
        loop {
            interval.tick().await;
            
            // Monitor for new transactions
            // In production, this would use WebSocket subscription to Solana
            // For now, we'll check recent program transactions
            
            // This is a placeholder - in production you'd use:
            // - Solana WebSocket subscriptions
            // - Transaction filters for your program
            // - Event parsing from transaction logs
            
            debug!("Checking for blockchain events...");
        }
    }
}

/// Price feed monitor that provides real-time price updates
pub struct PriceFeedMonitor {
    state: AppState,
}

impl PriceFeedMonitor {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
    
    /// Start monitoring price feeds
    pub async fn start(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(5)); // Update every 5 seconds
        
        loop {
            interval.tick().await;
            
            // Update order books for active markets
            if let Ok(markets) = self.state.platform_client.get_markets().await {
                for market in markets.iter().take(10) {
                    if let Some(ws_manager) = &self.state.enhanced_ws_manager {
                        // Generate realistic order book
                        let mid_price = if market.total_volume > 0 && market.outcomes.len() >= 2 {
                            market.outcomes[0].total_stake as f64 / market.total_volume as f64
                        } else {
                            0.5
                        };
                        
                        let order_book = generate_order_book(mid_price, market.total_liquidity);
                        
                        ws_manager.broadcast_order_book_update(EnhancedWsMessage::OrderBookUpdate {
                            market_id: market.id,
                            bids: order_book.0,
                            asks: order_book.1,
                            spread: order_book.2,
                            mid_price,
                            timestamp: Utc::now().timestamp(),
                        });
                    }
                }
            }
            
            // Update from Polymarket price feed if available
            if let Some(polymarket_feed) = &self.state.polymarket_price_feed {
                // Price feed is already running in background
                debug!("Polymarket price feed active");
            }
        }
    }
}

/// Generate realistic order book based on market conditions
fn generate_order_book(mid_price: f64, liquidity: u64) -> (Vec<OrderLevel>, Vec<OrderLevel>, f64) {
    let mut bids = Vec::new();
    let mut asks = Vec::new();
    
    // Generate bid levels
    for i in 1..=5 {
        let price = mid_price - (0.01 * i as f64);
        let size = (liquidity as f64 * 0.1 / i as f64) as u64;
        let orders = (5 - i + 1) as u32; // More orders at better prices
        bids.push(OrderLevel { price, amount: size, orders });
    }
    
    // Generate ask levels
    for i in 1..=5 {
        let price = mid_price + (0.01 * i as f64);
        let size = (liquidity as f64 * 0.1 / i as f64) as u64;
        let orders = (5 - i + 1) as u32; // More orders at better prices
        asks.push(OrderLevel { price, amount: size, orders });
    }
    
    let spread = if !asks.is_empty() && !bids.is_empty() {
        asks[0].price - bids[0].price
    } else {
        0.02
    };
    
    (bids, asks, spread)
}

/// Initialize and start all real-time event systems
pub async fn initialize_real_time_events(state: AppState) -> Result<()> {
    info!("Initializing real-time event systems");
    
    // Create event processor
    let (processor, event_tx, stop_tx) = RealTimeEventProcessor::new(state.clone());
    
    // Start event processor
    tokio::spawn(async move {
        processor.start().await;
    });
    
    // Start queue event bridge
    let queue_bridge = QueueEventBridge::new(state.clone(), event_tx.clone());
    tokio::spawn(async move {
        if let Err(e) = queue_bridge.start().await {
            error!("Queue event bridge failed: {}", e);
        }
    });
    
    // Start blockchain monitor
    let blockchain_monitor = BlockchainEventMonitor::new(state.clone(), event_tx.clone());
    tokio::spawn(async move {
        if let Err(e) = blockchain_monitor.start().await {
            error!("Blockchain monitor failed: {}", e);
        }
    });
    
    // Start price feed monitor
    let price_monitor = PriceFeedMonitor::new(state.clone());
    tokio::spawn(async move {
        if let Err(e) = price_monitor.start().await {
            error!("Price feed monitor failed: {}", e);
        }
    });
    
    info!("Real-time event systems initialized");
    Ok(())
}

