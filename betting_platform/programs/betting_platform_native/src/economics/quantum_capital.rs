//! Quantum Capital Efficiency Implementation
//!
//! Implements quantum capital efficiency where one deposit provides
//! credits for multiple proposals within a verse, enabling capital
//! efficiency through quantum superposition of bets.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    error::BettingPlatformError,
    state::{
        VersePDA, 
        ProposalPDA,
        Position,
        VerseStatus,
        quantum_accounts::QuantumPosition,
    },
};

/// Quantum credit structure for tracking multi-proposal exposure
#[derive(Debug, Clone)]
pub struct QuantumCredit {
    /// User who owns the credit
    pub user: Pubkey,
    /// Base deposit amount
    pub deposit: u64,
    /// Verse ID this credit applies to
    pub verse_id: u128,
    /// Credits available per proposal
    pub credits_per_proposal: u64,
    /// Active positions using this credit
    pub active_positions: Vec<u128>, // position IDs
    /// Total exposure across all positions
    pub total_exposure: u64,
    /// Timestamp of creation
    pub created_at: i64,
}

impl QuantumCredit {
    /// Create new quantum credit from deposit
    pub fn new(user: Pubkey, deposit: u64, verse_id: u128, created_at: i64) -> Self {
        Self {
            user,
            deposit,
            verse_id,
            credits_per_proposal: deposit, // 1:1 initially
            active_positions: Vec::new(),
            total_exposure: 0,
            created_at,
        }
    }
    
    /// Calculate available credit for a new position
    pub fn available_credit(&self, num_proposals: u8) -> u64 {
        if num_proposals == 0 {
            return 0;
        }
        
        // Each proposal gets full deposit as credit in quantum superposition
        // But total exposure is tracked to ensure solvency
        self.credits_per_proposal
    }
    
    /// Use credit for a position
    pub fn use_credit(&mut self, position_id: u128, amount: u64) -> Result<(), ProgramError> {
        if amount > self.credits_per_proposal {
            return Err(BettingPlatformError::InsufficientBalance.into());
        }
        
        self.active_positions.push(position_id);
        self.total_exposure = self.total_exposure.saturating_add(amount);
        
        Ok(())
    }
    
    /// Release credit when position closes
    pub fn release_credit(&mut self, position_id: u128, amount: u64) {
        self.active_positions.retain(|&id| id != position_id);
        self.total_exposure = self.total_exposure.saturating_sub(amount);
    }
    
    /// Check if credit can be withdrawn
    pub fn can_withdraw(&self) -> bool {
        self.active_positions.is_empty() && self.total_exposure == 0
    }
}

/// Calculate quantum capital efficiency for a verse
pub fn calculate_quantum_efficiency(
    verse: &VersePDA,
    user_deposit: u64,
) -> Result<QuantumEfficiency, ProgramError> {
    // Validate verse is active
    if verse.status != VerseStatus::Active {
        return Err(BettingPlatformError::VerseNotActive.into());
    }
    
    let num_proposals = verse.child_count as u64; // Use child_count as proxy for proposals
    
    // Base efficiency: 1 deposit provides credits for N proposals
    let base_multiplier = num_proposals as u64;
    
    // Depth bonus: deeper verses get more efficiency
    let depth_bonus = 1u64.saturating_add(verse.depth as u64 / 10);
    
    // Total effective capital
    let effective_capital = user_deposit
        .saturating_mul(base_multiplier)
        .saturating_mul(depth_bonus);
    
    // Maximum exposure allowed (safety factor)
    let max_exposure = user_deposit.saturating_mul(3); // 3x leverage max
    
    Ok(QuantumEfficiency {
        deposit: user_deposit,
        credits_per_proposal: user_deposit,
        total_available_credit: effective_capital,
        max_simultaneous_positions: num_proposals as u8,
        max_total_exposure: max_exposure,
        efficiency_multiplier: base_multiplier * depth_bonus,
    })
}

/// Quantum efficiency calculation result
#[derive(Debug)]
pub struct QuantumEfficiency {
    /// Original deposit amount
    pub deposit: u64,
    /// Credits available per proposal
    pub credits_per_proposal: u64,
    /// Total credit available across all proposals
    pub total_available_credit: u64,
    /// Maximum simultaneous positions
    pub max_simultaneous_positions: u8,
    /// Maximum total exposure allowed
    pub max_total_exposure: u64,
    /// Efficiency multiplier achieved
    pub efficiency_multiplier: u64,
}

