//! PDA setup and management for MMT token system
//!
//! Manages all Program Derived Addresses for the MMT ecosystem

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::{
    error::BettingPlatformError,
    mmt::{
        state::*,
        constants::*,
    },
};

/// PDA seeds for MMT system
pub mod seeds {
    pub const MMT_CONFIG: &[u8] = b"mmt_config";
    pub const MMT_MINT: &[u8] = b"mmt_mint";
    pub const MMT_TREASURY: &[u8] = b"mmt_treasury";
    pub const RESERVED_VAULT: &[u8] = b"reserved_vault";
    pub const STAKING_POOL: &[u8] = b"staking_pool";
    pub const STAKE_VAULT: &[u8] = b"stake_vault";
    pub const MAKER_REGISTRY: &[u8] = b"maker_registry";
    pub const EARLY_TRADERS: &[u8] = b"early_traders";
    pub const SEASON: &[u8] = b"season";
    pub const DISTRIBUTION: &[u8] = b"distribution";
    pub const USER_STAKE: &[u8] = b"user_stake";
    pub const MAKER_ACCOUNT: &[u8] = b"maker_account";
}

/// PDA derivation functions
pub struct PDADerivation;

impl PDADerivation {
    /// Derive MMT config PDA
    pub fn derive_mmt_config(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::MMT_CONFIG], program_id)
    }
    
    /// Derive MMT mint PDA
    pub fn derive_mmt_mint(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::MMT_MINT], program_id)
    }
    
    /// Derive treasury PDA
    pub fn derive_treasury(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::MMT_TREASURY], program_id)
    }
    
    /// Derive reserved vault PDA
    pub fn derive_reserved_vault(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::RESERVED_VAULT], program_id)
    }
    
    /// Derive staking pool PDA
    pub fn derive_staking_pool(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::STAKING_POOL], program_id)
    }
    
    /// Derive stake vault PDA
    pub fn derive_stake_vault(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::STAKE_VAULT], program_id)
    }
    
    /// Derive maker registry PDA
    pub fn derive_maker_registry(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::MAKER_REGISTRY], program_id)
    }
    
    /// Derive early traders registry PDA for a season
    pub fn derive_early_traders(program_id: &Pubkey, season: u8) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::EARLY_TRADERS, &[season]],
            program_id
        )
    }
    
    /// Derive season emission PDA
    pub fn derive_season_emission(program_id: &Pubkey, season: u8) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::SEASON, &[season]],
            program_id
        )
    }
    
    /// Derive distribution record PDA
    pub fn derive_distribution_record(
        program_id: &Pubkey,
        distribution_id: u64,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::DISTRIBUTION, &distribution_id.to_le_bytes()],
            program_id
        )
    }
    
    /// Derive user stake account PDA
    pub fn derive_user_stake(program_id: &Pubkey, user: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::USER_STAKE, user.as_ref()],
            program_id
        )
    }
    
    /// Derive maker account PDA
    pub fn derive_maker_account(program_id: &Pubkey, maker: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::MAKER_ACCOUNT, maker.as_ref()],
            program_id
        )
    }
}

