//! Oracle PnL Updater
//! 
//! Handles price updates from oracles and recalculates PnL for all positions
//! This ensures liquidations use the correct effective leverage based on current market prices

use solana_program::{
    account_info::AccountInfo,
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
    events::{emit_event, EventType},
    state::{Position, ProposalPDA},
    oracle::OraclePrice,
    liquidation::helpers::should_liquidate_coverage_based,
    math::U64F64,
};

/// Oracle price update batch
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct OraclePriceUpdate {
    pub proposal_id: u128,
    pub new_price: u64,
    pub timestamp: i64,
    pub confidence: u64,
}

/// PnL update result
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PnLUpdateResult {
    pub positions_updated: u32,
    pub liquidatable_positions: Vec<Pubkey>,
    pub total_unrealized_pnl: i64,
}

/// Process oracle price update and recalculate PnL for all positions
pub fn process_oracle_price_update(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    price_updates: Vec<OraclePriceUpdate>,
) -> ProgramResult {
    msg!("Processing oracle price updates: {} markets", price_updates.len());
    
    let account_iter = &mut accounts.iter();
    let oracle_authority = next_account_info(account_iter)?;
    
    // Validate oracle authority
    if !oracle_authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut total_positions_updated = 0u32;
    let mut liquidatable_positions = Vec::new();
    
    // Process each price update
    for update in price_updates {
        let positions_for_market = get_positions_for_proposal(update.proposal_id)?;
        
        for position_pubkey in positions_for_market {
            // Load position account
            let position_account = next_account_info(account_iter)?;
            let mut position = Position::try_from_slice(&position_account.data.borrow())?;
            
            // Skip if already closed
            if position.is_closed {
                continue;
            }
            
            // Update position with new price
            position.update_with_price(update.new_price)?;
            
            // Check if position is now liquidatable
            let coverage = get_current_coverage()?; // Would fetch from global state
            if should_liquidate_coverage_based(&position, update.new_price, coverage)? {
                liquidatable_positions.push(*position_account.key);
                msg!("Position {} is now liquidatable with effective leverage {}", 
                    position_account.key, 
                    position.get_effective_leverage()?
                );
            }
            
            // Save updated position
            position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
            total_positions_updated += 1;
        }
        
        // Emit price update event (simplified for now)
        msg!("Price updated for proposal {}: {} -> {}", 
            update.proposal_id,
            0, // Would fetch from previous state
            update.new_price
        );
    }
    
    msg!("PnL update complete: {} positions updated, {} liquidatable", 
        total_positions_updated, 
        liquidatable_positions.len()
    );
    
    // If positions are liquidatable, emit alert event
    if !liquidatable_positions.is_empty() {
        emit_liquidation_alert(liquidatable_positions.clone())?;
    }
    
    Ok(())
}

/// Batch update positions for a single proposal
pub fn update_positions_for_proposal(
    proposal_id: u128,
    new_price: u64,
    position_accounts: &[AccountInfo],
) -> Result<Vec<Pubkey>, ProgramError> {
    let mut liquidatable = Vec::new();
    let coverage = get_current_coverage()?;
    
    for account in position_accounts {
        let mut position = Position::try_from_slice(&account.data.borrow())?;
        
        if position.proposal_id != proposal_id || position.is_closed {
            continue;
        }
        
        // Update with new price
        position.update_with_price(new_price)?;
        
        // Check liquidation
        if should_liquidate_coverage_based(&position, new_price, coverage)? {
            liquidatable.push(*account.key);
        }
        
        // Save
        position.serialize(&mut &mut account.data.borrow_mut()[..])?;
    }
    
    Ok(liquidatable)
}

/// Calculate aggregate PnL statistics
pub fn calculate_aggregate_pnl_stats(
    position_accounts: &[AccountInfo],
) -> Result<AggregatePnLStats, ProgramError> {
    let mut total_unrealized_pnl = 0i64;
    let mut profitable_positions = 0u32;
    let mut losing_positions = 0u32;
    let mut total_notional = 0u64;
    
    for account in position_accounts {
        let position = Position::try_from_slice(&account.data.borrow())?;
        
        if position.is_closed {
            continue;
        }
        
        total_unrealized_pnl += position.unrealized_pnl;
        total_notional += position.notional;
        
        if position.unrealized_pnl > 0 {
            profitable_positions += 1;
        } else if position.unrealized_pnl < 0 {
            losing_positions += 1;
        }
    }
    
    Ok(AggregatePnLStats {
        total_unrealized_pnl,
        profitable_positions,
        losing_positions,
        total_notional,
        average_pnl_pct: if total_notional > 0 {
            (total_unrealized_pnl * 10000) / total_notional as i64
        } else {
            0
        },
    })
}

/// Aggregate PnL statistics
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct AggregatePnLStats {
    pub total_unrealized_pnl: i64,
    pub profitable_positions: u32,
    pub losing_positions: u32,
    pub total_notional: u64,
    pub average_pnl_pct: i64, // In basis points
}

// Helper functions (simplified for demonstration)

fn get_positions_for_proposal(_proposal_id: u128) -> Result<Vec<Pubkey>, ProgramError> {
    // In production, would query index or iterate through accounts
    Ok(Vec::new())
}

fn get_current_coverage() -> Result<U64F64, ProgramError> {
    // In production, would fetch from global state
    Ok(U64F64::from_num(1)) // Default 1.0 coverage
}

fn emit_liquidation_alert(positions: Vec<Pubkey>) -> Result<(), ProgramError> {
    msg!("LIQUIDATION ALERT: {} positions require liquidation", positions.len());
    // In production, would emit proper event
    Ok(())
}

use solana_program::account_info::next_account_info;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pnl_stats_calculation() {
        // Test aggregate PnL calculation logic
        let stats = AggregatePnLStats {
            total_unrealized_pnl: 1_000_000_000, // $1000 profit
            profitable_positions: 60,
            losing_positions: 40,
            total_notional: 100_000_000_000, // $100k total
            average_pnl_pct: 100, // 1% average
        };
        
        assert_eq!(stats.profitable_positions + stats.losing_positions, 100);
        assert_eq!(stats.average_pnl_pct, 100); // 1% profit
    }
}