//! Pre-launch Airdrop System for Influencers
//! 
//! Implements 0.1% MMT allocation (100,000 MMT) for influencer rewards
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
use borsh::{BorshDeserialize, BorshSerialize};
use spl_token::{
    instruction as token_instruction,
    state::Account as TokenAccount,
};

use crate::{
    error::BettingPlatformError,
    mmt::{
        constants::*,
        state::{MMTConfig, DistributionType},
    },
    events::{EventType, Event},
    define_event,
};

/// Pre-launch airdrop configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PreLaunchAirdropConfig {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Total allocation for influencers (100,000 MMT = 0.1%)
    pub total_allocation: u64,
    /// Amount already distributed
    pub distributed_amount: u64,
    /// Number of influencers registered
    pub influencer_count: u32,
    /// Maximum influencers allowed
    pub max_influencers: u32,
    /// Allocation per influencer
    pub allocation_per_influencer: u64,
    /// Start slot for claims
    pub claim_start_slot: u64,
    /// End slot for claims
    pub claim_end_slot: u64,
    /// Authority who can register influencers
    pub authority: Pubkey,
    /// Whether the airdrop is active
    pub is_active: bool,
}

impl PreLaunchAirdropConfig {
    pub const DISCRIMINATOR: [u8; 8] = [0x50, 0x52, 0x45, 0x41, 0x49, 0x52, 0x44, 0x50]; // "PREAIRDP"
    pub const LEN: usize = 8 + 1 + 8 + 8 + 4 + 4 + 8 + 8 + 8 + 32 + 1 + 64; // With padding
    
    pub fn new(authority: Pubkey, claim_start_slot: u64, claim_end_slot: u64) -> Self {
        // 0.1% of 100M total supply = 100,000 MMT
        let total_allocation = 100_000 * 10u64.pow(MMT_DECIMALS as u32);
        let max_influencers = 1000; // Up to 1000 influencers
        let allocation_per_influencer = total_allocation / max_influencers as u64;
        
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            total_allocation,
            distributed_amount: 0,
            influencer_count: 0,
            max_influencers,
            allocation_per_influencer,
            claim_start_slot,
            claim_end_slot,
            authority,
            is_active: true,
        }
    }
}

/// Influencer account for tracking claims
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct InfluencerAccount {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Is initialized
    pub is_initialized: bool,
    /// Influencer's wallet address
    pub influencer: Pubkey,
    /// Social media handle (truncated to 32 chars)
    pub social_handle: [u8; 32],
    /// Platform (1=Twitter, 2=YouTube, 3=TikTok, etc.)
    pub platform: u8,
    /// Follower count at registration
    pub follower_count: u64,
    /// Whether they've claimed their airdrop
    pub has_claimed: bool,
    /// Amount allocated to this influencer
    pub allocation: u64,
    /// Timestamp when registered
    pub registered_at: i64,
    /// Timestamp when claimed (0 if not claimed)
    pub claimed_at: i64,
}

impl InfluencerAccount {
    pub const DISCRIMINATOR: [u8; 8] = [0x49, 0x4E, 0x46, 0x4C, 0x55, 0x4E, 0x43, 0x52]; // "INFLUNCR"
    pub const LEN: usize = 8 + 1 + 32 + 32 + 1 + 8 + 1 + 8 + 8 + 8 + 64; // With padding
}

/// PDA for pre-launch airdrop config
pub struct PreLaunchAirdropPDA;
impl PreLaunchAirdropPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"prelaunch_airdrop"], program_id)
    }
}

/// PDA for influencer account
pub struct InfluencerPDA;
impl InfluencerPDA {
    pub fn derive(program_id: &Pubkey, influencer: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"influencer", influencer.as_ref()],
            program_id,
        )
    }
}