/// Initialize all core MMT PDAs
pub fn process_initialize_mmt_pdas(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let initializer = next_account_info(account_info_iter)?; // Authority
    let mmt_config = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let treasury = next_account_info(account_info_iter)?;
    let reserved_vault = next_account_info(account_info_iter)?;
    let staking_pool = next_account_info(account_info_iter)?;
    let stake_vault = next_account_info(account_info_iter)?;
    let maker_registry = next_account_info(account_info_iter)?;
    let season_emission = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let rent = next_account_info(account_info_iter)?;
    
    // Verify initializer is signer
    if !initializer.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Verify PDAs
    let (config_pda, config_bump) = PDADerivation::derive_mmt_config(program_id);
    if config_pda != *mmt_config.key {
        msg!("Invalid MMT config PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    let (mint_pda, mint_bump) = PDADerivation::derive_mmt_mint(program_id);
    if mint_pda != *mmt_mint.key {
        msg!("Invalid MMT mint PDA");
        return Err(ProgramError::InvalidSeeds);
    }
    
    // Create MMT config account
    let config_size = MMTConfig::LEN;
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(config_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            mmt_config.key,
            rent_lamports,
            config_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            mmt_config.clone(),
            system_program.clone(),
        ],
        &[&[seeds::MMT_CONFIG, &[config_bump]]],
    )?;
    
    // Initialize MMT config data
    let mut config_data = MMTConfig {
        discriminator: MMTConfig::DISCRIMINATOR,
        is_initialized: true,
        mint: *mmt_mint.key,
        authority: *initializer.key,
        total_supply: TOTAL_SUPPLY,
        circulating_supply: 0,
        season_allocation: SEASON_ALLOCATION,
        current_season: 1,
        season_start_slot: solana_program::clock::Clock::get()?.slot,
        season_emitted: 0,
        locked_supply: RESERVED_ALLOCATION,
        bump: config_bump,
    };
    
    config_data.serialize(&mut &mut mmt_config.data.borrow_mut()[..])?;
    
    // Create mint account
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            mmt_mint.key,
            Rent::from_account_info(rent)?
                .minimum_balance(spl_token::state::Mint::LEN),
            spl_token::state::Mint::LEN as u64,
            &spl_token::id(),
        ),
        &[
            initializer.clone(),
            mmt_mint.clone(),
            system_program.clone(),
        ],
        &[&[seeds::MMT_MINT, &[mint_bump]]],
    )?;
    
    // Initialize mint
    invoke_signed(
        &spl_token::instruction::initialize_mint(
            &spl_token::id(),
            mmt_mint.key,
            mmt_config.key,
            Some(mmt_config.key),
            MMT_DECIMALS,
        )?,
        &[
            mmt_mint.clone(),
            rent.clone(),
            token_program.clone(),
        ],
        &[&[seeds::MMT_MINT, &[mint_bump]]],
    )?;
    
    // Create treasury token account
    let (treasury_pda, treasury_bump) = PDADerivation::derive_treasury(program_id);
    create_token_account(
        initializer,
        treasury,
        mmt_mint.key,
        &treasury_pda,
        system_program,
        token_program,
        rent,
        &[seeds::MMT_TREASURY, &[treasury_bump]],
    )?;
    
    // Create reserved vault token account
    let (reserved_pda, reserved_bump) = PDADerivation::derive_reserved_vault(program_id);
    create_token_account(
        initializer,
        reserved_vault,
        mmt_mint.key,
        &reserved_pda,
        system_program,
        token_program,
        rent,
        &[seeds::RESERVED_VAULT, &[reserved_bump]],
    )?;
    
    // Create staking pool account
    let (pool_pda, pool_bump) = PDADerivation::derive_staking_pool(program_id);
    let pool_size = StakingPool::LEN;
    let pool_rent = Rent::from_account_info(rent)?
        .minimum_balance(pool_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            staking_pool.key,
            pool_rent,
            pool_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            staking_pool.clone(),
            system_program.clone(),
        ],
        &[&[seeds::STAKING_POOL, &[pool_bump]]],
    )?;
    
    // Initialize staking pool data
    let pool_data = StakingPool {
        discriminator: StakingPool::DISCRIMINATOR,
        is_initialized: true,
        total_staked: 0,
        total_stakers: 0,
        reward_per_slot: 0,
        last_update_slot: solana_program::clock::Clock::get()?.slot,
        accumulated_rewards_per_share: 0,
        rebate_percentage_base: 1500, // 15% in basis points
        total_fees_collected: 0,
        total_rebates_distributed: 0,
    };
    
    pool_data.serialize(&mut &mut staking_pool.data.borrow_mut()[..])?;
    
    // Create stake vault token account
    let (vault_pda, vault_bump) = PDADerivation::derive_stake_vault(program_id);
    create_token_account(
        initializer,
        stake_vault,
        mmt_mint.key,
        &vault_pda,
        system_program,
        token_program,
        rent,
        &[seeds::STAKE_VAULT, &[vault_bump]],
    )?;
    
    // Create maker registry
    let registry_size = 32; // Basic registry size
    let registry_rent = Rent::from_account_info(rent)?
        .minimum_balance(registry_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            maker_registry.key,
            registry_rent,
            registry_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            maker_registry.clone(),
            system_program.clone(),
        ],
        &[&[seeds::MAKER_REGISTRY, &[pool_bump]]],
    )?;
    
    // Initialize maker registry with raw bytes
    let registry_discriminator = *b"MAKEREG\0";
    maker_registry.data.borrow_mut()[0..8].copy_from_slice(&registry_discriminator);
    maker_registry.data.borrow_mut()[8..12].copy_from_slice(&0u32.to_le_bytes()); // total_makers
    maker_registry.data.borrow_mut()[12..20].copy_from_slice(&0u64.to_le_bytes()); // total_rewards_distributed
    maker_registry.data.borrow_mut()[20..22].copy_from_slice(&MIN_SPREAD_IMPROVEMENT_BP.to_le_bytes());
    maker_registry.data.borrow_mut()[22] = pool_bump;
    
    // Create first season emission account
    let (season_pda, season_bump) = PDADerivation::derive_season_emission(program_id, 1);
    let season_size = SeasonEmission::LEN;
    let season_rent = Rent::from_account_info(rent)?
        .minimum_balance(season_size);
    
    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            season_emission.key,
            season_rent,
            season_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            season_emission.clone(),
            system_program.clone(),
        ],
        &[&[seeds::SEASON, &[1u8], &[season_bump]]],
    )?;
    
    // Initialize season emission
    let season_data = SeasonEmission {
        discriminator: SeasonEmission::DISCRIMINATOR,
        is_initialized: true,
        season: 1,
        total_allocation: SEASON_ALLOCATION,
        emitted_amount: 0,
        maker_rewards: 0,
        staking_rewards: 0,
        early_trader_bonus: 0,
        start_slot: solana_program::clock::Clock::get()?.slot,
        end_slot: solana_program::clock::Clock::get()?.slot + SEASON_DURATION_SLOTS,
    };
    
    season_data.serialize(&mut &mut season_emission.data.borrow_mut()[..])?;
    
    // Mint total supply to treasury
    invoke_signed(
        &spl_token::instruction::mint_to(
            &spl_token::id(),
            mmt_mint.key,
            treasury.key,
            mmt_config.key,
            &[],
            TOTAL_SUPPLY,
        )?,
        &[
            mmt_mint.clone(),
            treasury.clone(),
            mmt_config.clone(),
            token_program.clone(),
        ],
        &[&[seeds::MMT_CONFIG, &[config_bump]]],
    )?;
    
    // Transfer 90M to reserved vault
    invoke_signed(
        &spl_token::instruction::transfer(
            &spl_token::id(),
            treasury.key,
            reserved_vault.key,
            mmt_config.key,
            &[],
            RESERVED_ALLOCATION,
        )?,
        &[
            treasury.clone(),
            reserved_vault.clone(),
            mmt_config.clone(),
            token_program.clone(),
        ],
        &[&[seeds::MMT_CONFIG, &[config_bump]]],
    )?;
    
    msg!("MMT PDAs initialized successfully");
    msg!("Total supply: {} MMT", TOTAL_SUPPLY / 10u64.pow(MMT_DECIMALS as u32));
    msg!("Reserved: {} MMT", RESERVED_ALLOCATION / 10u64.pow(MMT_DECIMALS as u32));
    msg!("Season 1 allocation: {} MMT", SEASON_ALLOCATION / 10u64.pow(MMT_DECIMALS as u32));
    
    Ok(())
}

