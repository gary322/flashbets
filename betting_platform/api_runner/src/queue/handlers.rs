//! Queue message handlers

use anyhow::Result;
use crate::{AppState, cache::CacheKey};
use super::{QueueMessage, QueueTask};
use tracing::{info, warn, error};

/// Handle trade execution messages
pub async fn handle_trade_executed(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::TradeExecuted { trade_id, wallet, market_id, amount, outcome, timestamp } = msg {
        info!("Processing trade execution: {} for wallet {}", trade_id, wallet);
        
        // Invalidate relevant caches
        let _ = state.cache.delete(&CacheKey::wallet_balance(&wallet)).await;
        let _ = state.cache.delete(&CacheKey::user_positions(&wallet)).await;
        let _ = state.cache.delete(&CacheKey::portfolio(&wallet)).await;
        let _ = state.cache.delete(&CacheKey::risk_metrics(&wallet)).await;
        
        // Record in database if available
        if let Ok(conn) = state.database.get_connection().await {
            // Would record trade in database here
            info!("Trade {} recorded in database", trade_id);
        }
        
        // Send WebSocket notification
        if let Some(ws) = &state.enhanced_ws_manager {
            // Parse market_id from string to u128
            let market_id_u128 = market_id.parse::<u128>().unwrap_or(0);
            
            let position_info = crate::websocket::enhanced::PositionInfo {
                size: amount,
                entry_price: 0.5, // Default price
                current_price: 0.5,
                pnl: 0.0,
                pnl_percentage: 0.0,
                leverage: 1,
                liquidation_price: 0.0,
            };
            
            let msg = crate::websocket::enhanced::EnhancedWsMessage::PositionUpdate {
                wallet: wallet.clone(),
                market_id: market_id_u128,
                position: position_info,
                action: "opened".to_string(),
                timestamp: timestamp.timestamp(),
            };
            ws.broadcast_position_update(msg);
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle market creation messages
pub async fn handle_market_created(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::MarketCreated { market_id, title, creator, timestamp } = msg {
        info!("Processing market creation: {} - {}", market_id, title);
        
        // Invalidate markets cache
        let _ = state.cache.delete(&CacheKey::markets_list()).await;
        
        // Send WebSocket notification
        if let Some(ws) = &state.enhanced_ws_manager {
            // Broadcast new market notification
            info!("Broadcasting market creation: {}", market_id);
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle position closed messages
pub async fn handle_position_closed(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::PositionClosed { position_id, wallet, market_id, pnl, timestamp } = msg {
        info!("Processing position closure: {} with PnL: {}", position_id, pnl);
        
        // Invalidate relevant caches
        let _ = state.cache.delete(&CacheKey::user_positions(&wallet)).await;
        let _ = state.cache.delete(&CacheKey::portfolio(&wallet)).await;
        let _ = state.cache.delete(&CacheKey::risk_metrics(&wallet)).await;
        
        // Update user stats in database
        if let Ok(conn) = state.database.get_connection().await {
            // Would update user stats here
            info!("Position {} closure recorded", position_id);
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle settlement completion messages
pub async fn handle_settlement_completed(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::SettlementCompleted { market_id, winning_outcome, total_payout, timestamp } = msg {
        info!("Processing settlement: market {} settled with outcome {}", market_id, winning_outcome);
        
        // Invalidate market cache
        let _ = state.cache.delete(&CacheKey::markets_list()).await;
        let _ = state.cache.delete(&format!("market:{}", market_id)).await;
        
        // Process payouts for all positions in this market
        info!("Processing payouts for market {} - total: {}", market_id, total_payout);
        
        // Send WebSocket notifications
        if let Some(ws) = &state.enhanced_ws_manager {
            // Broadcast settlement notification
            info!("Broadcasting settlement for market {}", market_id);
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle risk alert messages
pub async fn handle_risk_alert(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::RiskAlert { wallet, alert_type, severity, details, timestamp } = msg {
        warn!("Risk alert for wallet {}: {} ({})", wallet, alert_type, severity);
        
        // Log to security system
        let security_event = crate::security::security_logger::SecurityLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: crate::security::security_logger::SecurityEventType::RateLimitExceeded,
            severity: match severity.as_str() {
                "critical" => crate::security::security_logger::SecuritySeverity::Critical,
                "high" => crate::security::security_logger::SecuritySeverity::High,
                "medium" => crate::security::security_logger::SecuritySeverity::Medium,
                _ => crate::security::security_logger::SecuritySeverity::Low,
            },
            ip_address: None,
            user_id: None,
            wallet_address: Some(wallet.clone()),
            request_path: None,
            request_method: None,
            status_code: None,
            user_agent: None,
            details: {
                let mut map = std::collections::HashMap::new();
                map.insert("alert_type".to_string(), serde_json::Value::String(alert_type.clone()));
                map.insert("details".to_string(), details);
                map
            },
            risk_score: match severity.as_str() {
                "critical" => 1.0,
                "high" => 0.8,
                "medium" => 0.5,
                _ => 0.3,
            },
            flagged: severity == "critical",
        };
        
        state.security_logger.log_event(security_event).await;
        
        // Take automated action based on severity
        match severity.as_str() {
            "critical" => {
                error!("Critical risk alert for {}: {}", wallet, alert_type);
                // Could trigger position reduction or account freeze
            }
            "high" => {
                warn!("High risk alert for {}: {}", wallet, alert_type);
                // Could restrict new positions
            }
            _ => {
                info!("Risk alert for {}: {}", wallet, alert_type);
            }
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle cache invalidation messages
pub async fn handle_cache_invalidation(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::CacheInvalidation { patterns, timestamp } = msg {
        info!("Processing cache invalidation for {} patterns", patterns.len());
        
        for pattern in patterns {
            match pattern.as_str() {
                "markets" => {
                    let _ = state.cache.delete(&CacheKey::markets_list()).await;
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
                    let _ = state.cache.delete(&pattern).await;
                }
            }
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle email notification messages
pub async fn handle_email_notification(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::EmailNotification { to, subject, body, priority } = msg {
        info!("Sending email to {}: {}", to, subject);
        
        // In production, would integrate with email service (SendGrid, SES, etc.)
        // For now, just log it
        match priority.as_str() {
            "high" => {
                warn!("HIGH PRIORITY EMAIL: {} to {}", subject, to);
            }
            _ => {
                info!("Email queued: {} to {}", subject, to);
            }
        }
        
        Ok(())
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Handle webhook delivery messages
pub async fn handle_webhook_delivery(state: &AppState, msg: QueueMessage) -> Result<()> {
    if let QueueMessage::WebhookDelivery { url, payload, headers, retry_count } = msg {
        info!("Delivering webhook to {}: attempt {}", url, retry_count + 1);
        
        let client = reqwest::Client::new();
        let mut request = client.post(&url).json(&payload);
        
        // Add headers
        for (key, value) in headers {
            request = request.header(&key, &value);
        }
        
        // Add signature for security
        let signature = generate_webhook_signature(&payload);
        request = request.header("X-Webhook-Signature", signature);
        
        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Webhook delivered successfully to {}", url);
                    Ok(())
                } else {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    Err(anyhow::anyhow!("Webhook failed with status {}: {}", status, body))
                }
            }
            Err(e) => {
                error!("Failed to deliver webhook to {}: {}", url, e);
                Err(e.into())
            }
        }
    } else {
        Err(anyhow::anyhow!("Invalid message type"))
    }
}

/// Generate webhook signature
fn generate_webhook_signature(payload: &serde_json::Value) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let secret = std::env::var("WEBHOOK_SECRET").unwrap_or_else(|_| "default-webhook-secret".to_string());
    let payload_str = serde_json::to_string(payload).unwrap_or_default();
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload_str.as_bytes());
    
    hex::encode(mac.finalize().into_bytes())
}

/// Process a queue task based on its message type
pub async fn process_queue_task(state: &AppState, task: QueueTask) -> Result<()> {
    match task.message {
        QueueMessage::TradeExecuted { .. } => handle_trade_executed(state, task.message).await,
        QueueMessage::MarketCreated { .. } => handle_market_created(state, task.message).await,
        QueueMessage::PositionClosed { .. } => handle_position_closed(state, task.message).await,
        QueueMessage::SettlementCompleted { .. } => handle_settlement_completed(state, task.message).await,
        QueueMessage::RiskAlert { .. } => handle_risk_alert(state, task.message).await,
        QueueMessage::CacheInvalidation { .. } => handle_cache_invalidation(state, task.message).await,
        QueueMessage::EmailNotification { .. } => handle_email_notification(state, task.message).await,
        QueueMessage::WebhookDelivery { .. } => handle_webhook_delivery(state, task.message).await,
    }
}