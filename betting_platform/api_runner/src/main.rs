//! Betting Platform API Server
//! 
//! Standalone REST API server that connects the UI to the Solana smart contracts

use anyhow::Result;
use axum::{
    extract::{State, WebSocketUpgrade, FromRef, Path, Query, Extension},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post, put, delete},
    Router,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};
use std::{net::SocketAddr, sync::Arc, str::FromStr, env};
use tower_http::cors::CorsLayer;
use tracing::info;

mod rpc_client;
mod websocket;
mod handlers;
mod types;
pub mod integration;
mod verse_generator;
mod verse_catalog;
mod serialization;
mod auth;
mod error;
mod rate_limit;
mod config;
mod order_types;
mod quantum_engine;
mod risk_engine;
mod solana_funding;
mod wallet_utils;
mod seed_markets;
mod simple_rate_limit;
mod wallet_verification;
mod cache;
mod validation;
mod response;
mod mock_current_markets;
mod auth_handlers;
mod middleware;
mod trading_handlers;
mod position_handlers;
mod liquidity_handlers;
mod transaction_handlers;
mod transaction_signing;
mod pda;
mod risk_engine_ext;
mod staking_handlers;
mod risk_handlers;
mod quantum_handlers;
mod quantum_engine_ext;
mod db;
mod queue;
mod security;
mod quantum_settlement;
mod memory_management;
mod throughput_optimization;
mod response_types;
mod jwt_validation;
mod auth_endpoints;
mod rbac_authorization;
mod rbac_endpoints;
mod security_endpoints;
mod market_data_service;
mod market_handlers;
mod websocket_server;
mod websocket_client;
mod trading_engine;
mod trading_api;
mod solana_rpc_service;
mod solana_transaction_manager;
mod solana_endpoints;
mod solana_deployment_manager;
mod deployment_endpoints;
mod external_api_service;
mod external_api_endpoints;
mod typed_errors;
mod error_middleware;
mod error_handlers;
mod circuit_breaker;
mod circuit_breaker_middleware;
mod circuit_breaker_integration;
mod tracing_logger;
mod tracing_middleware;
mod correlation_context;
mod market_creation_service;
mod market_creation_endpoints;
mod trade_execution_service;
mod trade_execution_endpoints;
mod settlement_service;
mod settlement_endpoints;
mod handler_adapters;
mod test_data_manager;
mod test_data_endpoints;
mod mock_services;
mod mock_config;
mod mock_service_manager;
mod health_check_service;
mod health_check_endpoints;
mod environment_config;
mod environment_config_endpoints;
mod feature_flags;
mod feature_flag_endpoints;
mod feature_flag_middleware;
mod platform;
mod platform_fixes;
mod state_manager;
mod state_management_endpoints;
mod state_sync_middleware;
mod validation_framework;
mod domain_validators;
mod validation_middleware;
mod validation_endpoints;


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
    pub jwt_manager: Arc<jwt_validation::JwtManager>,
    pub authorization_service: Arc<rbac_authorization::AuthorizationService>,
    pub trading_engine: Arc<trading_engine::TradingEngine>,
    pub solana_rpc_service: Option<Arc<solana_rpc_service::SolanaRpcService>>,
    pub solana_tx_manager: Option<Arc<solana_transaction_manager::SolanaTransactionManager>>,
    pub solana_deployment_manager: Option<Arc<solana_deployment_manager::SolanaDeploymentManager>>,
    pub external_api_service: Option<Arc<external_api_service::ExternalApiService>>,
    pub circuit_breaker_manager: Option<Arc<circuit_breaker::CircuitBreakerManager>>,
    pub service_circuit_breakers: Option<Arc<circuit_breaker_middleware::ServiceCircuitBreakers>>,
    pub tracing_logger: Option<Arc<tracing_logger::TracingLogger>>,
    pub market_creation_service: Option<Arc<market_creation_service::MarketCreationService>>,
    pub trade_execution_service: Option<Arc<trade_execution_service::TradeExecutionService>>,
    pub settlement_service: Option<Arc<settlement_service::SettlementService>>,
    pub test_data_manager: Option<Arc<test_data_manager::TestDataManager>>,
    pub mock_service_manager: Option<Arc<mock_service_manager::MockServiceManager>>,
    pub health_check_service: Option<Arc<health_check_service::HealthCheckService>>,
    pub environment_config: Option<Arc<environment_config::EnvironmentConfigService>>,
    pub feature_flags: Option<Arc<feature_flags::FeatureFlagService>>,
    pub state_manager: Option<Arc<state_manager::StateManager>>,
    pub validation_service: Option<Arc<validation_framework::ValidationService>>,
    // Polymarket CLOB trading client (authenticated; optional in demo mode)
    pub polymarket_clob_client: Option<Arc<integration::polymarket_clob::PolymarketClobClient>>,
}

// Custom extractor for auth and correlation ID
#[derive(Debug, Clone)]
struct AuthAndCorrelation {
    user: jwt_validation::AuthenticatedUser,
    correlation_id: tracing_logger::CorrelationId,
}

#[async_trait::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthAndCorrelation
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = axum::response::Response;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let headers = &parts.headers;
        
        // Extract authorization header
        let auth_header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| typed_errors::AppError::new(
                typed_errors::ErrorKind::Unauthorized,
                "Missing authorization header",
                typed_errors::ErrorContext::new("auth", "extract_auth_and_correlation"),
            ).into_response())?;
        
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| typed_errors::AppError::new(
                typed_errors::ErrorKind::Unauthorized,
                "Invalid authorization header format",
                typed_errors::ErrorContext::new("auth", "extract_auth_and_correlation"),
            ).into_response())?;
        
        // Validate JWT
        let claims = app_state.jwt_manager
            .validate_token(token)
            .map_err(|e| typed_errors::AppError::new(
                typed_errors::ErrorKind::Unauthorized,
                format!("Token validation failed: {}", e),
                typed_errors::ErrorContext::new("auth", "extract_auth_and_correlation"),
            ).into_response())?;
        
        let user = jwt_validation::AuthenticatedUser { claims };
        
        // Extract correlation ID
        let correlation_id = headers
            .get("x-correlation-id")
            .and_then(|h| h.to_str().ok())
            .map(|s| tracing_logger::CorrelationId(s.to_string()))
            .unwrap_or_else(|| tracing_logger::CorrelationId(uuid::Uuid::new_v4().to_string()));
        
        Ok(AuthAndCorrelation { user, correlation_id })
    }
}

// FromRef implementations for extractors
impl FromRef<AppState> for Arc<jwt_validation::JwtManager> {
    fn from_ref(state: &AppState) -> Self {
        state.jwt_manager.clone()
    }
}

