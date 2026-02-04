//! Quantum position settlement system
//! Handles settlement of collapsed quantum positions with real market outcomes

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::{
    quantum_engine::{QuantumPosition, QuantumState, QuantumMeasurement},
    types::{Market, MarketOutcome},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumSettlement {
    pub position_id: String,
    pub wallet: String,
    pub market_id: u128,
    pub outcome: u8,
    pub collapsed_amount: u64,
    pub leverage: u32,
    pub entry_probability: f64,
    pub settlement_price: f64,
    pub pnl: i128,
    pub pnl_percentage: f64,
    pub settlement_time: DateTime<Utc>,
    pub quantum_bonus: f64, // Bonus for quantum risk-taking
    pub coherence_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumSettlementBatch {
    pub batch_id: String,
    pub market_id: u128,
    pub winning_outcome: u8,
    pub settlement_price: f64,
    pub settlements: Vec<QuantumSettlement>,
    pub total_payout: u64,
    pub total_quantum_bonus: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumSettlementEngine {
    pub quantum_bonus_rate: f64, // Base bonus rate for quantum positions
    pub coherence_decay_rate: f64, // How much coherence affects settlement
    pub entanglement_bonus: f64, // Bonus for entangled positions
}

impl QuantumSettlementEngine {
    pub fn new() -> Self {
        Self {
            quantum_bonus_rate: 0.05, // 5% base bonus
            coherence_decay_rate: 0.01, // 1% decay per hour
            entanglement_bonus: 0.02, // 2% extra for entangled
        }
    }

    /// Settle a collapsed quantum position
    pub async fn settle_quantum_position(
        &self,
        position: &QuantumPosition,
        market: &Market,
        winning_outcome: u8,
    ) -> Result<QuantumSettlement> {
        if !position.is_collapsed {
            return Err(anyhow!("Cannot settle uncollapsed quantum position"));
        }

        let collapsed_state = position.measurement_result
            .as_ref()
            .ok_or_else(|| anyhow!("No measurement result found"))?;

        // Calculate settlement based on collapsed state
        let is_winner = collapsed_state.outcome == winning_outcome;
        let settlement_price = self.calculate_settlement_price(market, winning_outcome)?;
        
        // Calculate base P&L
        let base_pnl = if is_winner {
            (collapsed_state.amount as f64 * settlement_price) as i128 - collapsed_state.amount as i128
        } else {
            -(collapsed_state.amount as i128)
        };

        // Calculate quantum bonuses
        let coherence_multiplier = self.calculate_coherence_multiplier(position)?;
        let quantum_bonus = self.calculate_quantum_bonus(
            position,
            base_pnl,
            coherence_multiplier,
        )?;

        // Apply leverage
        let leveraged_pnl = base_pnl * collapsed_state.leverage as i128;
        let total_pnl = leveraged_pnl + (quantum_bonus * collapsed_state.amount as f64) as i128;

        Ok(QuantumSettlement {
            position_id: position.id.clone(),
            wallet: position.wallet.clone(),
            market_id: collapsed_state.market_id,
            outcome: collapsed_state.outcome,
            collapsed_amount: collapsed_state.amount,
            leverage: collapsed_state.leverage,
            entry_probability: collapsed_state.probability,
            settlement_price,
            pnl: total_pnl,
            pnl_percentage: (total_pnl as f64 / collapsed_state.amount as f64) * 100.0,
            settlement_time: Utc::now(),
            quantum_bonus,
            coherence_multiplier,
        })
    }

    /// Settle all quantum positions for a resolved market
    pub async fn settle_market_quantum_positions(
        &self,
        market_id: u128,
        market: &Market,
        winning_outcome: u8,
        positions: Vec<QuantumPosition>,
    ) -> Result<QuantumSettlementBatch> {
        let mut settlements = Vec::new();
        let mut total_payout = 0u64;
        let mut total_quantum_bonus = 0.0;

        for position in positions {
            if position.is_collapsed {
                match self.settle_quantum_position(&position, market, winning_outcome).await {
                    Ok(settlement) => {
                        if settlement.pnl > 0 {
                            total_payout += settlement.pnl as u64;
                        }
                        total_quantum_bonus += settlement.quantum_bonus * settlement.collapsed_amount as f64;
                        settlements.push(settlement);
                    }
                    Err(e) => {
                        tracing::error!("Failed to settle quantum position {}: {}", position.id, e);
                    }
                }
            }
        }

        Ok(QuantumSettlementBatch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            market_id,
            winning_outcome,
            settlement_price: self.calculate_settlement_price(market, winning_outcome)?,
            settlements,
            total_payout,
            total_quantum_bonus,
            timestamp: Utc::now(),
        })
    }

    /// Calculate settlement price based on market outcome
    fn calculate_settlement_price(&self, market: &Market, winning_outcome: u8) -> Result<f64> {
        // For binary markets, winner gets 1.0, loser gets 0.0
        if market.outcomes.len() == 2 {
            return Ok(1.0);
        }

        // For multi-outcome markets, calculate based on total stake distribution
        let total_stake: u64 = market.outcomes.iter().map(|o| o.total_stake).sum();
        let winning_stake = market.outcomes.get(winning_outcome as usize)
            .map(|o| o.total_stake)
            .ok_or_else(|| anyhow!("Invalid winning outcome"))?;

        if winning_stake == 0 {
            return Ok(1.0); // No one bet on winner, full payout
        }

        Ok(total_stake as f64 / winning_stake as f64)
    }

    /// Calculate coherence multiplier based on position lifetime
    fn calculate_coherence_multiplier(&self, position: &QuantumPosition) -> Result<f64> {
        let created = DateTime::<Utc>::from_timestamp(position.created_at, 0)
            .ok_or_else(|| anyhow!("Invalid creation timestamp"))?;
        
        let measured = position.last_measured
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
            .unwrap_or_else(Utc::now);

        let duration_hours = (measured - created).num_hours() as f64;
        let coherence_factor = position.coherence_time as f64 / 3600.0; // Convert to hours

        // Higher multiplier for positions held longer while maintaining coherence
        let multiplier = 1.0 + (duration_hours / coherence_factor).min(1.0) * 0.1;
        
        Ok(multiplier)
    }

    /// Calculate quantum bonus based on position characteristics
    fn calculate_quantum_bonus(
        &self,
        position: &QuantumPosition,
        base_pnl: i128,
        coherence_multiplier: f64,
    ) -> Result<f64> {
        // Only apply bonus to winning positions
        if base_pnl <= 0 {
            return Ok(0.0);
        }

        let mut bonus = self.quantum_bonus_rate;

        // Bonus for maintaining superposition
        let superposition_bonus = position.states.len() as f64 * 0.01; // 1% per state
        bonus += superposition_bonus;

        // Bonus for entanglement
        if position.entanglement_group.is_some() {
            bonus += self.entanglement_bonus;
        }

        // Apply coherence multiplier
        bonus *= coherence_multiplier;

        // Cap at 20% maximum bonus
        Ok(bonus.min(0.2))
    }

    /// Calculate entanglement correlation bonus
    pub async fn calculate_entanglement_bonus(
        &self,
        positions: &[QuantumPosition],
        group_id: &str,
    ) -> Result<f64> {
        let group_positions: Vec<_> = positions
            .iter()
            .filter(|p| p.entanglement_group.as_ref() == Some(&group_id.to_string()))
            .collect();

        if group_positions.len() < 2 {
            return Ok(0.0);
        }

        // Calculate correlation between collapsed states
        let mut correlation_sum = 0.0;
        let mut count = 0;

        for i in 0..group_positions.len() {
            for j in i + 1..group_positions.len() {
                if let (Some(state_i), Some(state_j)) = (
                    &group_positions[i].measurement_result,
                    &group_positions[j].measurement_result,
                ) {
                    // Simple correlation: same market and outcome
                    if state_i.market_id == state_j.market_id && 
                       state_i.outcome == state_j.outcome {
                        correlation_sum += 1.0;
                    }
                    count += 1;
                }
            }
        }

        if count == 0 {
            return Ok(0.0);
        }

        let correlation = correlation_sum / count as f64;
        Ok(correlation * self.entanglement_bonus)
    }
}

/// Helper function to process quantum settlements into blockchain transactions
pub async fn process_quantum_settlements(
    settlements: &[QuantumSettlement],
    program_client: &crate::rpc_client::BettingPlatformClient,
) -> Result<Vec<String>> {
    let mut signatures = Vec::new();

    for settlement in settlements {
        if settlement.pnl > 0 {
            // Process payout
            match program_client.process_quantum_settlement(
                &settlement.wallet,
                &settlement.position_id,
                settlement.pnl as u64,
            ).await {
                Ok(sig) => signatures.push(sig),
                Err(e) => {
                    tracing::error!(
                        "Failed to process quantum settlement for position {}: {}",
                        settlement.position_id, e
                    );
                }
            }
        }
    }

    Ok(signatures)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quantum_settlement() {
        let engine = QuantumSettlementEngine::new();
        
        // Create test position
        let position = QuantumPosition {
            id: "test-pos".to_string(),
            wallet: "test-wallet".to_string(),
            states: vec![],
            entanglement_group: None,
            coherence_time: 3600,
            created_at: Utc::now().timestamp() - 1800, // 30 min ago
            last_measured: Some(Utc::now().timestamp()),
            is_collapsed: true,
            measurement_result: Some(QuantumState {
                market_id: 1,
                outcome: 0,
                amount: 1000000, // 1 USDC
                leverage: 2,
                amplitude: 0.707,
                phase: 0.0,
                probability: 0.5,
                entangled_with: vec![],
            }),
        };

        // Create test market
        let market = Market {
            id: 1,
            title: "Test Market".to_string(),
            description: "".to_string(),
            creator: Default::default(),
            outcomes: vec![
                MarketOutcome {
                    name: "Yes".to_string(),
                    total_stake: 1000000,
                },
                MarketOutcome {
                    name: "No".to_string(),
                    total_stake: 1000000,
                },
            ],
            amm_type: crate::types::AmmType::Cpmm,
            total_volume: 2000000,
            total_liquidity: 1000000,
            resolution_time: Utc::now().timestamp() + 3600,
            resolved: true,
            winning_outcome: Some(0),
            created_at: Utc::now().timestamp() - 86400,
            verse_id: Some(1),
        };

        let settlement = engine.settle_quantum_position(&position, &market, 0)
            .await
            .expect("Settlement should succeed");

        assert_eq!(settlement.wallet, "test-wallet");
        assert!(settlement.pnl > 0);
        assert!(settlement.quantum_bonus > 0.0);
    }
}