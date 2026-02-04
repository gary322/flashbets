use borsh::BorshDeserialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    system_program,
    sysvar::Sysvar,
};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::synthetics::{SyntheticWrapper, SyntheticType, WrapperStatus};

/// Create synthetic wrapper instruction
pub fn process_create_synthetic_wrapper(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    synthetic_id: u128,
    synthetic_type: SyntheticType,
    polymarket_markets: Vec<Pubkey>,
    initial_weights: Option<Vec<U64F64>>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let wrapper_account = next_account_info(account_info_iter)?;
    let creator = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify creator is signer
    if !creator.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify system program
    if *system_program.key != system_program::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Derive PDA for wrapper
    let (wrapper_pda, bump_seed) = Pubkey::find_program_address(
        &[b"synthetic", synthetic_id.to_le_bytes().as_ref()],
        program_id,
    );

    if wrapper_pda != *wrapper_account.key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Validate inputs
    if polymarket_markets.is_empty() {
        return Err(BettingPlatformError::NoMarketsProvided.into());
    }

    if polymarket_markets.len() > crate::synthetics::wrapper::MAX_MARKETS_PER_VERSE {
        return Err(BettingPlatformError::TooManyMarkets.into());
    }

    let weights = if let Some(w) = initial_weights {
        if w.len() != polymarket_markets.len() {
            return Err(BettingPlatformError::WeightMismatch.into());
        }
        
        // Verify weights sum to 1
        let mut sum = U64F64::from_num(0);
        for weight in &w {
            sum = sum.checked_add(*weight)?;
        }
        
        // Allow small rounding error
        let one = U64F64::from_num(1);
        let tolerance = U64F64::from_num(1_000); // 0.001 * 1e6
        let diff = if sum > one { 
            sum.checked_sub(one)? 
        } else { 
            one.checked_sub(sum)? 
        };
        
        if diff > tolerance {
            return Err(ProgramError::InvalidAccountData);
        }
        
        w
    } else {
        // Equal weights if not specified
        let equal_weight = U64F64::from_num(1) / U64F64::from_num(polymarket_markets.len() as u64);
        vec![equal_weight; polymarket_markets.len()]
    };

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Calculate space needed
    let space = SyntheticWrapper::LEN;

    // Get rent
    let rent = Rent::from_account_info(rent_sysvar)?;
    let rent_lamports = rent.minimum_balance(space);

    // Create account
    invoke_signed(
        &system_instruction::create_account(
            creator.key,
            wrapper_account.key,
            rent_lamports,
            space as u64,
            program_id,
        ),
        &[
            creator.clone(),
            wrapper_account.clone(),
            system_program.clone(),
        ],
        &[&[b"synthetic", synthetic_id.to_le_bytes().as_ref(), &[bump_seed]]],
    )?;

    // Initialize wrapper
    let mut wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id,
        synthetic_type,
        polymarket_markets,
        weights,
        derived_probability: U64F64::from_num(500_000), // Default to 50% (0.5 * 1e6)
        total_volume_7d: 0,
        last_update_slot: clock.slot,
        status: WrapperStatus::Active,
        is_verse_level: synthetic_type == SyntheticType::Verse,
        bump: bump_seed,
    };

    // Save market count before packing
    let market_count = wrapper.polymarket_markets.len();
    
    // Pack into account
    SyntheticWrapper::pack(wrapper, &mut wrapper_account.data.borrow_mut())?;

    msg!("Created synthetic wrapper {} with {} markets", 
        synthetic_id, 
        market_count
    );

    Ok(())
}

