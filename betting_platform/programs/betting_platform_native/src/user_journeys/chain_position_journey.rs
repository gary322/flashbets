//! Chain Position User Journey
//! 
//! Complete flow for creating and managing chain positions across multiple markets

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{GlobalConfigPDA, ProposalPDA, VersePDA, chain_accounts::{ChainPosition, ChainLeg, discriminators, PositionStatus}},
    amm::calculate_price_impact,
    events::{emit_event, EventType, ChainStepExecuted, ChainCompleted, ChainCreated},
    math::U64F64,
};

/// Chain position journey state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainPositionJourney {
    /// User public key
    pub user: Pubkey,
    
    /// Current step
    pub current_step: ChainStep,
    
    /// Chain configuration
    pub chain_id: Option<[u8; 32]>,
    pub legs: Vec<ChainLegConfig>,
    pub total_stake: u64,
    
    /// Execution tracking
    pub executed_legs: u8,
    pub current_payout: u64,
    pub final_payout: Option<u64>,
    
    /// Timestamps
    pub journey_start: i64,
    pub last_update: i64,
}

/// Chain journey steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ChainStep {
    /// Not started
    NotStarted,
    
    /// Configuring chain
    ConfiguringChain,
    
    /// Chain validated
    ChainValidated,
    
    /// First leg executed
    FirstLegExecuted,
    
    /// Intermediate legs executing
    IntermediateLegsExecuting,
    
    /// Final leg executed
    FinalLegExecuted,
    
    /// Chain completed
    ChainCompleted,
    
    /// Chain failed
    ChainFailed,
}

/// Chain leg configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainLegConfig {
    pub market_id: [u8; 32],
    pub outcome: u8,
    pub allocation_bps: u16, // Basis points of payout to allocate
}

