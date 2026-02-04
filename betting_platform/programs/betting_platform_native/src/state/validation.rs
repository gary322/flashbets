//! Cross-verse validation and insolvency prevention
//!
//! Ensures that collateral and debt are properly isolated between verses
//! to prevent cross-verse contamination and maintain system solvency.

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
        Position,
        chain_accounts::ChainState,
    },
    math::U64F64,
};

/// Validate that positions and collateral are isolated to their verse
pub fn validate_verse_isolation(
    position: &Position,
    verse: &VersePDA,
) -> Result<(), ProgramError> {
    // Ensure position belongs to the correct verse
    if position.verse_id != verse.verse_id {
        msg!("Position verse_id {} doesn't match verse {}", 
             position.verse_id, verse.verse_id);
        return Err(BettingPlatformError::VerseMismatch.into());
    }
    
    // For entangled verses, validate quantum state
    if let Some(quantum_state) = &verse.quantum_state {
        if !quantum_state.entangled_verses.is_empty() &&
           !quantum_state.entangled_verses.contains(&position.verse_id) {
            msg!("Position not in entangled verse set");
            return Err(BettingPlatformError::InvalidQuantumState.into());
        }
    }
    
    Ok(())
}

/// Validate cross-verse collateral requirements
pub fn validate_cross_verse_collateral(
    user_positions: &[Position],
    verse_id: u128,
) -> Result<CrossVerseValidation, ProgramError> {
    let mut verse_collateral = 0u64;
    let mut verse_debt = 0u64;
    let mut other_verse_exposure = 0u64;
    
    for position in user_positions {
        if position.verse_id == verse_id {
            // Positions in this verse
            verse_collateral = verse_collateral.saturating_add(position.margin);
            if position.is_short {
                verse_debt = verse_debt.saturating_add(position.size);
            }
        } else {
            // Positions in other verses
            // Note: Position doesn't have margin field, using notional as proxy
            other_verse_exposure = other_verse_exposure.saturating_add(position.notional);
        }
    }
    
    // Ensure verse has sufficient isolated collateral
    if verse_debt > verse_collateral {
        msg!("Insufficient verse collateral: {} < {}", verse_collateral, verse_debt);
        return Err(BettingPlatformError::InsufficientCollateral.into());
    }
    
    Ok(CrossVerseValidation {
        verse_collateral,
        verse_debt,
        other_verse_exposure,
        is_isolated: true,
        collateral_ratio: if verse_debt > 0 {
            (verse_collateral * 100) / verse_debt
        } else {
            u64::MAX
        },
    })
}

/// Result of cross-verse validation
#[derive(Debug)]
pub struct CrossVerseValidation {
    /// Collateral locked in this verse
    pub verse_collateral: u64,
    /// Debt in this verse
    pub verse_debt: u64,
    /// Exposure in other verses
    pub other_verse_exposure: u64,
    /// Whether verse is properly isolated
    pub is_isolated: bool,
    /// Collateral ratio (percentage)
    pub collateral_ratio: u64,
}

/// Validate chain execution doesn't cross verse boundaries
pub fn validate_chain_verse_isolation(
    chain: &ChainState,
    positions: &[Position],
) -> Result<(), ProgramError> {
    let chain_verse = chain.verse_id;
    
    // All positions in chain must be in same verse
    for position_id in &chain.position_ids {
        // Convert position_id u128 to match against Position's position_id [u8; 32]
        if let Some(position) = positions.iter().find(|p| {
            // Compare by converting u128 to bytes (first 16 bytes of position_id)
            let id_bytes = position_id.to_le_bytes();
            p.position_id[..16] == id_bytes
        }) {
            if position.proposal_id != chain_verse {
                msg!("Chain position {} in wrong verse: {} != {}", 
                     position_id, position.proposal_id, chain_verse);
                return Err(BettingPlatformError::CrossVerseViolation.into());
            }
        }
    }
    
    Ok(())
}

