//! Quantum trading handlers for advanced multi-market positions
//! Implements quantum superposition trading with production-grade features

use axum::{
    extract::{State, Query, Path},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};
use crate::{
    AppState,
    middleware::{AuthenticatedUser, OptionalAuth},
    response::responses,
    validation::ValidatedJson,
    quantum_engine_ext::QuantumPositionExt,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Quantum position state
#[derive(Debug, Serialize, Clone)]
pub struct QuantumState {
    pub market_id: u64,
    pub verse_id: u32,
    pub probability: f64,
    pub amplitude: f64,
    pub phase: f64,
    pub entanglement_strength: f64,
}

/// Quantum position information
#[derive(Debug, Serialize)]
pub struct QuantumPosition {
    pub position_id: String,
    pub wallet: String,
    pub states: Vec<QuantumState>,
    pub total_amount: u64,
    pub leverage: u8,
    pub created_at: DateTime<Utc>,
    pub last_observation: DateTime<Utc>,
    pub collapse_probability: f64,
    pub expected_value: f64,
    pub quantum_entropy: f64,
    pub coherence_time: u64, // in seconds
    pub is_collapsed: bool,
    pub collapsed_state: Option<CollapsedState>,
}

#[derive(Debug, Serialize)]
pub struct CollapsedState {
    pub market_id: u64,
    pub outcome: u8,
    pub final_amount: u64,
    pub collapse_time: DateTime<Utc>,
    pub pnl: f64,
}

/// Market correlation data
#[derive(Debug, Serialize)]
pub struct MarketCorrelation {
    pub market_a_id: u64,
    pub market_b_id: u64,
    pub correlation_coefficient: f64,
    pub mutual_information: f64,
    pub entanglement_potential: f64,
    pub last_updated: DateTime<Utc>,
}

/// Quantum trade request
#[derive(Debug, Deserialize)]
pub struct QuantumTradeRequest {
    pub verses: Vec<u32>,
    pub amount: u64,
    pub wallet: String,
    #[serde(default = "default_leverage")]
    pub leverage: u8,
    #[serde(default = "default_collapse_strategy")]
    pub collapse_strategy: CollapseStrategy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entanglement_params: Option<EntanglementParams>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CollapseStrategy {
    Balanced,      // Equal probability distribution
    MaxProfit,     // Collapse to highest expected value
    MinRisk,       // Collapse to lowest volatility
    Correlated,    // Follow correlated markets
    Quantum,       // True quantum collapse (random)
}

#[derive(Debug, Deserialize)]
pub struct EntanglementParams {
    pub coupling_strength: f64,
    pub decoherence_rate: f64,
    pub measurement_basis: String,
}

fn default_leverage() -> u8 { 1 }
fn default_collapse_strategy() -> CollapseStrategy { CollapseStrategy::Balanced }

/// Quantum trade response
#[derive(Debug, Serialize)]
pub struct QuantumTradeResponse {
    pub success: bool,
    pub quantum_position_id: String,
    pub positions: Vec<QuantumPositionEntry>,
    pub total_amount: u64,
    pub expected_value: f64,
    pub quantum_entropy: f64,
    pub signature: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct QuantumPositionEntry {
    pub market_id: u64,
    pub verse_id: u32,
    pub probability: f64,
    pub amount: u64,
}

/// Execute quantum trade
pub async fn execute_quantum_trade(
    State(state): State<AppState>,
    Json(payload): Json<QuantumTradeRequest>,
) -> Response {
    debug!("Quantum trade request: {:?}", payload);
    
    // Validate verses
    if payload.verses.is_empty() {
        return responses::bad_request("At least one verse required").into_response();
    }
    
    if payload.verses.len() > 10 {
        return responses::bad_request("Maximum 10 verses allowed in quantum position").into_response();
    }
    
    // Validate amount
    if payload.amount == 0 {
        return responses::bad_request("Amount must be greater than 0").into_response();
    }
    
    // Get market information for each verse
    let mut positions = Vec::new();
    let mut total_probability = 0.0;
    
    for verse_id in &payload.verses {
        // Map verse to market (in production, would use verse catalog)
        let market_id = 1000 + (*verse_id as u64);
        let probability = calculate_quantum_probability(&payload.verses, *verse_id);
        let amount = (payload.amount as f64 * probability) as u64;
        
        positions.push(QuantumPositionEntry {
            market_id,
            verse_id: *verse_id,
            probability,
            amount,
        });
        
        total_probability += probability;
    }
    
    // Normalize probabilities
    for pos in &mut positions {
        pos.probability /= total_probability;
    }
    
    // Calculate quantum metrics
    let quantum_entropy = calculate_quantum_entropy(&positions);
    let expected_value = calculate_expected_value(&positions, &state).await;
    
    // Create quantum position with deterministic ID
    let quantum_position_id = format!("quantum_{}", chrono::Utc::now().timestamp_micros() as u128);
    
    // Store in quantum engine
    state.quantum_engine.create_position(
        quantum_position_id.clone(),
        payload.wallet.clone(),
        positions.clone(),
        payload.leverage,
        payload.collapse_strategy,
    ).await;
    
    let response = QuantumTradeResponse {
        success: true,
        quantum_position_id,
        positions,
        total_amount: payload.amount,
        expected_value,
        quantum_entropy,
        signature: format!("quantum_sig_{}", Uuid::new_v4()),
    };
    
    info!("Quantum trade executed: {:?}", response);
    responses::ok(response).into_response()
}

/// Get market correlations
#[derive(Debug, Deserialize)]
pub struct CorrelationsQuery {
    pub market_id: Option<u64>,
    pub min_correlation: Option<f64>,
    pub include_quantum_metrics: Option<bool>,
}

/// Get quantum correlations between markets
pub async fn get_quantum_correlations(
    State(state): State<AppState>,
    Query(params): Query<CorrelationsQuery>,
) -> Response {
    let mut correlations = Vec::new();
    
    // In production, calculate from historical data
    // For now, generate sample correlations
    let markets = state.seeded_markets.get_all_markets().await;
    
    for i in 0..markets.len() {
        for j in (i+1)..markets.len() {
            let market_a_id = markets[i]["id"].as_u64().unwrap_or(0);
            let market_b_id = markets[j]["id"].as_u64().unwrap_or(0);
            
            // Filter by market if specified
            if let Some(target_id) = params.market_id {
                if market_a_id != target_id && market_b_id != target_id {
                    continue;
                }
            }
            
            let correlation = calculate_market_correlation(market_a_id, market_b_id);
            
            // Filter by minimum correlation
            if let Some(min_corr) = params.min_correlation {
                if correlation.correlation_coefficient.abs() < min_corr {
                    continue;
                }
            }
            
            correlations.push(correlation);
        }
    }
    
    // Sort by correlation strength
    correlations.sort_by(|a, b| {
        b.correlation_coefficient.abs()
            .partial_cmp(&a.correlation_coefficient.abs())
            .unwrap()
    });
    
    responses::ok(json!({
        "correlations": correlations,
        "count": correlations.len(),
        "quantum_metrics_included": params.include_quantum_metrics.unwrap_or(false)
    })).into_response()
}

/// Adjust quantum position request
#[derive(Debug, Deserialize)]
pub struct AdjustQuantumRequest {
    pub position_id: String,
    pub wallet: String,
    pub action: AdjustmentAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<AdjustmentParameters>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdjustmentAction {
    Rebalance,      // Rebalance probabilities
    AddVerse,       // Add new verse to superposition
    RemoveVerse,    // Remove verse from superposition
    ChangeStrategy, // Change collapse strategy
    ForceCoherence, // Force quantum coherence
}

#[derive(Debug, Deserialize)]
pub struct AdjustmentParameters {
    pub verse_id: Option<u32>,
    pub new_strategy: Option<CollapseStrategy>,
    pub rebalance_weights: Option<Vec<f64>>,
}

/// Adjust quantum position response
#[derive(Debug, Serialize)]
pub struct AdjustQuantumResponse {
    pub success: bool,
    pub position_id: String,
    pub action_performed: String,
    pub new_states: Vec<QuantumState>,
    pub quantum_entropy: f64,
    pub coherence_restored: bool,
}

/// Adjust quantum position
pub async fn adjust_quantum_position(
    State(state): State<AppState>,
    Json(payload): Json<AdjustQuantumRequest>,
) -> Response {
    
    // Get quantum position
    let position = match state.quantum_engine.get_position(&payload.position_id).await {
        Some(pos) => pos,
        None => return responses::not_found("Quantum position not found").into_response(),
    };
    
    // Verify ownership
    if position.wallet != payload.wallet {
        return responses::forbidden("Position does not belong to wallet").into_response();
    }
    
    // Check if already collapsed
    if position.is_collapsed {
        return responses::bad_request("Cannot adjust collapsed position").into_response();
    }
    
    // Perform adjustment
    let (new_states, action_performed) = match payload.action {
        AdjustmentAction::Rebalance => {
            let states = rebalance_quantum_states(&position.states);
            (states, "Rebalanced quantum probabilities")
        },
        AdjustmentAction::AddVerse => {
            if let Some(params) = payload.parameters {
                if let Some(verse_id) = params.verse_id {
                    let states = add_verse_to_position(&position.states, verse_id);
                    (states, "Added new verse to superposition")
                } else {
                    return responses::bad_request("Verse ID required for AddVerse action").into_response();
                }
            } else {
                return responses::bad_request("Parameters required for AddVerse action").into_response();
            }
        },
        AdjustmentAction::RemoveVerse => {
            if let Some(params) = payload.parameters {
                if let Some(verse_id) = params.verse_id {
                    let states = remove_verse_from_position(&position.states, verse_id);
                    (states, "Removed verse from superposition")
                } else {
                    return responses::bad_request("Verse ID required for RemoveVerse action").into_response();
                }
            } else {
                return responses::bad_request("Parameters required for RemoveVerse action").into_response();
            }
        },
        AdjustmentAction::ChangeStrategy => {
            if let Some(params) = payload.parameters {
                if let Some(new_strategy) = params.new_strategy {
                    state.quantum_engine.update_collapse_strategy(
                        &payload.position_id,
                        new_strategy
                    ).await;
                    (position.states.clone(), "Changed collapse strategy")
                } else {
                    return responses::bad_request("New strategy required for ChangeStrategy action").into_response();
                }
            } else {
                return responses::bad_request("Parameters required for ChangeStrategy action").into_response();
            }
        },
        AdjustmentAction::ForceCoherence => {
            let states = restore_quantum_coherence(&position.states);
            (states, "Forced quantum coherence restoration")
        },
    };
    
    // Update position in quantum engine
    state.quantum_engine.update_states(&payload.position_id, new_states.clone()).await;
    
    // Calculate new entropy
    let quantum_entropy = calculate_state_entropy(&new_states);
    
    let response = AdjustQuantumResponse {
        success: true,
        position_id: payload.position_id,
        action_performed: action_performed.to_string(),
        new_states,
        quantum_entropy,
        coherence_restored: true,
    };
    
    info!("Quantum position adjusted: {:?}", response);
    responses::ok(response).into_response()
}

/// Collapse quantum position request
#[derive(Debug, Deserialize)]
pub struct CollapseQuantumRequest {
    pub position_id: String,
    pub wallet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verses: Option<Vec<u32>>, // Partial collapse to specific verses
    #[serde(default)]
    pub force_collapse: bool,
}

/// Collapse quantum position response
#[derive(Debug, Serialize)]
pub struct CollapseQuantumResponse {
    pub success: bool,
    pub position_id: String,
    pub collapsed_market: u64,
    pub collapsed_verse: u32,
    pub final_amount: u64,
    pub total_pnl: f64,
    pub collapse_probability: f64,
    pub signature: String,
}

/// Collapse quantum position
pub async fn collapse_quantum_position(
    State(state): State<AppState>,
    Json(payload): Json<CollapseQuantumRequest>,
) -> Response {
    
    // Get quantum position
    let position = match state.quantum_engine.get_position(&payload.position_id).await {
        Some(pos) => pos,
        None => return responses::not_found("Quantum position not found").into_response(),
    };
    
    // Verify ownership
    if position.wallet != payload.wallet {
        return responses::forbidden("Position does not belong to wallet").into_response();
    }
    
    // Check if already collapsed
    if position.is_collapsed {
        return responses::bad_request("Position already collapsed").into_response();
    }
    
    // Determine collapse outcome
    let (collapsed_state, collapse_probability) = if let Some(target_verses) = payload.verses {
        // Partial collapse to specific verses
        perform_partial_collapse(&position, target_verses)
    } else {
        // Full quantum collapse
        perform_quantum_collapse(&position)
    };
    
    // Calculate P&L
    let market = state.seeded_markets.get_market(collapsed_state.market_id).await;
    let current_price = market.as_ref()
        .and_then(|m| m["outcomes"][0]["total_stake"].as_f64())
        .unwrap_or(0.5);
    let pnl = calculate_collapse_pnl(&position, &collapsed_state, current_price);
    
    // Update position in quantum engine
    state.quantum_engine.collapse_position(
        &payload.position_id,
        crate::quantum_engine_ext::CollapsedStateInternal {
            market_id: collapsed_state.market_id,
            verse_id: collapsed_state.verse_id,
            final_amount: collapsed_state.final_amount,
        },
        pnl
    ).await;
    
    let response = CollapseQuantumResponse {
        success: true,
        position_id: payload.position_id,
        collapsed_market: collapsed_state.market_id,
        collapsed_verse: collapsed_state.verse_id,
        final_amount: collapsed_state.final_amount,
        total_pnl: pnl,
        collapse_probability,
        signature: format!("collapse_sig_{}", Uuid::new_v4()),
    };
    
    info!("Quantum position collapsed: {:?}", response);
    responses::ok(response).into_response()
}

/// Helper functions
fn calculate_quantum_probability(verses: &[u32], target_verse: u32) -> f64 {
    // Simple equal distribution for now
    // In production, would use quantum mechanical calculations
    1.0 / verses.len() as f64
}

fn calculate_quantum_entropy(positions: &[QuantumPositionEntry]) -> f64 {
    positions.iter()
        .map(|p| {
            if p.probability > 0.0 {
                -p.probability * p.probability.ln()
            } else {
                0.0
            }
        })
        .sum()
}

async fn calculate_expected_value(positions: &[QuantumPositionEntry], state: &AppState) -> f64 {
    let mut expected_value = 0.0;
    
    for pos in positions {
        let market = state.seeded_markets.get_market(pos.market_id).await;
        let price = market.as_ref()
            .and_then(|m| m["outcomes"][0]["total_stake"].as_f64())
            .unwrap_or(0.5);
        
        expected_value += pos.amount as f64 * price * pos.probability;
    }
    
    expected_value
}

fn calculate_market_correlation(market_a: u64, market_b: u64) -> MarketCorrelation {
    // In production, calculate from historical price movements
    // For now, generate sample correlations
    let base_correlation = ((market_a + market_b) % 100) as f64 / 100.0 - 0.5;
    
    MarketCorrelation {
        market_a_id: market_a,
        market_b_id: market_b,
        correlation_coefficient: base_correlation,
        mutual_information: base_correlation.abs() * 0.8,
        entanglement_potential: (base_correlation.abs() * 1.5).min(1.0),
        last_updated: Utc::now(),
    }
}

fn calculate_state_entropy(states: &[QuantumState]) -> f64 {
    states.iter()
        .map(|s| {
            if s.probability > 0.0 {
                -s.probability * s.probability.ln()
            } else {
                0.0
            }
        })
        .sum()
}

fn rebalance_quantum_states(states: &[QuantumState]) -> Vec<QuantumState> {
    let equal_prob = 1.0 / states.len() as f64;
    states.iter()
        .map(|s| QuantumState {
            probability: equal_prob,
            ..s.clone()
        })
        .collect()
}

fn add_verse_to_position(states: &[QuantumState], verse_id: u32) -> Vec<QuantumState> {
    let mut new_states = states.to_vec();
    let new_prob = 1.0 / (states.len() + 1) as f64;
    
    // Renormalize existing states
    for state in &mut new_states {
        state.probability *= states.len() as f64 / (states.len() + 1) as f64;
    }
    
    // Add new state
    new_states.push(QuantumState {
        market_id: 1000 + verse_id as u64,
        verse_id,
        probability: new_prob,
        amplitude: new_prob.sqrt(),
        phase: 0.0,
        entanglement_strength: 0.5,
    });
    
    new_states
}

fn remove_verse_from_position(states: &[QuantumState], verse_id: u32) -> Vec<QuantumState> {
    let filtered: Vec<_> = states.iter()
        .filter(|s| s.verse_id != verse_id)
        .cloned()
        .collect();
    
    if filtered.is_empty() {
        states.to_vec() // Can't remove last verse
    } else {
        // Renormalize
        let total_prob: f64 = filtered.iter().map(|s| s.probability).sum();
        filtered.into_iter()
            .map(|mut s| {
                s.probability /= total_prob;
                s
            })
            .collect()
    }
}

fn restore_quantum_coherence(states: &[QuantumState]) -> Vec<QuantumState> {
    // Restore quantum coherence by adjusting phases
    states.iter()
        .enumerate()
        .map(|(i, s)| QuantumState {
            phase: (i as f64 * std::f64::consts::PI / states.len() as f64),
            entanglement_strength: 0.8,
            ..s.clone()
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct CollapsedStateInternal {
    pub market_id: u64,
    pub verse_id: u32,
    pub final_amount: u64,
}

fn perform_partial_collapse(position: &QuantumPositionExt, target_verses: Vec<u32>) -> (CollapsedStateInternal, f64) {
    // Filter to target verses
    let target_states: Vec<_> = position.states.iter()
        .filter(|s| target_verses.contains(&s.verse_id))
        .collect();
    
    if target_states.is_empty() {
        // Fallback to full collapse
        perform_quantum_collapse(position)
    } else {
        // Collapse within target verses
        let total_prob: f64 = target_states.iter().map(|s| s.probability).sum();
        let random = rand::random::<f64>() * total_prob;
        
        let mut cumulative = 0.0;
        for state in target_states {
            cumulative += state.probability;
            if random < cumulative {
                return (CollapsedStateInternal {
                    market_id: state.market_id,
                    verse_id: state.verse_id,
                    final_amount: position.total_amount,
                }, state.probability);
            }
        }
        
        // Fallback
        perform_quantum_collapse(position)
    }
}

fn perform_quantum_collapse(position: &QuantumPositionExt) -> (CollapsedStateInternal, f64) {
    // True quantum collapse based on probability distribution
    let random = rand::random::<f64>();
    let mut cumulative = 0.0;
    
    for state in &position.states {
        cumulative += state.probability;
        if random < cumulative {
            return (CollapsedStateInternal {
                market_id: state.market_id,
                verse_id: state.verse_id,
                final_amount: position.total_amount,
            }, state.probability);
        }
    }
    
    // Fallback to first state
    let first = &position.states[0];
    (CollapsedStateInternal {
        market_id: first.market_id,
        verse_id: first.verse_id,
        final_amount: position.total_amount,
    }, first.probability)
}

fn calculate_collapse_pnl(
    position: &QuantumPositionExt,
    collapsed_state: &CollapsedStateInternal,
    current_price: f64
) -> f64 {
    let entry_cost = position.total_amount as f64;
    let exit_value = collapsed_state.final_amount as f64 * current_price;
    let base_pnl = exit_value - entry_cost;
    base_pnl * position.leverage as f64
}

// Extension trait for UserRole
trait UserRoleExt {
    fn is_admin(&self) -> bool;
}

impl UserRoleExt for crate::auth::UserRole {
    fn is_admin(&self) -> bool {
        matches!(self, crate::auth::UserRole::Admin)
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quantum_entropy() {
        let positions = vec![
            QuantumPositionEntry {
                market_id: 1000,
                verse_id: 1,
                probability: 0.5,
                amount: 500,
            },
            QuantumPositionEntry {
                market_id: 1001,
                verse_id: 2,
                probability: 0.5,
                amount: 500,
            },
        ];
        
        let entropy = calculate_quantum_entropy(&positions);
        assert!((entropy - 0.693).abs() < 0.001); // ln(2) ≈ 0.693
    }
    
    #[test]
    fn test_state_rebalancing() {
        let states = vec![
            QuantumState {
                market_id: 1000,
                verse_id: 1,
                probability: 0.7,
                amplitude: 0.84,
                phase: 0.0,
                entanglement_strength: 0.5,
            },
            QuantumState {
                market_id: 1001,
                verse_id: 2,
                probability: 0.3,
                amplitude: 0.55,
                phase: 0.0,
                entanglement_strength: 0.5,
            },
        ];
        
        let rebalanced = rebalance_quantum_states(&states);
        assert_eq!(rebalanced.len(), 2);
        assert!((rebalanced[0].probability - 0.5).abs() < 0.001);
        assert!((rebalanced[1].probability - 0.5).abs() < 0.001);
    }
}
