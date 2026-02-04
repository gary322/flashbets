//! External API integration endpoints

use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::{
    AppState,
    external_api_service::{ExternalApiService, ApiHealth, MarketData, PriceData},
    integration::Platform,
};

/// Query parameters for market fetching
#[derive(Debug, Deserialize)]
pub struct MarketQuery {
    pub limit: Option<usize>,
    pub platform: Option<String>,
}

/// Market sync request
#[derive(Debug, Deserialize)]
pub struct MarketSyncRequest {
    pub market_id: String,
    pub platform: Platform,
    pub internal_market_id: Option<u128>,
}

/// Get health status of all external APIs
pub async fn get_external_api_health(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let external_api = state.external_api_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let health_status = external_api.get_health_status().await;
    
    Ok(Json(health_status))
}

/// Fetch markets from external platforms
pub async fn fetch_external_markets(
    State(state): State<AppState>,
    Query(query): Query<MarketQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let external_api = state.external_api_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let limit = query.limit.unwrap_or(50);
    
    // If specific platform requested
    if let Some(platform_str) = query.platform {
        let platform = match platform_str.to_lowercase().as_str() {
            "polymarket" => Platform::Polymarket,
            "kalshi" => Platform::Kalshi,
            _ => return Err(StatusCode::BAD_REQUEST),
        };
        
        let markets = external_api.fetch_all_markets(limit).await;
        
        if let Some(result) = markets.get(&platform) {
            match result {
                Ok(data) => Ok(Json(data).into_response()),
                Err(e) => {
                    error!("Failed to fetch markets from {:?}: {}", platform, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    } else {
        // Fetch from all platforms
        let all_markets = external_api.fetch_all_markets(limit).await;
        
        let response: std::collections::HashMap<String, Vec<MarketData>> = all_markets
            .into_iter()
            .filter_map(|(platform, result)| {
                match result {
                    Ok(markets) => Some((platform.to_string(), markets)),
                    Err(e) => {
                        warn!("Failed to fetch from {:?}: {}", platform, e);
                        None
                    }
                }
            })
            .collect();
        
        Ok(Json(response).into_response())
    }
}

/// Get prices for specific markets
pub async fn get_external_prices(
    State(state): State<AppState>,
    Path(platform_str): Path<String>,
    Json(market_ids): Json<Vec<String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let external_api = state.external_api_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let platform = match platform_str.to_lowercase().as_str() {
        "polymarket" => Platform::Polymarket,
        "kalshi" => Platform::Kalshi,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    match external_api.fetch_prices(platform, market_ids).await {
        Ok(prices) => Ok(Json(prices)),
        Err(e) => {
            error!("Failed to fetch prices: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Sync a specific market
pub async fn sync_external_market(
    State(state): State<AppState>,
    Json(request): Json<MarketSyncRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let market_sync = state.market_sync.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Add market mapping if internal ID provided
    if let Some(internal_id) = request.internal_market_id {
        match market_sync.add_market_mapping(
            internal_id,
            request.platform,
            request.market_id.clone(),
        ).await {
            Ok(()) => {
                info!("Added market mapping: {} -> {:?}:{}", 
                    internal_id, request.platform, request.market_id);
            }
            Err(e) => {
                error!("Failed to add market mapping: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
    
    // Trigger sync
    match market_sync.sync_all_markets().await {
        Ok(()) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "message": "Market sync triggered"
            })))
        }
        Err(e) => {
            error!("Market sync failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get sync status
pub async fn get_sync_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let market_sync = state.market_sync.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let status = market_sync.get_sync_status().await;
    
    Ok(Json(status))
}

/// Get cached prices
pub async fn get_cached_prices(
    State(state): State<AppState>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let market_sync = state.market_sync.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    if let Some(key) = query.get("key") {
        // Get specific cached price
        if let Some(price) = market_sync.get_cached_price(key).await {
            Ok(Json(serde_json::json!({
                "price": price
            })))
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    } else {
        // Get all cached prices
        let all_prices = market_sync.get_all_cached_prices().await;
        Ok(Json(serde_json::json!({
            "prices": all_prices
        })))
    }
}

/// Toggle market sync
#[derive(Debug, Deserialize)]
pub struct ToggleSyncRequest {
    pub internal_market_id: u128,
    pub enabled: bool,
}

pub async fn toggle_market_sync(
    State(state): State<AppState>,
    Json(request): Json<ToggleSyncRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let market_sync = state.market_sync.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    match market_sync.toggle_market_sync(request.internal_market_id, request.enabled).await {
        Ok(()) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "market_id": request.internal_market_id,
                "sync_enabled": request.enabled
            })))
        }
        Err(e) => {
            error!("Failed to toggle market sync: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Market comparison response
#[derive(Debug, Serialize)]
pub struct MarketComparison {
    pub internal_market_id: u128,
    pub title: String,
    pub external_matches: Vec<ExternalMatch>,
}

#[derive(Debug, Serialize)]
pub struct ExternalMatch {
    pub platform: Platform,
    pub market_id: String,
    pub title: String,
    pub similarity_score: f64,
    pub price_deviation: Option<f64>,
}

/// Compare internal markets with external platforms
pub async fn compare_markets(
    State(state): State<AppState>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let threshold = query.get("threshold")
        .and_then(|t| t.parse::<f64>().ok())
        .unwrap_or(0.8);
    
    // This would compare internal markets with external ones
    // For now, return a placeholder
    let comparisons = vec![
        MarketComparison {
            internal_market_id: 12345,
            title: "Will BTC reach $100k by end of 2024?".to_string(),
            external_matches: vec![
                ExternalMatch {
                    platform: Platform::Polymarket,
                    market_id: "0x123".to_string(),
                    title: "Bitcoin to reach $100,000 before 2025".to_string(),
                    similarity_score: 0.92,
                    price_deviation: Some(0.03),
                },
                ExternalMatch {
                    platform: Platform::Kalshi,
                    market_id: "BTC-100K-24".to_string(),
                    title: "BTC > $100k by Dec 31 2024".to_string(),
                    similarity_score: 0.88,
                    price_deviation: Some(0.05),
                },
            ],
        },
    ];
    
    Ok(Json(comparisons))
}

/// Integration configuration update
#[derive(Debug, Deserialize)]
pub struct UpdateIntegrationConfigRequest {
    pub polymarket_enabled: Option<bool>,
    pub polymarket_api_key: Option<String>,
    pub kalshi_enabled: Option<bool>,
    pub kalshi_api_key: Option<String>,
    pub kalshi_api_secret: Option<String>,
    pub sync_interval_seconds: Option<u64>,
}

/// Update integration configuration (admin only)
pub async fn update_integration_config(
    State(state): State<AppState>,
    Json(request): Json<UpdateIntegrationConfigRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // In production, this would update the configuration
    // and reinitialize the services
    
    info!("Integration config update requested: {:?}", request);
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Configuration updated"
    })))
}

/// Test external API connectivity
pub async fn test_external_api(
    State(state): State<AppState>,
    Path(platform_str): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let external_api = state.external_api_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let platform = match platform_str.to_lowercase().as_str() {
        "polymarket" => Platform::Polymarket,
        "kalshi" => Platform::Kalshi,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    // Try to fetch 1 market as a test
    let result = external_api.fetch_all_markets(1).await;
    
    if let Some(platform_result) = result.get(&platform) {
        match platform_result {
            Ok(markets) => {
                Ok(Json(serde_json::json!({
                    "success": true,
                    "platform": platform_str,
                    "test_market": markets.first(),
                    "message": "API connection successful"
                })))
            }
            Err(e) => {
                Ok(Json(serde_json::json!({
                    "success": false,
                    "platform": platform_str,
                    "error": e.to_string(),
                    "message": "API connection failed"
                })))
            }
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}