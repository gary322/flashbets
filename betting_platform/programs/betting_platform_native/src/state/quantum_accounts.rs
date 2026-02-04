//! Quantum account structures for quantum capital efficiency

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use crate::math::U64F64;

/// Quantum position tracking superposed bets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct QuantumPosition {
    /// Unique position identifier
    pub position_id: u128,
    /// User who owns the position
    pub user: Pubkey,
    /// Verse this position exists in
    pub verse_id: u128,
    /// Proposals this position covers (superposition)
    pub proposals: Vec<u128>,
    /// Base collateral amount
    pub collateral: u64,
    /// Effective exposure per proposal
    pub exposure_per_proposal: u64,
    /// Quantum state (superposition until collapse)
    pub quantum_state: QuantumState,
    /// Timestamp of creation
    pub created_at: i64,
}

/// Quantum state for superposition
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum QuantumState {
    /// Active superposition across proposals
    Superposition {
        /// Probability weights for each proposal
        weights: Vec<u16>, // Basis points (10000 = 100%)
    },
    /// Collapsed to specific outcome
    Collapsed {
        /// Winning proposal
        winning_proposal: u128,
        /// Collapse timestamp
        collapsed_at: i64,
    },
}

impl QuantumPosition {
    /// Check if position is in superposition
    pub fn is_superposed(&self) -> bool {
        matches!(self.quantum_state, QuantumState::Superposition { .. })
    }
    
    /// Get total exposure across all proposals
    pub fn total_exposure(&self) -> u64 {
        self.exposure_per_proposal
            .saturating_mul(self.proposals.len() as u64)
    }
    
    /// Calculate effective leverage
    pub fn effective_leverage(&self) -> u64 {
        if self.collateral == 0 {
            return 0;
        }
        self.total_exposure() / self.collateral
    }
}

/// Default implementation for Position (used in tests)
#[cfg(test)]
impl Default for crate::state::Position {
    fn default() -> Self {
        Self {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user: Pubkey::default(),
            proposal_id: 0,
            position_id: [0; 32],
            outcome: 0,
            size: 0,
            notional: 0,
            leverage: 0,
            entry_price: 0,
            liquidation_price: 0,
            is_long: false,
            created_at: 0,
            entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 0,
            margin: 0,
            collateral: 0,
            is_short: false,
            last_mark_price: 0,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        }
    }
}