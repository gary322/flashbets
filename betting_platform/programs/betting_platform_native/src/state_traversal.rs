//! State traversal operations
//!
//! Implements efficient traversal of verse hierarchy with O(log n) lookups

use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
};

use crate::{
    error::BettingPlatformError,
    math::{U64F64, U128F128},
    state::VersePDA,
};

/// Maximum depth for verse hierarchy
pub const MAX_DEPTH: u8 = 32;

/// State traversal implementation
pub struct StateTraversal;

/// Child information for traversal
#[derive(Debug, Clone)]
pub struct ChildInfo {
    pub verse_id: [u8; 32],
    pub derived_prob: U64F64,
    pub weight: u64,
    pub correlation: U64F64,
}

impl StateTraversal {
    /// Find root verse by traversing up the hierarchy
    /// O(depth) lookups, max 32 steps <1ms on Solana
    pub fn find_root_verse(
        verse: &VersePDA,
        verse_accounts: &[AccountInfo],
    ) -> Result<Pubkey, ProgramError> {
        let mut current_verse = verse.clone();
        let mut depth = 0;

        while depth < MAX_DEPTH {
            match current_verse.parent_id {
                Some(parent_id) => {
                    // Find parent account
                    let parent_account = verse_accounts
                        .iter()
                        .find(|account| {
                            if let Ok(parent) = VersePDA::try_from_slice(&account.data.borrow()) {
                                parent.verse_id == parent_id
                            } else {
                                false
                            }
                        })
                        .ok_or(BettingPlatformError::VerseNotFound)?;

                    current_verse = VersePDA::try_from_slice(&parent_account.data.borrow())?;
                    depth += 1;
                }
                None => {
                    // Found root
                    return verse_accounts
                        .iter()
                        .find(|account| {
                            if let Ok(v) = VersePDA::try_from_slice(&account.data.borrow()) {
                                v.verse_id == current_verse.verse_id
                            } else {
                                false
                            }
                        })
                        .map(|account| *account.key)
                        .ok_or(BettingPlatformError::VerseNotFound.into());
                }
            }
        }

        Err(BettingPlatformError::MaxDepthExceeded.into())
    }

    /// Compute derived probability for a verse based on its children
    /// Formula: Prob_verse = Σ (prob_i * weight_i) / Σ weight_i
    pub fn compute_derived_probability(
        verse: &VersePDA,
        child_accounts: &[AccountInfo],
    ) -> Result<U64F64, ProgramError> {
        let children = Self::load_children(verse, child_accounts)?;

        if children.is_empty() {
            return Ok(U64F64::from_num(0));
        }

        // Use U128F128 for intermediate calculations to avoid overflow
        let mut weighted_sum = U128F128::from_num(0u64);
        let mut total_weight = U128F128::from_num(0u64);

        for child in children {
            let prob = U128F128::from_num(child.derived_prob.to_num() as u128);
            let weight = U128F128::from_num(child.weight as u128);

            weighted_sum = weighted_sum
                .checked_add(prob.checked_mul(weight).ok_or(BettingPlatformError::MathOverflow)?)
                .ok_or(BettingPlatformError::MathOverflow)?;
            
            total_weight = total_weight
                .checked_add(weight)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }

        if total_weight > U128F128::from_num(0u64) {
            let result = weighted_sum
                .checked_div(total_weight)
                .ok_or(BettingPlatformError::DivisionByZero)?;
            
            // Convert back to U64F64
            Ok(result.to_u64f64())
        } else {
            Ok(U64F64::from_num(0))
        }
    }

