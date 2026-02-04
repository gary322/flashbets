//! MMT Token Distribution Management
//! 
//! Manages token emission, season transitions, and treasury operations
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
    state::{MMTConfig, SeasonEmission, TreasuryAccount, DistributionRecord, DistributionType},
};
use crate::BettingPlatformError;

/// Distribute MMT tokens from treasury
pub fn process_distribute_emission(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    distribution_type: DistributionType,
    amount: u64,
    distribution_id: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Season emission account
    // 1. MMT config account
    // 2. Distribution record account (PDA, uninitialized)
    // 3. Treasury account
    // 4. Treasury token account (source)
    // 5. Recipient token account (destination)
    // 6. Authority (signer)
    // 7. System program
    // 8. Token program
    // 9. Clock sysvar
    // 10. Rent sysvar
    
    let season_emission_account = next_account_info(account_info_iter)?;
    let mmt_config_account = next_account_info(account_info_iter)?;
    let distribution_record_account = next_account_info(account_info_iter)?;
    let treasury_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let recipient_token_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Load accounts
    let mut season = SeasonEmission::unpack(&season_emission_account.data.borrow())?;
    let config = MMTConfig::unpack(&mmt_config_account.data.borrow())?;
    let treasury = TreasuryAccount::unpack(&treasury_account.data.borrow())?;
    
    // Verify authority
    if config.authority != *authority.key {
        msg!("Invalid authority");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify season is active
    if clock.slot < season.start_slot || clock.slot >= season.end_slot {
        msg!("Season is not active");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify distribution doesn't exceed season allocation
    let new_emitted = season.emitted_amount
        .checked_add(amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    if new_emitted > season.total_allocation {
        msg!("Distribution would exceed season allocation");
        return Err(ProgramError::InsufficientFunds);
    }
    
    // Verify distribution record PDA
    let (record_pda, record_bump) = Pubkey::find_program_address(
        &[DISTRIBUTION_RECORD_SEED, &distribution_id.to_le_bytes()],
        program_id,
    );
    if record_pda != *distribution_record_account.key {
        msg!("Invalid distribution record PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create distribution record account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            distribution_record_account.key,
            rent.minimum_balance(DistributionRecord::LEN),
            DistributionRecord::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            distribution_record_account.clone(),
            system_program.clone(),
        ],
        &[&[DISTRIBUTION_RECORD_SEED, &distribution_id.to_le_bytes(), &[record_bump]]],
    )?;
    
    // Get treasury bump for PDA
    let (treasury_pda, treasury_bump) = Pubkey::find_program_address(
        &[MMT_TREASURY_SEED],
        program_id,
    );
    if treasury_pda != *treasury_account.key {
        msg!("Invalid treasury PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Transfer tokens from treasury
    invoke_signed(
        &token_instruction::transfer(
            &spl_token::id(),
            treasury_token_account.key,
            recipient_token_account.key,
            treasury_account.key,
            &[],
            amount,
        )?,
        &[
            treasury_token_account.clone(),
            recipient_token_account.clone(),
            treasury_account.clone(),
            token_program.clone(),
        ],
        &[&[MMT_TREASURY_SEED, &[treasury_bump]]],
    )?;
    
    // Update season emission tracking
    season.emitted_amount = new_emitted;
    
    match distribution_type {
        DistributionType::MakerReward => {
            season.maker_rewards = season.maker_rewards
                .checked_add(amount)
                .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        },
        DistributionType::StakingReward => {
            season.staking_rewards = season.staking_rewards
                .checked_add(amount)
                .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        },
        DistributionType::EarlyTraderBonus => {
            season.early_trader_bonus = season.early_trader_bonus
                .checked_add(amount)
                .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        },
        _ => {}
    }
    
    // Save values before packing
    let season_number = season.season;
    let total_allocation = season.total_allocation;
    
    SeasonEmission::pack(season, &mut season_emission_account.data.borrow_mut())?;
    
    // Initialize distribution record
    let recipient_token = TokenAccount::unpack(&recipient_token_account.data.borrow())?;
    let mut record = DistributionRecord::unpack_unchecked(&distribution_record_account.data.borrow())?;
    record.discriminator = DistributionRecord::DISCRIMINATOR;
    record.is_initialized = true;
    record.distribution_type = distribution_type.clone();
    record.recipient = recipient_token.owner;
    record.amount = amount;
    record.slot = clock.slot;
    record.season = season_number;
    record.transaction_signature = [0u8; 64]; // Would be set to actual transaction signature
    
    DistributionRecord::pack(record, &mut distribution_record_account.data.borrow_mut())?;
    
    msg!("Distributed {} MMT, type: {:?}, season {} emission: {}/{}",
        amount / 10u64.pow(MMT_DECIMALS as u32),
        distribution_type,
        season_number,
        new_emitted / 10u64.pow(MMT_DECIMALS as u32),
        total_allocation / 10u64.pow(MMT_DECIMALS as u32)
    );
    
    Ok(())
}

/// Transition to the next season
pub fn process_transition_season(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. MMT config account
    // 1. Current season emission account
    // 2. Next season emission account (PDA, uninitialized)
    // 3. Authority (signer)
    // 4. System program
    // 5. Clock sysvar
    // 6. Rent sysvar
    
    let mmt_config_account = next_account_info(account_info_iter)?;
    let current_season_account = next_account_info(account_info_iter)?;
    let next_season_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Load accounts
    let mut config = MMTConfig::unpack(&mmt_config_account.data.borrow())?;
    let current_season = SeasonEmission::unpack(&current_season_account.data.borrow())?;
    
    // Verify authority
    if config.authority != *authority.key {
        msg!("Invalid authority");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify current season has ended
    if clock.slot < current_season.end_slot {
        msg!("Current season has not ended yet");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Calculate unused allocation to roll over
    let unused = current_season.total_allocation
        .saturating_sub(current_season.emitted_amount);
    
    let next_season_number = config.current_season + 1;
    
    // Verify next season PDA
    let (next_season_pda, next_season_bump) = Pubkey::find_program_address(
        &[SEASON_EMISSION_SEED, &[next_season_number]],
        program_id,
    );
    if next_season_pda != *next_season_account.key {
        msg!("Invalid next season PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create next season account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            next_season_account.key,
            rent.minimum_balance(SeasonEmission::LEN),
            SeasonEmission::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            next_season_account.clone(),
            system_program.clone(),
        ],
        &[&[SEASON_EMISSION_SEED, &[next_season_number], &[next_season_bump]]],
    )?;
    
    // Initialize next season
    let mut next_season = SeasonEmission::unpack_unchecked(&next_season_account.data.borrow())?;
    next_season.discriminator = SeasonEmission::DISCRIMINATOR;
    next_season.is_initialized = true;
    next_season.season = next_season_number;
    next_season.total_allocation = SEASON_ALLOCATION
        .checked_add(unused)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    next_season.emitted_amount = 0;
    next_season.maker_rewards = 0;
    next_season.staking_rewards = 0;
    next_season.early_trader_bonus = 0;
    next_season.start_slot = clock.slot;
    next_season.end_slot = clock.slot + SEASON_DURATION_SLOTS;
    
    // Save values before packing
    let next_season_allocation = next_season.total_allocation;
    let next_season_start_slot = next_season.start_slot;
    
    SeasonEmission::pack(next_season, &mut next_season_account.data.borrow_mut())?;
    
    // Update config
    config.current_season = next_season_number;
    config.season_start_slot = next_season_start_slot;
    config.season_emitted = 0;
    
    MMTConfig::pack(config, &mut mmt_config_account.data.borrow_mut())?;
    
    msg!("Transitioned to season {}, allocation: {} MMT (including {} MMT rollover)",
        next_season_number,
        next_season_allocation / 10u64.pow(MMT_DECIMALS as u32),
        unused / 10u64.pow(MMT_DECIMALS as u32)
    );
    
    Ok(())
}

/// Update treasury balance (called after transfers)
pub fn process_update_treasury_balance(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Treasury account
    // 1. Treasury token account
    // 2. Authority (signer)
    
    let treasury_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Load accounts
    let mut treasury = TreasuryAccount::unpack(&treasury_account.data.borrow())?;
    let token_account = TokenAccount::unpack(&treasury_token_account.data.borrow())?;
    
    // Update balance
    let old_balance = treasury.balance;
    treasury.balance = token_account.amount;
    
    if old_balance > treasury.balance {
        let distributed = old_balance - treasury.balance;
        treasury.total_distributed = treasury.total_distributed
            .checked_add(distributed)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    }
    
    let new_balance = treasury.balance;
    TreasuryAccount::pack(treasury, &mut treasury_account.data.borrow_mut())?;
    
    msg!("Updated treasury balance to {} MMT", 
        new_balance / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

/// Calculate remaining season allocation
pub fn calculate_remaining_allocation(
    season: &SeasonEmission,
) -> u64 {
    season.total_allocation.saturating_sub(season.emitted_amount)
}

/// Calculate emission rate for remaining season
pub fn calculate_emission_rate(
    season: &SeasonEmission,
    current_slot: u64,
) -> Result<u64, ProgramError> {
    if current_slot >= season.end_slot {
        return Ok(0);
    }
    
    let remaining_slots = season.end_slot
        .checked_sub(current_slot)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    let remaining_allocation = calculate_remaining_allocation(season);
    
    if remaining_slots == 0 {
        return Ok(0);
    }
    
    Ok(remaining_allocation / remaining_slots)
}

/// Check if distribution is valid
pub fn validate_distribution(
    season: &SeasonEmission,
    amount: u64,
    current_slot: u64,
) -> Result<(), ProgramError> {
    // Check season is active
    if current_slot < season.start_slot || current_slot >= season.end_slot {
        msg!("Season is not active");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check allocation limit
    let new_emitted = season.emitted_amount
        .checked_add(amount)
        .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
    
    if new_emitted > season.total_allocation {
        msg!("Distribution would exceed season allocation");
        return Err(ProgramError::InsufficientFunds);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remaining_allocation() {
        let season = SeasonEmission {
            discriminator: SeasonEmission::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            total_allocation: SEASON_ALLOCATION,
            emitted_amount: 1_000_000_000_000, // 1M emitted
            maker_rewards: 500_000_000_000,
            staking_rewards: 500_000_000_000,
            early_trader_bonus: 0,
            start_slot: 1000,
            end_slot: 1000 + SEASON_DURATION_SLOTS,
        };
        
        let remaining = calculate_remaining_allocation(&season);
        assert_eq!(remaining, SEASON_ALLOCATION - 1_000_000_000_000);
    }

    #[test]
    fn test_emission_rate() {
        let season = SeasonEmission {
            discriminator: SeasonEmission::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            total_allocation: SEASON_ALLOCATION,
            emitted_amount: 0,
            maker_rewards: 0,
            staking_rewards: 0,
            early_trader_bonus: 0,
            start_slot: 1000,
            end_slot: 1000 + SEASON_DURATION_SLOTS,
        };
        
        // At start of season
        let rate = calculate_emission_rate(&season, 1000).unwrap();
        assert_eq!(rate, SEASON_ALLOCATION / SEASON_DURATION_SLOTS);
        
        // Halfway through season
        let half_slots = SEASON_DURATION_SLOTS / 2;
        let rate_half = calculate_emission_rate(&season, 1000 + half_slots).unwrap();
        assert_eq!(rate_half, SEASON_ALLOCATION / half_slots);
    }

    #[test]
    fn test_distribution_validation() {
        let season = SeasonEmission {
            discriminator: SeasonEmission::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            total_allocation: SEASON_ALLOCATION,
            emitted_amount: SEASON_ALLOCATION - 1_000_000, // Almost fully emitted
            maker_rewards: 0,
            staking_rewards: 0,
            early_trader_bonus: 0,
            start_slot: 1000,
            end_slot: 2000,
        };
        
        // Should succeed with remaining amount
        assert!(validate_distribution(&season, 1_000_000, 1500).is_ok());
        
        // Should fail with too much
        assert!(validate_distribution(&season, 1_000_001, 1500).is_err());
        
        // Should fail outside season
        assert!(validate_distribution(&season, 100, 999).is_err());
        assert!(validate_distribution(&season, 100, 2001).is_err());
    }
}