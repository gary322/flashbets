//! Unified liquidation entry point
//!
//! Provides a single entry point for all liquidation types

use solana_program::{
    account_info::{AccountInfo, next_account_info},
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
    events::{Event, EventType},
    liquidation::{
        partial_liquidate::process_partial_liquidate,
        chain_liquidation::{ChainLiquidationProcessor, ChainLiquidationResult},
        queue::{LiquidationQueue, process::process_priority_liquidation},
        calculate_risk_score_with_price,
        halt_mechanism::{process_liquidation_with_halt_check, LiquidationHaltState},
    },
    math::U64F64,
    state::{Position, ChainState, ChainPosition, ProposalPDA},
};

/// Liquidation type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum LiquidationType {
    /// Single position liquidation
    SinglePosition { position_index: u8 },
    
    /// Chain liquidation
    Chain { chain_id: u128 },
    
    /// Batch liquidation from queue
    BatchFromQueue { max_liquidations: u8 },
    
    /// Emergency liquidation
    Emergency { position_pubkey: Pubkey },
}

/// Unified liquidation result
#[derive(Debug)]
pub struct UnifiedLiquidationResult {
    pub liquidation_type: LiquidationType,
    pub total_liquidated: u64,
    pub positions_affected: u32,
    pub keeper_rewards: u64,
    pub success: bool,
}

/// Process unified liquidation instruction
pub fn process_liquidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    liquidation_type: LiquidationType,
) -> ProgramResult {
    msg!("Processing unified liquidation: {:?}", liquidation_type);
    
    // Get halt state account (should be last account for all liquidation types)
    let halt_state_account = accounts.last()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let mut halt_state = LiquidationHaltState::try_from_slice(&halt_state_account.data.borrow())?;
    
    // Process liquidation and track for halt mechanism
    let result = match liquidation_type {
        LiquidationType::SinglePosition { position_index } => {
            // Delegate to partial liquidation
            process_partial_liquidate(program_id, accounts, position_index)
        }
        
        LiquidationType::Chain { chain_id } => {
            // Process chain liquidation
            process_chain_liquidation(program_id, accounts, chain_id)
        }
        
        LiquidationType::BatchFromQueue { max_liquidations } => {
            // Process batch from queue
            process_priority_liquidation(program_id, accounts, max_liquidations)
        }
        
        LiquidationType::Emergency { position_pubkey } => {
            // Process emergency liquidation
            process_emergency_liquidation(program_id, accounts, &position_pubkey)
        }
    };
    
    // If liquidation succeeded, update halt tracking
    if result.is_ok() {
        // For now, use a placeholder liquidation value
        // In production, this would be calculated from the actual liquidation
        let liquidation_value = 10_000_000_000; // $10k placeholder
        
        // Process liquidation for halt mechanism
        process_liquidation_with_halt_check(
            &mut halt_state,
            liquidation_value,
            Clock::get()?.slot,
        )?;
        
        // Save updated halt state
        halt_state.serialize(&mut &mut halt_state_account.data.borrow_mut()[..])?;
    }
    
    result
}

/// Process chain liquidation
fn process_chain_liquidation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    chain_id: u128,
) -> ProgramResult {
    msg!("Processing chain liquidation for chain_id: {}", chain_id);
    
    let account_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_iter)?;
    let chain_state_account = next_account_info(account_iter)?;
    
    // Validate keeper
    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load chain state
    let mut chain_state = ChainState::try_from_slice(&chain_state_account.data.borrow())?;
    
    // Validate chain
    if chain_state.chain_id != chain_id {
        return Err(BettingPlatformError::InvalidChainId.into());
    }
    
    // Collect chain positions from remaining accounts
    let mut chain_positions = Vec::new();
    let mut current_prices = Vec::new();
    
    // Get oracle account for price feeds
    let oracle_account = next_account_info(account_iter)?;
    
    while let Ok(position_account) = next_account_info(account_iter) {
        // Try to deserialize as ChainPosition
        if let Ok(position) = ChainPosition::try_from_slice(&position_account.data.borrow()) {
            let proposal_id = position.proposal_id;
            
            // Fetch actual price from oracle
            let current_price = fetch_price_from_oracle(oracle_account, proposal_id)?;
            
            chain_positions.push(position);
            current_prices.push((proposal_id, current_price));
        }
    }
    
    // Process liquidation
    let result = ChainLiquidationProcessor::liquidate_chain(
        &mut chain_state,
        &mut chain_positions,
        keeper_account,
        &current_prices,
    )?;
    
    // Save updated chain state
    chain_state.serialize(&mut &mut chain_state_account.data.borrow_mut()[..])?;
    
    // Emit unified liquidation event
    UnifiedLiquidationExecuted {
        liquidation_type: LiquidationType::Chain { chain_id },
        keeper: *keeper_account.key,
        total_liquidated: result.total_liquidated,
        positions_affected: result.positions_liquidated,
        keeper_rewards: result.keeper_rewards,
        success: true,
        slot: Clock::get()?.slot,
    }.emit();
    
    msg!(
        "Chain liquidation completed: liquidated={}, positions={}, rewards={}",
        result.total_liquidated,
        result.positions_liquidated,
        result.keeper_rewards
    );
    
    Ok(())
}

