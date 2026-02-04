//! Cross-platform integration handlers

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

use crate::{
    AppState,
    integration::{Platform, MarketMapping, ExternalPrice, IntegrationConfig},
    handlers,
};

/// Get integration status
pub async fn get_integration_status(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(ref sync_service) = state.market_sync {
        let status = sync_service.get_sync_status().await;
        Json(serde_json::json!({
            "status": "active",
            "last_sync": status.last_sync.to_rfc3339(),
            "next_sync": status.next_sync.to_rfc3339(),
            "total_syncs": status.total_syncs,
            "failed_syncs": status.failed_syncs,
            "active_mappings": status.active_mappings,
            "platforms": {
                "polymarket": state.integration_config.polymarket_enabled,
                "kalshi": state.integration_config.kalshi_enabled,
            }
        }))
    } else {
        Json(serde_json::json!({
            "status": "disabled",
            "message": "Integration service not configured"
        }))
    }
}

/// Get external prices for a market
pub async fn get_external_prices(
    Path(market_id): Path<u128>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Some(ref price_feed) = state.price_feed {
        let prices = price_feed.get_all_prices_for_market(&market_id.to_string()).await;
        
        let response: Vec<_> = prices.into_iter().map(|p| {
            serde_json::json!({
                "platform": p.platform,
                "market_id": p.market_id,
                "outcome_prices": p.outcome_prices,
                "liquidity": p.liquidity,
                "volume_24h": p.volume_24h,
                "timestamp": p.timestamp,
                "confidence": p.confidence,
            })
        }).collect();
        
        Json(serde_json::json!({
            "market_id": market_id,
            "prices": response,
            "count": response.len(),
        }))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Price feed service not available").into_response()
    }
}

/// Sync external markets
pub async fn sync_external_markets(
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Some(ref sync_service) = state.market_sync {
        // Trigger manual sync
        tokio::spawn({
            let service = sync_service.clone();
            async move {
                if let Err(e) = service.sync_all_markets().await {
                    error!("Manual sync failed: {}", e);
                }
            }
        });
        
        Json(serde_json::json!({
            "status": "sync_initiated",
            "message": "Market synchronization started in background"
        }))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available").into_response()
    }
}

/// Add market mapping
#[derive(Debug, Deserialize)]
pub struct AddMappingRequest {
    pub internal_id: u128,
    pub platform: Platform,
    pub external_id: String,
}