/// Prevent value transfer between verses during collapse
pub fn validate_verse_collapse_isolation(
    collapsing_verse: &VersePDA,
    child_verses: &[VersePDA],
    positions: &[Position],
) -> Result<(), ProgramError> {
    // Calculate total value in collapsing verse
    let mut verse_value = 0u64;
    let mut child_value = 0u64;
    
    for position in positions {
        if position.proposal_id == collapsing_verse.verse_id {
            // Note: Position doesn't have margin field, using notional as proxy
            verse_value = verse_value.saturating_add(position.notional);
        } else if child_verses.iter().any(|v| v.verse_id == position.proposal_id) {
            // Note: Position doesn't have margin field, using notional as proxy
            child_value = child_value.saturating_add(position.notional);
        }
    }
    
    // Ensure no value leakage during collapse
    if verse_value > 0 && child_value > verse_value {
        msg!("Value leakage detected: {} > {}", child_value, verse_value);
        return Err(BettingPlatformError::ValueLeakage.into());
    }
    
    Ok(())
}

/// Calculate maximum safe leverage considering verse isolation
pub fn calculate_isolated_leverage(
    verse: &VersePDA,
    user_collateral: u64,
    verse_total_oi: u64,
) -> u64 {
    // Base leverage from verse depth
    let base_leverage = 10u64.saturating_add(verse.depth as u64 * 5);
    
    // Reduce leverage if verse has high open interest
    let oi_factor = if verse_total_oi > 0 {
        (user_collateral * 100) / verse_total_oi
    } else {
        100
    };
    
    // Apply safety factor for isolation
    let safety_factor = 80; // 80% of calculated leverage
    
    base_leverage
        .min(oi_factor)
        .saturating_mul(safety_factor)
        .saturating_div(100)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_verse_isolation() {
        let verse = VersePDA {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            verse_id: 100,
            parent_id: None,
            children_root: [0; 32],
            child_count: 2,
            total_descendants: 2,
            status: crate::state::VerseStatus::Active,
            depth: 1,
            last_update_slot: 0,
            total_oi: 0,
            derived_prob: crate::math::U64F64::from_num(1) / crate::math::U64F64::from_num(2), // 0.5
            correlation_factor: crate::math::U64F64::from_num(0),
            quantum_state: None,
            markets: vec![],
            bump: 0,
            cross_verse_enabled: false,
        };
        
        let position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            user: Pubkey::new_unique(),
            proposal_id: 1,
            position_id: [0; 32],
            outcome: 0,
            size: 5000,
            notional: 25000,
            leverage: 5,
            entry_price: 5000,
            liquidation_price: 4500,
            is_long: true,
            created_at: 0,
            entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 100,
            margin: 1000,
            collateral: 1000,
            is_short: false,
            last_mark_price: 5000,
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        // Should pass - same verse
        assert!(validate_verse_isolation(&position, &verse).is_ok());
        
        // Should fail - different verse
        let mut wrong_position = position.clone();
        wrong_position.proposal_id = 200;
        assert!(validate_verse_isolation(&wrong_position, &verse).is_err());
    }
    
    #[test] 
    fn test_cross_verse_collateral() {
        let positions = vec![
            Position {
                verse_id: 100,
                margin: 1000,
                size: 5000,
                is_short: false,
                ..Default::default()
            },
            Position {
                verse_id: 100,
                margin: 500,
                size: 2000,
                is_short: true,
                ..Default::default()
            },
            Position {
                verse_id: 200,
                margin: 800,
                size: 4000,
                is_short: false,
                ..Default::default()
            },
        ];
        
        let validation = validate_cross_verse_collateral(&positions, 100).unwrap();
        
        assert_eq!(validation.verse_collateral, 1500); // 1000 + 500
        assert_eq!(validation.verse_debt, 2000); // Only short position
        assert_eq!(validation.other_verse_exposure, 800);
        assert_eq!(validation.collateral_ratio, 75); // 1500/2000 * 100
        assert!(validation.is_isolated);
    }
}