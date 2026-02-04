//! MMT Token Configuration and Initialization
//! 
//! Core token setup and management functions
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
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use spl_token::{
    instruction as token_instruction,
    state::{Account as TokenAccount, Mint},
};

use crate::mmt::{
    constants::*,
    state::{MMTConfig, SeasonEmission, TreasuryAccount, ReservedVault},
};

/// Initialize the MMT token system
pub fn process_initialize_mmt(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. MMT config account (PDA, uninitialized)
    // 1. MMT mint account (PDA, uninitialized)
    // 2. Season emission account (PDA, uninitialized)
    // 3. Treasury account (PDA, uninitialized)
    // 4. Treasury token account (PDA, uninitialized)
    // 5. Reserved vault account (PDA, uninitialized)
    // 6. Reserved vault token account (PDA, uninitialized)
    // 7. Authority (signer, payer)
    // 8. System program
    // 9. Token program
    // 10. Rent sysvar
    // 11. Clock sysvar
    
    let mmt_config_account = next_account_info(account_info_iter)?;
    let mmt_mint_account = next_account_info(account_info_iter)?;
    let season_emission_account = next_account_info(account_info_iter)?;
    let treasury_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let reserved_vault_account = next_account_info(account_info_iter)?;
    let reserved_vault_token_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let rent = &Rent::from_account_info(rent_sysvar)?;
    let clock = &Clock::from_account_info(clock_sysvar)?;
    
    // Verify PDAs
    let (config_pda, config_bump) = Pubkey::find_program_address(
        &[MMT_CONFIG_SEED],
        program_id,
    );
    if config_pda != *mmt_config_account.key {
        msg!("Invalid MMT config PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    let (mint_pda, mint_bump) = Pubkey::find_program_address(
        &[MMT_MINT_SEED],
        program_id,
    );
    if mint_pda != *mmt_mint_account.key {
        msg!("Invalid MMT mint PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    let (season_pda, _season_bump) = Pubkey::find_program_address(
        &[SEASON_EMISSION_SEED, &[1u8]],
        program_id,
    );
    if season_pda != *season_emission_account.key {
        msg!("Invalid season emission PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    let (treasury_pda, treasury_bump) = Pubkey::find_program_address(
        &[MMT_TREASURY_SEED],
        program_id,
    );
    if treasury_pda != *treasury_account.key {
        msg!("Invalid treasury PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    let (reserved_vault_pda, vault_bump) = Pubkey::find_program_address(
        &[MMT_RESERVED_VAULT_SEED],
        program_id,
    );
    if reserved_vault_pda != *reserved_vault_account.key {
        msg!("Invalid reserved vault PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create MMT config account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            mmt_config_account.key,
            rent.minimum_balance(MMTConfig::LEN),
            MMTConfig::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            mmt_config_account.clone(),
            system_program.clone(),
        ],
        &[&[MMT_CONFIG_SEED, &[config_bump]]],
    )?;
    
    // Create MMT mint account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            mmt_mint_account.key,
            rent.minimum_balance(Mint::LEN),
            Mint::LEN as u64,
            &spl_token::id(),
        ),
        &[
            authority.clone(),
            mmt_mint_account.clone(),
            system_program.clone(),
        ],
        &[&[MMT_MINT_SEED, &[mint_bump]]],
    )?;
    
    // Initialize mint with config PDA as authority
    invoke(
        &token_instruction::initialize_mint(
            &spl_token::id(),
            mmt_mint_account.key,
            mmt_config_account.key,
            Some(mmt_config_account.key),
            MMT_DECIMALS,
        )?,
        &[
            mmt_mint_account.clone(),
            rent_sysvar.clone(),
        ],
    )?;
    
    // Create treasury account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            treasury_account.key,
            rent.minimum_balance(TreasuryAccount::LEN),
            TreasuryAccount::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            treasury_account.clone(),
            system_program.clone(),
        ],
        &[&[MMT_TREASURY_SEED, &[treasury_bump]]],
    )?;
    
    // Create treasury token account
    let treasury_token_pda = get_associated_token_address(
        treasury_account.key,
        mmt_mint_account.key,
    );
    create_associated_token_account(
        authority,
        treasury_account.key,
        mmt_mint_account.key,
        &treasury_token_pda,
        token_program,
        system_program,
        rent_sysvar,
    )?;
    
    // Create reserved vault account
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            reserved_vault_account.key,
            rent.minimum_balance(ReservedVault::LEN),
            ReservedVault::LEN as u64,
            program_id,
        ),
        &[
            authority.clone(),
            reserved_vault_account.clone(),
            system_program.clone(),
        ],
        &[&[MMT_RESERVED_VAULT_SEED, &[vault_bump]]],
    )?;
    
    // Create reserved vault token account
    let vault_token_pda = get_associated_token_address(
        reserved_vault_account.key,
        mmt_mint_account.key,
    );
    create_associated_token_account(
        authority,
        reserved_vault_account.key,
        mmt_mint_account.key,
        &vault_token_pda,
        token_program,
        system_program,
        rent_sysvar,
    )?;
    
    // Create season emission account
    create_season_emission_account(
        program_id,
        season_emission_account,
        authority,
        system_program,
        rent,
        1, // Season 1
    )?;
    
    // Initialize MMT config
    let mut config = MMTConfig::unpack_unchecked(&mmt_config_account.data.borrow())?;
    config.discriminator = MMTConfig::DISCRIMINATOR;
    config.is_initialized = true;
    config.mint = *mmt_mint_account.key;
    config.authority = *authority.key;
    config.total_supply = TOTAL_SUPPLY;
    config.circulating_supply = 0;
    config.season_allocation = SEASON_ALLOCATION;
    config.current_season = 1;
    config.season_start_slot = clock.slot;
    config.season_emitted = 0;
    config.locked_supply = RESERVED_ALLOCATION;
    config.bump = config_bump;
    
    MMTConfig::pack(config, &mut mmt_config_account.data.borrow_mut())?;
    
    // Initialize season emission
    let mut season = SeasonEmission::unpack_unchecked(&season_emission_account.data.borrow())?;
    season.discriminator = SeasonEmission::DISCRIMINATOR;
    season.is_initialized = true;
    season.season = 1;
    season.total_allocation = SEASON_ALLOCATION;
    season.emitted_amount = 0;
    season.maker_rewards = 0;
    season.staking_rewards = 0;
    season.early_trader_bonus = 0;
    season.start_slot = clock.slot;
    season.end_slot = clock.slot + SEASON_DURATION_SLOTS;
    
    SeasonEmission::pack(season, &mut season_emission_account.data.borrow_mut())?;
    
    // Initialize treasury
    let mut treasury = TreasuryAccount::unpack_unchecked(&treasury_account.data.borrow())?;
    treasury.discriminator = TreasuryAccount::DISCRIMINATOR;
    treasury.is_initialized = true;
    treasury.vault = treasury_token_pda;
    treasury.authority = *mmt_config_account.key;
    treasury.balance = 0;
    treasury.total_distributed = 0;
    treasury.bump = treasury_bump;
    
    TreasuryAccount::pack(treasury, &mut treasury_account.data.borrow_mut())?;
    
    // Initialize reserved vault
    let mut vault = ReservedVault::unpack_unchecked(&reserved_vault_account.data.borrow())?;
    vault.discriminator = ReservedVault::DISCRIMINATOR;
    vault.is_initialized = true;
    vault.locked_amount = RESERVED_ALLOCATION;
    vault.authority = *mmt_config_account.key; // Will be changed to system program after locking
    vault.lock_timestamp = clock.unix_timestamp;
    vault.is_permanently_locked = false; // Will be set to true after minting and locking
    vault.bump = vault_bump;
    
    ReservedVault::pack(vault, &mut reserved_vault_account.data.borrow_mut())?;
    
    // Mint total supply to treasury first
    invoke_signed(
        &token_instruction::mint_to(
            &spl_token::id(),
            mmt_mint_account.key,
            treasury_token_account.key,
            mmt_config_account.key,
            &[],
            TOTAL_SUPPLY,
        )?,
        &[
            mmt_mint_account.clone(),
            treasury_token_account.clone(),
            mmt_config_account.clone(),
        ],
        &[&[MMT_CONFIG_SEED, &[config_bump]]],
    )?;
    
    // Transfer 90M to reserved vault
    invoke_signed(
        &token_instruction::transfer(
            &spl_token::id(),
            treasury_token_account.key,
            reserved_vault_token_account.key,
            treasury_account.key,
            &[],
            RESERVED_ALLOCATION,
        )?,
        &[
            treasury_token_account.clone(),
            reserved_vault_token_account.clone(),
            treasury_account.clone(),
        ],
        &[&[MMT_TREASURY_SEED, &[treasury_bump]]],
    )?;
    
    // Update treasury balance
    let mut treasury_data = TreasuryAccount::unpack(&treasury_account.data.borrow())?;
    treasury_data.balance = SEASON_ALLOCATION; // 10M for current season
    TreasuryAccount::pack(treasury_data, &mut treasury_account.data.borrow_mut())?;
    
    msg!("MMT token initialized successfully");
    msg!("Total supply: {} MMT", TOTAL_SUPPLY / 10u64.pow(MMT_DECIMALS as u32));
    msg!("Season 1 allocation: {} MMT", SEASON_ALLOCATION / 10u64.pow(MMT_DECIMALS as u32));
    msg!("Reserved (locked): {} MMT", RESERVED_ALLOCATION / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

/// Lock the reserved vault permanently
pub fn process_lock_reserved_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Accounts expected:
    // 0. Reserved vault account (PDA)
    // 1. Reserved vault token account
    // 2. Authority (must be current authority)
    // 3. System program
    // 4. Token program
    
    let reserved_vault_account = next_account_info(account_info_iter)?;
    let reserved_vault_token_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let _system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    let mut vault = ReservedVault::unpack(&reserved_vault_account.data.borrow())?;
    
    // Verify authority
    if vault.authority != *authority.key {
        msg!("Invalid authority for vault lock");
        return Err(ProgramError::IncorrectProgramId);
    }
    
    // Verify vault is not already locked
    if vault.is_permanently_locked {
        msg!("Vault is already permanently locked");
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Set token account authority to system program (effectively burning control)
    invoke(
        &token_instruction::set_authority(
            &spl_token::id(),
            reserved_vault_token_account.key,
            Some(&solana_program::system_program::id()),
            spl_token::instruction::AuthorityType::AccountOwner,
            &vault.authority,
            &[],
        )?,
        &[
            reserved_vault_token_account.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )?;
    
    // Update vault state
    vault.authority = solana_program::system_program::id();
    vault.is_permanently_locked = true;
    
    ReservedVault::pack(vault, &mut reserved_vault_account.data.borrow_mut())?;
    
    msg!("Reserved vault permanently locked with {} MMT", RESERVED_ALLOCATION / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

/// Helper function to create season emission account
fn create_season_emission_account<'a>(
    program_id: &Pubkey,
    season_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &Rent,
    season_number: u8,
) -> ProgramResult {
    let (season_pda, season_bump) = Pubkey::find_program_address(
        &[SEASON_EMISSION_SEED, &[season_number]],
        program_id,
    );
    
    if season_pda != *season_account.key {
        msg!("Invalid season emission PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            season_account.key,
            rent.minimum_balance(SeasonEmission::LEN),
            SeasonEmission::LEN as u64,
            program_id,
        ),
        &[
            payer.clone(),
            season_account.clone(),
            system_program.clone(),
        ],
        &[&[SEASON_EMISSION_SEED, &[season_number], &[season_bump]]],
    )?;
    
    Ok(())
}

/// Helper function to get associated token address (simplified)
fn get_associated_token_address(
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
) -> Pubkey {
    let token_program_id = spl_token::id();
    let seeds = &[
        wallet_address.as_ref(),
        token_program_id.as_ref(),
        token_mint_address.as_ref(),
    ];
    
    let (pda, _) = Pubkey::find_program_address(seeds, &spl_associated_token_account::id());
    pda
}

/// Helper function to create associated token account
fn create_associated_token_account<'a>(
    payer: &AccountInfo<'a>,
    wallet_address: &Pubkey,
    token_mint_address: &Pubkey,
    associated_token_address: &Pubkey,
    token_program: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &spl_associated_token_account::instruction::create_associated_token_account(
            payer.key,
            wallet_address,
            token_mint_address,
            &spl_token::id(),
        ),
        &[
            payer.clone(),
            token_program.clone(),
            system_program.clone(),
            rent_sysvar.clone(),
        ],
    )?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Clock;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_mmt_config_initialization() {
        let program_id = Pubkey::new_unique();
        let authority = Keypair::new();
        
        // Derive PDAs
        let (config_pda, _) = Pubkey::find_program_address(&[MMT_CONFIG_SEED], &program_id);
        let (mint_pda, _) = Pubkey::find_program_address(&[MMT_MINT_SEED], &program_id);
        
        // Verify addresses are deterministic
        let (config_pda2, _) = Pubkey::find_program_address(&[MMT_CONFIG_SEED], &program_id);
        assert_eq!(config_pda, config_pda2);
        
        let (mint_pda2, _) = Pubkey::find_program_address(&[MMT_MINT_SEED], &program_id);
        assert_eq!(mint_pda, mint_pda2);
    }
}