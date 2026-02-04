//! MMT Early Trader Registry
//! 
//! Manages first 100 traders per season for 2x rewards
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{clock::Clock, Sysvar},
};

use crate::mmt::{
    constants::*,
    state::{EarlyTraderRegistry, MakerAccount, SeasonEmission},
};

/// Initialize early trader registry for a season
pub fn process_initialize_early_trader_registry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    season: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Early trader registry account (PDA, uninitialized)
    // 1. Season emission account (to verify season exists)
    // 2. Authority (signer, payer)
    // 3. System program
    // 4. Rent sysvar
    
    let registry_account = next_account_info(account_info_iter)?;
    let season_emission_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Verify season exists
    let season_emission = SeasonEmission::unpack(&season_emission_account.data.borrow())?;
    if season_emission.season != season {
        msg!("Season mismatch");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify registry PDA
    let (registry_pda, registry_bump) = Pubkey::find_program_address(
        &[EARLY_TRADER_REGISTRY_SEED, &[season]],
        program_id,
    );
    if registry_pda != *registry_account.key {
        msg!("Invalid early trader registry PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create registry account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            registry_account.key,
            rent.minimum_balance(EarlyTraderRegistry::LEN),
            EarlyTraderRegistry::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            registry_account.clone(),
            system_program.clone(),
        ],
        &[&[EARLY_TRADER_REGISTRY_SEED, &[season], &[registry_bump]]],
    )?;
    
    // Initialize registry
    let mut registry = EarlyTraderRegistry::unpack_unchecked(&registry_account.data.borrow())?;
    registry.discriminator = EarlyTraderRegistry::DISCRIMINATOR;
    registry.is_initialized = true;
    registry.season = season;
    registry.count = 0;
    registry.traders = Vec::with_capacity(EarlyTraderRegistry::MAX_TRADERS);
    
    EarlyTraderRegistry::pack(registry, &mut registry_account.data.borrow_mut())?;
    
    msg!("Early trader registry initialized for season {}", season);
    
    Ok(())
}