fn main() -> Result<()> {
    // Create custom runtime with more worker threads for better concurrency
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(32) // Increased from default to handle concurrent requests
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> Result<()> {
    // Initialize enhanced tracing
    tracing_logger::TracingLogger::init_subscriber();

    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize Solana RPC client
    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string());
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        rpc_url.clone(),
        CommitmentConfig::confirmed(),
    ));

    // Program ID from environment or use test ID
    let program_id = std::env::var("PROGRAM_ID")
        .unwrap_or_else(|_| "HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca".to_string());
    let program_id = Pubkey::from_str(&program_id)?;

    info!("Connecting to RPC: {}", rpc_url);
    info!("Program ID: {}", program_id);

    // Initialize WebSocket manager
    let ws_manager = Arc::new(websocket::WebSocketManager::new());
    
    // Initialize enhanced WebSocket manager
    let enhanced_ws_manager = Arc::new(websocket::enhanced::EnhancedWebSocketManager::new());

    // Initialize integration config
    let integration_config = integration::IntegrationConfig {
        polymarket_enabled: std::env::var("POLYMARKET_ENABLED")
            .unwrap_or_else(|_| "true".to_string()) == "true",
        polymarket_api_key: std::env::var("POLYMARKET_API_KEY").ok(),
        polymarket_webhook_secret: std::env::var("POLYMARKET_WEBHOOK_SECRET").ok(),
        kalshi_enabled: std::env::var("KALSHI_ENABLED")
            .unwrap_or_else(|_| "true".to_string()) == "true",
        kalshi_api_key: std::env::var("KALSHI_API_KEY").ok(),
        kalshi_api_secret: std::env::var("KALSHI_API_SECRET").ok(),
        sync_interval_seconds: std::env::var("SYNC_INTERVAL_SECONDS")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60),
        max_price_deviation: 0.05,
        min_liquidity_usd: 10_000.0,
    };

    // Initialize price feed service
    let price_feed = Arc::new(integration::PriceFeedService::new());

    // Initialize market sync service
    let platform_client = Arc::new(rpc_client::BettingPlatformClient::new(
        rpc_client.clone(),
        program_id,
    ));
    
    // Initialize market sync service
    let market_sync = if integration_config.polymarket_enabled || integration_config.kalshi_enabled {
        match integration::MarketSyncService::new(integration_config.clone(), platform_client.clone()) {
            Ok(service) => {
                info!("Market sync service initialized");
                Some(Arc::new(service))
            }
            Err(e) => {
                tracing::error!("Failed to initialize market sync service: {}", e);
                None
            }
        }
    } else {
        info!("Market sync service disabled");
        None
    };
    
    // Start market sync service if enabled
    if let Some(ref sync_service) = market_sync {
        if let Err(e) = sync_service.start().await {
            tracing::error!("Failed to start market sync service: {}", e);
        } else {
            info!("Market sync service started");
        }
    }

    // Initialize order matching engine
    let order_engine = Arc::new(order_types::OrderMatchingEngine::new());
    
    // Initialize quantum engine
    let quantum_engine = Arc::new(quantum_engine::QuantumEngine::new());
    
    // Initialize risk engine
    let risk_engine = Arc::new(risk_engine::RiskEngine::new());
    
    // Initialize seeded markets
    let seeded_markets = Arc::new(seed_markets::SeededMarketStore::new());
    
    // Initialize wallet verification service
    let wallet_verification = Arc::new(wallet_verification::WalletVerificationService::new());
    
    // Start wallet verification cleanup task
    wallet_verification::WalletVerificationService::start_cleanup_task(wallet_verification.clone());
    
    // Initialize cache service
    let cache_config = cache::CacheConfig {
        redis_url: std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        enabled: std::env::var("CACHE_ENABLED").unwrap_or_else(|_| "true".to_string()) == "true",
        default_ttl: std::env::var("CACHE_TTL").unwrap_or_else(|_| "300".to_string()).parse().unwrap_or(300),
        ..Default::default()
    };
    let cache = Arc::new(cache::CacheService::new(cache_config).await?);
    
    // Initialize database with optimized configuration
    let expected_load = std::env::var("EXPECTED_CONCURRENT_USERS")
        .unwrap_or_else(|_| "2000".to_string())
        .parse()
        .unwrap_or(2000);
    
    let optimized_config = if expected_load > 2000 {
        db::pool_optimization::OptimizedPoolConfig::high_load()
    } else if expected_load > 500 {
        db::pool_optimization::OptimizedPoolConfig::medium_load()
    } else {
        db::pool_optimization::OptimizedPoolConfig::low_load()
    };
    
    info!("Using optimized database pool for {} concurrent users", expected_load);
    info!("Pool configuration: max_connections={}, min_idle={}", 
        optimized_config.max_connections, optimized_config.min_idle);
    
    let db_config = db::DatabaseConfig {
        url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://betting_user:betting_pass@localhost/betting_platform".to_string()
        }),
        max_connections: std::env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| optimized_config.max_connections.to_string())
            .parse()
            .unwrap_or(optimized_config.max_connections),
        min_connections: std::env::var("DB_MIN_CONNECTIONS")
            .unwrap_or_else(|_| optimized_config.min_idle.to_string())
            .parse()
            .unwrap_or(optimized_config.min_idle),
        connection_timeout: optimized_config.connection_timeout,
        idle_timeout: optimized_config.idle_timeout,
        max_lifetime: optimized_config.max_lifetime,
    };
    
    // Initialize database with fallback support
    let database = Arc::new(db::fallback::FallbackDatabase::new(db_config).await?);
    
    // Try to run migrations if database is available
    if let Err(e) = database.run_migrations().await {
        tracing::warn!("Failed to run migrations: {}. Database may be partially functional.", e);
    }
    
    if database.is_degraded().await {
        tracing::warn!("API running in degraded mode without database. Using fallback data sources.");
    } else {
        info!("Database initialized successfully");
    }
    
    // Initialize funded trading client if enabled
    let funded_trading_client = if std::env::var("ENABLE_AUTO_FUNDING").unwrap_or_else(|_| "false".to_string()) == "true" {
        let funding_config = solana_funding::FundingConfig {
            airdrop_amount: 1_000_000_000, // 1 SOL
            min_balance_threshold: 100_000_000, // 0.1 SOL
            auto_fund_enabled: true,
            funding_source: std::env::var("FUNDING_SOURCE_KEY").ok(),
        };
        
        Some(Arc::new(solana_funding::FundedTradingClient::new(
            rpc_url.clone(),
            program_id,
            funding_config
        )))
    } else {
        None
    };
    
    // Initialize Polymarket public client (no auth required)
    let polymarket_public_client = Arc::new(
        integration::polymarket_public::PolymarketPublicClient::new()
            .expect("Failed to create Polymarket public client")
    );
    
    // Initialize Polymarket price feed
    let polymarket_price_feed = if integration_config.polymarket_enabled {
        let feed = Arc::new(integration::polymarket_price_feed::PolymarketPriceFeed::new(
            polymarket_public_client.clone(),
            price_feed.clone(),
            30, // Update every 30 seconds
        ));
        
        // Start the price feed
        if let Err(e) = feed.start().await {
            tracing::error!("Failed to start Polymarket price feed: {}", e);
            None
        } else {
            info!("Polymarket real-time price feed started");
            Some(feed)
        }
    } else {
        None
    };
    
    // Initialize environment configuration service
    let config_dir = std::path::PathBuf::from(
        std::env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string())
    );
    
    let environment_config = match environment_config::EnvironmentConfigService::new(config_dir) {
        Ok(service) => {
            let service_arc = Arc::new(service);
            
            // Start watching for config changes
            if env::var("CONFIG_WATCH_ENABLED").unwrap_or_else(|_| "true".to_string()) == "true" {
                service_arc.clone().watch_for_changes().await;
                info!("Environment configuration service initialized with file watching");
            } else {
                info!("Environment configuration service initialized");
            }
            
            Some(service_arc)
        }
        Err(e) => {
            tracing::warn!("Failed to initialize environment configuration service: {}. Using defaults.", e);
            None
        }
    };
    
    // Initialize feature flag service
    let mut feature_flag_service = feature_flags::FeatureFlagService::new(environment_config.clone());
    feature_flag_service.init_default().await;
    let feature_flags = Some(Arc::new(feature_flag_service));
    info!("Feature flag service initialized");
    
    // Initialize state manager
    let state_manager_config = state_manager::StateManagerConfig {
        snapshot_interval: std::time::Duration::from_secs(
            std::env::var("STATE_SNAPSHOT_INTERVAL_SECS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300)
        ),
        max_snapshots: std::env::var("STATE_MAX_SNAPSHOTS")
            .unwrap_or_else(|_| "100".to_string())
            .parse()
            .unwrap_or(100),
        enable_persistence: std::env::var("STATE_PERSISTENCE_ENABLED")
            .unwrap_or_else(|_| "true".to_string()) == "true",
        broadcast_changes: std::env::var("STATE_BROADCAST_CHANGES")
            .unwrap_or_else(|_| "true".to_string()) == "true",
    };
    
    let state_manager = state_manager::StateManager::new(state_manager_config);
    let state_manager = Some(Arc::new(state_manager));
    info!("State manager initialized");
    
    // Initialize validation service
    let validation_cache_enabled = std::env::var("VALIDATION_CACHE_ENABLED")
        .unwrap_or_else(|_| "true".to_string()) == "true";
    let validation_cache_ttl = std::time::Duration::from_secs(
        std::env::var("VALIDATION_CACHE_TTL_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .unwrap_or(300)
    );
    
    let validation_service = validation_middleware::initialize_validation_service(
        validation_cache_enabled,
        validation_cache_ttl,
    ).await;
    let validation_service = Some(validation_service);
    info!("Validation service initialized with cache: {}, TTL: {}s", validation_cache_enabled, validation_cache_ttl.as_secs());
    
    // Initialize external API service
    let external_api_service = if integration_config.polymarket_enabled || integration_config.kalshi_enabled {
        let service = Arc::new(external_api_service::ExternalApiService::new(integration_config.clone()));
        
        // Initialize the service
        if let Err(e) = service.initialize().await {
            tracing::error!("Failed to initialize external API service: {}", e);
            None
        } else {
            // Start health monitoring
            service.start_health_monitoring().await;
            info!("External API service initialized with health monitoring");
            Some(service)
        }
    } else {
        info!("External API service disabled");
        None
    };

    // Initialize authenticated Polymarket CLOB client (optional; required for order endpoints).
    let polymarket_clob_client = if integration_config.polymarket_enabled {
        let testnet = std::env::var("POLYMARKET_TESTNET")
            .unwrap_or_else(|_| "false".to_string())
            .eq_ignore_ascii_case("true");

        match integration::polymarket_auth::PolymarketAuthConfig::from_env() {
            Ok(auth_config) => match integration::polymarket_clob::PolymarketClobClient::new(auth_config, testnet) {
                Ok(client) => {
                    info!("Polymarket CLOB client initialized");
                    Some(Arc::new(client))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize Polymarket CLOB client: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::info!("Polymarket auth not configured (order endpoints disabled): {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Initialize security logger
    let security_logger_config = security::security_logger::SecurityLoggerConfig {
        log_file_path: std::env::var("SECURITY_LOG_PATH").unwrap_or_else(|_| "logs/security.log".to_string()),
        max_file_size: std::env::var("SECURITY_LOG_MAX_SIZE")
            .unwrap_or_else(|_| "104857600".to_string()) // 100MB default
            .parse()
            .unwrap_or(104857600),
        rotation_enabled: true,
        retention_days: std::env::var("SECURITY_LOG_RETENTION_DAYS")
            .unwrap_or_else(|_| "90".to_string())
            .parse()
            .unwrap_or(90),
        alerts_enabled: std::env::var("SECURITY_ALERTS_ENABLED").unwrap_or_else(|_| "true".to_string()) == "true",
        alert_threshold: 0.8,
        event_rate_limit: 100,
    };
    
    let security_logger = Arc::new(security::security_logger::SecurityLogger::new(security_logger_config));
    info!("Security logger initialized");
    
    // Initialize queue service
    let queue_config = queue::QueueConfig {
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
    
    let queue_service = if queue_config.enabled {
        match queue::QueueService::new(queue_config).await {
            Ok(service) => {
                info!("Queue service initialized");
                Some(Arc::new(service))
            }
            Err(e) => {
                tracing::error!("Failed to initialize queue service: {}", e);
                None
            }
        }
    } else {
        info!("Queue service disabled");
        None
    };
    
    // Initialize circuit breaker manager
    let circuit_breaker_config = circuit_breaker_middleware::create_default_circuit_breaker_config();
    let circuit_breaker_manager = Arc::new(circuit_breaker::CircuitBreakerManager::new(circuit_breaker_config));
    info!("Circuit breaker manager initialized");
    
    // Initialize service circuit breakers
    let service_circuit_breakers = Arc::new(circuit_breaker_middleware::ServiceCircuitBreakers::new());
    info!("Service circuit breakers initialized");
    
    // Initialize JWT manager
    let jwt_config = jwt_validation::JwtConfig {
        secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            tracing::warn!("JWT_SECRET not set, using default (NOT FOR PRODUCTION)");
            "your-256-bit-secret-key-change-this-in-production".to_string()
        }),
        expiration_minutes: std::env::var("JWT_EXPIRATION_MINUTES")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60),
        refresh_expiration_days: std::env::var("JWT_REFRESH_EXPIRATION_DAYS")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30),
        issuer: "betting-platform".to_string(),
    };
    let jwt_manager = Arc::new(jwt_validation::JwtManager::new(jwt_config));
    info!("JWT validation configured with {} minute token expiration", 60);
    
    // Initialize authorization service
    let authorization_service = Arc::new(rbac_authorization::AuthorizationService::new());
    info!("RBAC authorization service initialized");
    
    // Initialize trading engine
    let trading_engine = trading_api::init_trading_engine(None);
    info!("Trading engine initialized with order matching");
    
    // Initialize Solana RPC service
    let (solana_rpc_service, solana_tx_manager, solana_deployment_manager) = 
        match initialize_solana_services(&rpc_url, program_id).await {
            Ok((rpc, tx)) => {
                let deployment_manager = Arc::new(solana_deployment_manager::SolanaDeploymentManager::new(
                    rpc.clone(),
                    tx.clone(),
                ));
                (Some(rpc), Some(tx), Some(deployment_manager))
            },
            Err(e) => {
                tracing::warn!("Failed to initialize Solana services: {}. Running in degraded mode.", e);
                (None, None, None)
            }
        };
    
    // Create app state
    let mut state = AppState {
        rpc_client,
        program_id,
        ws_manager: ws_manager.clone(),
        enhanced_ws_manager: Some(enhanced_ws_manager.clone()),
        platform_client,
        integration_config,
        market_sync,
        price_feed: Some(price_feed),
        order_engine,
        quantum_engine,
        risk_engine,
        funded_trading_client,
        seeded_markets,
        wallet_verification,
        cache,
        polymarket_public_client,
        polymarket_price_feed,
        database,
        queue_service,
        security_logger,
        jwt_manager,
        authorization_service,
        trading_engine,
        solana_rpc_service,
        solana_tx_manager,
        solana_deployment_manager,
        external_api_service,
        circuit_breaker_manager: Some(circuit_breaker_manager),
        service_circuit_breakers: Some(service_circuit_breakers),
        tracing_logger: Some(Arc::new(tracing_logger::TracingLogger::new(tracing::Level::INFO))),
        market_creation_service: None, // Will be initialized after state creation
        trade_execution_service: None, // Will be initialized after state creation
        settlement_service: None, // Will be initialized after state creation
        test_data_manager: None, // Will be initialized after state creation
        mock_service_manager: None, // Will be initialized after state creation
        health_check_service: None, // Will be initialized after state creation
        environment_config,
        feature_flags,
        state_manager,
        validation_service,
        polymarket_clob_client,
    };

    // Start WebSocket broadcast task
    let ws_state = state.clone();
    tokio::spawn(async move {
        websocket::start_market_updates(ws_state).await;
    });
    
    // Start enhanced WebSocket updates
    let enhanced_ws_state = state.clone();
    tokio::spawn(async move {
        websocket::enhanced::start_enhanced_market_updates(enhanced_ws_state).await;
    });
    
    // Initialize real-time event system
    let real_time_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = websocket::real_events::initialize_real_time_events(real_time_state).await {
            tracing::error!("Failed to initialize real-time events: {}", e);
        }
    });
    info!("Real-time event system initialized");
    
    // Start queue workers if enabled
    if state.queue_service.is_some() {
        let worker_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = queue::worker::start_queue_workers(worker_state).await {
                tracing::error!("Failed to start queue workers: {}", e);
            }
        });
        info!("Queue workers started");
    }
    
    // Start enhanced WebSocket v3 background tasks
    let ws_v3_state = state.clone();
    tokio::spawn(async move {
        websocket_server::start_websocket_tasks(ws_v3_state).await;
    });
    info!("WebSocket v3 background tasks started");

    // Initialize market creation service
    if let Some(solana_rpc) = state.solana_rpc_service.clone() {
        let market_creation_service = Arc::new(market_creation_service::MarketCreationService::new(
            solana_rpc.clone(),
            state.database.clone(),
            state.enhanced_ws_manager.clone().unwrap_or_else(|| {
                Arc::new(websocket::enhanced::EnhancedWebSocketManager::new())
            }),
            state.tracing_logger.clone().unwrap_or_else(|| {
                Arc::new(tracing_logger::TracingLogger::new(tracing::Level::INFO))
            }),
            program_id,
        ));
        
        // Initialize trade execution service
        let trade_execution_service = Arc::new(trade_execution_service::TradeExecutionService::new(
            state.trading_engine.clone(),
            state.risk_engine.clone(),
            solana_rpc.clone(),
            state.database.clone(),
            state.enhanced_ws_manager.clone().unwrap_or_else(|| {
                Arc::new(websocket::enhanced::EnhancedWebSocketManager::new())
            }),
            state.tracing_logger.clone().unwrap_or_else(|| {
                Arc::new(tracing_logger::TracingLogger::new(tracing::Level::INFO))
            }),
            state.service_circuit_breakers.clone().unwrap_or_else(|| {
                Arc::new(circuit_breaker_middleware::ServiceCircuitBreakers::new())
            }),
            program_id,
        ));
        
        // Initialize settlement service
        let settlement_authority = if let Ok(keypair_str) = std::env::var("SETTLEMENT_KEYPAIR") {
            // Try to decode the keypair from base58
            match bs58::decode(&keypair_str).into_vec() {
                Ok(bytes) => match Keypair::from_bytes(&bytes) {
                    Ok(kp) => kp,
                    Err(_) => {
                        tracing::warn!("Invalid SETTLEMENT_KEYPAIR format, using random keypair");
                        Keypair::new()
                    }
                },
                Err(_) => {
                    tracing::warn!("Failed to decode SETTLEMENT_KEYPAIR, using random keypair");
                    Keypair::new()
                }
            }
        } else {
            tracing::warn!("SETTLEMENT_KEYPAIR not set, using random keypair (NOT FOR PRODUCTION)");
            Keypair::new()
        };
        
        let settlement_service = Arc::new(settlement_service::SettlementService::new(
            solana_rpc.clone(),
            state.database.clone(),
            state.trading_engine.clone(),
            state.enhanced_ws_manager.clone(),
            program_id,
            settlement_authority.pubkey(),
        ));
        
        // Update state services
        state.market_creation_service = Some(market_creation_service);
        state.trade_execution_service = Some(trade_execution_service);
        state.settlement_service = Some(settlement_service);
        
        // Initialize test data manager (only in development)
        if env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()) == "development" {
            let test_data_config = test_data_manager::TestDataConfig {
                auto_cleanup: true,
                cleanup_interval_minutes: 30,
                default_expiry_minutes: 120,
                database_prefix: "test_".to_string(),
                seed_data_path: None,
            };
            
            match test_data_manager::TestDataManager::new(test_data_config, state.database.clone()).await {
                Ok(manager) => {
                    let manager_arc = Arc::new(manager);
                    manager_arc.start_cleanup_task();
                    state.test_data_manager = Some(manager_arc);
                    info!("Test data manager initialized");
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize test data manager: {}", e);
                }
            }
            
            // Initialize mock service manager
            let mock_enabled = env::var("MOCK_SERVICES_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true";
            if mock_enabled {
                let mock_config = mock_config::load_mock_config();
                let mut mock_manager = mock_service_manager::MockServiceManager::new(mock_config);
                
                if let Err(e) = mock_manager.initialize().await {
                    tracing::warn!("Failed to initialize mock services: {}", e);
                } else {
                    // Inject mock services into app state
                    if let Err(e) = mock_manager.inject_into_app_state(&mut state).await {
                        tracing::warn!("Failed to inject mock services: {}", e);
                    } else {
                        state.mock_service_manager = Some(Arc::new(mock_manager));
                        info!("Mock services initialized and injected");
                    }
                }
            }
        }
        
        info!("Market creation, trade execution, and settlement services initialized");
    } else {
        tracing::warn!("Solana RPC or trading engine not available, market creation and trade execution services not initialized");
    }
    
    // Initialize health check service
    let health_check_config = health_check_service::HealthCheckConfig::default();
    let health_service = Arc::new(health_check_service::HealthCheckService::new(health_check_config));
    
    // Register components for health checking
    health_service.register_component("database".to_string(), state.database.clone()).await;
    health_service.register_component("trading_engine".to_string(), state.trading_engine.clone()).await;
    
    if let Some(solana_rpc) = &state.solana_rpc_service {
        health_service.register_component("solana_rpc".to_string(), solana_rpc.clone()).await;
    }
    
    if let Some(ws_manager) = &state.enhanced_ws_manager {
        health_service.register_component("websocket".to_string(), ws_manager.clone()).await;
    }
    
    if let Some(circuit_breakers) = &state.circuit_breaker_manager {
        health_service.register_component("circuit_breakers".to_string(), circuit_breakers.clone()).await;
    }
    
    if let Some(external_api) = &state.external_api_service {
        health_service.register_component("external_apis".to_string(), external_api.clone()).await;
    }
    
    // Start background health checks
    health_service.clone().start_background_checks();
    
    // Add to state
    state.health_check_service = Some(health_service);
    
    info!("Health check service initialized with {} components", 6);
    
    // Start state synchronization background tasks
    let state_arc = Arc::new(state.clone());
    if state_arc.state_manager.is_some() {
        state_sync_middleware::start_state_sync_tasks(state_arc.clone());
        info!("State synchronization background tasks started");
    }

    // Create rate limiting layer
    let rate_limit_layer = simple_rate_limit::create_rate_limit_layer();
    
    // Create optimized layers for high throughput
    let optimized_layers = throughput_optimization::create_optimized_layers();
    
    // Build the router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        
        // Auth endpoints
        .route("/api/auth/login", post(auth_endpoints::login))
        .route("/api/auth/refresh", post(auth_endpoints::refresh_token))
        // .route("/api/auth/logout", post(handler_adapters::logout_adapter))
        // .route("/api/auth/user", get(handler_adapters::get_user_info_adapter))
        .route("/api/auth/validate", post(auth_endpoints::validate_token))
        
        // RBAC endpoints
        // .route("/api/rbac/permissions", get(handler_adapters::get_user_permissions_adapter))
        // .route("/api/rbac/grant-permission", post(handler_adapters::grant_permission_adapter))
        // .route("/api/rbac/update-role", post(handler_adapters::update_user_role_adapter))
        
        // RBAC-protected endpoints
        // .route("/api/admin/positions/all", get(handler_adapters::view_all_positions_adapter))
        // .route("/api/admin/system/config", post(handler_adapters::update_system_config_adapter))
        // .route("/api/markets/create-authorized", post(handler_adapters::create_market_authorized_adapter))
        
        // Program info
        .route("/api/program/info", get(handlers::get_program_info))
        
        // Polymarket proxy
        .route("/api/polymarket/markets", get(handlers::proxy_polymarket_markets))
        
        // Market endpoints
        .route("/api/markets", get(handlers::get_markets))
        .route("/api/markets/:id", get(handlers::get_market))
        .route("/api/markets/create", post(handlers::create_market))
        .route("/api/markets/:id/orderbook", get(handlers::get_market_orderbook))
        
        // Enhanced market endpoints
        .route("/api/v2/markets", get(market_handlers::get_markets_enhanced))
        .route("/api/v2/markets/:id", get(market_handlers::get_market_by_id))
        .route("/api/v2/markets/stats", get(market_handlers::get_market_statistics))
        
        // Trading endpoints
        .route("/api/trade/place", post(handlers::place_trade))
        .route("/api/trade/place-funded", post(handlers::place_funded_trade))
        .route("/api/trade/close", post(handlers::close_position))
        
        // Trading engine endpoints (v2)
        // .route("/api/v2/orders", post(handler_adapters::place_order_adapter))
        // .route("/api/v2/orders", get(handler_adapters::get_user_orders_adapter))
        // .route("/api/v2/orders/:order_id/cancel", post(handler_adapters::cancel_order_adapter))
        .route("/api/v2/orderbook/:market_id/:outcome", get(trading_api::get_order_book))
        .route("/api/v2/trades/:market_id", get(trading_api::get_recent_trades))
        .route("/api/v2/ticker/:market_id", get(trading_api::get_market_ticker))
        
        // Trading endpoints (for test compatibility)
        .route("/trades", post(trading_handlers::place_trade))
        .route("/trades/history", get(trading_handlers::get_trade_history))
        .route("/trades/:order_id/cancel", post(trading_handlers::cancel_order))
        
        // Position endpoints
        .route("/api/positions/:wallet", get(handlers::get_positions))
        .route("/api/positions", get(handlers::get_positions_query))
        
        // Position management endpoints (for test compatibility)
        .route("/positions", get(position_handlers::get_positions))
        .route("/positions/:id/partial-close", post(position_handlers::partial_close_position))
        .route("/positions/:id/close", post(position_handlers::close_position))
        .route("/positions/pnl", get(position_handlers::get_pnl))
        .route("/api/portfolio/:wallet", get(handlers::get_portfolio))
        .route("/api/risk/:wallet", get(handlers::get_risk_metrics))
        
        // Wallet endpoints
        .route("/api/wallet/balance/:wallet", get(handlers::get_balance))
        .route("/api/wallet/demo/create", post(handlers::create_demo_account))
        .route("/api/demo/create", post(handlers::create_demo_account)) // Alias for tests
        
        // Polygon wallet endpoints
        .route("/api/wallet/polygon/balance/:address", get(handlers::wallet_http::get_wallet_balance))
        .route("/api/wallet/polygon/outcome-balance/:address", get(handlers::wallet_http::get_outcome_balance))
        .route("/api/wallet/polygon/gas-price", get(handlers::wallet_http::get_gas_price))
        .route("/api/wallet/polygon/transaction", get(handlers::wallet_http::get_transaction_receipt))
        .route("/api/wallet/polygon/estimate-gas", post(handlers::wallet_http::estimate_gas_approval))
        .route("/api/wallet/polygon/nonce/:address", get(handlers::wallet_http::get_wallet_nonce))
        
        // Settlement endpoints
        .route("/api/settlement/status/:market_id", get(handlers::settlement::get_settlement_status))
        .route("/api/settlement/user/:wallet", get(handlers::settlement::get_user_settlements))
        .route("/api/settlement/pending", get(handlers::settlement::get_pending_settlements))
        .route("/api/settlement/webhook", post(handlers::settlement::handle_settlement_webhook))
        .route("/api/settlement/historical", get(handlers::settlement::get_historical_settlements))
        .route("/api/settlement/oracle/:market_id", get(handlers::settlement::get_settlement_oracle))
        
        // Test data management endpoints (development only)
        
        // Verse endpoints
        .route("/api/verses", get(handlers::get_verses))
        .route("/api/verses/:id", get(handlers::get_verse))
        
        // Quantum endpoints
        .route("/api/quantum/positions/:wallet", get(handlers::get_quantum_positions))
        .route("/api/quantum/create", post(handlers::create_quantum_position))
        .route("/api/quantum/states/:market_id", get(handlers::get_quantum_states))
        
        // Quantum settlement endpoints
        .route("/api/quantum/settlement/position", post(handlers::quantum_settlement_handlers::settle_quantum_position))
        .route("/api/quantum/settlement/market", post(handlers::quantum_settlement_handlers::settle_market_quantum_positions))
        .route("/api/quantum/settlement/history", get(handlers::quantum_settlement_handlers::get_quantum_settlements))
        .route("/api/quantum/settlement/status/:market_id", get(handlers::quantum_settlement_handlers::get_quantum_settlement_status))
        .route("/api/quantum/settlement/trigger", post(handlers::quantum_settlement_handlers::trigger_quantum_settlement))
        
        // DeFi endpoints
        .route("/api/defi/stake", post(handlers::stake_mmt))
        .route("/api/defi/pools", get(handlers::get_liquidity_pools))
        
        // Liquidity management endpoints (for test compatibility)
        .route("/liquidity/add", post(liquidity_handlers::add_liquidity))
        .route("/liquidity/remove", post(liquidity_handlers::remove_liquidity))
        .route("/liquidity/stats", get(liquidity_handlers::get_liquidity_stats))
        .route("/liquidity/pools", get(liquidity_handlers::get_all_pools))
        
        // Staking endpoints (for test compatibility)
        .route("/staking/stake", post(staking_handlers::stake_tokens))
        .route("/staking/unstake", post(staking_handlers::unstake_tokens))
        .route("/staking/rewards", get(staking_handlers::get_rewards))
        .route("/staking/rewards/claim", post(staking_handlers::claim_rewards))
        .route("/staking/pools", get(staking_handlers::get_staking_pools))
        
        // Quantum trading endpoints (for test compatibility)
        .route("/quantum/trade", post(quantum_handlers::execute_quantum_trade))
        .route("/quantum/correlations", get(quantum_handlers::get_quantum_correlations))
        .route("/quantum/adjust", post(quantum_handlers::adjust_quantum_position))
        .route("/quantum/collapse", post(quantum_handlers::collapse_quantum_position))
        
        // Risk management endpoints (for test compatibility)
        .route("/risk/limits", post(risk_handlers::set_risk_limits))
        .route("/risk/limits", get(risk_handlers::get_risk_limits))
        .route("/risk/margin", get(risk_handlers::get_margin_status))
        .route("/risk/simulate-shock", post(risk_handlers::simulate_shock))
        .route("/risk/auto-deleverage", post(risk_handlers::auto_deleverage))
        .route("/risk/test-liquidation", post(risk_handlers::test_liquidation))
        
        // WebSocket endpoints
        .route("/ws", get(ws_handler))
        .route("/ws/v2", get(enhanced_ws_handler))
        .route("/ws/v3", get(websocket_server::handle_websocket_upgrade))
        
        // Integration endpoints
        .route("/api/integration/status", get(handlers::integration_simple::get_integration_status))
        .route("/api/integration/sync", post(handlers::integration_simple::sync_external_markets))
        .route("/api/integration/polymarket/markets", get(handlers::integration_simple::get_polymarket_markets_enhanced))
        
        // Real-time price feed endpoints
        .route("/api/prices/:market_id", get(handlers::price_feed::get_market_price))
        .route("/api/prices/track", post(handlers::price_feed::track_market_prices))
        .route("/api/prices/ws", get(handlers::price_feed::price_feed_websocket))
        
        // Test endpoints
        .route("/api/test/verse-match", post(handlers::test_verse_match))
        
        // Order endpoints
        .route("/api/orders/limit", post(handlers::place_limit_order))
        .route("/api/orders/stop", post(handlers::place_stop_order))
        .route("/api/orders/:order_id/cancel", post(handlers::cancel_order))
        .route("/api/orders/:wallet", get(handlers::get_orders))
        
        // Polymarket order endpoints
        .route("/api/orders/submit", post(submit_order_handler))
        .route("/api/orders/:order_id/status", get(get_order_status_handler))
        .route("/api/orders/:order_id/cancel", delete(cancel_order_handler))
        .route("/api/orders", get(get_open_orders_handler))
        
        // Wallet verification endpoints
        .route("/api/wallet/challenge/:wallet", get(handlers::generate_wallet_challenge))
        .route("/api/wallet/verify", post(handlers::verify_wallet_signature))
        .route("/api/wallet/status/:wallet", get(handlers::check_wallet_verification))
        
        // Authentication endpoints (for test compatibility)
        .route("/auth/wallet", post(auth_handlers::authenticate_wallet))
        .route("/auth/refresh", post(auth_handlers::refresh_token))
        .route("/auth/logout", post(auth_handlers::logout))
        .route("/auth/user", post(auth_handlers::get_user_info))
        
        // Transaction signing endpoints
        .route("/api/transaction/prepare", post(transaction_handlers::prepare_transaction))
        .route("/api/transaction/submit", post(transaction_handlers::submit_transaction))
        .route("/api/transaction/status/:signature", get(transaction_handlers::get_transaction_status))
        .route("/api/transaction/estimate-fee", post(transaction_handlers::estimate_transaction_fee))
        
        // Database endpoints
        .route("/api/db/user/login", post(handlers::db_handlers::record_user_login))
        .route("/api/db/user/:wallet/stats", get(handlers::db_handlers::get_user_stats))
        .route("/api/db/trade/record", post(handlers::db_handlers::record_trade))
        .route("/api/db/trades/:wallet", get(handlers::db_handlers::get_user_trades))
        .route("/api/db/status", get(handlers::db_handlers::get_db_status))
        
        // Cache management endpoints
        .route("/api/cache/stats", get(handlers::cache_handlers::get_cache_stats))
        .route("/api/cache/stats/clear", post(handlers::cache_handlers::clear_cache_stats))
        .route("/api/cache/health", get(handlers::cache_handlers::cache_health_check))
        .route("/api/cache/invalidate", post(handlers::cache_handlers::invalidate_cache))
        .route("/api/cache/warm", post(handlers::cache_handlers::warm_cache))
        .route("/api/cache/key/:key", get(handlers::cache_handlers::get_cache_key))
        .route("/api/cache/ttl", post(handlers::cache_handlers::set_cache_ttl))
        .route("/api/cache/clear", post(handlers::cache_handlers::clear_all_cache))
        
        // Queue management endpoints
        .route("/api/queue/stats", get(handlers::queue_handlers::get_queue_stats))
        .route("/api/queue/lengths", get(handlers::queue_handlers::get_queue_lengths))
        .route("/api/queue/publish/test", post(handlers::queue_handlers::publish_test_message))
        .route("/api/queue/publish/delayed", post(handlers::queue_handlers::publish_delayed_message))
        .route("/api/queue/clear/:queue", post(handlers::queue_handlers::clear_queue))
        
        // Security monitoring endpoints
        // .route("/api/security/events", get(handler_adapters::get_security_events_adapter))
        // .route("/api/security/stats", get(handler_adapters::get_security_stats_adapter))
        // .route("/api/security/alerts/config", post(handler_adapters::update_alert_config_adapter))
        // .route("/api/security/ip/:ip", post(handler_adapters::manage_ip_block_adapter))
        // .route("/api/security/search", post(handler_adapters::search_security_logs_adapter))
        // .route("/api/security/export", post(handler_adapters::export_security_logs_adapter))
        // .route("/api/security/dashboard", get(handler_adapters::get_security_dashboard_adapter))
        
        // Solana RPC endpoints
        .route("/api/solana/rpc/health", get(solana_endpoints::get_rpc_health))
        .route("/api/solana/tx/manager-status", get(solana_endpoints::get_transaction_manager_status))
        .route("/api/solana/tx/status", get(solana_endpoints::get_transaction_status_enhanced))
        .route("/api/solana/account/:address", get(solana_endpoints::get_account_info))
        .route("/api/solana/accounts/batch", post(solana_endpoints::get_multiple_accounts))
        .route("/api/solana/blockhash/recent", get(solana_endpoints::get_recent_blockhash))
        // .route("/api/solana/tx/simulate", post(handler_adapters::simulate_transaction_adapter))
        .route("/api/solana/program/:program_id/accounts", get(solana_endpoints::get_program_accounts))
        .route("/api/solana/airdrop", post(solana_endpoints::request_airdrop))
        
        // Smart contract deployment endpoints
        // .route("/api/deployment/register", post(handler_adapters::register_program_adapter))
        // .route("/api/deployment/deploy", post(handler_adapters::deploy_program_adapter))
        // .route("/api/deployment/upgrade", post(handler_adapters::upgrade_program_adapter))
        // .route("/api/deployment/initialize", post(handler_adapters::initialize_program_adapter))
        .route("/api/deployment/status/:program_name", get(deployment_endpoints::get_deployment_status))
        .route("/api/deployment/all", get(deployment_endpoints::get_all_deployments))
        .route("/api/deployment/verify/:program_id", get(deployment_endpoints::verify_deployment))
        .route("/api/deployment/manager/status", get(deployment_endpoints::get_deployment_manager_status))
        .route("/api/deployment/idl/:program_name", get(deployment_endpoints::get_program_idl))
        
        // External API routes
        .route("/api/external/health", get(external_api_endpoints::get_external_api_health))
        // .route("/api/external/markets", get(handler_adapters::fetch_external_markets_adapter))
        // .route("/api/external/prices/:platform", post(handler_adapters::get_external_prices_adapter))
        .route("/api/external/sync", post(external_api_endpoints::sync_external_market))
        .route("/api/external/sync/status", get(external_api_endpoints::get_sync_status))
        .route("/api/external/cache/prices", get(external_api_endpoints::get_cached_prices))
        .route("/api/external/sync/toggle", post(external_api_endpoints::toggle_market_sync))
        .route("/api/external/compare", get(external_api_endpoints::compare_markets))
        .route("/api/external/config", post(external_api_endpoints::update_integration_config))
        // .route("/api/external/test/:platform", get(handler_adapters::test_external_api_adapter))
        
        // Circuit breaker routes
        .route("/api/circuit-breakers/health", get(circuit_breaker_middleware::circuit_breaker_health))
        // .route("/api/circuit-breakers/reset", post(handler_adapters::reset_circuit_breakers_adapter))
        
        // Market creation routes
        // .route("/api/markets/create", post(handler_adapters::create_market_adapter))
        // .route("/api/markets/:id/update", put(handler_adapters::update_market_adapter))
        // .route("/api/markets/:id", get(market_creation_endpoints::get_market)) // Duplicate - already defined above
        .route("/api/markets/:id/stats", get(market_creation_endpoints::get_market_stats))
        // .route("/api/markets/list", get(handler_adapters::list_markets_adapter))
        
        // Trade execution routes
        // .route("/api/trades/execute", post(handler_adapters::execute_trade_adapter))
        // .route("/api/trades/orders/:order_id/cancel", delete(trade_execution_endpoints::cancel_order))
        // .route("/api/trades/orders", get(trade_execution_endpoints::get_user_orders))
        // .route("/api/trades/history", get(handler_adapters::get_trade_history_adapter))
        // .route("/api/trades/order-book/:market_id", get(trade_execution_endpoints::get_order_book))
        // .route("/api/trades/stats", get(trade_execution_endpoints::get_execution_stats))
        
        // Settlement routes
        .route("/api/settlement/initiate", post(initiate_settlement_handler))
        .route("/api/settlement/oracles/:market_id", get(query_oracles_handler))
        // .route("/api/settlement/status/:market_id", get(get_settlement_status_handler)) // Duplicate - already defined above
        // .route("/api/settlement/user", get(get_user_settlements_handler)) // TODO: Fix Handler trait
        .route("/api/settlement/history", get(get_settlement_history_handler))
        
        // Test data management routes
        // .route("/api/test-data/create", post(create_test_data_handler)) // TODO: Fix Handler trait
        .route("/api/test-data/list", get(list_test_data_handler))
        .route("/api/test-data/:id", get(get_test_data_handler))
        .route("/api/test-data/cleanup", post(cleanup_test_data_handler))
        .route("/api/test-data/report", get(get_test_data_report_handler))
        .route("/api/test-data/tokens", post(create_test_tokens_handler))
        // .route("/api/test-data/reset", post(reset_test_database_handler)) // TODO: Fix Handler trait
        
        // Mock service routes (only in development with mock services enabled)
        .route("/api/mock/stats", get(get_mock_stats_handler))
        .route("/api/mock/simulate/market", post(simulate_market_activity_handler))
        .route("/api/mock/market/outcome", post(set_market_outcome_handler))
        
        // Health check endpoints
        .route("/api/health/live", get(health_check_endpoints::liveness_probe))
        .route("/api/health/ready", get(readiness_probe_handler))
        .route("/api/health/check", get(comprehensive_health_check_handler))
        .route("/api/health/component/:component", get(get_component_health_handler))
        .route("/api/health/metrics", get(health_metrics_handler))
        .route("/api/health/trigger", post(trigger_health_check_handler))
        .route("/api/health/history", get(get_health_history_handler))
        
        // Environment configuration endpoints (admin only)
        .route("/api/config", get(get_config_handler))
        .route("/api/config/:key", get(get_config_value_handler))
        .route("/api/config/override", post(set_config_override_handler))
        .route("/api/config/reload", post(reload_config_handler))
        .route("/api/config/export", get(export_config_handler))
        .route("/api/config/diff", get(get_config_diff_handler))
        .route("/api/config/validate", get(validate_config_handler))
        
        // Feature flag endpoints
        .route("/api/feature-flags", get(get_flags_handler))
        .route("/api/feature-flags", post(create_flag_handler))
        .route("/api/feature-flags/evaluate", post(evaluate_flags_handler))
        .route("/api/feature-flags/stats", get(get_stats_handler))
        .route("/api/feature-flags/cache/clear", post(clear_cache_handler))
        .route("/api/feature-flags/:name", get(get_flag_handler))
        .route("/api/feature-flags/:name", put(update_flag_handler))
        .route("/api/feature-flags/:name", delete(delete_flag_handler))
        
        // State management endpoints
        .route("/api/v1/state/keys", get(list_state_keys_handler))
        .route("/api/v1/state/stats", get(get_state_stats_handler))
        .route("/api/v1/state/snapshot", post(create_snapshot_handler))
        .route("/api/v1/state/cas", post(compare_and_swap_state_handler))
        .route("/api/v1/state/events", get(state_events_websocket_handler))
        .route("/api/v1/state/:key", get(get_state_handler))
        .route("/api/v1/state/:key", put(set_state_handler))
        .route("/api/v1/state/:key", delete(remove_state_handler))
        
        // Validation management endpoints
        .route("/api/validation/schemas", post(register_schema_handler))
        .route("/api/validation/schemas/:name", get(get_schema_handler))
        .route("/api/validation/validate/:schema", post(validate_data_handler))
        .route("/api/validation/config", put(update_middleware_config_handler))
        .route("/api/validation/stats", get(get_stats_validation_handler))
        .route("/api/validation/cache/clear", post(clear_cache_validation_handler))
        .route("/api/validation/endpoints", post(configure_endpoint_validation_handler))
        
        // Add middleware layers
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            tracing_middleware::tracing_middleware
        ))
        .layer(axum::middleware::from_fn(error_middleware::error_handling_middleware))
        .layer(axum::middleware::from_fn_with_state(
            Arc::new(state.clone()),
            validation_middleware::validation_middleware
        ))
        .layer(axum::middleware::from_fn_with_state(
            Arc::new(state.clone()),
            state_sync_middleware::state_sync_middleware
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security::comprehensive_middleware::comprehensive_security_middleware
        ))
        .layer(optimized_layers)
        .layer(rate_limit_layer)
        .layer(CorsLayer::permissive())
        
        // Add state
        .with_state(state.clone());

    // Start the server (configurable via env)
    let host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    info!("API server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let std_listener = listener.into_std()?;
    
    // Apply TCP optimizations for high throughput
    if let Err(e) = throughput_optimization::optimize_tcp_socket(&std_listener) {
        tracing::warn!("Failed to optimize TCP socket: {}", e);
    }
    
    axum::Server::from_tcp(std_listener)?
        .tcp_nodelay(true)
        .tcp_keepalive(Some(std::time::Duration::from_secs(75)))
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

/// Initialize Solana services with fallback
async fn initialize_solana_services(
    rpc_url: &str,
    program_id: Pubkey,
) -> Result<(Arc<solana_rpc_service::SolanaRpcService>, Arc<solana_transaction_manager::SolanaTransactionManager>)> {
    // Configure Solana RPC service
    let rpc_config = solana_rpc_service::SolanaRpcConfig {
        endpoints: vec![
            rpc_url.to_string(),
            // Add fallback endpoints
            "https://api.mainnet-beta.solana.com".to_string(),
            "https://solana-api.projectserum.com".to_string(),
        ],
        max_retries: 3,
        retry_delay_ms: 1000,
        health_check_interval: std::time::Duration::from_secs(30),
        request_timeout: std::time::Duration::from_secs(30),
        max_concurrent_requests: 100,
        enable_fallback: true,
        commitment: solana_sdk::commitment_config::CommitmentLevel::Confirmed,
    };
    
    let rpc_service = Arc::new(solana_rpc_service::SolanaRpcService::new(rpc_config).await?);
    info!("Solana RPC service initialized with {} endpoints", 3);
    
    // Configure transaction manager
    let tx_config = solana_transaction_manager::TransactionManagerConfig {
        program_id,
        compute_budget_units: 200_000,
        default_priority: solana_transaction_manager::TransactionPriority::Medium,
        enable_versioned_transactions: true,
        enable_priority_fees: true,
        max_transaction_retries: 3,
        confirmation_timeout: std::time::Duration::from_secs(30),
    };
    
    let tx_manager = Arc::new(solana_transaction_manager::SolanaTransactionManager::new(
        tx_config,
        rpc_service.clone(),
    ));
    info!("Solana transaction manager initialized");
    
    Ok((rpc_service, tx_manager))
}

async fn health_check() -> impl IntoResponse {
    // Log memory stats for monitoring
    memory_management::log_memory_stats().await;
    
    Json(serde_json::json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "memory_management": "active",
        "websocket_channel_size": 1000,
        "cache_pool_max": 10
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket::handle_socket(socket, state))
}

async fn enhanced_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket::enhanced::handle_enhanced_socket(socket, state))
}

