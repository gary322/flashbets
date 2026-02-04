//! Test utilities and factories for creating test objects

use crate::*;
use crate::websocket::enhanced::{EnhancedWsMessage, EnhancedWebSocketManager};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

/// Test factory for creating quantum states
pub mod quantum_factory {
    use super::*;
    use crate::quantum_engine::{QuantumState, QuantumPosition, EntanglementGroup};
    use rand::Rng;
    
    pub fn create_quantum_state(market_id: u128, outcome: u8) -> QuantumState {
        let mut rng = rand::thread_rng();
        let probability: f64 = rng.gen_range(0.1..1.0); // Raw probability
        let amplitude = probability.sqrt();
        
        QuantumState {
            market_id,
            outcome,
            amount: rng.gen_range(1000..100000),
            leverage: rng.gen_range(1..10),
            amplitude,
            phase: rng.gen_range(0.0..std::f64::consts::TAU),
            probability, // Will be normalized later
            entangled_with: vec![],
        }
    }
    
    pub fn create_quantum_position(wallet: &str, num_states: usize) -> QuantumPosition {
        let mut states = Vec::new();
        let mut total_prob = 0.0;
        
        // Create states
        for i in 0..num_states {
            let state = create_quantum_state(1000 + i as u128, (i % 2) as u8);
            total_prob += state.probability;
            states.push(state);
        }
        
        // Normalize probabilities
        for state in &mut states {
            state.probability /= total_prob;
            state.amplitude = state.probability.sqrt();
        }
        
        QuantumPosition {
            id: format!("test_quantum_{}", uuid::Uuid::new_v4()),
            wallet: wallet.to_string(),
            states,
            entanglement_group: None,
            coherence_time: 3600,
            created_at: Utc::now().timestamp(),
            last_measured: None,
            is_collapsed: false,
            measurement_result: None,
        }
    }
    
    pub fn create_entangled_positions(wallets: &[&str]) -> (Vec<QuantumPosition>, EntanglementGroup) {
        let mut positions = Vec::new();
        let mut position_ids = Vec::new();
        
        for wallet in wallets {
            let mut position = create_quantum_position(wallet, 3);
            position.entanglement_group = Some("test_entanglement".to_string());
            position_ids.push(position.id.clone());
            positions.push(position);
        }
        
        // Create correlation matrix
        let n = wallets.len();
        let mut correlation_matrix = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    correlation_matrix[i][j] = 1.0;
                } else {
                    correlation_matrix[i][j] = 0.5; // 50% correlation
                }
            }
        }
        
        let group = EntanglementGroup {
            id: "test_entanglement".to_string(),
            positions: position_ids,
            correlation_matrix,
            created_at: Utc::now().timestamp(),
        };
        
        (positions, group)
    }
}

/// Test factory for risk-related objects
pub mod risk_factory {
    use super::*;
    use crate::risk_engine::{Greeks, RiskMetrics, PositionRisk};
    
    pub fn create_greeks() -> Greeks {
        Greeks {
            delta: 0.6,
            gamma: 0.1,
            theta: -0.05,
            vega: 0.2,
            rho: 0.03,
        }
    }
    
    pub fn create_risk_metrics(wallet: &str) -> RiskMetrics {
        let mut correlation_matrix = HashMap::new();
        correlation_matrix.insert("BTC".to_string(), 0.8);
        correlation_matrix.insert("ETH".to_string(), 0.7);
        
        RiskMetrics {
            portfolio_value: 100000.0,
            total_exposure: 50000.0,
            leverage_ratio: 2.5,
            margin_used: 20000.0,
            margin_available: 30000.0,
            margin_ratio: 0.4,
            unrealized_pnl: 5000.0,
            realized_pnl: 2000.0,
            max_drawdown: -10000.0,
            sharpe_ratio: 1.5,
            sortino_ratio: 2.0,
            win_rate: 0.65,
            profit_factor: 1.8,
            var_95: -5000.0,
            var_99: -8000.0,
            expected_shortfall: -9000.0,
            beta: 1.2,
            alpha: 0.05,
            correlation_matrix,
            risk_score: 45.0,
        }
    }
    
    pub fn create_position_risk(position_id: &str, market_id: u128) -> PositionRisk {
        PositionRisk {
            position_id: position_id.to_string(),
            market_id,
            current_value: 11000.0,
            entry_value: 10000.0,
            unrealized_pnl: 1000.0,
            greeks: create_greeks(),
            risk_contribution: 0.05,
            margin_requirement: 2000.0,
            liquidation_price: 0.4,
            time_to_expiry: Some(3600),
            volatility: 0.35,
            correlation_risk: 0.25,
        }
    }
}

/// Test factory for market objects
pub mod market_factory {
    use super::*;
    use crate::types::{Market, MarketOutcome, AmmType};
    
    pub fn create_test_market(id: u128, title: &str) -> Market {
        Market {
            id,
            title: title.to_string(),
            description: format!("Test market for {}", title),
            creator: Pubkey::new_unique(),
            outcomes: vec![
                MarketOutcome {
                    name: "Yes".to_string(),
                    total_stake: 1000000,
                },
                MarketOutcome {
                    name: "No".to_string(),
                    total_stake: 1000000,
                },
            ],
            amm_type: AmmType::PmAmm,
            total_liquidity: 2000000,
            total_volume: 5000000,
            resolution_time: Utc::now().timestamp() + 86400,
            resolved: false,
            winning_outcome: None,
            created_at: Utc::now().timestamp(),
            verse_id: Some((id % 400) as u128),
        }
    }
    
