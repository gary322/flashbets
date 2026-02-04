//! Queue management HTTP handlers

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{AppState, response::ApiResponse, queue::{QueueMessage, QueueChannels, QueueService}};
use chrono::Utc;

/// Get queue statistics
pub async fn get_queue_stats(
    State(state): State<AppState>,
) -> Response {
    if let Some(queue_service) = &state.queue_service {
        let stats = queue_service.get_stats().await;
        ApiResponse::success(stats).into_response()
    } else {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", "Queue service not available").into_response()
    }
}

/// Get queue lengths
pub async fn get_queue_lengths(
    State(state): State<AppState>,
) -> Response {
    if let Some(queue_service) = &state.queue_service {
        let mut lengths = std::collections::HashMap::new();
        
        // Get length of each queue
        for (name, channel) in [
            ("trades", QueueChannels::TRADES),
            ("markets", QueueChannels::MARKETS),
            ("settlements", QueueChannels::SETTLEMENTS),
            ("risk_alerts", QueueChannels::RISK_ALERTS),
            ("notifications", QueueChannels::NOTIFICATIONS),
            ("general", QueueChannels::GENERAL),
            ("dead_letter", QueueChannels::DEAD_LETTER),
        ] {
            match queue_service.get_queue_length(channel).await {
                Ok(length) => {
                    lengths.insert(name.to_string(), length);
                }
                Err(e) => {
                    tracing::warn!("Failed to get length for queue {}: {}", name, e);
                }
            }
        }
        
        ApiResponse::success(lengths).into_response()
    } else {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", "Queue service not available").into_response()
    }
}

/// Publish a test message to queue
pub async fn publish_test_message(
    State(state): State<AppState>,
    Json(payload): Json<PublishTestRequest>,
) -> Response {
    if let Some(queue_service) = &state.queue_service {
        let message = match payload.message_type.as_str() {
            "trade" => QueueMessage::TradeExecuted {
                trade_id: format!("test_{}", uuid::Uuid::new_v4()),
                wallet: payload.wallet.unwrap_or_else(|| "test_wallet".to_string()),
                market_id: payload.market_id.unwrap_or_else(|| "test_market".to_string()),
                amount: payload.amount.unwrap_or(1000),
                outcome: payload.outcome.unwrap_or(0),
                timestamp: Utc::now(),
            },
            "market" => QueueMessage::MarketCreated {
                market_id: format!("test_{}", uuid::Uuid::new_v4()),
                title: payload.title.unwrap_or_else(|| "Test Market".to_string()),
                creator: payload.wallet.unwrap_or_else(|| "test_creator".to_string()),
                timestamp: Utc::now(),
            },
            "risk" => QueueMessage::RiskAlert {
                wallet: payload.wallet.unwrap_or_else(|| "test_wallet".to_string()),
                alert_type: payload.alert_type.unwrap_or_else(|| "test_alert".to_string()),
                severity: payload.severity.unwrap_or_else(|| "medium".to_string()),
                details: serde_json::json!({"test": true}),
                timestamp: Utc::now(),
            },
            "cache" => QueueMessage::CacheInvalidation {
                patterns: payload.patterns.unwrap_or_else(|| vec!["test:*".to_string()]),
                timestamp: Utc::now(),
            },
            _ => {
                return ApiResponse::<()>::error("BAD_REQUEST", "Invalid message type").into_response();
            }
        };
        
        let channel = match payload.message_type.as_str() {
            "trade" => QueueChannels::TRADES,
            "market" => QueueChannels::MARKETS,
            "risk" => QueueChannels::RISK_ALERTS,
            "cache" => QueueChannels::GENERAL,
            _ => QueueChannels::GENERAL,
        };
        
        match queue_service.publish(channel, message).await {
            Ok(_) => ApiResponse::success(serde_json::json!({
                "message": "Test message published",
                "channel": channel,
                "type": payload.message_type
            })).into_response(),
            Err(e) => ApiResponse::<()>::error("QUEUE_ERROR", &format!("Failed to publish: {}", e)).into_response(),
        }
    } else {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", "Queue service not available").into_response()
    }
}

/// Clear a specific queue (admin only)
pub async fn clear_queue(
    Path(queue_name): Path<String>,
    State(state): State<AppState>,
) -> Response {
    // This should have proper admin authentication
    
    if let Some(queue_service) = &state.queue_service {
        let channel = match queue_name.as_str() {
            "trades" => QueueChannels::TRADES,
            "markets" => QueueChannels::MARKETS,
            "settlements" => QueueChannels::SETTLEMENTS,
            "risk_alerts" => QueueChannels::RISK_ALERTS,
            "notifications" => QueueChannels::NOTIFICATIONS,
            "general" => QueueChannels::GENERAL,
            "dead_letter" => QueueChannels::DEAD_LETTER,
            _ => {
                return ApiResponse::<()>::error("BAD_REQUEST", "Invalid queue name").into_response();
            }
        };
        
        match queue_service.clear_queue(channel).await {
            Ok(_) => ApiResponse::success(serde_json::json!({
                "message": "Queue cleared",
                "queue": queue_name
            })).into_response(),
            Err(e) => ApiResponse::<()>::error("QUEUE_ERROR", &format!("Failed to clear queue: {}", e)).into_response(),
        }
    } else {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", "Queue service not available").into_response()
    }
}

/// Publish a delayed message
pub async fn publish_delayed_message(
    State(state): State<AppState>,
    Json(payload): Json<PublishDelayedRequest>,
) -> Response {
    if let Some(queue_service) = &state.queue_service {
        let message = QueueMessage::CacheInvalidation {
            patterns: payload.patterns,
            timestamp: Utc::now(),
        };
        
        match queue_service.publish_delayed(
            QueueChannels::GENERAL,
            message,
            payload.delay_seconds,
        ).await {
            Ok(_) => ApiResponse::success(serde_json::json!({
                "message": "Delayed message published",
                "delay_seconds": payload.delay_seconds
            })).into_response(),
            Err(e) => ApiResponse::<()>::error("QUEUE_ERROR", &format!("Failed to publish delayed message: {}", e)).into_response(),
        }
    } else {
        ApiResponse::<()>::error("SERVICE_UNAVAILABLE", "Queue service not available").into_response()
    }
}

// Request types
#[derive(Debug, Deserialize)]
pub struct PublishTestRequest {
    pub message_type: String,
    pub wallet: Option<String>,
    pub market_id: Option<String>,
    pub amount: Option<u64>,
    pub outcome: Option<u8>,
    pub title: Option<String>,
    pub alert_type: Option<String>,
    pub severity: Option<String>,
    pub patterns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct PublishDelayedRequest {
    pub patterns: Vec<String>,
    pub delay_seconds: i64,
}

use serde_json;
use uuid;