/// Process quantum collapse when verse resolves
pub fn process_quantum_collapse(
    credit: &mut QuantumCredit,
    winning_proposal: u8,
    positions: &[Position],
) -> Result<QuantumCollapseResult, ProgramError> {
    let mut total_pnl = 0i64;
    let mut margin_returned = 0u64;
    
    for position in positions {
        // Convert position_id from [u8; 32] to u128 for comparison
        let position_id_u128 = u128::from_le_bytes(position.position_id[0..16].try_into().unwrap());
        
        if credit.active_positions.contains(&position_id_u128) {
            if position.outcome == winning_proposal {
                // Winner: return margin + profit (double the margin as simple calculation)
                let pnl = position.margin as i64; // Profit equals margin for winner
                total_pnl = total_pnl.saturating_add(pnl);
                margin_returned = margin_returned.saturating_add(position.margin);
            } else {
                // Loser: PnL is negative (lose margin)
                total_pnl = total_pnl.saturating_sub(position.margin as i64);
            }
        }
    }
    
    // Clear all positions after collapse
    credit.active_positions.clear();
    credit.total_exposure = 0;
    
    Ok(QuantumCollapseResult {
        total_pnl,
        margin_returned,
        original_deposit: credit.deposit,
        final_balance: if total_pnl >= 0 {
            credit.deposit.saturating_add(total_pnl as u64)
        } else {
            credit.deposit.saturating_sub(total_pnl.abs() as u64)
        },
    })
}

/// Result of quantum collapse
#[derive(Debug)]
pub struct QuantumCollapseResult {
    /// Total P&L across all positions
    pub total_pnl: i64,
    /// Margin returned from winning positions
    pub margin_returned: u64,
    /// Original deposit amount
    pub original_deposit: u64,
    /// Final balance after collapse
    pub final_balance: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::pubkey::Pubkey;
    
    #[test]
    fn test_quantum_credit_creation() {
        let user = Pubkey::new_unique();
        let credit = QuantumCredit::new(user, 1000, 1, 0);
        
        assert_eq!(credit.deposit, 1000);
        assert_eq!(credit.credits_per_proposal, 1000);
        assert_eq!(credit.total_exposure, 0);
        assert!(credit.active_positions.is_empty());
    }
    
    #[test]
    fn test_quantum_efficiency_calculation() {
        let mut verse = VersePDA {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            verse_id: 1,
            parent_id: None,
            children_root: [0; 32],
            child_count: 5,
            total_descendants: 5,
            status: crate::state::VerseStatus::Active,
            depth: 2,
            last_update_slot: 0,
            total_oi: 0,
            derived_prob: crate::math::U64F64::from_num(0),
            correlation_factor: crate::math::U64F64::from_num(0),
            quantum_state: None,
            markets: Vec::new(),
            bump: 0,
            cross_verse_enabled: false,
        };
        
        let efficiency = calculate_quantum_efficiency(&verse, 1000).unwrap();
        
        // 5 proposals * (1 + 2/10) depth bonus = 5 * 1 = 5x efficiency
        assert_eq!(efficiency.efficiency_multiplier, 5);
        assert_eq!(efficiency.credits_per_proposal, 1000);
        assert_eq!(efficiency.max_simultaneous_positions, 5);
        assert_eq!(efficiency.max_total_exposure, 3000); // 3x leverage max
    }
    
    #[test]
    fn test_credit_usage() {
        let user = Pubkey::new_unique();
        let mut credit = QuantumCredit::new(user, 1000, 1, 0);
        
        // Use credit for position
        assert!(credit.use_credit(1, 500).is_ok());
        assert_eq!(credit.total_exposure, 500);
        assert_eq!(credit.active_positions.len(), 1);
        
        // Try to exceed credit limit
        assert!(credit.use_credit(2, 1500).is_err());
        
        // Use more credit within limit
        assert!(credit.use_credit(2, 800).is_ok());
        assert_eq!(credit.total_exposure, 1300);
        assert_eq!(credit.active_positions.len(), 2);
        
        // Release credit
        credit.release_credit(1, 500);
        assert_eq!(credit.total_exposure, 800);
        assert_eq!(credit.active_positions.len(), 1);
    }
}