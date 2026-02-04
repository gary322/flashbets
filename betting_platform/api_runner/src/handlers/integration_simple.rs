//! Simplified cross-platform integration handlers

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::info;

use crate::AppState;

/// Get integration status
pub async fn get_integration_status(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "active",
        "platforms": {
            "polymarket": state.integration_config.polymarket_enabled,
            "kalshi": state.integration_config.kalshi_enabled,
        },
        "sync_interval_seconds": state.integration_config.sync_interval_seconds,
    }))
}

/// Get enhanced Polymarket markets
pub async fn get_polymarket_markets_enhanced(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Try to fetch from real Polymarket API first
    if state.integration_config.polymarket_enabled {
        // Use the public client that doesn't require authentication
        let limit = params.get("limit")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(20);
            
        info!("Fetching {} markets from Polymarket public API", limit);
        match state.polymarket_public_client.get_markets(limit).await {
            Ok(polymarket_markets) => {
                info!("Successfully fetched {} markets from Polymarket", polymarket_markets.len());
                // Convert Polymarket format to our format
                let markets: Vec<serde_json::Value> = polymarket_markets.into_iter().map(|pm| {
                    pm.to_internal_format()
                }).collect();
                
                return Json(json!({
                    "markets": markets,
                    "count": markets.len(),
                    "source": "polymarket_live",
                    "message": "Real-time Polymarket data"
                })).into_response();
            }
            Err(e) => {
                info!("Failed to fetch Polymarket data: {}", e);
                return Json(json!({
                    "error": {
                        "code": "POLYMARKET_PARSE_ERROR",
                        "message": format!("Failed to parse Polymarket response: {}", e)
                    }
                })).into_response();
            }
        }
    }
    
    // Return empty markets if Polymarket is unavailable
    Json(json!({
        "markets": [],
        "count": 0,
        "source": "none",
        "message": "No market data available"
    })).into_response()
}

/// Sync external markets (stub)
pub async fn sync_external_markets(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "sync_initiated",
        "message": "Market synchronization started"
    }))
}