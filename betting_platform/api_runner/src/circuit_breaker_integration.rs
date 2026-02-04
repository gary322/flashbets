//! Circuit breaker integration examples for various services

use std::sync::Arc;
use crate::{
    circuit_breaker::CircuitBreaker,
    circuit_breaker_middleware::{
        with_database_circuit_breaker,
        with_redis_circuit_breaker,
        with_solana_circuit_breaker,
        with_external_api_circuit_breaker,
    },
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Example: Database operation with circuit breaker
pub async fn get_user_with_circuit_breaker(
    user_id: &str,
    db_pool: &crate::db::fallback::FallbackDatabase,
    breaker: &Arc<CircuitBreaker>,
) -> Result<serde_json::Value, AppError> {
    with_database_circuit_breaker(breaker, || async {
        // Simulate database query
        match db_pool.get_pool() {
            Ok(pool) => {
                // In production, this would be an actual query
                // For now, return mock data
                Ok(serde_json::json!({
                    "id": user_id,
                    "wallet": "mock_wallet",
                    "created_at": chrono::Utc::now(),
                }))
            }
            Err(_) => {
                let context = ErrorContext::new("database", "get_user");
                Err(AppError::new(
                    ErrorKind::DatabaseError,
                    "Database connection failed",
                    context,
                ))
            }
        }
    }).await
}

/// Example: Redis operation with circuit breaker
pub async fn get_cached_market_with_circuit_breaker(
    market_id: u128,
    cache: &crate::cache::CacheService,
    breaker: &Arc<CircuitBreaker>,
) -> Result<Option<serde_json::Value>, AppError> {
    with_redis_circuit_breaker(breaker, || async {
        let key = format!("market:{}", market_id);
        
        Ok(cache.get::<serde_json::Value>(&key).await)
    }).await
}

/// Example: Solana RPC operation with circuit breaker
pub async fn get_account_balance_with_circuit_breaker(
    account: &solana_sdk::pubkey::Pubkey,
    rpc_service: &crate::solana_rpc_service::SolanaRpcService,
    breaker: &Arc<CircuitBreaker>,
) -> Result<u64, AppError> {
    with_solana_circuit_breaker(breaker, || async {
        match rpc_service.get_balance(account).await {
            Ok(balance) => Ok(balance),
            Err(e) => {
                let context = ErrorContext::new("solana_rpc", "get_balance")
                    .with_metadata("account", serde_json::json!(account.to_string()));
                Err(AppError::new(
                    ErrorKind::SolanaRpcError,
                    format!("Failed to get balance: {}", e),
                    context,
                ))
            }
        }
    }).await
}

/// Example: External API operation with circuit breaker
pub async fn fetch_polymarket_data_with_circuit_breaker(
    market_id: &str,
    api_service: &crate::external_api_service::ExternalApiService,
    breaker: &Arc<CircuitBreaker>,
) -> Result<Vec<crate::external_api_service::PriceData>, AppError> {
    with_external_api_circuit_breaker(breaker, || async {
        match api_service.fetch_prices(
            crate::integration::Platform::Polymarket,
            vec![market_id.to_string()],
        ).await {
            Ok(prices) => Ok(prices),
            Err(e) => {
                let context = ErrorContext::new("external_api", "fetch_polymarket")
                    .with_metadata("market_id", serde_json::json!(market_id));
                Err(AppError::new(
                    ErrorKind::ExternalServiceError,
                    format!("Polymarket API failed: {}", e),
                    context,
                ))
            }
        }
    }).await
}

/// Example: Composite operation with multiple circuit breakers
pub async fn execute_trade_with_circuit_breakers(
    trade_request: TradeRequest,
    state: &crate::AppState,
) -> Result<TradeResponse, AppError> {
    let service_breakers = state.service_circuit_breakers.as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ConfigurationError,
            "Service circuit breakers not configured",
            ErrorContext::new("trading", "execute_trade"),
        ))?;
    
    // Step 1: Check user balance in database
    let user_balance = get_user_balance_with_circuit_breaker(
        &trade_request.user_id,
        &state.database,
        service_breakers.database(),
    ).await?;
    
    // Step 2: Check cached market data
    let market_data = get_cached_market_with_circuit_breaker(
        trade_request.market_id,
        &state.cache,
        service_breakers.redis(),
    ).await?;
    
    // If not in cache, fetch from external API
    let market_data = match market_data {
        Some(data) => data,
        None => {
            // Fetch from external API
            let external_data = fetch_polymarket_data_with_circuit_breaker(
                &trade_request.market_id.to_string(),
                state.external_api_service.as_ref().ok_or_else(|| {
                    AppError::new(
                        ErrorKind::ConfigurationError,
                        "External API service not configured",
                        ErrorContext::new("trading", "fetch_market"),
                    )
                })?,
                service_breakers.external_api(),
            ).await?;
            
            // Convert to internal format
            serde_json::json!({
                "market_id": trade_request.market_id,
                "prices": external_data,
            })
        }
    };
    
    // Step 3: Execute on-chain transaction
    if trade_request.on_chain {
        let tx_result = execute_onchain_with_circuit_breaker(
            &trade_request,
            state.solana_rpc_service.as_ref().ok_or_else(|| {
                AppError::new(
                    ErrorKind::ConfigurationError,
                    "Solana RPC service not configured",
                    ErrorContext::new("trading", "onchain_execution"),
                )
            })?,
            service_breakers.solana_rpc(),
        ).await?;
        
        Ok(TradeResponse {
            success: true,
            trade_id: uuid::Uuid::new_v4().to_string(),
            transaction_signature: Some(tx_result),
            market_data: Some(market_data),
        })
    } else {
        Ok(TradeResponse {
            success: true,
            trade_id: uuid::Uuid::new_v4().to_string(),
            transaction_signature: None,
            market_data: Some(market_data),
        })
    }
}