// Helper function to extract authentication and correlation ID from headers
async fn extract_auth_and_correlation(
    headers: &HeaderMap,
    state: &Arc<AppState>,
) -> Result<(jwt_validation::AuthenticatedUser, tracing_logger::CorrelationId), typed_errors::AppError> {
    // Extract authorization token
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            "Missing authorization header",
            typed_errors::ErrorContext::new("auth", "extract_auth_and_correlation"),
        ))?;
    
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            "Invalid authorization header format",
            typed_errors::ErrorContext::new("auth", "extract_auth_and_correlation"),
        ))?;
    
    // Validate JWT
    let claims = state.jwt_manager
        .validate_token(token)
        .map_err(|e| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            format!("Token validation failed: {}", e),
            typed_errors::ErrorContext::new("auth", "extract_auth_and_correlation"),
        ))?;
    
    let user = jwt_validation::AuthenticatedUser { claims };
    
    // Extract correlation ID
    let correlation_id = headers
        .get("x-correlation-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| tracing_logger::CorrelationId(s.to_string()))
        .unwrap_or_else(|| tracing_logger::CorrelationId(uuid::Uuid::new_v4().to_string()));
    
    Ok((user, correlation_id))
}

// Handler wrappers for test data endpoints
async fn create_test_data_handler(
    State(state): State<AppState>,
    Json(payload): Json<test_data_endpoints::CreateTestDataRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let state_arc = Arc::new(state);
    
    // Create dummy auth for now
    let user = jwt_validation::AuthenticatedUser {
        claims: jwt_validation::JwtClaims {
            sub: "test".to_string(),
            exp: 9999999999,
            iat: 0,
            nbf: 0,
            jti: "test".to_string(),
            wallet: "test".to_string(),
            role: "admin".to_string(),
        }
    };
    let correlation_id = tracing_logger::CorrelationId(uuid::Uuid::new_v4().to_string());
    
    match test_data_endpoints::create_test_data(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
        Json(payload),
    ).await {
        Ok(_response) => Ok(Json(serde_json::json!({"status": "ok"}))),
        Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn list_test_data_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<test_data_endpoints::TestDataQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, correlation_id) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    test_data_endpoints::list_test_data(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
        Query(query),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_test_data_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, correlation_id) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    test_data_endpoints::get_test_data(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
        Path(id),
    ).await
    .map_err(|e| e.into_response())
}