/// Update synthetic wrapper weights
pub fn process_update_wrapper_weights(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    synthetic_id: u128,
    new_weights: Vec<U64F64>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let wrapper_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify PDA
    let (wrapper_pda, _) = Pubkey::find_program_address(
        &[b"synthetic", synthetic_id.to_le_bytes().as_ref()],
        program_id,
    );

    if wrapper_pda != *wrapper_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Unpack wrapper
    let mut wrapper = SyntheticWrapper::unpack(&wrapper_account.data.borrow())?;

    // Verify initialized
    if !wrapper.is_initialized {
        return Err(ProgramError::UninitializedAccount);
    }

    // Verify synthetic ID matches
    if wrapper.synthetic_id != synthetic_id {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify weights length
    if new_weights.len() != wrapper.polymarket_markets.len() {
        return Err(BettingPlatformError::WeightMismatch.into());
    }

    // Verify weights sum to 1
    let mut sum = U64F64::from_num(0);
    for weight in &new_weights {
        sum = sum.checked_add(*weight)?;
    }

    let one = U64F64::from_num(1);
    let tolerance = U64F64::from_num(1_000); // 0.001 * 1e6
    let diff = if sum > one { 
        sum.checked_sub(one)? 
    } else { 
        one.checked_sub(sum)? 
    };

    if diff > tolerance {
        return Err(ProgramError::InvalidAccountData);
    }

    // Update weights
    wrapper.weights = new_weights;

    // Update timestamp
    let clock = Clock::from_account_info(clock_sysvar)?;
    wrapper.last_update_slot = clock.slot;

    // Pack back
    SyntheticWrapper::pack(wrapper, &mut wrapper_account.data.borrow_mut())?;

    msg!("Updated weights for synthetic wrapper {}", synthetic_id);

    Ok(())
}

/// Halt synthetic wrapper
pub fn process_halt_wrapper(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    synthetic_id: u128,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let wrapper_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;

    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Add authority verification (must be admin)
    // Load global config to check admin authority
    let global_config_account = next_account_info(account_info_iter)?;
    let global_config = crate::state::GlobalConfigPDA::deserialize(&mut &global_config_account.data.borrow()[..])?;
    
    if authority.key != &global_config.update_authority {
        msg!("Authority {} is not update_authority {}", authority.key, global_config.update_authority);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }

    // Verify PDA
    let (wrapper_pda, _) = Pubkey::find_program_address(
        &[b"synthetic", synthetic_id.to_le_bytes().as_ref()],
        program_id,
    );

    if wrapper_pda != *wrapper_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Unpack wrapper
    let mut wrapper = SyntheticWrapper::unpack(&wrapper_account.data.borrow())?;

    // Verify initialized
    if !wrapper.is_initialized {
        return Err(ProgramError::UninitializedAccount);
    }

    // Update status
    wrapper.status = WrapperStatus::Halted;

    // Pack back
    SyntheticWrapper::pack(wrapper, &mut wrapper_account.data.borrow_mut())?;

    msg!("Halted synthetic wrapper {}", synthetic_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Clock;
    use solana_sdk::account::Account;

    #[test]
    fn test_create_wrapper_validation() {
        // Test empty markets
        let result = process_create_synthetic_wrapper(
            &Pubkey::new_unique(),
            &[],
            1,
            SyntheticType::Verse,
            vec![], // Empty markets
            None,
        );
        assert!(result.is_err());

        // Test too many markets
        let many_markets = vec![Pubkey::new_unique(); 100];
        let result = process_create_synthetic_wrapper(
            &Pubkey::new_unique(),
            &[],
            1,
            SyntheticType::Verse,
            many_markets,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_weight_validation() {
        let markets = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        
        // Test mismatched weights
        let bad_weights = vec![U64F64::from_num(1) / U64F64::from_num(2)]; // 0.5 - Only 1 weight for 2 markets
        let result = process_create_synthetic_wrapper(
            &Pubkey::new_unique(),
            &[],
            1,
            SyntheticType::Verse,
            markets.clone(),
            Some(bad_weights),
        );
        assert!(result.is_err());

        // Test weights not summing to 1
        let bad_sum_weights = vec![U64F64::from_num(3) / U64F64::from_num(10), U64F64::from_num(3) / U64F64::from_num(10)]; // 0.3, 0.3
        let result = process_create_synthetic_wrapper(
            &Pubkey::new_unique(),
            &[],
            1,
            SyntheticType::Verse,
            markets,
            Some(bad_sum_weights),
        );
        assert!(result.is_err());
    }
}