/// Execute on-chain transaction with circuit breaker
async fn execute_onchain_with_circuit_breaker(
    trade: &TradeRequest,
    rpc_service: &crate::solana_rpc_service::SolanaRpcService,
    breaker: &Arc<CircuitBreaker>,
) -> Result<String, AppError> {
    with_solana_circuit_breaker(breaker, || async {
        // In production, this would build and send actual transaction
        // For now, simulate transaction
        Ok("mock_transaction_signature".to_string())
    }).await
}

/// Helper function to get user balance
async fn get_user_balance_with_circuit_breaker(
    user_id: &str,
    db: &crate::db::fallback::FallbackDatabase,
    breaker: &Arc<CircuitBreaker>,
) -> Result<u64, AppError> {
    with_database_circuit_breaker(breaker, || async {
        // In production, query actual balance
        Ok(1000000) // Mock balance
    }).await
}

// Example request/response types
#[derive(serde::Deserialize)]
pub struct TradeRequest {
    pub user_id: String,
    pub market_id: u128,
    pub amount: u64,
    pub side: String,
    pub on_chain: bool,
}

#[derive(serde::Serialize)]
pub struct TradeResponse {
    pub success: bool,
    pub trade_id: String,
    pub transaction_signature: Option<String>,
    pub market_data: Option<serde_json::Value>,
}

/// Circuit breaker patterns for different scenarios
pub mod patterns {
    use super::*;
    use std::time::Duration;
    use crate::circuit_breaker::CircuitBreakerConfig;
    
    /// Fast-fail pattern for critical operations
    pub fn fast_fail_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 1,
            reset_timeout: Duration::from_secs(10),
            half_open_max_calls: 1,
            failure_window: Duration::from_secs(30),
            min_calls: 3,
            failure_rate_threshold: 0.3,
            slow_call_duration: Duration::from_secs(2),
            slow_call_rate_threshold: 0.3,
        }
    }
    
    /// Slow-recovery pattern for expensive operations
    pub fn slow_recovery_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 10,
            success_threshold: 5,
            reset_timeout: Duration::from_secs(120),
            half_open_max_calls: 5,
            failure_window: Duration::from_secs(300),
            min_calls: 20,
            failure_rate_threshold: 0.7,
            slow_call_duration: Duration::from_secs(10),
            slow_call_rate_threshold: 0.7,
        }
    }
    
    /// Balanced pattern for general use
    pub fn balanced_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig::default()
    }
}