async fn cleanup_test_data_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<test_data_endpoints::CleanupRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, correlation_id) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    test_data_endpoints::cleanup_test_data(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_test_data_report_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, correlation_id) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    test_data_endpoints::get_test_data_report(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
    ).await
    .map_err(|e| e.into_response())
}

async fn create_test_tokens_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, correlation_id) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    test_data_endpoints::create_test_tokens(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn reset_test_database_handler(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let state_arc = Arc::new(state);
    
    // Create dummy auth for now
    let user = jwt_validation::AuthenticatedUser {
        claims: jwt_validation::JwtClaims {
            sub: "test".to_string(),
            exp: 9999999999,
            iat: 0,
            nbf: 0,
            jti: "test".to_string(),
            wallet: "test".to_string(),
            role: "admin".to_string(),
        }
    };
    let correlation_id = tracing_logger::CorrelationId(uuid::Uuid::new_v4().to_string());
    
    match test_data_endpoints::reset_test_database(
        State(state_arc),
        Extension(user),
        Extension(correlation_id),
    ).await {
        Ok(_response) => Ok(Json(serde_json::json!({"status": "ok"}))),
        Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// Handler wrappers for settlement endpoints
async fn initiate_settlement_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<settlement_service::SettlementRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    // Extract role authorization
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            "Missing authorization header",
            typed_errors::ErrorContext::new("settlement", "initiate_settlement"),
        ))?;
    
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            "Invalid authorization header format",
            typed_errors::ErrorContext::new("settlement", "initiate_settlement"),
        ))?;
    
    // Validate JWT and check role
    let claims = state.jwt_manager
        .validate_token(token)
        .map_err(|e| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            format!("Token validation failed: {}", e),
            typed_errors::ErrorContext::new("settlement", "initiate_settlement"),
        ))?;
    
    let user = jwt_validation::AuthenticatedUser { claims };
    
    // Create RequireRole
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    settlement_endpoints::initiate_settlement(
        State(state),
        role,
        Json(payload),
    ).await
}

