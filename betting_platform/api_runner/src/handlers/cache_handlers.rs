//! Cache management handlers

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};
use crate::{AppState, response::ApiResponse, cache::CacheKey};
use serde::{Deserialize, Serialize};

/// Get cache statistics
pub async fn get_cache_stats(
    State(state): State<AppState>,
) -> Response {
    let stats = state.cache.get_stats().await;
    ApiResponse::success(stats).into_response()
}

/// Clear cache statistics
pub async fn clear_cache_stats(
    State(state): State<AppState>,
) -> Response {
    state.cache.clear_stats().await;
    ApiResponse::success(json!({
        "message": "Cache statistics cleared"
    })).into_response()
}

/// Health check for cache
pub async fn cache_health_check(
    State(state): State<AppState>,
) -> Response {
    let healthy = state.cache.health_check().await;
    
    if healthy {
        ApiResponse::success(json!({
            "status": "healthy",
            "service": "redis"
        })).into_response()
    } else {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", "Cache service is not healthy").into_response()
    }
}

/// Invalidate specific cache patterns
pub async fn invalidate_cache(
    State(state): State<AppState>,
    Json(payload): Json<InvalidateCacheRequest>,
) -> Response {
    let mut invalidated = Vec::new();
    
    for pattern in payload.patterns {
        // Simple pattern-based invalidation
        match pattern.as_str() {
            "markets" => {
                let _ = state.cache.delete(&CacheKey::markets_list()).await;
                // Would need to iterate through individual market keys
            }
            "verses" => {
                let _ = state.cache.delete(&CacheKey::verses_list()).await;
            }
            pattern if pattern.starts_with("wallet:") => {
                if let Some(wallet) = pattern.split(':').nth(1) {
                    let _ = state.cache.delete(&CacheKey::wallet_balance(wallet)).await;
                    let _ = state.cache.delete(&CacheKey::user_positions(wallet)).await;
                    let _ = state.cache.delete(&CacheKey::portfolio(wallet)).await;
                    let _ = state.cache.delete(&CacheKey::risk_metrics(wallet)).await;
                }
            }
            _ => {
                // Direct key deletion
                let _ = state.cache.delete(&pattern).await;
            }
        }
        invalidated.push(pattern);
    }
    
    ApiResponse::success(json!({
        "invalidated": invalidated,
        "count": invalidated.len()
    })).into_response()
}

/// Warm up cache with common data
pub async fn warm_cache(
    State(state): State<AppState>,
) -> Response {
    let mut warmed = Vec::new();
    
    // Warm up markets list
    match state.platform_client.get_markets().await {
        Ok(markets) => {
            if let Err(e) = state.cache.set(&CacheKey::markets_list(), &markets, Some(60)).await {
                tracing::warn!("Failed to warm markets cache: {}", e);
            } else {
                warmed.push("markets:list".to_string());
                
                // Also cache individual markets
                for market in &markets {
                    let key = CacheKey::market(market.id);
                    if let Err(e) = state.cache.set(&key, &market, Some(60)).await {
                        tracing::warn!("Failed to warm market {} cache: {}", market.id, e);
                    } else {
                        warmed.push(key.clone());
                    }
                }
            }
        }
        Err(e) => tracing::warn!("Failed to fetch markets for cache warming: {}", e),
    }
    
    // Warm up verses (using verse catalog instead)
    let verses: Vec<_> = crate::verse_catalog::VERSE_CATALOG.values().collect();
    if let Err(e) = state.cache.set(&CacheKey::verses_list(), &verses, Some(3600)).await {
        tracing::warn!("Failed to warm verses cache: {}", e);
    } else {
        warmed.push("verses:list".to_string());
    }
    
    ApiResponse::success(json!({
        "warmed": warmed,
        "count": warmed.len()
    })).into_response()
}

/// Get specific cache key value (for debugging)
pub async fn get_cache_key(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Response {
    match state.cache.get::<serde_json::Value>(&key).await {
        Some(value) => ApiResponse::success(json!({
            "key": key,
            "value": value,
            "exists": true
        })).into_response(),
        None => ApiResponse::success(json!({
            "key": key,
            "exists": false
        })).into_response(),
    }
}

/// Set cache TTL for a key
pub async fn set_cache_ttl(
    State(state): State<AppState>,
    Json(payload): Json<SetTtlRequest>,
) -> Response {
    match state.cache.expire(&payload.key, payload.ttl).await {
        Ok(_) => ApiResponse::success(json!({
            "key": payload.key,
            "ttl": payload.ttl,
            "success": true
        })).into_response(),
        Err(e) => ApiResponse::<()>::error("CACHE_ERROR", &format!("Failed to set TTL: {}", e)).into_response(),
    }
}

// Request types
#[derive(Debug, Deserialize)]
pub struct InvalidateCacheRequest {
    pub patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetTtlRequest {
    pub key: String,
    pub ttl: u64,
}

use serde_json::json;

/// Clear all cache entries (admin only)
pub async fn clear_all_cache(State(state): State<AppState>) -> Response {
    // This would need proper admin authentication
    // For now, just return an error
    ApiResponse::<()>::error("FORBIDDEN", "Admin access required").into_response()
}