//! Price caching for resolution
//!
//! Caches final prices for efficient settlement processing

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
};

/// Price cache discriminator
pub const PRICE_CACHE_DISCRIMINATOR: [u8; 8] = [201, 89, 45, 167, 234, 78, 156, 23];

/// Price cache state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct PriceCache {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Verse ID
    pub verse_id: u128,
    
    /// Market prices at resolution
    pub market_prices: Vec<MarketPrice>,
    
    /// Cache timestamp
    pub cached_at: i64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Is finalized
    pub is_finalized: bool,
    
    /// Total markets
    pub total_markets: u32,
}

impl PriceCache {
    pub const BASE_SIZE: usize = DISCRIMINATOR_SIZE + 16 + 4 + 8 + 8 + 1 + 4;
    
    pub fn space(max_markets: usize) -> usize {
        Self::BASE_SIZE + (max_markets * std::mem::size_of::<MarketPrice>())
    }
    
    pub fn new(verse_id: u128) -> Self {
        Self {
            discriminator: PRICE_CACHE_DISCRIMINATOR,
            verse_id,
            market_prices: Vec::new(),
            cached_at: 0,
            last_update_slot: 0,
            is_finalized: false,
            total_markets: 0,
        }
    }
    
    pub fn add_market_price(&mut self, price: MarketPrice) -> Result<(), ProgramError> {
        // Check if market already exists
        if self.market_prices.iter().any(|p| p.market_id == price.market_id) {
            return Err(BettingPlatformError::DuplicateEntry.into());
        }
        
        self.market_prices.push(price);
        self.total_markets += 1;
        Ok(())
    }
    