async fn query_oracles_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(market_id): Path<u128>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let (user, _) = extract_auth_and_correlation(&headers, &Arc::new(state.clone())).await?;
    
    settlement_endpoints::query_oracles(
        State(state),
        Path(market_id),
        user,
    ).await
}

async fn get_settlement_status_handler(
    State(state): State<AppState>,
    Path(market_id): Path<u128>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    settlement_endpoints::get_settlement_status(
        State(state),
        Path(market_id),
    ).await
}

async fn get_user_settlements_handler(
    State(state): State<AppState>,
    Query(query): Query<settlement_endpoints::SettlementQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let state_arc = Arc::new(state.clone());
    
    // Create a dummy authenticated user for now
    let user = jwt_validation::AuthenticatedUser {
        claims: jwt_validation::JwtClaims {
            sub: "test".to_string(),
            exp: 9999999999,
            iat: 0,
            nbf: 0,
            jti: "test".to_string(),
            wallet: "test".to_string(),
            role: "user".to_string(),
        }
    };
    
    match settlement_endpoints::get_user_settlements(
        State(state_arc.as_ref().clone()),
        user,
        Query(query),
    ).await {
        Ok(_response) => Ok(Json(serde_json::json!({"status": "ok"}))),
        Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_settlement_history_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<settlement_endpoints::SettlementQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    // Extract role authorization
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            "Missing authorization header",
            typed_errors::ErrorContext::new("settlement", "get_settlement_history"),
        ).into_response())?;
    
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            "Invalid authorization header format",
            typed_errors::ErrorContext::new("settlement", "get_settlement_history"),
        ).into_response())?;
    
    // Validate JWT and check role
    let claims = state.jwt_manager
        .validate_token(token)
        .map_err(|e| typed_errors::AppError::new(
            typed_errors::ErrorKind::Unauthorized,
            format!("Token validation failed: {}", e),
            typed_errors::ErrorContext::new("settlement", "get_settlement_history"),
        ).into_response())?;
    
    let user = jwt_validation::AuthenticatedUser { claims };
    
    // Create RequireRole
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    settlement_endpoints::get_settlement_history(
        State(state),
        role,
        Query(query),
    ).await
    .map_err(|e| e.into_response())
}