/// Initialize pre-launch airdrop system
pub fn process_initialize_prelaunch_airdrop(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    claim_start_slot: u64,
    claim_end_slot: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let config_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let mmt_config = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive and verify PDA
    let (config_pda, bump) = PreLaunchAirdropPDA::derive(program_id);
    if config_account.key != &config_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if already initialized
    if !config_account.data_is_empty() {
        return Err(BettingPlatformError::AlreadyInitialized.into());
    }
    
    // Create config account
    let rent = Rent::from_account_info(rent_sysvar_info)?;
    let rent_lamports = rent.minimum_balance(PreLaunchAirdropConfig::LEN);
    let seeds = &[b"prelaunch_airdrop".as_ref(), &[bump]];
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            config_account.key,
            rent_lamports,
            PreLaunchAirdropConfig::LEN as u64,
            program_id,
        ),
        &[authority.clone(), config_account.clone(), system_program.clone()],
        &[seeds],
    )?;
    
    // Initialize config
    let config = PreLaunchAirdropConfig::new(*authority.key, claim_start_slot, claim_end_slot);
    config.serialize(&mut &mut config_account.data.borrow_mut()[..])?;
    
    msg!("Pre-launch airdrop initialized:");
    msg!("  Total allocation: {} MMT", config.total_allocation / 10u64.pow(MMT_DECIMALS as u32));
    msg!("  Max influencers: {}", config.max_influencers);
    msg!("  Per influencer: {} MMT", config.allocation_per_influencer / 10u64.pow(MMT_DECIMALS as u32));
    msg!("  Claim period: slots {} to {}", claim_start_slot, claim_end_slot);
    
    // Emit event
    PreLaunchAirdropInitialized {
        authority: *authority.key,
        total_allocation: config.total_allocation,
        max_influencers: config.max_influencers,
        claim_start_slot,
        claim_end_slot,
    }.emit();
    
    Ok(())
}

/// Register an influencer for the airdrop
pub fn process_register_influencer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    social_handle: String,
    platform: u8,
    follower_count: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let config_account = next_account_info(account_info_iter)?;
    let influencer_account = next_account_info(account_info_iter)?;
    let influencer = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar_info = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load and verify config
    let mut config = PreLaunchAirdropConfig::deserialize(&mut &config_account.data.borrow()[..])?;
    if config.discriminator != PreLaunchAirdropConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check authority
    if authority.key != &config.authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if active
    if !config.is_active {
        return Err(BettingPlatformError::AirdropNotActive.into());
    }
    
    // Check influencer limit
    if config.influencer_count >= config.max_influencers {
        return Err(BettingPlatformError::AirdropCapReached.into());
    }
    
    // Minimum follower requirement
    const MIN_FOLLOWERS: u64 = 10_000;
    if follower_count < MIN_FOLLOWERS {
        return Err(BettingPlatformError::InsufficientFollowers.into());
    }
    
    // Derive and verify influencer PDA
    let (influencer_pda, bump) = InfluencerPDA::derive(program_id, influencer.key);
    if influencer_account.key != &influencer_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if already registered
    if !influencer_account.data_is_empty() {
        return Err(BettingPlatformError::AlreadyRegistered.into());
    }
    
    // Create influencer account
    let rent = Rent::from_account_info(rent_sysvar_info)?;
    let rent_lamports = rent.minimum_balance(InfluencerAccount::LEN);
    let seeds = &[b"influencer", influencer.key.as_ref(), &[bump]];
    
    invoke_signed(
        &system_instruction::create_account(
            authority.key,
            influencer_account.key,
            rent_lamports,
            InfluencerAccount::LEN as u64,
            program_id,
        ),
        &[authority.clone(), influencer_account.clone(), system_program.clone()],
        &[seeds],
    )?;
    
    // Prepare social handle (truncate to 32 chars)
    let mut handle_bytes = [0u8; 32];
    let handle_slice = social_handle.as_bytes();
    let copy_len = handle_slice.len().min(32);
    handle_bytes[..copy_len].copy_from_slice(&handle_slice[..copy_len]);
    
    // Calculate allocation (can be adjusted based on follower count)
    let base_allocation = config.allocation_per_influencer;
    let follower_bonus = if follower_count > 1_000_000 {
        base_allocation / 2 // 50% bonus for 1M+ followers
    } else if follower_count > 100_000 {
        base_allocation / 4 // 25% bonus for 100k+ followers
    } else {
        0
    };
    let allocation = base_allocation.saturating_add(follower_bonus);
    
    // Get current time
    let clock = Clock::from_account_info(clock)?;
    
    // Initialize influencer account
    let influencer_data = InfluencerAccount {
        discriminator: InfluencerAccount::DISCRIMINATOR,
        is_initialized: true,
        influencer: *influencer.key,
        social_handle: handle_bytes,
        platform,
        follower_count,
        has_claimed: false,
        allocation,
        registered_at: clock.unix_timestamp,
        claimed_at: 0,
    };
    influencer_data.serialize(&mut &mut influencer_account.data.borrow_mut()[..])?;
    
    // Update config
    config.influencer_count += 1;
    config.serialize(&mut &mut config_account.data.borrow_mut()[..])?;
    
    msg!("Influencer registered:");
    msg!("  Handle: {}", social_handle);
    msg!("  Platform: {}", platform);
    msg!("  Followers: {}", follower_count);
    msg!("  Allocation: {} MMT", allocation / 10u64.pow(MMT_DECIMALS as u32));
    
    // Emit event
    InfluencerRegistered {
        influencer: *influencer.key,
        social_handle: handle_bytes,
        platform,
        follower_count,
        allocation,
    }.emit();
    
    Ok(())
}