    pub fn get_market_price(&self, market_id: u128) -> Option<&MarketPrice> {
        self.market_prices.iter().find(|p| p.market_id == market_id)
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != PRICE_CACHE_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Market price at resolution
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct MarketPrice {
    /// Market ID
    pub market_id: u128,
    
    /// Final outcome
    pub outcome: u8,
    
    /// Outcome prices (basis points)
    pub outcome_prices: Vec<u16>,
    
    /// Total volume
    pub total_volume: u64,
    
    /// Resolution timestamp
    pub resolved_at: i64,
}

pub mod initialize {
    use super::*;
    
    /// Initialize price cache for a verse
    pub fn process_initialize_cache(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        verse_id: u128,
    ) -> ProgramResult {
        msg!("Initializing price cache for verse {}", verse_id);
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let authority = next_account_info(account_info_iter)?;
        let price_cache_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = next_account_info(account_info_iter)?;
        
        // Verify authority is signer
        if !authority.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Derive price cache PDA
        let (cache_pda, bump_seed) = Pubkey::find_program_address(
            &[
                b"price_cache",
                &verse_id.to_le_bytes(),
            ],
            program_id,
        );
        
        // Verify PDA matches
        if cache_pda != *price_cache_account.key {
            msg!("Invalid price cache PDA");
            return Err(ProgramError::InvalidSeeds);
        }
        
        // Check if already initialized
        if price_cache_account.data_len() > 0 {
            msg!("Price cache already initialized");
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        
        // Calculate required space (support up to 100 markets)
        let cache_size = PriceCache::space(100);
        
        // Create account
        let rent_lamports = Rent::from_account_info(rent)?
            .minimum_balance(cache_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority.key,
                price_cache_account.key,
                rent_lamports,
                cache_size as u64,
                program_id,
            ),
            &[
                authority.clone(),
                price_cache_account.clone(),
                system_program.clone(),
            ],
            &[&[b"price_cache", &verse_id.to_le_bytes(), &[bump_seed]]],
        )?;
        
        // Initialize cache
        let cache = PriceCache::new(verse_id);
        
        // Log initialization
        msg!("Price cache initialized:");
        msg!("  Verse ID: {}", verse_id);
        msg!("  Max markets: 100");
        
        // Serialize and save
        cache.serialize(&mut &mut price_cache_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

pub mod update {
    use super::*;
    
    /// Update price cache with market resolution
    pub fn process_update_cache(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        verse_id: u128,
        new_price: u64,
    ) -> ProgramResult {
        msg!("Updating price cache for verse {}", verse_id);
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let oracle = next_account_info(account_info_iter)?;
        let price_cache_account = next_account_info(account_info_iter)?;
        let market_account = next_account_info(account_info_iter)?;
        let resolution_account = next_account_info(account_info_iter)?;
        let clock = next_account_info(account_info_iter)?;
        
        // Verify oracle is signer
        if !oracle.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Load price cache
        let mut cache = PriceCache::try_from_slice(&price_cache_account.data.borrow())?;
        cache.validate()?;
        
        // Verify verse matches
        if cache.verse_id != verse_id {
            return Err(BettingPlatformError::InvalidAccountData.into());
        }
        
        // Verify cache is not finalized
        if cache.is_finalized {
            msg!("Price cache is already finalized");
            return Err(BettingPlatformError::InvalidOperation.into());
        }
        
        // Extract market ID from market account
        let market_data = market_account.data.borrow();
        if market_data.len() < 24 {
            return Err(BettingPlatformError::InvalidAccountData.into());
        }
        let mut market_id_bytes = [0u8; 16];
        market_id_bytes.copy_from_slice(&market_data[8..24]);
        let market_id = u128::from_le_bytes(market_id_bytes);
        
        // Load resolution state to get final outcome
        let resolution_state = crate::state::resolution_accounts::ResolutionState::try_from_slice(
            &resolution_account.data.borrow()
        )?;
        
        let final_outcome = resolution_state.final_outcome
            .ok_or(BettingPlatformError::MarketNotResolved)?;
        
        // Create market price entry
        let market_price = MarketPrice {
            market_id,
            outcome: final_outcome,
            outcome_prices: vec![0, 10000], // Binary market: 0% and 100%
            total_volume: 0, // Would be extracted from market
            resolved_at: Clock::from_account_info(clock)?.unix_timestamp,
        };
        
        // Add to cache
        cache.add_market_price(market_price)?;
        
        // Update cache metadata
        cache.cached_at = Clock::from_account_info(clock)?.unix_timestamp;
        cache.last_update_slot = Clock::from_account_info(clock)?.slot;
        
        // Log update
        msg!("Price cache updated:");
        msg!("  Market ID: {}", market_id);
        msg!("  Final outcome: {}", final_outcome);
        msg!("  Total markets cached: {}", cache.total_markets);
        
        // Serialize and save
        cache.serialize(&mut &mut price_cache_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

/// Finalize price cache after all markets resolved
pub fn process_finalize_cache(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    verse_id: u128,
) -> ProgramResult {
    msg!("Finalizing price cache for verse {}", verse_id);
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let authority = next_account_info(account_info_iter)?;
    let price_cache_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load price cache
    let mut cache = PriceCache::try_from_slice(&price_cache_account.data.borrow())?;
    cache.validate()?;
    
    // Verify verse matches
    if cache.verse_id != verse_id {
        return Err(BettingPlatformError::InvalidAccountData.into());
    }
    
    // Verify not already finalized
    if cache.is_finalized {
        msg!("Price cache already finalized");
        return Err(BettingPlatformError::InvalidOperation.into());
    }
    
    // Finalize cache
    cache.is_finalized = true;
    cache.cached_at = Clock::from_account_info(clock)?.unix_timestamp;
    cache.last_update_slot = Clock::from_account_info(clock)?.slot;
    
    // Log finalization
    msg!("Price cache finalized:");
    msg!("  Verse ID: {}", verse_id);
    msg!("  Total markets: {}", cache.total_markets);
    msg!("  Finalized at: {}", cache.cached_at);
    
    // Serialize and save
    cache.serialize(&mut &mut price_cache_account.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Get cached prices for settlement
pub fn get_cached_prices(
    price_cache_account: &AccountInfo,
    market_ids: &[u128],
) -> Result<Vec<Option<MarketPrice>>, ProgramError> {
    let cache = PriceCache::try_from_slice(&price_cache_account.data.borrow())?;
    cache.validate()?;
    
    let mut prices = Vec::new();
    for market_id in market_ids {
        prices.push(cache.get_market_price(*market_id).cloned());
    }
    
    Ok(prices)
}