// Handler wrappers for polymarket order endpoints
async fn submit_order_handler(
    State(state): State<AppState>,
    Json(payload): Json<handlers::polymarket_orders::SubmitOrderRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    handlers::polymarket_orders::submit_order(
        State(state_arc),
        Json(payload),
    ).await
}

async fn get_order_status_handler(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    handlers::polymarket_orders::get_order_status(
        State(state_arc),
        Path(order_id),
    ).await
}

async fn cancel_order_handler(
    State(state): State<AppState>,
    Path(order_id): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    handlers::polymarket_orders::cancel_order(
        State(state_arc),
        Path(order_id),
    ).await
}

async fn get_open_orders_handler(
    State(state): State<AppState>,
    Query(params): Query<handlers::polymarket_orders::GetOrdersParams>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    handlers::polymarket_orders::get_open_orders(
        State(state_arc),
        Query(params),
    ).await
}

// Handler wrappers for mock service endpoints
async fn get_mock_stats_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    mock_service_manager::endpoints::get_mock_stats(
        State(state_arc),
        Extension(user),
    ).await
    .map_err(|e| e.into_response())
}

async fn simulate_market_activity_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<mock_service_manager::endpoints::SimulateMarketActivityRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    mock_service_manager::endpoints::simulate_market_activity(
        State(state_arc),
        Extension(user),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn set_market_outcome_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<mock_service_manager::endpoints::SetMarketOutcomeRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    mock_service_manager::endpoints::set_market_outcome(
        State(state_arc),
        Extension(user),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

// Handler wrappers for health check endpoints
async fn readiness_probe_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    health_check_endpoints::readiness_probe(State(state_arc)).await
        .map_err(|e| e.into_response())
}

async fn comprehensive_health_check_handler(
    State(state): State<AppState>,
    Query(params): Query<health_check_endpoints::HealthCheckQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    health_check_endpoints::comprehensive_health_check(State(state_arc), Query(params)).await
        .map_err(|e| e.into_response())
}

async fn get_component_health_handler(
    State(state): State<AppState>,
    Path(component): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    health_check_endpoints::get_component_health(State(state_arc), Path(component)).await
        .map_err(|e| e.into_response())
}

async fn health_metrics_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    health_check_endpoints::health_metrics(State(state_arc)).await
        .map_err(|e| e.into_response())
}

