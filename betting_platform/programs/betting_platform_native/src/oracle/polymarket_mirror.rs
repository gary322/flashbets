//! Polymarket Market Mirroring
//!
//! Mirrors Polymarket markets, probabilities, and resolutions

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    constants::*,
};

/// Polymarket market mirror state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PolymarketMirror {
    /// Polymarket market ID
    pub polymarket_id: [u8; 32],
    
    /// Market title
    pub title: String,
    
    /// Market description
    pub description: String,
    
    /// Outcome labels
    pub outcomes: Vec<String>,
    
    /// Current probabilities (basis points)
    pub probabilities: Vec<u64>,
    
    /// Market resolution
    pub resolution: MarketResolution,
    
    /// Last sync timestamp
    pub last_sync: i64,
    
    /// Mirror status
    pub status: MirrorStatus,
}

/// Market resolution state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum MarketResolution {
    Unresolved,
    Resolved { winning_outcome: u8, resolved_at: i64 },
    Invalid { reason: String },
}

/// Mirror status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MirrorStatus {
    Active,
    Syncing,
    Paused,
    Resolved,
}

/// Sync Polymarket market data
pub fn sync_polymarket_market(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    polymarket_id: [u8; 32],
    title: String,
    description: String,
    outcomes: Vec<String>,
    probabilities: Vec<u64>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let mirror_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Validate probabilities sum to 100%
    let prob_sum: u64 = probabilities.iter().sum();
    if prob_sum != 10000 { // 100% in basis points
        msg!("Invalid probabilities: sum {} != 10000", prob_sum);
        return Err(BettingPlatformError::InvalidProbabilities.into());
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Create or update mirror
    let mut mirror = if mirror_account.data_len() > 0 {
        PolymarketMirror::try_from_slice(&mirror_account.data.borrow())?
    } else {
        PolymarketMirror {
            polymarket_id,
            title: String::new(),
            description: String::new(),
            outcomes: Vec::new(),
            probabilities: Vec::new(),
            resolution: MarketResolution::Unresolved,
            last_sync: 0,
            status: MirrorStatus::Active,
        }
    };
    
    // Update mirror data
    mirror.polymarket_id = polymarket_id;
    mirror.title = title;
    mirror.description = description;
    mirror.outcomes = outcomes;
    mirror.probabilities = probabilities;
    mirror.last_sync = clock.unix_timestamp;
    mirror.status = MirrorStatus::Active;
    
    // Serialize back
    mirror.serialize(&mut &mut mirror_account.data.borrow_mut()[..])?;
    
    msg!("Synced Polymarket market: {:?}", mirror.polymarket_id);
    
    Ok(())
}

/// Sync Polymarket resolution
pub fn sync_polymarket_resolution(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    polymarket_id: [u8; 32],
    winning_outcome: Option<u8>,
    invalid_reason: Option<String>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let mirror_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get mirror state
    let mut mirror = PolymarketMirror::try_from_slice(&mirror_account.data.borrow())?;
    
    // Verify market ID matches
    if mirror.polymarket_id != polymarket_id {
        return Err(BettingPlatformError::MarketMismatch.into());
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Update resolution
    match (winning_outcome, invalid_reason) {
        (Some(outcome), None) => {
            if outcome as usize >= mirror.outcomes.len() {
                return Err(BettingPlatformError::InvalidOutcome.into());
            }
            mirror.resolution = MarketResolution::Resolved {
                winning_outcome: outcome,
                resolved_at: clock.unix_timestamp,
            };
            mirror.status = MirrorStatus::Resolved;
        },
        (None, Some(reason)) => {
            mirror.resolution = MarketResolution::Invalid { reason };
            mirror.status = MirrorStatus::Resolved;
        },
        _ => {
            return Err(BettingPlatformError::InvalidResolution.into());
        }
    }
    
    mirror.last_sync = clock.unix_timestamp;
    
    // Serialize back
    mirror.serialize(&mut &mut mirror_account.data.borrow_mut()[..])?;
    
    msg!("Synced Polymarket resolution: {:?}", mirror.resolution);
    
    Ok(())
}

/// Get mirrored market data
pub fn get_mirrored_market(
    mirror_account: &AccountInfo,
) -> Result<PolymarketMirror, ProgramError> {
    let mirror = PolymarketMirror::try_from_slice(&mirror_account.data.borrow())?;
    
    // Check if mirror is active
    if mirror.status != MirrorStatus::Active && mirror.status != MirrorStatus::Resolved {
        return Err(BettingPlatformError::MirrorNotActive.into());
    }
    
    // Check staleness
    let clock = Clock::get()?;
    let time_since_sync = clock.unix_timestamp - mirror.last_sync;
    if time_since_sync > MAX_ORACLE_STALENESS && mirror.status != MirrorStatus::Resolved {
        return Err(BettingPlatformError::StaleOracle.into());
    }
    
    Ok(mirror)
}

/// Batch sync multiple markets
pub fn batch_sync_markets(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_updates: Vec<MarketUpdate>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Process each market update
    for (i, update) in market_updates.iter().enumerate() {
        let mirror_account = next_account_info(account_info_iter)?;
        
        // Create or update mirror
        let mut mirror = if mirror_account.data_len() > 0 {
            PolymarketMirror::try_from_slice(&mirror_account.data.borrow())?
        } else {
            PolymarketMirror {
                polymarket_id: update.polymarket_id,
                title: String::new(),
                description: String::new(),
                outcomes: Vec::new(),
                probabilities: Vec::new(),
                resolution: MarketResolution::Unresolved,
                last_sync: 0,
                status: MirrorStatus::Active,
            }
        };
        
        // Update probabilities
        mirror.probabilities = update.probabilities.clone();
        mirror.last_sync = clock.unix_timestamp;
        
        // Serialize back
        mirror.serialize(&mut &mut mirror_account.data.borrow_mut()[..])?;
    }
    
    msg!("Batch synced {} markets", market_updates.len());
    
    Ok(())
}

/// Market update struct for batch operations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketUpdate {
    pub polymarket_id: [u8; 32],
    pub probabilities: Vec<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_probability_validation() {
        // Valid probabilities (sum to 100%)
        let valid_probs = vec![6000, 4000]; // 60%, 40%
        let sum: u64 = valid_probs.iter().sum();
        assert_eq!(sum, 10000);
        
        // Invalid probabilities
        let invalid_probs = vec![6000, 3000]; // 60%, 30% = 90%
        let sum: u64 = invalid_probs.iter().sum();
        assert_ne!(sum, 10000);
    }
    
    #[test]
    fn test_resolution_states() {
        // Test resolved state
        let resolution = MarketResolution::Resolved {
            winning_outcome: 0,
            resolved_at: 1234567890,
        };
        assert!(matches!(resolution, MarketResolution::Resolved { .. }));
        
        // Test invalid state
        let resolution = MarketResolution::Invalid {
            reason: "Market cancelled".to_string(),
        };
        assert!(matches!(resolution, MarketResolution::Invalid { .. }));
    }
}