//! MMT Maker Rewards System
//! 
//! Rewards market makers for spread improvements
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{clock::Clock, Sysvar},
};
use spl_token::{
    instruction as token_instruction,
    state::Account as TokenAccount,
};

use crate::mmt::{
    constants::*,
    state::{MakerAccount, MakerMetrics, SeasonEmission, TreasuryAccount, EarlyTraderRegistry},
};
use crate::BettingPlatformError;

/// Initialize a maker account
pub fn process_initialize_maker_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Maker account (PDA, uninitialized)
    // 1. Maker (signer, payer)
    // 2. System program
    // 3. Rent sysvar
    
    let maker_account = next_account_info(account_info_iter)?;
    let maker = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify maker is signer
    if !maker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Verify maker account PDA
    let (maker_pda, maker_bump) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, maker.key.as_ref()],
        program_id,
    );
    if maker_pda != *maker_account.key {
        msg!("Invalid maker account PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create maker account
    invoke_signed(
        &system_instruction::create_account(
            maker.key,
            maker_account.key,
            rent.minimum_balance(MakerAccount::LEN),
            MakerAccount::LEN as u64,
            program_id,
        ),
        &[
            maker.clone(),
            maker_account.clone(),
            system_program.clone(),
        ],
        &[&[MAKER_ACCOUNT_SEED, maker.key.as_ref(), &[maker_bump]]],
    )?;
    
    // Initialize maker account
    let mut account = MakerAccount::unpack_unchecked(&maker_account.data.borrow())?;
    account.discriminator = MakerAccount::DISCRIMINATOR;
    account.is_initialized = true;
    account.owner = *maker.key;
    account.metrics = MakerMetrics {
        total_volume: 0,
        spread_improvements: 0,
        trades_count: 0,
        average_spread_improvement_bp: 0,
        last_trade_slot: 0,
    };
    account.pending_rewards = 0;
    account.total_rewards_claimed = 0;
    account.is_early_trader = false;
    
    MakerAccount::pack(account, &mut maker_account.data.borrow_mut())?;
    
    msg!("Maker account initialized for {}", maker.key);
    
    Ok(())
}

/// Record a maker trade and calculate rewards
pub fn process_record_maker_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    notional: u64,
    spread_improvement_bp: u16,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Maker account (PDA)
    // 1. Season emission account
    // 2. Early trader registry (optional, for checking early trader status)
    // 3. Maker (signer)
    // 4. Clock sysvar
    
    let maker_account_info = next_account_info(account_info_iter)?;
    let season_emission_account = next_account_info(account_info_iter)?;
    let early_trader_registry_account = next_account_info(account_info_iter)?;
    let maker = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify maker is signer
    if !maker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify minimum spread improvement
    if spread_improvement_bp < MIN_SPREAD_IMPROVEMENT_BP {
        msg!("Spread improvement {} bp is below minimum {} bp", 
            spread_improvement_bp, MIN_SPREAD_IMPROVEMENT_BP);
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify minimum trade volume for rewards
    if notional < MIN_TRADE_VOLUME_FOR_REWARDS {
        msg!("Trade volume {} is below minimum {} for rewards", 
            notional, MIN_TRADE_VOLUME_FOR_REWARDS);
        return Err(ProgramError::InvalidArgument);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Load accounts
    let mut maker_account = MakerAccount::unpack(&maker_account_info.data.borrow())?;
    let mut season = SeasonEmission::unpack(&season_emission_account.data.borrow())?;
    
    // Verify ownership
    if maker_account.owner != *maker.key {
        msg!("Invalid maker account owner");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Check if season is active
    if clock.slot < season.start_slot || clock.slot >= season.end_slot {
        msg!("Season is not active");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check anti-wash trading rules
    if maker_account.metrics.last_trade_slot > 0 {
        let slots_since_last_trade = clock.slot
            .saturating_sub(maker_account.metrics.last_trade_slot);
        
        if slots_since_last_trade < MIN_SLOTS_BETWEEN_TRADES {
            msg!("Trade too soon after last trade. Wait {} more slots", 
                MIN_SLOTS_BETWEEN_TRADES - slots_since_last_trade);
            return Err(ProgramError::InvalidArgument);
        }
    }
    
    // Check if maker is an early trader
    if !maker_account.is_early_trader && early_trader_registry_account.data_len() > 0 {
        let registry = EarlyTraderRegistry::unpack(&early_trader_registry_account.data.borrow())?;
        if registry.traders.contains(maker.key) {
            maker_account.is_early_trader = true;
        }
    }
    
    // Update maker metrics
    maker_account.metrics.total_volume = maker_account.metrics.total_volume
        .checked_add(notional)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    maker_account.metrics.spread_improvements = maker_account.metrics.spread_improvements
        .checked_add(spread_improvement_bp as u64)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    maker_account.metrics.trades_count = maker_account.metrics.trades_count
        .checked_add(1)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    // Update average spread improvement
    maker_account.metrics.average_spread_improvement_bp = 
        maker_account.metrics.spread_improvements / maker_account.metrics.trades_count as u64;
    
    maker_account.metrics.last_trade_slot = clock.slot;
    
    // Calculate base reward
    // reward = notional * spread_improvement_bp / 10000
    let base_reward = (notional as u128)
        .checked_mul(spread_improvement_bp as u128)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
        .checked_div(10000)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())? as u64;
    
    // Apply early trader multiplier
    let reward = if maker_account.is_early_trader {
        base_reward
            .checked_mul(EARLY_TRADER_MULTIPLIER as u64)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
    } else {
        base_reward
    };
    
    // Check if reward exceeds remaining season allocation
    let remaining_allocation = season.total_allocation
        .saturating_sub(season.emitted_amount);
    
    let final_reward = reward.min(remaining_allocation);
    
    // Update pending rewards
    maker_account.pending_rewards = maker_account.pending_rewards
        .checked_add(final_reward)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    // Update season emission tracking
    season.maker_rewards = season.maker_rewards
        .checked_add(final_reward)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    season.emitted_amount = season.emitted_amount
        .checked_add(final_reward)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    // Save early trader status for logging
    let is_early_trader = maker_account.is_early_trader;
    
    // Save state
    MakerAccount::pack(maker_account, &mut maker_account_info.data.borrow_mut())?;
    SeasonEmission::pack(season, &mut season_emission_account.data.borrow_mut())?;
    
    msg!("Maker trade recorded: {} notional, {} bp improvement, {} MMT reward{}",
        notional / 10u64.pow(6), // Assuming 6 decimals for display
        spread_improvement_bp,
        final_reward / 10u64.pow(MMT_DECIMALS as u32),
        if is_early_trader { " (2x early trader bonus)" } else { "" }
    );
    
    Ok(())
}

/// Claim accumulated maker rewards
pub fn process_claim_maker_rewards(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Maker account (PDA)
    // 1. Treasury account
    // 2. Treasury token account (source)
    // 3. Maker token account (destination)
    // 4. Maker (signer)
    // 5. Token program
    
    let maker_account_info = next_account_info(account_info_iter)?;
    let treasury_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let maker_token_account = next_account_info(account_info_iter)?;
    let maker = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Verify maker is signer
    if !maker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut maker_account = MakerAccount::unpack(&maker_account_info.data.borrow())?;
    let treasury = TreasuryAccount::unpack(&treasury_account.data.borrow())?;
    
    // Verify ownership
    if maker_account.owner != *maker.key {
        msg!("Invalid maker account owner");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Check pending rewards
    if maker_account.pending_rewards == 0 {
        msg!("No rewards to claim");
        return Err(ProgramError::InvalidArgument);
    }
    
    let rewards_to_claim = maker_account.pending_rewards;
    
    // Get treasury bump for PDA
    let (treasury_pda, treasury_bump) = Pubkey::find_program_address(
        &[MMT_TREASURY_SEED],
        program_id,
    );
    if treasury_pda != *treasury_account.key {
        msg!("Invalid treasury PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Transfer rewards from treasury
    invoke_signed(
        &token_instruction::transfer(
            &spl_token::id(),
            treasury_token_account.key,
            maker_token_account.key,
            treasury_account.key,
            &[],
            rewards_to_claim,
        )?,
        &[
            treasury_token_account.clone(),
            maker_token_account.clone(),
            treasury_account.clone(),
            token_program.clone(),
        ],
        &[&[MMT_TREASURY_SEED, &[treasury_bump]]],
    )?;
    
    // Update maker account
    maker_account.pending_rewards = 0;
    maker_account.total_rewards_claimed = maker_account.total_rewards_claimed
        .checked_add(rewards_to_claim)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    MakerAccount::pack(maker_account, &mut maker_account_info.data.borrow_mut())?;
    
    msg!("Claimed {} MMT maker rewards", 
        rewards_to_claim / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

/// Get maker metrics (view function)
pub fn get_maker_metrics(
    maker_account: &MakerAccount,
) -> MakerMetrics {
    maker_account.metrics.clone()
}

/// Calculate potential reward for a trade (view function)
pub fn calculate_potential_reward(
    notional: u64,
    spread_improvement_bp: u16,
    is_early_trader: bool,
) -> u64 {
    let base_reward = (notional as u128)
        .checked_mul(spread_improvement_bp as u128)
        .unwrap_or(0)
        .checked_div(10000)
        .unwrap_or(0) as u64;
    
    if is_early_trader {
        base_reward.saturating_mul(EARLY_TRADER_MULTIPLIER as u64)
    } else {
        base_reward
    }
}

/// Check if a maker qualifies for rewards
pub fn check_maker_eligibility(
    notional: u64,
    spread_improvement_bp: u16,
    last_trade_slot: u64,
    current_slot: u64,
) -> Result<(), ProgramError> {
    // Check minimum spread improvement
    if spread_improvement_bp < MIN_SPREAD_IMPROVEMENT_BP {
        msg!("Spread improvement too low");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check minimum trade volume
    if notional < MIN_TRADE_VOLUME_FOR_REWARDS {
        msg!("Trade volume too low");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check anti-wash trading
    if last_trade_slot > 0 {
        let slots_since_last = current_slot.saturating_sub(last_trade_slot);
        if slots_since_last < MIN_SLOTS_BETWEEN_TRADES {
            msg!("Trade too soon after last trade");
            return Err(ProgramError::InvalidArgument);
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reward_calculation() {
        // Test base reward calculation
        let notional = 1_000_000_000; // 1000 USDC
        let spread_improvement = 5; // 5 bp
        
        let base_reward = calculate_potential_reward(notional, spread_improvement, false);
        // 1000 * 5 / 10000 = 0.5 USDC worth of rewards
        assert_eq!(base_reward, 500_000); // 0.5 with 6 decimals
        
        // Test early trader bonus
        let early_reward = calculate_potential_reward(notional, spread_improvement, true);
        assert_eq!(early_reward, base_reward * EARLY_TRADER_MULTIPLIER as u64);
    }

    #[test]
    fn test_eligibility_checks() {
        let current_slot = 1000;
        
        // Should pass with valid parameters
        assert!(check_maker_eligibility(
            MIN_TRADE_VOLUME_FOR_REWARDS,
            MIN_SPREAD_IMPROVEMENT_BP,
            0,
            current_slot
        ).is_ok());
        
        // Should fail with low volume
        assert!(check_maker_eligibility(
            MIN_TRADE_VOLUME_FOR_REWARDS - 1,
            MIN_SPREAD_IMPROVEMENT_BP,
            0,
            current_slot
        ).is_err());
        
        // Should fail with low spread improvement
        assert!(check_maker_eligibility(
            MIN_TRADE_VOLUME_FOR_REWARDS,
            0,
            0,
            current_slot
        ).is_err());
        
        // Should fail with wash trading
        assert!(check_maker_eligibility(
            MIN_TRADE_VOLUME_FOR_REWARDS,
            MIN_SPREAD_IMPROVEMENT_BP,
            current_slot - MIN_SLOTS_BETWEEN_TRADES + 1,
            current_slot
        ).is_err());
    }

    #[test]
    fn test_metrics_update() {
        let mut metrics = MakerMetrics {
            total_volume: 0,
            spread_improvements: 0,
            trades_count: 0,
            average_spread_improvement_bp: 0,
            last_trade_slot: 0,
        };
        
        // Simulate adding trades
        for i in 1..=10 {
            metrics.total_volume += 1_000_000_000; // 1000 USDC per trade
            metrics.spread_improvements += 5; // 5 bp per trade
            metrics.trades_count += 1;
            
            metrics.average_spread_improvement_bp = 
                metrics.spread_improvements / metrics.trades_count as u64;
        }
        
        assert_eq!(metrics.total_volume, 10_000_000_000);
        assert_eq!(metrics.trades_count, 10);
        assert_eq!(metrics.average_spread_improvement_bp, 5);
    }
}