async fn trigger_health_check_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    health_check_endpoints::trigger_health_check(
        State(state_arc),
        Extension(user),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_health_history_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<health_check_endpoints::PaginationParams>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    health_check_endpoints::get_health_history(
        State(state_arc),
        Extension(user),
        Query(params),
    ).await
    .map_err(|e| e.into_response())
}

// Handler wrappers for environment config endpoints
async fn get_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<environment_config_endpoints::ConfigQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::get_config(
        State(state_arc),
        Extension(user),
        Query(params),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_config_value_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::get_config_value(
        State(state_arc),
        Extension(user),
        Path(key),
    ).await
    .map_err(|e| e.into_response())
}

async fn set_config_override_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<environment_config_endpoints::ConfigUpdateRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::set_config_override(
        State(state_arc),
        Extension(user),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn reload_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::reload_config(
        State(state_arc),
        Extension(user),
    ).await
    .map_err(|e| e.into_response())
}

async fn export_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<environment_config_endpoints::ConfigQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::export_config(
        State(state_arc),
        Extension(user),
        Query(params),
    ).await.map_err(|e| e.into_response())
}

async fn get_config_diff_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::get_config_diff(
        State(state_arc),
        Extension(user),
    ).await
    .map_err(|e| e.into_response())
}

async fn validate_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    environment_config_endpoints::validate_config(
        State(state_arc),
        Extension(user),
    ).await
    .map_err(|e| e.into_response())
}

