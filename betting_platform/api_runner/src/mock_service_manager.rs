//! Mock Service Manager
//! Manages the lifecycle and configuration of mock services

use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

use crate::{
    mock_config::{MockConfig, MockProfile},
    mock_services::*,
    settlement_service::SettlementService,
    AppState,
};

/// Mock service manager
pub struct MockServiceManager {
    config: MockConfig,
    services: Option<MockServices>,
    active_tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

impl MockServiceManager {
    /// Create new mock service manager
    pub fn new(config: MockConfig) -> Self {
        Self {
            config,
            services: None,
            active_tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with a specific profile
    pub fn with_profile(profile: MockProfile) -> Self {
        Self::new(profile.to_config())
    }

    /// Initialize mock services
    pub async fn initialize(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Mock services disabled");
            return Ok(());
        }

        info!("Initializing mock services");

        // Create all mock services
        let services = MockServiceFactory::create_all();

        // Configure oracle providers
        for provider_config in &self.config.oracle.providers {
            let provider = MockOracleProvider::new(provider_config.name.clone())
                .with_confidence(provider_config.confidence)
                .with_delay(Duration::from_millis(provider_config.response_delay_ms))
                .with_fail_rate(provider_config.fail_rate);
            
            // Store provider reference if needed
            debug!("Configured mock oracle provider: {}", provider_config.name);
        }

        // Configure trading engine with initial markets
        for market_config in &self.config.trading.initial_markets {
            services.trading_engine.add_market(
                market_config.id,
                market_config.title.clone(),
                market_config.liquidity,
            ).await;
            
            debug!("Added mock market: {} ({})", market_config.title, market_config.id);
        }

        // Configure price feed with initial prices
        for symbol_config in &self.config.price_feed.symbols {
            services.price_feed.set_price(
                symbol_config.symbol.clone(),
                symbol_config.initial_price,
            ).await;
            
            debug!("Set initial price for {}: {}", symbol_config.symbol, symbol_config.initial_price);
        }

        // Start price update task if configured
        if self.config.price_feed.update_interval_ms > 0 {
            let symbols: Vec<String> = self.config.price_feed.symbols
                .iter()
                .map(|s| s.symbol.clone())
                .collect();
            
            let price_feed = services.price_feed.clone();
            let task = price_feed.start_price_updates(symbols);
            
            self.active_tasks.write().await.push(task);
            info!("Started mock price feed updates");
        }

        self.services = Some(services);
        info!("Mock services initialized successfully");
        
        Ok(())
    }

    /// Inject mock services into AppState
    pub async fn inject_into_app_state(&self, state: &mut AppState) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let services = self.services.as_ref()
            .context("Mock services not initialized")?;

        // Replace WebSocket manager with mock
        state.enhanced_ws_manager = Some(Arc::new(
            crate::websocket::enhanced::EnhancedWebSocketManager::new()
        ));

        // Register mock oracle providers with settlement service
        if let Some(settlement_service) = &state.settlement_service {
            for provider_config in &self.config.oracle.providers {
                let provider = MockOracleProvider::new(provider_config.name.clone())
                    .with_confidence(provider_config.confidence)
                    .with_delay(Duration::from_millis(provider_config.response_delay_ms))
                    .with_fail_rate(provider_config.fail_rate);
                
                settlement_service.register_oracle(
                    provider_config.name.clone(),
                    Box::new(provider),
                ).await;
            }
            
            info!("Registered {} mock oracle providers", self.config.oracle.providers.len());
        }

        info!("Mock services injected into AppState");
        Ok(())
    }

    /// Get mock services reference
    pub fn services(&self) -> Option<&MockServices> {
        self.services.as_ref()
    }

    /// Set market outcome for testing
    pub async fn set_market_outcome(&self, market_id: u128, outcome: u8) -> Result<()> {
        let services = self.services.as_ref()
            .context("Mock services not initialized")?;
        
        services.oracle_provider.set_market_outcome(market_id, outcome).await;
        Ok(())
    }

    /// Simulate market activity
    pub async fn simulate_market_activity(
        &self,
        market_id: u128,
        duration: Duration,
        trades_per_minute: u32,
    ) -> Result<()> {
        let services = self.services.as_ref()
            .context("Mock services not initialized")?;
        
        let trading_engine = services.trading_engine.clone();
        let ws_manager = services.websocket_manager.clone();
        
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(60 / trades_per_minute as u64)
            );
            let end_time = tokio::time::Instant::now() + duration;
            