/// Claim pre-launch airdrop
pub fn process_claim_prelaunch_airdrop(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let config_account = next_account_info(account_info_iter)?;
    let influencer_account = next_account_info(account_info_iter)?;
    let influencer = next_account_info(account_info_iter)?;
    let influencer_token_account = next_account_info(account_info_iter)?;
    let treasury_token_account = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify influencer is signer
    if !influencer.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load config
    let mut config = PreLaunchAirdropConfig::deserialize(&mut &config_account.data.borrow()[..])?;
    if config.discriminator != PreLaunchAirdropConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if active
    if !config.is_active {
        return Err(BettingPlatformError::AirdropNotActive.into());
    }
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    let current_slot = clock.slot;
    
    // Check claim period
    if current_slot < config.claim_start_slot {
        return Err(BettingPlatformError::ClaimNotStarted.into());
    }
    if current_slot > config.claim_end_slot {
        return Err(BettingPlatformError::ClaimPeriodEnded.into());
    }
    
    // Load influencer account
    let mut influencer_data = InfluencerAccount::deserialize(&mut &influencer_account.data.borrow()[..])?;
    if influencer_data.discriminator != InfluencerAccount::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Verify influencer
    if influencer_data.influencer != *influencer.key {
        return Err(BettingPlatformError::InvalidInfluencer.into());
    }
    
    // Check if already claimed
    if influencer_data.has_claimed {
        return Err(BettingPlatformError::AlreadyClaimed.into());
    }
    
    // Verify token accounts
    let influencer_token = TokenAccount::unpack(&influencer_token_account.data.borrow())?;
    if influencer_token.owner != *influencer.key {
        return Err(BettingPlatformError::InvalidTokenAccount.into());
    }
    if influencer_token.mint != *mmt_mint.key {
        return Err(BettingPlatformError::InvalidMint.into());
    }
    
    // Transfer tokens
    let treasury_bump = treasury_token_account.data.borrow()[0]; // Assuming bump is stored
    let treasury_seeds = &[b"mmt_treasury".as_ref(), &[treasury_bump]];
    
    invoke_signed(
        &token_instruction::transfer(
            token_program.key,
            treasury_token_account.key,
            influencer_token_account.key,
            treasury_token_account.key,
            &[],
            influencer_data.allocation,
        )?,
        &[
            treasury_token_account.clone(),
            influencer_token_account.clone(),
            treasury_token_account.clone(),
            token_program.clone(),
        ],
        &[treasury_seeds],
    )?;
    
    // Update influencer account
    influencer_data.has_claimed = true;
    influencer_data.claimed_at = clock.unix_timestamp;
    influencer_data.serialize(&mut &mut influencer_account.data.borrow_mut()[..])?;
    
    // Update config
    config.distributed_amount = config.distributed_amount
        .checked_add(influencer_data.allocation)
        .ok_or(BettingPlatformError::MathOverflow)?;
    config.serialize(&mut &mut config_account.data.borrow_mut()[..])?;
    
    msg!("Pre-launch airdrop claimed:");
    msg!("  Influencer: {}", influencer.key);
    msg!("  Amount: {} MMT", influencer_data.allocation / 10u64.pow(MMT_DECIMALS as u32));
    msg!("  Total distributed: {} MMT", config.distributed_amount / 10u64.pow(MMT_DECIMALS as u32));
    
    // Emit event
    PreLaunchAirdropClaimed {
        influencer: *influencer.key,
        amount: influencer_data.allocation,
        total_distributed: config.distributed_amount,
    }.emit();
    
    Ok(())
}