/// Register a trader as an early trader
pub fn process_register_early_trader(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    season: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Early trader registry account
    // 1. Maker account (may be uninitialized)
    // 2. Trader (signer)
    // 3. System program
    // 4. Clock sysvar
    // 5. Rent sysvar
    
    let registry_account = next_account_info(account_info_iter)?;
    let maker_account = next_account_info(account_info_iter)?;
    let trader = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify trader is signer
    if !trader.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let clock = &Clock::from_account_info(clock_sysvar)?;
    let rent = &solana_program::sysvar::rent::Rent::from_account_info(rent_sysvar)?;
    
    // Load registry
    let mut registry = EarlyTraderRegistry::unpack(&registry_account.data.borrow())?;
    
    // Verify season matches
    if registry.season != season {
        msg!("Season mismatch");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check if still accepting early traders
    if registry.count >= EARLY_TRADER_LIMIT {
        msg!("Early trader limit reached");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check if trader already registered
    if registry.traders.contains(trader.key) {
        msg!("Trader already registered as early trader");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Verify maker account PDA
    let (maker_pda, maker_bump) = Pubkey::find_program_address(
        &[MAKER_ACCOUNT_SEED, trader.key.as_ref()],
        program_id,
    );
    if maker_pda != *maker_account.key {
        msg!("Invalid maker account PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create maker account if needed
    if maker_account.data_len() == 0 {
        invoke_signed(
            &system_instruction::create_account(
                trader.key,
                maker_account.key,
                rent.minimum_balance(MakerAccount::LEN),
                MakerAccount::LEN as u64,
                program_id,
            ),
            &[
                trader.clone(),
                maker_account.clone(),
                system_program.clone(),
            ],
            &[&[MAKER_ACCOUNT_SEED, trader.key.as_ref(), &[maker_bump]]],
        )?;
        
        // Initialize new maker account
        let mut account = MakerAccount::unpack_unchecked(&maker_account.data.borrow())?;
        account.discriminator = MakerAccount::DISCRIMINATOR;
        account.is_initialized = true;
        account.owner = *trader.key;
        account.metrics.total_volume = 0;
        account.metrics.spread_improvements = 0;
        account.metrics.trades_count = 0;
        account.metrics.average_spread_improvement_bp = 0;
        account.metrics.last_trade_slot = 0;
        account.pending_rewards = 0;
        account.total_rewards_claimed = 0;
        account.is_early_trader = true; // Mark as early trader
        
        MakerAccount::pack(account, &mut maker_account.data.borrow_mut())?;
    } else {
        // Update existing maker account
        let mut account = MakerAccount::unpack(&maker_account.data.borrow())?;
        
        // Verify ownership
        if account.owner != *trader.key {
            msg!("Invalid maker account owner");
            return Err(ProgramError::IncorrectProgramId);
        }
        
        account.is_early_trader = true;
        MakerAccount::pack(account, &mut maker_account.data.borrow_mut())?;
    }
    
    // Add trader to registry
    registry.traders.push(*trader.key);
    registry.count += 1;
    
    let trader_count = registry.count;
    EarlyTraderRegistry::pack(registry, &mut registry_account.data.borrow_mut())?;
    
    msg!("Registered early trader #{} for season {}", trader_count, season);
    
    Ok(())
}

/// Check if a trader is registered as an early trader
pub fn is_early_trader(
    registry: &EarlyTraderRegistry,
    trader: &Pubkey,
) -> bool {
    registry.traders.contains(trader)
}

/// Get number of remaining early trader slots
pub fn get_remaining_early_trader_slots(
    registry: &EarlyTraderRegistry,
) -> u32 {
    EARLY_TRADER_LIMIT.saturating_sub(registry.count)
}

/// Get early trader statistics
pub fn get_early_trader_stats(
    registry: &EarlyTraderRegistry,
) -> (u32, u32) {
    (registry.count, EARLY_TRADER_LIMIT)
}

/// Verify early trader eligibility
pub fn verify_early_trader_eligibility(
    registry: &EarlyTraderRegistry,
    trader: &Pubkey,
) -> Result<(), ProgramError> {
    // Check if limit reached
    if registry.count >= EARLY_TRADER_LIMIT {
        msg!("Early trader limit reached");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Check if already registered
    if registry.traders.contains(trader) {
        msg!("Already registered as early trader");
        return Err(ProgramError::InvalidArgument);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_early_trader_limit() {
        let mut registry = EarlyTraderRegistry {
            discriminator: EarlyTraderRegistry::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            count: 0,
            traders: Vec::new(),
        };
        
        // Add traders up to limit
        for i in 0..EARLY_TRADER_LIMIT {
            let trader = Pubkey::new_unique();
            assert!(verify_early_trader_eligibility(&registry, &trader).is_ok());
            registry.traders.push(trader);
            registry.count += 1;
        }
        
        // Should fail when at limit
        let extra_trader = Pubkey::new_unique();
        assert!(verify_early_trader_eligibility(&registry, &extra_trader).is_err());
        
        // Check remaining slots
        assert_eq!(get_remaining_early_trader_slots(&registry), 0);
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = EarlyTraderRegistry {
            discriminator: EarlyTraderRegistry::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            count: 0,
            traders: Vec::new(),
        };
        
        let trader = Pubkey::new_unique();
        
        // First registration should succeed
        assert!(verify_early_trader_eligibility(&registry, &trader).is_ok());
        registry.traders.push(trader);
        registry.count += 1;
        
        // Second registration should fail
        assert!(verify_early_trader_eligibility(&registry, &trader).is_err());
    }

    #[test]
    fn test_early_trader_lookup() {
        let mut registry = EarlyTraderRegistry {
            discriminator: EarlyTraderRegistry::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            count: 0,
            traders: Vec::new(),
        };
        
        let trader1 = Pubkey::new_unique();
        let trader2 = Pubkey::new_unique();
        let trader3 = Pubkey::new_unique();
        
        registry.traders.push(trader1);
        registry.traders.push(trader2);
        registry.count = 2;
        
        assert!(is_early_trader(&registry, &trader1));
        assert!(is_early_trader(&registry, &trader2));
        assert!(!is_early_trader(&registry, &trader3));
    }

    #[test]
    fn test_stats() {
        let registry = EarlyTraderRegistry {
            discriminator: EarlyTraderRegistry::DISCRIMINATOR,
            is_initialized: true,
            season: 1,
            count: 50,
            traders: vec![Pubkey::new_unique(); 50],
        };
        
        let (current, max) = get_early_trader_stats(&registry);
        assert_eq!(current, 50);
        assert_eq!(max, EARLY_TRADER_LIMIT);
        assert_eq!(get_remaining_early_trader_slots(&registry), 50);
    }
}