// Handler wrappers for validation endpoints
async fn register_schema_handler(
    State(state): State<AppState>,
    Json(payload): Json<validation_endpoints::RegisterSchemaRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::register_schema(
        State(state_arc),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_schema_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::get_schema(
        State(state_arc),
        Path(name),
    ).await
    .map_err(|e| e.into_response())
}

async fn validate_data_handler(
    State(state): State<AppState>,
    Path(schema): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::validate_data(
        State(state_arc),
        Path(schema),
        Json(data),
    ).await
    .map_err(|e| e.into_response())
}

async fn update_middleware_config_handler(
    State(state): State<AppState>,
    Json(config): Json<validation_middleware::ValidationMiddlewareConfig>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::update_middleware_config(
        State(state_arc),
        Json(config),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_stats_validation_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::get_stats(
        State(state_arc),
    ).await
    .map_err(|e| e.into_response())
}

async fn clear_cache_validation_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::clear_cache(
        State(state_arc),
    ).await
    .map_err(|e| e.into_response())
}

async fn configure_endpoint_validation_handler(
    State(state): State<AppState>,
    Json(request): Json<validation_endpoints::ConfigureEndpointRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    validation_endpoints::configure_endpoint_validation(
        State(state_arc),
        Json(request),
    ).await
    .map_err(|e| e.into_response())
}

// Handler wrappers for state management endpoints
async fn state_events_websocket_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: axum::extract::WebSocketUpgrade,
) -> axum::response::Response {
    let state_arc = Arc::new(state);
    
    // Extract user without error mapping since WebSocket upgrade can't return an error
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::state_events_websocket(
        State(state_arc),
        Extension(auth_user),
        ws,
    ).await.into_response()
}

async fn list_state_keys_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<state_management_endpoints::StateQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::list_state_keys(
        State(state_arc),
        Extension(auth_user),
        Query(params),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_state_stats_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::get_state_stats(
        State(state_arc),
        Extension(auth_user),
    ).await
    .map_err(|e| e.into_response())
}

async fn create_snapshot_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::create_snapshot(
        State(state_arc),
        Extension(auth_user),
    ).await
    .map_err(|e| e.into_response())
}

async fn compare_and_swap_state_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<state_management_endpoints::CompareAndSwapRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::compare_and_swap_state(
        State(state_arc),
        Extension(auth_user),
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_state_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Query(params): Query<state_management_endpoints::StateQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::get_state(
        State(state_arc),
        Extension(auth_user),
        Path(key),
        Query(params),
    ).await
    .map_err(|e| e.into_response())
}

async fn set_state_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(value): Json<serde_json::Value>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    // Create SetStateRequest
    let request = state_management_endpoints::SetStateRequest {
        key: key.clone(),
        value: value.clone(),
        metadata: None,
    };
    
    state_management_endpoints::set_state(
        State(state_arc),
        Extension(auth_user),
        Json(request),
    ).await
    .map_err(|e| e.into_response())
}

async fn remove_state_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Convert to middleware::auth::AuthenticatedUser
    let auth_user = middleware::auth::AuthenticatedUser {
        wallet: user.claims.wallet.clone(),
        role: auth::UserRole::User, // TODO: Get role from claims
        claims: auth::Claims {
            sub: user.claims.sub.clone(),
            exp: user.claims.exp,
            iat: user.claims.iat,
            jti: user.claims.jti.clone(),
            wallet: user.claims.wallet.clone(),
            role: auth::UserRole::User, // TODO: Get role from claims.role string
        },
    };
    
    state_management_endpoints::remove_state(
        State(state_arc),
        Extension(auth_user),
        Path(key),
    ).await
    .map_err(|e| e.into_response())
}

// Handler wrappers for feature flag endpoints
async fn get_flags_handler(
    State(state): State<AppState>,
    Query(query): Query<feature_flag_endpoints::FlagQuery>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    feature_flag_endpoints::get_flags(
        State(state_arc),
        Query(query),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_flag_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    feature_flag_endpoints::get_flag(
        State(state_arc),
        Path(name),
    ).await
    .map_err(|e| e.into_response())
}

async fn create_flag_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<feature_flags::FeatureFlag>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(axum::response::IntoResponse::into_response(
            (StatusCode::FORBIDDEN, "Admin role required")
        ));
    }
    
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    feature_flag_endpoints::create_flag(
        State(state_arc),
        role,
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn evaluate_flags_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<feature_flag_endpoints::EvaluationRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    feature_flag_endpoints::evaluate_flags(
        State(state_arc),
        user,
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn get_stats_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(axum::response::IntoResponse::into_response(
            (StatusCode::FORBIDDEN, "Admin role required")
        ));
    }
    
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    feature_flag_endpoints::get_stats(
        State(state_arc),
        role,
    ).await
    .map_err(|e| e.into_response())
}

async fn clear_cache_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(axum::response::IntoResponse::into_response(
            (StatusCode::FORBIDDEN, "Admin role required")
        ));
    }
    
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    feature_flag_endpoints::clear_cache(
        State(state_arc),
        role,
    ).await
    .map_err(|e| e.into_response())
}

async fn update_flag_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(payload): Json<feature_flag_endpoints::FlagUpdateRequest>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(axum::response::IntoResponse::into_response(
            (StatusCode::FORBIDDEN, "Admin role required")
        ));
    }
    
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    feature_flag_endpoints::update_flag(
        State(state_arc),
        Path(name),
        role,
        Json(payload),
    ).await
    .map_err(|e| e.into_response())
}

async fn delete_flag_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let state_arc = Arc::new(state);
    let (user, _) = match extract_auth_and_correlation(&headers, &state_arc).await {
        Ok(result) => result,
        Err(e) => return Err(e.into_response()),
    };
    
    // Check admin role
    if user.claims.role != "admin" {
        return Err(axum::response::IntoResponse::into_response(
            (StatusCode::FORBIDDEN, "Admin role required")
        ));
    }
    
    let role = rbac_authorization::RequireRole {
        user: user.clone(),
        role: rbac_authorization::Role::Admin,
    };
    
    feature_flag_endpoints::delete_flag(
        State(state_arc),
        Path(name),
        role,
    ).await
    .map_err(|e| e.into_response())
}