/// Process emergency liquidation
fn process_emergency_liquidation(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_pubkey: &Pubkey,
) -> ProgramResult {
    msg!("Processing emergency liquidation for position: {}", position_pubkey);
    
    let account_iter = &mut accounts.iter();
    let keeper_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let emergency_authority = next_account_info(account_iter)?;
    
    // Validate accounts
    if !keeper_account.is_signer || !emergency_authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    if position_account.key != position_pubkey {
        return Err(BettingPlatformError::InvalidPosition.into());
    }
    
    // Load position
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;
    
    // In emergency mode, liquidate entire position
    let liquidated_amount = position.size;
    let keeper_reward = (liquidated_amount as u128 * 10 / 10000) as u64; // 0.1% emergency reward
    
    // Update position
    position.size = 0;
    position.is_closed = true;
    position.partial_liq_accumulator = liquidated_amount;
    
    // Save position
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Emit event
    UnifiedLiquidationExecuted {
        liquidation_type: LiquidationType::Emergency { position_pubkey: *position_pubkey },
        keeper: *keeper_account.key,
        total_liquidated: liquidated_amount,
        positions_affected: 1,
        keeper_rewards: keeper_reward,
        success: true,
        slot: Clock::get()?.slot,
    }.emit();
    
    msg!(
        "Emergency liquidation completed: liquidated={}, reward={}",
        liquidated_amount,
        keeper_reward
    );
    
    Ok(())
}

/// Check if liquidation is needed for a position
pub fn check_liquidation_needed(
    position: &Position,
    current_price: u64,
) -> Result<bool, ProgramError> {
    // Calculate risk score
    let risk_score = calculate_risk_score_with_price(
        position,
        U64F64::from_num(current_price),
    )?;
    
    // Check if above liquidation threshold
    Ok(risk_score >= crate::keeper_liquidation::LIQUIDATION_THRESHOLD)
}

/// Get liquidation type for position
pub fn get_liquidation_type(
    position: &Position,
    is_chain_position: bool,
    is_emergency: bool,
) -> LiquidationType {
    if is_emergency {
        LiquidationType::Emergency {
            position_pubkey: Pubkey::new_from_array(position.position_id),
        }
    } else if is_chain_position {
        // Would need chain_id, simplified here
        LiquidationType::Chain { chain_id: 0 }
    } else {
        LiquidationType::SinglePosition { position_index: 0 }
    }
}

/// Unified liquidation executed event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UnifiedLiquidationExecuted {
    pub liquidation_type: LiquidationType,
    pub keeper: Pubkey,
    pub total_liquidated: u64,
    pub positions_affected: u32,
    pub keeper_rewards: u64,
    pub success: bool,
    pub slot: u64,
}

impl Event for UnifiedLiquidationExecuted {
    fn event_type() -> EventType {
        EventType::LiquidationExecuted
    }
    
    fn emit(&self) {
        msg!("BETTING_PLATFORM_EVENT");
        msg!("TYPE:{:?}", Self::event_type());
        
        if let Ok(data) = self.try_to_vec() {
            msg!("DATA:{}", bs58::encode(&data).into_string());
        }
        
        msg!(
            "UnifiedLiquidation: type={:?}, liquidated={}, positions={}, rewards={}",
            self.liquidation_type,
            self.total_liquidated,
            self.positions_affected,
            self.keeper_rewards
        );
    }
}

/// Fetch price from oracle for a specific proposal
fn fetch_price_from_oracle(
    oracle_account: &AccountInfo,
    proposal_id: u128,
) -> Result<u64, ProgramError> {
    // Deserialize the oracle data (ProposalPDA contains prices)
    use borsh::BorshDeserialize;
    let oracle_data = ProposalPDA::deserialize(&mut &oracle_account.data.borrow()[..])?;
    
    // Validate oracle data
    if oracle_data.state != crate::state::ProposalState::Active {
        return Err(BettingPlatformError::InvalidMarketState.into());
    }
    
    // Find the price for this proposal
    // In a real implementation, you might need to map proposal_id to an outcome index
    // For now, we'll use a simple approach where proposal_id maps to outcome
    let outcome_index = (proposal_id as usize) % oracle_data.prices.len();
    
    let price = oracle_data.prices.get(outcome_index)
        .ok_or(BettingPlatformError::InvalidOutcome)?;
    
    // Validate price is reasonable (not 0 or too high)
    if *price == 0 || *price > 10000 {
        return Err(BettingPlatformError::InvalidPrice.into());
    }
    
    Ok(*price)
}