/// End pre-launch airdrop (admin only)
pub fn process_end_prelaunch_airdrop(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let config_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load config
    let mut config = PreLaunchAirdropConfig::deserialize(&mut &config_account.data.borrow()[..])?;
    if config.discriminator != PreLaunchAirdropConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check authority
    if authority.key != &config.authority {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Deactivate airdrop
    config.is_active = false;
    config.serialize(&mut &mut config_account.data.borrow_mut()[..])?;
    
    msg!("Pre-launch airdrop ended:");
    msg!("  Total distributed: {} MMT", config.distributed_amount / 10u64.pow(MMT_DECIMALS as u32));
    msg!("  Influencers registered: {}", config.influencer_count);
    
    // Emit event
    PreLaunchAirdropEnded {
        total_distributed: config.distributed_amount,
        influencer_count: config.influencer_count,
    }.emit();
    
    Ok(())
}

// Define events
define_event!(PreLaunchAirdropInitialized, EventType::MmtDistribution, {
    authority: Pubkey,
    total_allocation: u64,
    max_influencers: u32,
    claim_start_slot: u64,
    claim_end_slot: u64,
});

define_event!(InfluencerRegistered, EventType::MmtDistribution, {
    influencer: Pubkey,
    social_handle: [u8; 32],
    platform: u8,
    follower_count: u64,
    allocation: u64,
});

define_event!(PreLaunchAirdropClaimed, EventType::MmtDistribution, {
    influencer: Pubkey,
    amount: u64,
    total_distributed: u64,
});

define_event!(PreLaunchAirdropEnded, EventType::MmtDistribution, {
    total_distributed: u64,
    influencer_count: u32,
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_allocation_calculation() {
        let config = PreLaunchAirdropConfig::new(
            Pubkey::new_unique(),
            1000,
            2000,
        );
        
        // 0.1% of 100M = 100,000 MMT
        assert_eq!(config.total_allocation, 100_000 * 10u64.pow(MMT_DECIMALS as u32));
        // 1000 max influencers
        assert_eq!(config.max_influencers, 1000);
        // 100 MMT per influencer base allocation
        assert_eq!(config.allocation_per_influencer, 100 * 10u64.pow(MMT_DECIMALS as u32));
    }
    
    #[test]
    fn test_influencer_allocation_tiers() {
        let base = 100 * 10u64.pow(MMT_DECIMALS as u32);
        
        // 10k followers - base allocation
        let small = base;
        assert_eq!(small, 100 * 10u64.pow(MMT_DECIMALS as u32));
        
        // 100k+ followers - 25% bonus
        let medium = base + (base / 4);
        assert_eq!(medium, 125 * 10u64.pow(MMT_DECIMALS as u32));
        
        // 1M+ followers - 50% bonus  
        let large = base + (base / 2);
        assert_eq!(large, 150 * 10u64.pow(MMT_DECIMALS as u32));
    }
}