    pub fn create_test_markets(count: usize) -> Vec<Market> {
        (0..count)
            .map(|i| create_test_market(1000 + i as u128, &format!("Test Market {}", i)))
            .collect()
    }
}

/// Test factory for WebSocket messages
pub mod websocket_factory {
    use super::*;
    use crate::websocket::enhanced::{EnhancedWsMessage, Subscription};
    
    pub fn create_market_update(market_id: u128) -> EnhancedWsMessage {
        EnhancedWsMessage::MarketUpdate {
            market_id,
            yes_price: 0.55,
            no_price: 0.45,
            volume: 1000000,
            liquidity: 500000,
            trades_24h: 150,
            timestamp: Utc::now().timestamp(),
        }
    }
    
    pub fn create_position_update(wallet: &str, market_id: u128) -> EnhancedWsMessage {
        EnhancedWsMessage::PositionUpdate {
            wallet: wallet.to_string(),
            market_id,
            position: crate::websocket::enhanced::PositionInfo {
                size: 10000,
                entry_price: 0.5,
                current_price: 0.55,
                pnl: 500.0,
                pnl_percentage: 10.0,
                leverage: 5,
                liquidation_price: 0.3,
            },
            action: "updated".to_string(),
            timestamp: Utc::now().timestamp(),
        }
    }
    
    pub fn create_subscription(market_id: u128) -> Subscription {
        Subscription::MarketUpdates { market_id }
    }
}

/// Mock RPC client for testing
pub struct MockRpcClient {
    pub markets: Arc<RwLock<Vec<Market>>>,
    pub fail_next_call: Arc<RwLock<bool>>,
}

impl MockRpcClient {
    pub fn new() -> Self {
        Self {
            markets: Arc::new(RwLock::new(market_factory::create_test_markets(10))),
            fail_next_call: Arc::new(RwLock::new(false)),
        }
    }
    
    pub async fn get_markets(&self) -> Result<Vec<Market>, anyhow::Error> {
        if *self.fail_next_call.read().await {
            *self.fail_next_call.write().await = false;
            return Err(anyhow::anyhow!("RPC call failed"));
        }
        
        Ok(self.markets.read().await.clone())
    }
    
    pub async fn set_fail_next(&self) {
        *self.fail_next_call.write().await = true;
    }
}

/// Test helpers
pub mod helpers {
    use super::*;
    
    /// Create a test wallet address
    pub fn test_wallet(index: u32) -> String {
        format!("test_wallet_{:03}", index)
    }
    
    /// Create a test AppState
    pub async fn create_test_app_state() -> AppState {
        use crate::{
            quantum_engine::QuantumEngine,
            risk_engine::RiskEngine,
            order_types::OrderMatchingEngine,
            seed_markets::SeededMarketStore,
            wallet_verification::WalletVerificationService,
            cache::CacheService,
            integration::polymarket_public::PolymarketPublicClient,
        };
        
        let rpc_client = Arc::new(solana_client::rpc_client::RpcClient::new("http://localhost:8899".to_string()));
        let program_id = Pubkey::new_unique();
        
        AppState {
            rpc_client: rpc_client.clone(),
            program_id,
            ws_manager: Arc::new(crate::websocket::WebSocketManager::new()),
            enhanced_ws_manager: Some(Arc::new(crate::websocket::enhanced::EnhancedWebSocketManager::new())),
            platform_client: Arc::new(crate::rpc_client::BettingPlatformClient::new(rpc_client, program_id)),
            integration_config: {
                let mut config = integration::IntegrationConfig::default();
                config.polymarket_enabled = false;
                config.kalshi_enabled = false;
                config
            },
            market_sync: None,
            price_feed: None,
            order_engine: Arc::new(OrderMatchingEngine::new()),
            quantum_engine: Arc::new(QuantumEngine::new()),
            risk_engine: Arc::new(RiskEngine::new()),
            funded_trading_client: None,
            seeded_markets: Arc::new(SeededMarketStore::new()),
            wallet_verification: Arc::new(WalletVerificationService::new()),
            cache: {
                let config = crate::cache::CacheConfig {
                    enabled: false, // Disable for tests
                    ..Default::default()
                };
                Arc::new(tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        crate::cache::CacheService::new(config).await.unwrap()
                    })
                }))
            },
            polymarket_public_client: Arc::new(PolymarketPublicClient::new().unwrap()),
            polymarket_price_feed: None,
            database: Arc::new(tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    crate::db::Database::new(crate::db::DatabaseConfig::default()).await.unwrap()
                })
            })),
            queue_service: None,
            security_logger: Arc::new(crate::security::security_logger::SecurityLogger::new(
                crate::security::security_logger::SecurityLoggerConfig::default()
            )),
        }
    }
    
    /// Assert float equality with tolerance
    pub fn assert_float_eq(a: f64, b: f64, tolerance: f64) {
        assert!(
            (a - b).abs() < tolerance,
            "Float values not equal: {} != {} (tolerance: {})",
            a, b, tolerance
        );
    }
    
    /// Create random bytes
    pub fn random_bytes(len: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut bytes = vec![0u8; len];
        rng.fill_bytes(&mut bytes);
        bytes
    }
}