/// Create and execute a chain position
pub fn create_chain_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    legs: Vec<ChainLegConfig>,
    initial_stake: u64,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let chain_position_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Collect proposal accounts for each leg
    let mut proposal_accounts = Vec::new();
    let mut verse_accounts = Vec::new();
    for _ in 0..legs.len() {
        proposal_accounts.push(next_account_info(account_iter)?);
        verse_accounts.push(next_account_info(account_iter)?);
    }
    
    // Verify user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    msg!("Creating chain position with {} legs", legs.len());
    
    // Step 1: Validate chain configuration
    msg!("Step 1: Validating chain configuration");
    
    // Verify leg count
    if legs.len() < 2 || legs.len() > 8 {
        msg!("Invalid leg count: {}. Must be between 2 and 8", legs.len());
        return Err(BettingPlatformError::InvalidChainConfiguration.into());
    }
    
    // Verify allocation totals 100%
    let total_allocation: u16 = legs.iter().map(|l| l.allocation_bps).sum();
    if total_allocation != 10000 {
        msg!("Invalid allocation total: {} bps. Must equal 10000", total_allocation);
        return Err(BettingPlatformError::InvalidChainConfiguration.into());
    }
    
    // Step 2: Validate all markets
    msg!("Step 2: Validating all markets");
    let mut chain_legs = Vec::new();
    
    for (i, leg_config) in legs.iter().enumerate() {
        let proposal = ProposalPDA::try_from_slice(&proposal_accounts[i].data.borrow())?;
        let verse = VersePDA::try_from_slice(&verse_accounts[i].data.borrow())?;
        
        // Verify market is active
        if !proposal.is_active() {
            msg!("Market {} is not active", i);
            return Err(BettingPlatformError::MarketHalted.into());
        }
        
        // Verify outcome is valid
        if leg_config.outcome >= proposal.outcomes {
            msg!("Invalid outcome {} for market {}", leg_config.outcome, i);
            return Err(BettingPlatformError::InvalidOutcome.into());
        }
        
        // Create chain leg
        let chain_leg = ChainLeg {
            proposal_id: u128::from_le_bytes(leg_config.market_id[0..16].try_into().unwrap()),
            outcome: leg_config.outcome,
            size: 0, // Will be calculated based on allocation
            leverage: 10000, // 1x leverage
            allocation_bps: leg_config.allocation_bps,
            executed: false,
            pnl: 0,
        };
        
        chain_legs.push(chain_leg);
    }
    
    // Step 3: Calculate total cost including fees
    msg!("Step 3: Calculating total cost");
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    let base_fee_bps = global_config.fee_base as u64;
    let chain_fee_multiplier = 150; // 1.5x fee for chain positions
    let total_fee_bps = (base_fee_bps * chain_fee_multiplier) / 100;
    let fee_amount = (initial_stake * total_fee_bps) / 10000;
    let total_cost = initial_stake + fee_amount;
    
    msg!("Initial stake: {}", initial_stake);
    msg!("Chain fee: {} ({} bps)", fee_amount, total_fee_bps);
    msg!("Total cost: {}", total_cost);
    
    // Step 4: Transfer funds to vault
    msg!("Step 4: Transferring funds to vault");
    solana_program::program::invoke(
        &solana_program::system_instruction::transfer(
            user_account.key,
            vault_account.key,
            total_cost,
        ),
        &[user_account.clone(), vault_account.clone(), system_program.clone()],
    )?;
    
    // Step 5: Create chain position
    msg!("Step 5: Creating chain position");
    let chain_id = generate_chain_id(user_account.key, Clock::get()?.slot);
    let chain_id_u128 = u128::from_le_bytes(chain_id[0..16].try_into().unwrap());
    let position_id = u128::from_le_bytes(chain_id[16..32].try_into().unwrap());
    
    let chain_position = ChainPosition {
        discriminator: discriminators::CHAIN_POSITION,
        chain_id: chain_id_u128,
        position_id,
        proposal_id: u128::from_le_bytes(legs[0].market_id[0..16].try_into().unwrap()),
        step_index: 0,
        outcome: legs[0].outcome,
        size: initial_stake,
        initial_stake,
        leverage: 10000, // 1x leverage
        entry_price: 0, // Will be set when position is taken
        status: PositionStatus::Open,
        is_long: true,
        realized_pnl: 0,
        created_at: Clock::get()?.unix_timestamp,
        closed_at: None,
        total_payout: 0,
        legs: vec![],
    };
    
    // Save chain position
    chain_position.serialize(&mut &mut chain_position_account.data.borrow_mut()[..])?;
    
    // Step 6: Execute first leg
    msg!("Step 6: Executing first leg");
    let first_leg = &legs[0];
    let first_proposal = ProposalPDA::try_from_slice(&proposal_accounts[0].data.borrow())?;
    
    // Calculate allocation for first leg
    let first_leg_stake = (initial_stake * first_leg.allocation_bps as u64) / 10000;
    
    // Get current price and calculate impact
    let entry_price = first_proposal.prices[first_leg.outcome as usize];
    let price_impact = calculate_price_impact(&proposal_accounts[0].data.borrow(), first_leg.outcome, first_leg_stake, true)?;
    let execution_price = entry_price + price_impact;
    
    msg!("First leg execution:");
    msg!("  Market: {:?}", first_leg.market_id);
    msg!("  Outcome: {}", first_leg.outcome);
    msg!("  Stake: {}", first_leg_stake);
    msg!("  Execution price: {}", execution_price);
    
    // Execute trade on AMM
    let execution_price = crate::amm::execute_trade(
        &mut proposal_accounts[0].data.borrow_mut()[..],
        first_leg.outcome,
        first_leg_stake,
        true, // Long for chain positions
    )?;
    
    // Emit chain position created event
    emit_event(EventType::ChainCreated, &ChainCreated {
        chain_id: chain_position.chain_id,
        user: *user_account.key,
        verse_id: u128::from_le_bytes(first_proposal.verse_id[0..16].try_into().unwrap()),
        initial_deposit: initial_stake,
        steps: legs.len() as u8,
    });
    
    msg!("Chain position created successfully!");
    msg!("Chain ID: {:?}", chain_position.chain_id);
    msg!("First leg executed, waiting for outcome...");
    
    Ok(())
}

