//! Real-time price feed handlers

use axum::{
    extract::{State, Path, Query, ws::{WebSocket, WebSocketUpgrade}},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, error, debug};
use futures_util::{StreamExt, SinkExt};
use tokio::time::{timeout, Duration};

use crate::{
    AppState,
    response::responses,
    integration::{Platform, price_feed::PriceUpdate},
};

/// Get current price for a market
pub async fn get_market_price(
    Path(market_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Try to parse as u128 for internal market ID
    if let Ok(internal_id) = market_id.parse::<u128>() {
        // Check if we have price feed service
        if let Some(price_feed) = &state.price_feed {
            // Get all prices for this market
            let prices = price_feed.get_all_prices_for_market(&internal_id.to_string()).await;
            
            if !prices.is_empty() {
                let response = json!({
                    "market_id": internal_id,
                    "prices": prices.iter().map(|p| json!({
                        "platform": p.platform,
                        "outcome_prices": p.outcome_prices,
                        "liquidity": p.liquidity,
                        "volume_24h": p.volume_24h,
                        "timestamp": p.timestamp,
                        "confidence": p.confidence,
                    })).collect::<Vec<_>>(),
                    "count": prices.len(),
                });
                
                return Json(response).into_response();
            }
        }
    }
    
    // Try Polymarket ID
    if let Some(polymarket_feed) = &state.polymarket_price_feed {
        if let Some(prices) = polymarket_feed.get_current_price(&market_id).await {
            let response = json!({
                "market_id": market_id,
                "prices": [{
                    "platform": "polymarket",
                    "outcome_prices": prices,
                    "timestamp": chrono::Utc::now().timestamp(),
                }],
                "count": 1,
            });
            
            return Json(response).into_response();
        }
    }
    
    responses::not_found("No price data available for this market").into_response()
}

/// Track a market for real-time price updates
#[derive(Deserialize)]
pub struct TrackMarketRequest {
    pub polymarket_id: String,
    pub internal_id: String,
}

pub async fn track_market_prices(
    State(state): State<AppState>,
    Json(payload): Json<TrackMarketRequest>,
) -> impl IntoResponse {
    if let Some(polymarket_feed) = &state.polymarket_price_feed {
        // Set up tracking and aggregation
        match polymarket_feed.setup_aggregation(
            payload.internal_id.clone(),
            payload.polymarket_id.clone(),
            vec![], // No additional sources yet
        ).await {
            Ok(_) => {
                info!("Started tracking prices for market {} -> {}", 
                    payload.polymarket_id, payload.internal_id);
                
                responses::ok(json!({
                    "message": "Market price tracking enabled",
                    "polymarket_id": payload.polymarket_id,
                    "internal_id": payload.internal_id,
                })).into_response()
            }
            Err(e) => {
                error!("Failed to track market prices: {}", e);
                responses::internal_error(&format!("Failed to enable price tracking: {}", e)).into_response()
            }
        }
    } else {
        responses::service_unavailable("Price feed service not available").into_response()
    }
}

/// WebSocket handler for real-time price updates
pub async fn price_feed_websocket(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_price_feed_socket(socket, state))
}

async fn handle_price_feed_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    
    // Subscribe to price updates
    let price_feed = match &state.price_feed {
        Some(feed) => feed,
        None => {
            let _ = sender.send(axum::extract::ws::Message::Text(
                json!({
                    "error": "Price feed service not available"
                }).to_string()
            )).await;
            return;
        }
    };
    
    let mut price_subscriber = price_feed.subscribe();
    
    // Send initial connection message
    let _ = sender.send(axum::extract::ws::Message::Text(
        json!({
            "type": "connected",
            "message": "Connected to real-time price feed"
        }).to_string()
    )).await;
    
    // Handle incoming messages and price updates
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages
            msg = receiver.next() => {
                match msg {
                    Some(Ok(axum::extract::ws::Message::Text(text))) => {
                        if let Ok(request) = serde_json::from_str::<PriceFeedRequest>(&text) {
                            handle_price_feed_request(&mut sender, &state, request).await;
                        }
                    }
                    Some(Ok(axum::extract::ws::Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
            
            // Handle price updates
            Ok(price_update) = price_subscriber.recv() => {
                let message = json!({
                    "type": "price_update",
                    "data": {
                        "market_id": price_update.market_id,
                        "platform": price_update.platform,
                        "old_prices": price_update.old_prices,
                        "new_prices": price_update.new_prices,
                        "liquidity": price_update.liquidity,
                        "volume_24h": price_update.volume_24h,
                        "timestamp": price_update.timestamp,
                        "confidence": price_update.confidence,
                    }
                });
                
                if sender.send(axum::extract::ws::Message::Text(message.to_string())).await.is_err() {
                    break;
                }
            }
            
            // Send periodic heartbeat
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                if sender.send(axum::extract::ws::Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        }
    }
    
    debug!("Price feed WebSocket connection closed");
}

#[derive(Deserialize)]
struct PriceFeedRequest {
    #[serde(rename = "type")]
    request_type: String,
    market_id: Option<String>,
}

async fn handle_price_feed_request(
    sender: &mut futures_util::stream::SplitSink<WebSocket, axum::extract::ws::Message>,
    state: &AppState,
    request: PriceFeedRequest,
) {
    match request.request_type.as_str() {
        "subscribe" => {
            if let Some(market_id) = request.market_id {
                // In a full implementation, we'd track per-connection subscriptions
                let _ = sender.send(axum::extract::ws::Message::Text(
                    json!({
                        "type": "subscribed",
                        "market_id": market_id,
                    }).to_string()
                )).await;
            }
        }
        "unsubscribe" => {
            if let Some(market_id) = request.market_id {
                let _ = sender.send(axum::extract::ws::Message::Text(
                    json!({
                        "type": "unsubscribed",
                        "market_id": market_id,
                    }).to_string()
                )).await;
            }
        }
        _ => {
            let _ = sender.send(axum::extract::ws::Message::Text(
                json!({
                    "error": "Unknown request type"
                }).to_string()
            )).await;
        }
    }
}