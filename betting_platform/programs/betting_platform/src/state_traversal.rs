use anchor_lang::prelude::*;
use crate::account_structs::{VersePDA, U64F64, U128F128};
use crate::errors::ErrorCode;

pub struct StateTraversal;

impl StateTraversal {
    // CLAUDE.md: "O(depth) lookups =32 steps <1ms on Solana"
    pub fn find_root_verse<'info>(
        verse: &Account<'info, VersePDA>,
        verse_accounts: &'info [AccountInfo<'info>],
    ) -> Result<Pubkey> {
        let mut current_key = verse.key();
        let mut depth = 0;

        while depth < 32 {
            // Find the account with matching key
            let current_account = verse_accounts
                .iter()
                .find(|a| a.key() == current_key)
                .ok_or(ErrorCode::VerseNotFound)?;
            
            // Load the verse
            let current_verse = Account::<VersePDA>::try_from(current_account)?;

            match current_verse.parent_id {
                Some(parent_id) => {
                    // Convert parent_id [u8; 32] to Pubkey
                    current_key = Pubkey::new_from_array(parent_id);
                    depth += 1;
                }
                None => return Ok(current_key), // Found root
            }
        }

        Err(ErrorCode::MaxDepthExceeded.into())
    }

    // Aggregate probabilities up the tree
    pub fn compute_derived_probability<'info>(
        verse: &Account<'info, VersePDA>,
        child_accounts: &'info [AccountInfo<'info>],
    ) -> Result<U64F64> {
        let children = Self::load_children(verse, child_accounts)?;

        if children.is_empty() {
            return Ok(0);
        }

        // CLAUDE.md: "Prob_verse = Σ (prob_i * weight_i) / Σ weight_i"
        let mut weighted_sum = 0u128;
        let mut total_weight = 0u128;

        for child in children {
            let prob = child.derived_prob as u128;
            let weight = child.weight as u128;

            weighted_sum = weighted_sum.saturating_add(prob.saturating_mul(weight));
            total_weight = total_weight.saturating_add(weight);
        }

        if total_weight > 0 {
            let result = weighted_sum / total_weight;
            Ok(result as u64)
        } else {
            Ok(0)
        }
    }

    // Calculate correlation factor for tail loss
    pub fn compute_correlation_factor<'info>(
        verse: &Account<'info, VersePDA>,
        child_accounts: &'info [AccountInfo<'info>],
    ) -> Result<U64F64> {
        let children = Self::load_children(verse, child_accounts)?;

        if children.len() < 2 {
            return Ok(0);
        }

        // CLAUDE.md: "corr_factor = Σ (corr_ij * weight_i * weight_j) / Σ weights"
        let mut correlation_sum = 0u128;
        let mut weight_sum = 0u128;

        for i in 0..children.len() {
            for j in (i + 1)..children.len() {
                let corr = Self::calculate_pairwise_correlation(&children[i], &children[j])?;
                let weight_product = (children[i].weight as u128).saturating_mul(children[j].weight as u128);

                correlation_sum = correlation_sum.saturating_add((corr as u128).saturating_mul(weight_product));
                weight_sum = weight_sum.saturating_add(weight_product);
            }
        }

        if weight_sum > 0 {
            let result = correlation_sum / weight_sum;
            Ok(result as u64)
        } else {
            Ok(0)
        }
    }

    fn calculate_pairwise_correlation(
        child1: &ChildInfo,
        child2: &ChildInfo,
    ) -> Result<U64F64> {
        // Pearson correlation from 7-day price history
        // Simplified: use stored correlation values
        let corr = (child1.correlation + child2.correlation) / 2;
        Ok(corr)
    }


    fn load_children<'info>(
        verse: &Account<'info, VersePDA>,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<Vec<ChildInfo>> {
        let mut children = Vec::new();

        for account in accounts {
            if let Ok(child_verse) = Account::<VersePDA>::try_from(account) {
                if child_verse.parent_id == Some(verse.verse_id) {
                    children.push(ChildInfo {
                        verse_id: child_verse.verse_id,
                        derived_prob: child_verse.derived_prob,
                        weight: child_verse.total_oi, // Use OI as weight
                        correlation: child_verse.correlation_factor,
                    });
                }
            }
        }

        Ok(children)
    }
}

struct ChildInfo {
    verse_id: [u8; 32],
    derived_prob: U64F64,
    weight: u64,
    correlation: U64F64,
}