/// Process chain position leg resolution
pub fn process_chain_leg_resolution(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    chain_id: u128,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let chain_position_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    // Load chain position
    let mut chain_position = ChainPosition::try_from_slice(&chain_position_account.data.borrow())?;
    
    // Verify chain ID
    if chain_position.chain_id != chain_id {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Verify chain position is open
    if chain_position.status != PositionStatus::Open {
        msg!("Chain position is not active: {:?}", chain_position.status);
        return Ok(());
    }
    
    let current_leg_idx = chain_position.step_index as usize;
    if current_leg_idx >= chain_position.legs.len() {
        msg!("All legs already processed");
        return Ok(());
    }
    
    msg!("Processing chain leg {} resolution", current_leg_idx);
    
    // Load proposal for current leg
    let proposal_account = next_account_info(account_iter)?;
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Check if proposal is resolved
    if proposal.resolution.is_none() {
        msg!("Current leg market not yet resolved");
        return Ok(());
    }
    
    let resolution = proposal.resolution.unwrap();
    let leg = &mut chain_position.legs[current_leg_idx];
    
    // Step 1: Check if leg won
    msg!("Step 1: Checking leg outcome");
    let leg_won = resolution.outcome == leg.outcome;
    
    if !leg_won {
        msg!("Chain leg {} lost. Chain position failed.", current_leg_idx);
        chain_position.status = PositionStatus::Closed;
        // Update closed_at instead of updated_at
        chain_position.closed_at = Some(Clock::get()?.unix_timestamp);
        chain_position.serialize(&mut &mut chain_position_account.data.borrow_mut()[..])?;
        
        // Use ChainStepExecuted to indicate failure
        emit_event(EventType::ChainStepExecuted, &ChainStepExecuted {
            chain_id,
            step_index: current_leg_idx as u8,
            step_type: "FAILED".to_string(),
            position_created: None,
            current_balance: chain_position.total_payout,
        });
        
        return Ok(());
    }
    
    msg!("Chain leg {} won!", current_leg_idx);
    
    // Step 2: Calculate payout for this leg
    let leg_payout = if current_leg_idx == 0 {
        // First leg payout based on initial stake
        (chain_position.initial_stake * 2 * leg.allocation_bps as u64) / 10000
    } else {
        // Subsequent legs based on accumulated payout
        (chain_position.total_payout * 2 * leg.allocation_bps as u64) / 10000
    };
    
    leg.executed = true;
    leg.pnl = leg_payout as i64;
    chain_position.total_payout += leg_payout;
    
    msg!("Leg payout: {}", leg_payout);
    msg!("Total payout so far: {}", chain_position.total_payout);
    
    // Step 3: Check if this was the last leg
    if current_leg_idx == chain_position.legs.len() - 1 {
        msg!("All legs completed! Chain position successful!");
        chain_position.status = PositionStatus::Closed;
        
        // Transfer final payout to user
        msg!("Transferring {} to user", chain_position.total_payout);
        solana_program::program::invoke(
            &solana_program::system_instruction::transfer(
                vault_account.key,
                user_account.key,
                chain_position.total_payout,
            ),
            &[vault_account.clone(), user_account.clone(), system_program.clone()],
        )?;
        
        emit_event(EventType::ChainCompleted, &ChainCompleted {
            chain_id,
            final_balance: chain_position.total_payout,
            total_pnl: (chain_position.total_payout as i64) - (chain_position.initial_stake as i64),
            positions_created: chain_position.step_index as u32,
        });
    } else {
        // Step 4: Execute next leg
        let next_leg_idx = current_leg_idx + 1;
        let next_leg = &chain_position.legs[next_leg_idx];
        let next_proposal_account = next_account_info(account_iter)?;
        let mut next_proposal = ProposalPDA::try_from_slice(&next_proposal_account.data.borrow())?;
        
        msg!("Executing next leg {}", next_leg_idx);
        
        // Calculate stake for next leg
        let next_leg_stake = (chain_position.total_payout * next_leg.allocation_bps as u64) / 10000;
        
        // Execute trade
        let entry_price = next_proposal.prices[next_leg.outcome as usize];
        let price_impact = calculate_price_impact(&next_proposal_account.data.borrow(), next_leg.outcome, next_leg_stake, true)?;
        let execution_price = entry_price + price_impact;
        
        let execution_price = crate::amm::execute_trade(
            &mut next_proposal_account.data.borrow_mut()[..],
            next_leg.outcome,
            next_leg_stake,
            true,
        )?;
        
        // Save updated proposal
        next_proposal.serialize(&mut &mut next_proposal_account.data.borrow_mut()[..])?;
        
        chain_position.step_index = next_leg_idx as u8;
        
        emit_event(EventType::ChainStepExecuted, &ChainStepExecuted {
            chain_id,
            step_index: next_leg_idx as u8,
            step_type: "LEG_EXECUTED".to_string(),
            position_created: Some(chain_position.position_id),
            current_balance: next_leg_stake,
        });
    }
    
    // Save updated chain position
    // No updated_at field, using closed_at for tracking updates
    chain_position.serialize(&mut &mut chain_position_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Get chain position status
pub fn get_chain_position_status(
    chain_position: &ChainPosition,
) -> ChainPositionStatus {
    let executed_legs = chain_position.legs.iter().filter(|l| l.executed).count();
    let total_legs = chain_position.legs.len();
    let current_multiplier = if chain_position.initial_stake > 0 {
        (chain_position.total_payout * 100) / chain_position.initial_stake
    } else {
        0
    };
    
    let potential_max_payout = calculate_max_potential_payout(
        chain_position.initial_stake,
        &chain_position.legs,
    );
    
    ChainPositionStatus {
        chain_id: chain_position.chain_id,
        owner: Pubkey::default(), // Owner not stored in ChainPosition
        status: chain_position.status,
        total_legs: total_legs as u8,
        executed_legs: executed_legs as u8,
        current_leg: chain_position.step_index,
        initial_stake: chain_position.initial_stake,
        current_payout: chain_position.total_payout,
        current_multiplier,
        potential_max_payout,
        created_at: chain_position.created_at,
        updated_at: chain_position.closed_at.unwrap_or(chain_position.created_at),
    }
}

/// Generate unique chain ID
fn generate_chain_id(user: &Pubkey, slot: u64) -> [u8; 32] {
    use solana_program::keccak;
    let mut data = Vec::new();
    data.extend_from_slice(user.as_ref());
    data.extend_from_slice(&slot.to_le_bytes());
    data.extend_from_slice(&Clock::get().unwrap_or_default().unix_timestamp.to_le_bytes());
    keccak::hash(&data).to_bytes()
}

/// Calculate maximum potential payout
fn calculate_max_potential_payout(initial_stake: u64, legs: &[ChainLeg]) -> u64 {
    let mut payout = initial_stake;
    
    for leg in legs {
        // Each winning leg doubles the allocated portion
        let leg_payout = (payout * 2 * leg.allocation_bps as u64) / 10000;
        payout += leg_payout;
    }
    
    payout
}

/// Chain position status
#[derive(Debug)]
pub struct ChainPositionStatus {
    pub chain_id: u128,
    pub owner: Pubkey,
    pub status: PositionStatus,
    pub total_legs: u8,
    pub executed_legs: u8,
    pub current_leg: u8,
    pub initial_stake: u64,
    pub current_payout: u64,
    pub current_multiplier: u64,
    pub potential_max_payout: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_max_payout_calculation() {
        let legs = vec![
            ChainLeg {
                proposal_id: 0u128,
                outcome: 0,
                size: 0,
                leverage: 10000,
                allocation_bps: 5000, // 50%
                executed: false,
                pnl: 0,
            },
            ChainLeg {
                proposal_id: 1u128,
                outcome: 1,
                size: 0,
                leverage: 10000,
                allocation_bps: 5000, // 50%
                executed: false,
                pnl: 0,
            },
        ];
        
        let initial_stake = 1000;
        let max_payout = calculate_max_potential_payout(initial_stake, &legs);
        
        // First leg: 1000 * 2 * 0.5 = 1000, total = 2000
        // Second leg: 2000 * 2 * 0.5 = 2000, total = 4000
        assert_eq!(max_payout, 4000);
    }
    
    #[test]
    fn test_chain_validation() {
        // Test allocation validation
        let legs = vec![
            ChainLegConfig {
                market_id: [0; 32],
                outcome: 0,
                allocation_bps: 6000,
            },
            ChainLegConfig {
                market_id: [1; 32],
                outcome: 1,
                allocation_bps: 3000,
            },
        ];
        
        let total: u16 = legs.iter().map(|l| l.allocation_bps).sum();
        assert_eq!(total, 9000); // Should be 10000 for valid chain
    }
}