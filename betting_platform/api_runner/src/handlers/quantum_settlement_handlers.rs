//! Quantum settlement HTTP handlers

use axum::{
    extract::{Path, State, Query},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{
    AppState, 
    response::ApiResponse,
    quantum_settlement::{QuantumSettlement, QuantumSettlementBatch, QuantumSettlementEngine},
};
use chrono::Utc;

#[derive(Debug, Deserialize)]
pub struct SettleQuantumPositionRequest {
    pub position_id: String,
    pub market_id: u128,
    pub winning_outcome: u8,
}

#[derive(Debug, Deserialize)]
pub struct SettleMarketQuantumRequest {
    pub market_id: u128,
    pub winning_outcome: u8,
}

#[derive(Debug, Deserialize)]
pub struct QuantumSettlementQuery {
    pub wallet: Option<String>,
    pub market_id: Option<u128>,
    pub from_timestamp: Option<i64>,
    pub to_timestamp: Option<i64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct QuantumSettlementStatus {
    pub total_positions: usize,
    pub settled_positions: usize,
    pub pending_positions: usize,
    pub total_payout: u64,
    pub total_quantum_bonus: f64,
    pub average_coherence_multiplier: f64,
}

/// Settle a single quantum position
pub async fn settle_quantum_position(
    State(state): State<AppState>,
    Json(payload): Json<SettleQuantumPositionRequest>,
) -> Response {
    // Get quantum position
    let position = match state.quantum_engine.get_quantum_position(&payload.position_id).await {
        Ok(pos) => pos,
        Err(e) => return ApiResponse::<()>::error("NOT_FOUND", &format!("Quantum position not found: {}", e)).into_response(),
    };

    // Get market
    let market = match state.platform_client.get_market(payload.market_id).await {
        Ok(Some(market)) => market,
        Ok(None) => return ApiResponse::<()>::error("NOT_FOUND", "Market not found").into_response(),
        Err(e) => return ApiResponse::<()>::error("RPC_ERROR", &format!("Failed to fetch market: {}", e)).into_response(),
    };

    // Verify market is resolved
    if !market.resolved || market.winning_outcome.is_none() {
        return ApiResponse::<()>::error("BAD_REQUEST", "Market not resolved yet").into_response();
    }

    let winning_outcome = market.winning_outcome.unwrap();
    if winning_outcome != payload.winning_outcome {
        return ApiResponse::<()>::error("BAD_REQUEST", "Incorrect winning outcome").into_response();
    }

    // Create settlement engine
    let engine = QuantumSettlementEngine::new();

    // Settle the position
    match engine.settle_quantum_position(&position, &market, winning_outcome).await {
        Ok(settlement) => {
            // Process payout on blockchain
            if settlement.pnl > 0 {
                match state.platform_client.process_quantum_settlement(
                    &settlement.wallet,
                    &settlement.position_id,
                    settlement.pnl as u64,
                ).await {
                    Ok(signature) => {
                        tracing::info!("Quantum settlement processed: {}", signature);
                    }
                    Err(e) => {
                        tracing::error!("Failed to process quantum settlement: {}", e);
                        return ApiResponse::<()>::error("BLOCKCHAIN_ERROR", &format!("Settlement failed: {}", e)).into_response();
                    }
                }
            }

            // Record in database
            if let Err(e) = record_quantum_settlement(&state, &settlement).await {
                tracing::error!("Failed to record settlement: {}", e);
            }

            // Send queue message
            if let Some(queue) = &state.queue_service {
                let msg = crate::queue::QueueMessage::SettlementCompleted {
                    market_id: settlement.market_id.to_string(),
                    winning_outcome,
                    total_payout: settlement.pnl.max(0) as u64,
                    timestamp: Utc::now(),
                };
                let _ = queue.publish(crate::queue::QueueChannels::SETTLEMENTS, msg).await;
            }

            ApiResponse::success(settlement).into_response()
        }
        Err(e) => ApiResponse::<()>::error("SETTLEMENT_ERROR", &format!("Failed to settle position: {}", e)).into_response(),
    }
}

/// Settle all quantum positions for a market
pub async fn settle_market_quantum_positions(
    State(state): State<AppState>,
    Json(payload): Json<SettleMarketQuantumRequest>,
) -> Response {
    // Get market
    let market = match state.platform_client.get_market(payload.market_id).await {
        Ok(Some(market)) => market,
        Ok(None) => return ApiResponse::<()>::error("NOT_FOUND", "Market not found").into_response(),
        Err(e) => return ApiResponse::<()>::error("RPC_ERROR", &format!("Failed to fetch market: {}", e)).into_response(),
    };

    // Verify market is resolved
    if !market.resolved || market.winning_outcome != Some(payload.winning_outcome) {
        return ApiResponse::<()>::error("BAD_REQUEST", "Market not resolved or incorrect outcome").into_response();
    }

    // Get all quantum positions for this market
    let positions = match state.quantum_engine.get_market_quantum_states(payload.market_id).await {
        Ok(states) => {
            // Get full positions for each state
            let mut positions = Vec::new();
            for state in states {
                // This is simplified - in practice we'd have a method to get positions by market
                // For now, we'll skip this as we need a proper way to map states to positions
            }
            positions
        }
        Err(e) => {
            tracing::error!("Failed to get quantum positions: {}", e);
            Vec::new()
        }
    };

    if positions.is_empty() {
        return ApiResponse::<()>::error("NOT_FOUND", "No quantum positions found for market").into_response();
    }

    // Create settlement engine
    let engine = QuantumSettlementEngine::new();

    // Settle all positions
    match engine.settle_market_quantum_positions(
        payload.market_id,
        &market,
        payload.winning_outcome,
        positions,
    ).await {
        Ok(batch) => {
            // Process payouts on blockchain
            let signatures = match crate::quantum_settlement::process_quantum_settlements(
                &batch.settlements,
                &state.platform_client,
            ).await {
                Ok(sigs) => sigs,
                Err(e) => {
                    tracing::error!("Failed to process quantum settlements: {}", e);
                    Vec::new()
                }
            };

            // Record in database
            for settlement in &batch.settlements {
                if let Err(e) = record_quantum_settlement(&state, settlement).await {
                    tracing::error!("Failed to record settlement: {}", e);
                }
            }

            ApiResponse::success(serde_json::json!({
                "batch": batch,
                "signatures": signatures,
            })).into_response()
        }
        Err(e) => ApiResponse::<()>::error("SETTLEMENT_ERROR", &format!("Failed to settle market positions: {}", e)).into_response(),
    }
}

/// Get quantum settlement history
pub async fn get_quantum_settlements(
    State(state): State<AppState>,
    Query(query): Query<QuantumSettlementQuery>,
) -> Response {
    // This would query from database
    // For now, return mock data
    let settlements = vec![
        QuantumSettlement {
            position_id: "mock-pos-1".to_string(),
            wallet: query.wallet.clone().unwrap_or_else(|| "default-wallet".to_string()),
            market_id: query.market_id.unwrap_or(1),
            outcome: 0,
            collapsed_amount: 100000,
            leverage: 2,
            entry_probability: 0.5,
            settlement_price: 1.0,
            pnl: 100000,
            pnl_percentage: 100.0,
            settlement_time: Utc::now(),
            quantum_bonus: 0.05,
            coherence_multiplier: 1.1,
        },
    ];

    ApiResponse::success(settlements).into_response()
}

/// Get quantum settlement status for a market
pub async fn get_quantum_settlement_status(
    Path(market_id): Path<u128>,
    State(state): State<AppState>,
) -> Response {
    // Get market quantum positions
    let positions = match state.quantum_engine.get_market_quantum_states(market_id).await {
        Ok(states) => states,
        Err(e) => {
            return ApiResponse::<()>::error("ERROR", &format!("Failed to get quantum states: {}", e)).into_response();
        }
    };

    let total_positions = positions.len();
    // Count settled positions - for now we'll assume all are pending
    let settled_positions = 0;
    let pending_positions = total_positions;

    // Calculate aggregates (simplified)
    let total_payout = settled_positions as u64 * 100000; // Mock
    let total_quantum_bonus = settled_positions as f64 * 0.05;
    let average_coherence_multiplier = 1.1;

    let status = QuantumSettlementStatus {
        total_positions,
        settled_positions,
        pending_positions,
        total_payout,
        total_quantum_bonus,
        average_coherence_multiplier,
    };

    ApiResponse::success(status).into_response()
}

/// Trigger automatic quantum settlement for expired positions
pub async fn trigger_quantum_settlement(
    State(state): State<AppState>,
) -> Response {
    // Get all positions past coherence time
    let measurements = match state.quantum_engine.get_measurements().await {
        Ok(m) => m,
        Err(e) => {
            return ApiResponse::<()>::error("ERROR", &format!("Failed to get measurements: {}", e)).into_response();
        }
    };

    let mut settled_count = 0;
    let mut failed_count = 0;

    // Process each expired position
    for measurement in measurements {
        if measurement.caused_collapse {
            // Position already collapsed, check if needs settlement
            // This is simplified - in practice we'd check settlement status
            settled_count += 1;
        }
    }

    ApiResponse::success(serde_json::json!({
        "settled": settled_count,
        "failed": failed_count,
        "timestamp": Utc::now(),
    })).into_response()
}

/// Helper function to record settlement in database
async fn record_quantum_settlement(
    state: &AppState,
    settlement: &QuantumSettlement,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = state.database.get_connection().await?;
    
    // Insert settlement record
    let query = r#"
        INSERT INTO quantum_settlements 
        (position_id, wallet, market_id, outcome, amount, pnl, quantum_bonus, settlement_time)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    "#;
    
    conn.execute(
        query,
        &[
            &settlement.position_id,
            &settlement.wallet,
            &(settlement.market_id as i64),
            &(settlement.outcome as i16),
            &(settlement.collapsed_amount as i64),
            &(settlement.pnl as i64),
            &settlement.quantum_bonus,
            &settlement.settlement_time,
        ],
    ).await?;
    
    Ok(())
}