pub async fn add_market_mapping(
    State(state): State<AppState>,
    Json(request): Json<AddMappingRequest>,
) -> impl IntoResponse {
    if let Some(ref sync_service) = state.market_sync {
        match sync_service.add_market_mapping(
            request.internal_id,
            request.platform,
            request.external_id.clone(),
        ).await {
            Ok(_) => Json(serde_json::json!({
                "status": "success",
                "mapping": {
                    "internal_id": request.internal_id,
                    "platform": request.platform,
                    "external_id": request.external_id,
                }
            })).into_response(),
            Err(e) => {
                error!("Failed to add market mapping: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to add mapping").into_response()
            }
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available").into_response()
    }
}

/// Toggle market sync
#[derive(Debug, Deserialize)]
pub struct ToggleSyncRequest {
    pub enabled: bool,
}

pub async fn toggle_market_sync(
    Path(market_id): Path<u128>,
    State(state): State<AppState>,
    Json(request): Json<ToggleSyncRequest>,
) -> impl IntoResponse {
    if let Some(ref sync_service) = state.market_sync {
        match sync_service.toggle_market_sync(market_id, request.enabled).await {
            Ok(_) => Json(serde_json::json!({
                "status": "success",
                "market_id": market_id,
                "sync_enabled": request.enabled,
            })).into_response(),
            Err(e) => {
                error!("Failed to toggle sync: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to toggle sync").into_response()
            }
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available").into_response()
    }
}

/// Get Polymarket markets (enhanced)
pub async fn get_polymarket_markets_enhanced(
    Query(params): Query<PolymarketQueryParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Some(ref sync_service) = state.market_sync {
        // Try to get from cache first
        let cached_prices = sync_service.get_all_cached_prices().await;
        
        let polymarket_prices: Vec<_> = cached_prices.into_iter()
            .filter(|(key, _)| key.starts_with("polymarket:"))
            .map(|(_, price)| price)
            .filter(|p| {
                // Apply filters
                if let Some(min_liq) = params.min_liquidity {
                    if p.liquidity < min_liq {
                        return false;
                    }
                }
                if let Some(min_vol) = params.min_volume {
                    if p.volume_24h < min_vol {
                        return false;
                    }
                }
                true
            })
            .take(params.limit.unwrap_or(50))
            .map(|p| serde_json::json!({
                "market_id": p.market_id,
                "outcome_prices": p.outcome_prices,
                "liquidity": p.liquidity,
                "volume_24h": p.volume_24h,
                "confidence": p.confidence,
                "cached_at": p.timestamp,
            }))
            .collect();
        
        Json(serde_json::json!({
            "markets": polymarket_prices,
            "count": polymarket_prices.len(),
            "source": "cache",
        }))
    } else {
        // Fallback to proxy endpoint
        crate::handlers::proxy_polymarket_markets().await
    }
}

/// Query parameters for Polymarket
#[derive(Debug, Deserialize)]
pub struct PolymarketQueryParams {
    pub limit: Option<usize>,
    pub min_liquidity: Option<f64>,
    pub min_volume: Option<f64>,
}

/// Get Kalshi markets
pub async fn get_kalshi_markets(
    Query(params): Query<KalshiQueryParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Some(ref sync_service) = state.market_sync {
        // Get from cache
        let cached_prices = sync_service.get_all_cached_prices().await;
        
        let kalshi_prices: Vec<_> = cached_prices.into_iter()
            .filter(|(key, _)| key.starts_with("kalshi:"))
            .map(|(_, price)| price)
            .filter(|p| {
                // Apply status filter
                if params.status == Some("closed".to_string()) && p.volume_24h == 0.0 {
                    return false;
                }
                true
            })
            .take(params.limit.unwrap_or(50))
            .map(|p| serde_json::json!({
                "ticker": p.market_id,
                "yes_price": (p.outcome_prices.get(0).unwrap_or(&0.5) * 100.0) as i32,
                "no_price": (p.outcome_prices.get(1).unwrap_or(&0.5) * 100.0) as i32,
                "liquidity": p.liquidity,
                "volume_24h": p.volume_24h,
                "cached_at": p.timestamp,
            }))
            .collect();
        
        Json(serde_json::json!({
            "markets": kalshi_prices,
            "count": kalshi_prices.len(),
            "source": "cache",
        }))
    } else {
        Json(serde_json::json!({
            "error": "Kalshi integration not configured",
            "markets": []
        }))
    }
}

/// Query parameters for Kalshi
#[derive(Debug, Deserialize)]
pub struct KalshiQueryParams {
    pub limit: Option<usize>,
    pub status: Option<String>,
}

/// Subscribe to price updates via WebSocket
pub async fn subscribe_price_updates(
    State(state): State<AppState>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_price_subscription(socket, state))
}

/// Handle WebSocket price subscription
async fn handle_price_subscription(
    socket: axum::extract::ws::WebSocket,
    state: AppState,
) {
    use axum::extract::ws::Message;
    use tokio::time::{timeout, Duration};
    
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to price feed
    if let Some(ref price_feed) = state.price_feed {
        let mut price_rx = price_feed.subscribe();
        
        // Send price updates to client
        tokio::spawn(async move {
            while let Ok(update) = price_rx.recv().await {
                let msg = serde_json::json!({
                    "type": "price_update",
                    "data": {
                        "market_id": update.market_id,
                        "platform": update.platform,
                        "prices": update.new_prices,
                        "liquidity": update.liquidity,
                        "timestamp": update.timestamp,
                    }
                });
                
                if sender.send(Message::Text(msg.to_string())).await.is_err() {
                    break;
                }
            }
        });
        
        // Handle incoming messages (subscriptions)
        while let Ok(Some(Ok(msg))) = timeout(Duration::from_secs(60), receiver.next()).await {
            match msg {
                Message::Text(text) => {
                    if let Ok(cmd) = serde_json::from_str::<SubscriptionCommand>(&text) {
                        match cmd.action.as_str() {
                            "subscribe" => {
                                if let Some(market_id) = cmd.market_id {
                                    info!("Client subscribed to market {}", market_id);
                                    // Could implement market-specific subscriptions here
                                }
                            }
                            "unsubscribe" => {
                                if let Some(market_id) = cmd.market_id {
                                    info!("Client unsubscribed from market {}", market_id);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct SubscriptionCommand {
    action: String,
    market_id: Option<String>,
}

/// Configure integration settings
#[derive(Debug, Deserialize)]
pub struct ConfigureIntegrationRequest {
    pub polymarket_enabled: Option<bool>,
    pub polymarket_api_key: Option<String>,
    pub kalshi_enabled: Option<bool>,
    pub kalshi_api_key: Option<String>,
    pub sync_interval_seconds: Option<u64>,
}

pub async fn configure_integration(
    State(state): State<AppState>,
    Json(request): Json<ConfigureIntegrationRequest>,
) -> impl IntoResponse {
    // In production, this would update the configuration
    // For now, just return the current config
    Json(serde_json::json!({
        "status": "success",
        "config": {
            "polymarket_enabled": request.polymarket_enabled.unwrap_or(state.integration_config.polymarket_enabled),
            "kalshi_enabled": request.kalshi_enabled.unwrap_or(state.integration_config.kalshi_enabled),
            "sync_interval_seconds": request.sync_interval_seconds.unwrap_or(state.integration_config.sync_interval_seconds),
        }
    }))
}

use futures_util::{SinkExt, StreamExt};