    /// Calculate correlation factor for tail loss
    /// Formula: corr_factor = Σ (corr_ij * weight_i * weight_j) / Σ weights
    pub fn compute_correlation_factor(
        verse: &VersePDA,
        child_accounts: &[AccountInfo],
    ) -> Result<U64F64, ProgramError> {
        let children = Self::load_children(verse, child_accounts)?;

        if children.len() < 2 {
            return Ok(U64F64::from_num(0));
        }

        let mut correlation_sum = U128F128::from_num(0u64);
        let mut weight_sum = U128F128::from_num(0u64);

        for i in 0..children.len() {
            for j in (i + 1)..children.len() {
                let corr = Self::calculate_pairwise_correlation(&children[i], &children[j])?;
                let weight_product = (children[i].weight as u128)
                    .checked_mul(children[j].weight as u128)
                    .ok_or(BettingPlatformError::MathOverflow)?;

                let corr_weighted = U128F128::from_num(corr.to_num() as u128)
                    .checked_mul(U128F128::from_num(weight_product))
                    .ok_or(BettingPlatformError::MathOverflow)?;

                correlation_sum = correlation_sum
                    .checked_add(corr_weighted)
                    .ok_or(BettingPlatformError::MathOverflow)?;
                
                weight_sum = weight_sum
                    .checked_add(U128F128::from_num(weight_product))
                    .ok_or(BettingPlatformError::MathOverflow)?;
            }
        }

        if weight_sum > U128F128::from_num(0u64) {
            let result = correlation_sum
                .checked_div(weight_sum)
                .ok_or(BettingPlatformError::DivisionByZero)?;
            
            Ok(result.to_u64f64())
        } else {
            Ok(U64F64::from_num(0))
        }
    }

    /// Calculate pairwise correlation between two children
    /// Simplified: use average of stored correlation values
    fn calculate_pairwise_correlation(
        child1: &ChildInfo,
        child2: &ChildInfo,
    ) -> Result<U64F64, ProgramError> {
        // In production, this would calculate Pearson correlation from 7-day price history
        // For now, use simplified average
        let avg = (child1.correlation.to_num() + child2.correlation.to_num()) / 2;
        Ok(U64F64::from_num(avg))
    }

    /// Load child verses
    fn load_children(
        verse: &VersePDA,
        accounts: &[AccountInfo],
    ) -> Result<Vec<ChildInfo>, ProgramError> {
        let mut children = Vec::new();

        for account in accounts {
            if let Ok(child_verse) = VersePDA::try_from_slice(&account.data.borrow()) {
                if child_verse.parent_id == Some(verse.verse_id) {
                    // Convert u128 verse_id to [u8; 32]
                    let mut verse_id_bytes = [0u8; 32];
                    verse_id_bytes[..16].copy_from_slice(&child_verse.verse_id.to_le_bytes());
                    
                    children.push(ChildInfo {
                        verse_id: verse_id_bytes,
                        derived_prob: child_verse.derived_prob,
                        weight: child_verse.total_oi, // Use open interest as weight
                        correlation: child_verse.correlation_factor,
                    });
                }
            }
        }

        Ok(children)
    }

    /// Aggregate open interest up the tree
    pub fn aggregate_open_interest(
        verse: &VersePDA,
        child_accounts: &[AccountInfo],
    ) -> Result<u64, ProgramError> {
        let children = Self::load_children(verse, child_accounts)?;
        
        let mut total_oi = verse.total_oi;
        
        for child in children {
            total_oi = total_oi
                .checked_add(child.weight)
                .ok_or(BettingPlatformError::Overflow)?;
        }
        
        Ok(total_oi)
    }

    /// Update verse with aggregated values
    pub fn update_verse_aggregates(
        verse: &mut VersePDA,
        child_accounts: &[AccountInfo],
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Compute derived probability
        verse.derived_prob = Self::compute_derived_probability(verse, child_accounts)?;
        
        // Compute correlation factor
        verse.correlation_factor = Self::compute_correlation_factor(verse, child_accounts)?;
        
        // Aggregate open interest
        verse.total_oi = Self::aggregate_open_interest(verse, child_accounts)?;
        
        // Update timestamp
        verse.last_update_slot = current_slot;
        
        msg!("Updated verse {} aggregates: prob={}, corr={}, oi={}", 
            verse.verse_id,
            verse.derived_prob.to_num(),
            verse.correlation_factor.to_num(),
            verse.total_oi
        );
        
        Ok(())
    }
}