/// Create a token account PDA
fn create_token_account<'a>(
    payer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    mint: &Pubkey,
    owner: &Pubkey,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
    seeds: &[&[u8]],
) -> ProgramResult {
    let rent_lamports = Rent::from_account_info(rent)?
        .minimum_balance(spl_token::state::Account::LEN);
    
    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            account.key,
            rent_lamports,
            spl_token::state::Account::LEN as u64,
            &spl_token::id(),
        ),
        &[
            payer.clone(),
            account.clone(),
            system_program.clone(),
        ],
        &[seeds],
    )?;
    
    invoke_signed(
        &spl_token::instruction::initialize_account(
            &spl_token::id(),
            account.key,
            mint,
            owner,
        )?,
        &[
            account.clone(),
            rent.clone(),
            token_program.clone(),
        ],
        &[seeds],
    )?;
    
    Ok(())
}

/// Verify all MMT PDAs are properly initialized
pub fn verify_mmt_pdas(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Result<bool, ProgramError> {
    let account_info_iter = &mut accounts.iter();
    
    let mmt_config = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let treasury = next_account_info(account_info_iter)?;
    let reserved_vault = next_account_info(account_info_iter)?;
    let staking_pool = next_account_info(account_info_iter)?;
    let stake_vault = next_account_info(account_info_iter)?;
    
    // Verify config
    let config_data = MMTConfig::try_from_slice(&mmt_config.data.borrow())?;
    if config_data.discriminator != MMTConfig::DISCRIMINATOR {
        msg!("Invalid MMT config discriminator");
        return Ok(false);
    }
    
    // Verify mint
    let mint = spl_token::state::Mint::unpack(&mmt_mint.data.borrow())?;
    if mint.supply != TOTAL_SUPPLY {
        msg!("Invalid mint supply: {} vs expected {}", mint.supply, TOTAL_SUPPLY);
        return Ok(false);
    }
    
    // Verify treasury
    let treasury_account = spl_token::state::Account::unpack(&treasury.data.borrow())?;
    if treasury_account.mint != *mmt_mint.key {
        msg!("Treasury has wrong mint");
        return Ok(false);
    }
    
    // Verify reserved vault
    let reserved_account = spl_token::state::Account::unpack(&reserved_vault.data.borrow())?;
    if reserved_account.amount != RESERVED_ALLOCATION {
        msg!("Reserved vault has wrong amount: {} vs expected {}", 
            reserved_account.amount, RESERVED_ALLOCATION);
        return Ok(false);
    }
    
    // Verify staking pool
    let pool_data = StakingPool::try_from_slice(&staking_pool.data.borrow())?;
    if pool_data.discriminator != StakingPool::DISCRIMINATOR {
        msg!("Invalid staking pool discriminator");
        return Ok(false);
    }
    
    msg!("All MMT PDAs verified successfully");
    Ok(true)
}

// Helper function
use solana_program::account_info::next_account_info;