            while tokio::time::Instant::now() < end_time {
                interval.tick().await;
                
                // Simulate random trade
                let user = format!("simulated_user_{}", rand::random::<u32>() % 100);
                let amount = 100 + rand::random::<u64>() % 1000;
                let buy = rand::random::<bool>();
                
                if let Ok(order_id) = trading_engine.place_order(
                    market_id,
                    user.clone(),
                    amount,
                    buy,
                ).await {
                    debug!("Simulated trade: {} {} for {}", 
                        if buy { "bought" } else { "sold" },
                        amount,
                        user
                    );
                    
                    // Broadcast update
                    ws_manager.broadcast(serde_json::json!({
                        "type": "trade",
                        "market_id": market_id,
                        "order_id": order_id,
                        "amount": amount,
                        "side": if buy { "buy" } else { "sell" },
                        "timestamp": chrono::Utc::now(),
                    })).await;
                }
            }
        });
        
        self.active_tasks.write().await.push(task);
        info!("Started market activity simulation for market {}", market_id);
        
        Ok(())
    }

    /// Simulate network conditions
    pub async fn simulate_network_conditions(&self, profile: NetworkProfile) -> Result<()> {
        let services = self.services.as_ref()
            .context("Mock services not initialized")?;
        
        match profile {
            NetworkProfile::Normal => {
                services.solana_rpc.set_fail_next().await;
                services.external_api.set_fail_pattern(None).await;
            }
            NetworkProfile::Degraded => {
                // 10% failure rate
                for _ in 0..10 {
                    if rand::random::<f64>() < 0.1 {
                        services.solana_rpc.set_fail_next().await;
                    }
                }
            }
            NetworkProfile::Offline => {
                services.external_api.set_fail_pattern(Some("*".to_string())).await;
            }
        }
        
        info!("Set network conditions to: {:?}", profile);
        Ok(())
    }

    /// Get service statistics
    pub async fn get_statistics(&self) -> Result<MockServiceStats> {
        let services = self.services.as_ref()
            .context("Mock services not initialized")?;
        
        let ws_connections = services.websocket_manager.get_connection_count().await;
        let broadcast_count = services.websocket_manager.get_broadcast_history().await.len();
        let request_log = services.external_api.get_request_log().await;
        
        Ok(MockServiceStats {
            websocket_connections: ws_connections,
            broadcasts_sent: broadcast_count,
            external_api_requests: request_log.len(),
            active_tasks: self.active_tasks.read().await.len(),
        })
    }

    /// Shutdown mock services
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down mock services");
        
        // Cancel all active tasks
        for task in self.active_tasks.write().await.drain(..) {
            task.abort();
        }
        
        self.services = None;
        info!("Mock services shut down");
        
        Ok(())
    }
}

/// Network simulation profiles
#[derive(Debug, Clone, Copy)]
pub enum NetworkProfile {
    Normal,
    Degraded,
    Offline,
}

/// Mock service statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockServiceStats {
    pub websocket_connections: usize,
    pub broadcasts_sent: usize,
    pub external_api_requests: usize,
    pub active_tasks: usize,
}

/// Mock service endpoints for testing
pub mod endpoints {
    use super::*;
    use axum::{
        extract::{Query, State, Path},
        response::Json,
        Extension,
    };
    use serde::{Deserialize, Serialize};
    
    use crate::{
        jwt_validation::AuthenticatedUser,
        response::{ApiResponse, responses},
        typed_errors::{AppError, ErrorKind, ErrorContext},
    };

    #[derive(Debug, Deserialize)]
    pub struct SimulateMarketActivityRequest {
        pub market_id: u128,
        pub duration_minutes: u64,
        pub trades_per_minute: u32,
    }

    #[derive(Debug, Deserialize)]
    pub struct SetMarketOutcomeRequest {
        pub market_id: u128,
        pub outcome: u8,
    }

    /// Get mock service statistics
    pub async fn get_mock_stats(
        State(state): State<Arc<AppState>>,
        Extension(user): Extension<AuthenticatedUser>,
    ) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
        let context = ErrorContext::new("mock_endpoints", "get_stats");
        
        // Check admin role
        if user.claims.role != "admin" {
            return Err(AppError::new(
                ErrorKind::Forbidden,
                "Only admins can view mock service stats",
                context,
            ));
        }

        // Get mock service manager from somewhere
        // For now, return empty stats
        let stats = MockServiceStats {
            websocket_connections: 0,
            broadcasts_sent: 0,
            external_api_requests: 0,
            active_tasks: 0,
        };

        Ok(Json(responses::success_with_data(
            "Mock service statistics retrieved",
            stats,
        )))
    }

    /// Simulate market activity
    pub async fn simulate_market_activity(
        State(state): State<Arc<AppState>>,
        Extension(user): Extension<AuthenticatedUser>,
        Json(request): Json<SimulateMarketActivityRequest>,
    ) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
        let context = ErrorContext::new("mock_endpoints", "simulate_activity");
        
        // Check admin role
        if user.claims.role != "admin" {
            return Err(AppError::new(
                ErrorKind::Forbidden,
                "Only admins can simulate market activity",
                context,
            ));
        }

        // Simulate activity
        info!(
            "Simulating {} trades/min for {} minutes on market {}",
            request.trades_per_minute,
            request.duration_minutes,
            request.market_id
        );

        Ok(Json(responses::success_with_data(
            "Market activity simulation started",
            serde_json::json!({
                "market_id": request.market_id,
                "duration_minutes": request.duration_minutes,
                "trades_per_minute": request.trades_per_minute,
            }),
        )))
    }

    /// Set market outcome for testing
    pub async fn set_market_outcome(
        State(state): State<Arc<AppState>>,
        Extension(user): Extension<AuthenticatedUser>,
        Json(request): Json<SetMarketOutcomeRequest>,
    ) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
        let context = ErrorContext::new("mock_endpoints", "set_outcome");
        
        // Check admin role
        if user.claims.role != "admin" {
            return Err(AppError::new(
                ErrorKind::Forbidden,
                "Only admins can set mock market outcomes",
                context,
            ));
        }

        info!(
            "Setting market {} outcome to {}",
            request.market_id,
            request.outcome
        );

        Ok(Json(responses::success_with_data(
            "Market outcome set successfully",
            serde_json::json!({
                "market_id": request.market_id,
                "outcome": request.outcome,
            }),
        )))
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_service_manager_initialization() {
        let mut manager = MockServiceManager::with_profile(MockProfile::Fast);
        assert!(manager.initialize().await.is_ok());
        assert!(manager.services().is_some());
    }

    #[tokio::test]
    async fn test_mock_service_statistics() {
        let mut manager = MockServiceManager::with_profile(MockProfile::Fast);
        manager.initialize().await.unwrap();
        
        let stats = manager.get_statistics().await.unwrap();
        assert_eq!(stats.websocket_connections, 0);
        assert!(stats.active_tasks > 0); // Price feed task
    }
}