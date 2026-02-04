//! Betting Platform API Library
//! 
//! Exposes modules for testing and integration

use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

// Public modules for tests
pub mod verse_catalog;
pub mod verse_generator;
pub mod types;
pub mod auth;
pub mod response;
pub mod config;
pub mod error;
pub mod wallet_utils;
pub mod wallet_verification;
pub mod seed_markets;
pub mod websocket;
pub mod rate_limit;
pub mod simple_rate_limit;
pub mod validation;
pub mod cache;
pub mod serialization;
pub mod rpc_client;
pub mod solana_funding;
pub mod mock_current_markets;
pub mod risk_engine;
pub mod risk_engine_ext;
pub mod quantum_engine;
pub mod quantum_engine_ext;
pub mod order_types;
pub mod middleware;
pub mod integration;
pub mod security;
pub mod transaction_signing;
pub mod pda;
pub mod db;
pub mod queue;
pub mod quantum_settlement;
pub mod jwt_validation;
pub mod rbac_authorization;
pub mod market_data_service;
pub mod trading_engine;
pub mod solana_rpc_service;
pub mod solana_transaction_manager;
pub mod solana_endpoints;
pub mod solana_deployment_manager;
pub mod deployment_endpoints;
pub mod external_api_service;
pub mod external_api_endpoints;
pub mod typed_errors;
pub mod error_middleware;
pub mod error_handlers;
pub mod circuit_breaker;
pub mod circuit_breaker_middleware;
pub mod circuit_breaker_integration;
pub mod tracing_logger;
pub mod tracing_middleware;
pub mod correlation_context;
pub mod market_creation_service;
pub mod market_creation_endpoints;
pub mod trade_execution_service;
pub mod trade_execution_endpoints;
pub mod settlement_service;
pub mod settlement_endpoints;
pub mod test_data_manager;
pub mod test_data_endpoints;
pub mod mock_services;
pub mod mock_config;
pub mod mock_service_manager;
pub mod health_check_service;
pub mod health_check_endpoints;
pub mod environment_config;
pub mod environment_config_endpoints;
pub mod feature_flags;
pub mod feature_flag_endpoints;
pub mod feature_flag_middleware;
pub mod platform;
pub mod platform_fixes;
pub mod state_manager;
pub mod state_management_endpoints;
pub mod state_sync_middleware;
pub mod validation_framework;
pub mod domain_validators;
pub mod validation_middleware;
pub mod validation_endpoints;

#[allow(dead_code)]#[cfg(test)]
pub mod test_utils;
#[allow(dead_code)]#[cfg(test)]
mod pda_test;

// Handler modules
pub mod auth_handlers;
pub mod trading_handlers;
pub mod position_handlers;
pub mod liquidity_handlers;
pub mod staking_handlers;
pub mod quantum_handlers;
pub mod risk_handlers;
pub mod transaction_handlers;
pub mod services;

// Re-export commonly used types
pub use types::*;
pub use auth::{Claims, UserRole, AuthService, AuthConfig};
pub use response::{ApiResponse, responses};

// AppState struct that handlers expect
#[derive(Clone)]
pub struct AppState {
    pub rpc_client: Arc<RpcClient>,
    pub program_id: Pubkey,
    pub ws_manager: Arc<websocket::WebSocketManager>,
    pub enhanced_ws_manager: Option<Arc<websocket::enhanced::EnhancedWebSocketManager>>,
    pub platform_client: Arc<rpc_client::BettingPlatformClient>,
    pub integration_config: integration::IntegrationConfig,
    pub market_sync: Option<Arc<integration::MarketSyncService>>,
    pub price_feed: Option<Arc<integration::PriceFeedService>>,
    pub order_engine: Arc<order_types::OrderMatchingEngine>,
    pub quantum_engine: Arc<quantum_engine::QuantumEngine>,
    pub risk_engine: Arc<risk_engine::RiskEngine>,
    pub funded_trading_client: Option<Arc<solana_funding::FundedTradingClient>>,
    pub seeded_markets: Arc<seed_markets::SeededMarketStore>,
    pub wallet_verification: Arc<wallet_verification::WalletVerificationService>,
    pub cache: Arc<cache::CacheService>,
    pub polymarket_public_client: Arc<integration::polymarket_public::PolymarketPublicClient>,
    pub polymarket_price_feed: Option<Arc<integration::polymarket_price_feed::PolymarketPriceFeed>>,
    pub database: Arc<db::fallback::FallbackDatabase>,
    pub queue_service: Option<Arc<queue::QueueService>>,
    pub security_logger: Arc<security::security_logger::SecurityLogger>,
    pub jwt_manager: Arc<crate::jwt_validation::JwtManager>,
    pub authorization_service: Arc<crate::rbac_authorization::AuthorizationService>,
    pub market_data_service: Arc<crate::market_data_service::MarketDataService>,
    pub trading_engine: Arc<crate::trading_engine::TradingEngine>,
    pub solana_rpc_service: Option<Arc<crate::solana_rpc_service::SolanaRpcService>>,
    pub solana_tx_manager: Option<Arc<crate::solana_transaction_manager::SolanaTransactionManager>>,
    pub solana_deployment_manager: Option<Arc<crate::solana_deployment_manager::SolanaDeploymentManager>>,
    pub external_api_service: Option<Arc<crate::external_api_service::ExternalApiService>>,
    pub circuit_breaker_manager: Option<Arc<crate::circuit_breaker::CircuitBreakerManager>>,
    pub service_circuit_breakers: Option<Arc<crate::circuit_breaker_middleware::ServiceCircuitBreakers>>,
    pub tracing_logger: Option<Arc<crate::tracing_logger::TracingLogger>>,
    pub market_creation_service: Option<Arc<crate::market_creation_service::MarketCreationService>>,
    pub trade_execution_service: Option<Arc<crate::trade_execution_service::TradeExecutionService>>,
    pub settlement_service: Option<Arc<crate::settlement_service::SettlementService>>,
    pub test_data_manager: Option<Arc<crate::test_data_manager::TestDataManager>>,
    pub mock_service_manager: Option<Arc<crate::mock_service_manager::MockServiceManager>>,
    pub health_check_service: Option<Arc<crate::health_check_service::HealthCheckService>>,
    pub environment_config: Option<Arc<crate::environment_config::EnvironmentConfigService>>,
    pub feature_flags: Option<Arc<crate::feature_flags::FeatureFlagService>>,
    pub state_manager: Option<Arc<crate::state_manager::StateManager>>,
    pub validation_service: Option<Arc<crate::validation_framework::ValidationService>>,
    // Polymarket integration
    pub polymarket_repository: Option<Arc<crate::db::polymarket_repository::PolymarketRepository>>,
    pub polymarket_order_service: Option<Arc<crate::services::PolymarketOrderService>>,
    pub polymarket_clob_client: Option<Arc<crate::integration::polymarket_clob::PolymarketClobClient>>,
    pub polymarket_ctf_client: Option<Arc<crate::integration::polymarket_ctf::PolymarketCtfClient>>,
    pub polymarket_ws_client: Option<Arc<tokio::sync::RwLock<crate::integration::polymarket_ws::PolymarketWsClient>>>,
}