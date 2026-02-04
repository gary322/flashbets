//! Hybrid AMM conversion logic
//!
//! Handles conversion between different AMM types and liquidity migration

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    error::BettingPlatformError,
    events::{Event, AMMTypeConverted},
    state::accounts::AMMType,
    state::amm_accounts::{HybridMarket, MarketState},
};

/// Convert a market from one AMM type to another
pub fn convert_amm_type(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    from_type: AMMType,
    to_type: AMMType,
) -> ProgramResult {
    msg!("Converting AMM type from {:?} to {:?}", from_type, to_type);

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let admin = next_account_info(account_info_iter)?;
    let hybrid_market = next_account_info(account_info_iter)?;
    let source_amm = next_account_info(account_info_iter)?;
    let target_amm = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(admin)?;
    validate_writable(hybrid_market)?;
    validate_writable(source_amm)?;
    validate_writable(target_amm)?;

    // Load hybrid market
    let mut market = HybridMarket::try_from_slice(&hybrid_market.data.borrow())?;

    // Verify market ID
    if market.market_id != market_id {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify current AMM type
    if market.current_amm != from_type {
        return Err(BettingPlatformError::InvalidAMMType.into());
    }

    // Validate conversion is allowed
    validate_amm_conversion(from_type, to_type, &market)?;

    // Pause market during conversion
    market.state = MarketState::Paused;
    market.serialize(&mut &mut hybrid_market.data.borrow_mut()[..])?;

    // Extract state from source AMM
    let conversion_data = extract_amm_state(source_amm, from_type)?;

    // Initialize target AMM with converted state
    initialize_target_amm(target_amm, to_type, &conversion_data, &market)?;

    // Update hybrid market
    market.current_amm = to_type;
    market.state = MarketState::Active;
    market.last_conversion = Clock::get()?.unix_timestamp;
    market.conversion_count += 1;

    // Save updated market
    market.serialize(&mut &mut hybrid_market.data.borrow_mut()[..])?;

    // Emit event
    AMMTypeConverted {
        market_id,
        from_type: from_type as u8,
        to_type: to_type as u8,
        timestamp: Clock::get()?.unix_timestamp,
        liquidity_preserved: conversion_data.total_liquidity,
    }
    .emit();

    msg!("AMM type conversion completed");
    Ok(())
}

/// Migrate liquidity between AMM types
pub fn migrate_liquidity(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market_id: u128,
    amount: u64,
) -> ProgramResult {
    msg!("Migrating liquidity");

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let provider = next_account_info(account_info_iter)?;
    let hybrid_market = next_account_info(account_info_iter)?;
    let source_amm = next_account_info(account_info_iter)?;
    let target_amm = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(provider)?;
    validate_writable(source_amm)?;
    validate_writable(target_amm)?;

    // Load hybrid market
    let market = HybridMarket::try_from_slice(&hybrid_market.data.borrow())?;

    // Check market is active
    if market.state != MarketState::Active {
        return Err(BettingPlatformError::MarketNotActive.into());
    }

    // Verify provider has liquidity in source AMM
    let provider_liquidity = get_provider_liquidity(source_amm, provider.key, market.current_amm)?;
    
    if provider_liquidity < amount {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Calculate conversion ratio
    let conversion_ratio = calculate_conversion_ratio(market.current_amm, market.target_amm)?;
    let target_amount = (amount as u128 * conversion_ratio as u128 / 10000) as u64;

    // Remove liquidity from source
    remove_liquidity_from_amm(source_amm, provider.key, amount, market.current_amm)?;

    // Add liquidity to target
    add_liquidity_to_amm(target_amm, provider.key, target_amount, market.target_amm)?;

    msg!("Migrated {} liquidity", amount);
    Ok(())
}

/// Validate that AMM conversion is allowed
fn validate_amm_conversion(
    from: AMMType,
    to: AMMType,
    market: &HybridMarket,
) -> ProgramResult {
    // Don't allow converting to same type
    if from == to {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Validate based on number of outcomes
    match (from, to) {
        // LMSR -> PM-AMM allowed for multi-outcome markets
        (AMMType::LMSR, AMMType::PMAMM) => {
            if market.num_outcomes < 2 {
                return Err(BettingPlatformError::InvalidConversion.into());
            }
        }
        // PM-AMM -> L2-AMM allowed for continuous markets
        (AMMType::PMAMM, AMMType::L2AMM) => {
            if market.num_outcomes < 10 {
                return Err(BettingPlatformError::InvalidConversion.into());
            }
        }
        // L2-AMM -> LMSR allowed for simplification
        (AMMType::L2AMM, AMMType::LMSR) => {
            if market.num_outcomes > 64 {
                return Err(BettingPlatformError::InvalidConversion.into());
            }
        }
        _ => {}
    }

    Ok(())
}

/// Extract state from source AMM
fn extract_amm_state(
    amm_account: &AccountInfo,
    amm_type: AMMType,
) -> Result<ConversionData, ProgramError> {
    match amm_type {
        AMMType::LMSR => {
            use crate::state::amm_accounts::LSMRMarket;
            let market = LSMRMarket::try_from_slice(&amm_account.data.borrow())?;
            
            Ok(ConversionData {
                num_outcomes: market.num_outcomes,
                total_liquidity: market.b_parameter,
                outcome_shares: market.shares.clone(),
                current_prices: calculate_lmsr_prices(&market)?,
                total_volume: market.total_volume,
            })
        }
        AMMType::PMAMM => {
            use crate::state::amm_accounts::PMAMMPool;
            let pool = PMAMMPool::try_from_slice(&amm_account.data.borrow())?;
            
            Ok(ConversionData {
                num_outcomes: pool.num_outcomes,
                total_liquidity: pool.reserves.iter().sum(),
                outcome_shares: pool.reserves.clone(),
                current_prices: calculate_pmamm_prices(&pool)?,
                total_volume: pool.total_volume,
            })
        }
        AMMType::L2AMM => {
            use crate::state::amm_accounts::L2AMMPool;
            let pool = L2AMMPool::try_from_slice(&amm_account.data.borrow())?;
            
            // Convert distribution to discrete outcomes
            let outcome_shares = pool.distribution.iter()
                .map(|bin| bin.weight)
                .collect();
            
            Ok(ConversionData {
                num_outcomes: pool.discretization_points as u8,
                total_liquidity: pool.liquidity_parameter,
                outcome_shares,
                current_prices: vec![5000; pool.discretization_points as usize], // Placeholder
                total_volume: pool.total_volume,
            })
        }
        AMMType::Hybrid => {
            // For hybrid, extract from the underlying AMM
            use crate::state::amm_accounts::HybridMarket;
            let market = HybridMarket::try_from_slice(&amm_account.data.borrow())?;
            
            // Hybrid markets aggregate data from multiple AMMs
            Ok(ConversionData {
                num_outcomes: market.num_outcomes,
                total_liquidity: market.total_liquidity,
                outcome_shares: vec![market.total_liquidity / market.num_outcomes as u64; market.num_outcomes as usize],
                current_prices: vec![5000; market.num_outcomes as usize], // Equal prices initially
                total_volume: market.total_volume,
            })
        }
    }
}

/// Initialize target AMM with converted state
fn initialize_target_amm(
    amm_account: &AccountInfo,
    amm_type: AMMType,
    data: &ConversionData,
    market: &HybridMarket,
) -> ProgramResult {
    match amm_type {
        AMMType::LMSR => {
            // Initialize LMSR with equivalent liquidity parameter
            msg!("Initializing LMSR with liquidity {}", data.total_liquidity);
            // Implementation would call LMSR initialization
        }
        AMMType::PMAMM => {
            // Initialize PM-AMM with reserves proportional to prices
            msg!("Initializing PM-AMM with {} outcomes", data.num_outcomes);
            // Implementation would call PM-AMM initialization
        }
        AMMType::L2AMM => {
            // Initialize L2-AMM with distribution based on shares
            msg!("Initializing L2-AMM with {} bins", data.num_outcomes);
            // Implementation would call L2-AMM initialization
        }
        AMMType::Hybrid => {
            // For hybrid, delegate to the optimal AMM type
            let optimal_amm = if data.num_outcomes == 2 {
                AMMType::LMSR
            } else {
                AMMType::PMAMM
            };
            msg!("Initializing Hybrid AMM delegating to {:?}", optimal_amm);
            // Recursive call with optimal AMM type
            initialize_target_amm(amm_account, optimal_amm, data, market)?;
        }
    }
    
    Ok(())
}

#[derive(Debug, Clone)]
struct ConversionData {
    num_outcomes: u8,
    total_liquidity: u64,
    outcome_shares: Vec<u64>,
    current_prices: Vec<u64>,
    total_volume: u64,
}

/// Calculate current prices in LMSR market
fn calculate_lmsr_prices(market: &crate::state::amm_accounts::LSMRMarket) -> Result<Vec<u64>, ProgramError> {
    use crate::amm::lmsr::math::calculate_price;
    
    let mut prices = Vec::with_capacity(market.num_outcomes as usize);
    for i in 0..market.num_outcomes {
        let price = calculate_price(&market.shares, i, market.b_parameter)?;
        prices.push(price);
    }
    Ok(prices)
}

/// Calculate current prices in PM-AMM pool
fn calculate_pmamm_prices(pool: &crate::state::amm_accounts::PMAMMPool) -> Result<Vec<u64>, ProgramError> {
    use crate::amm::pmamm::math::calculate_probabilities;
    calculate_probabilities(pool)
}

/// Get provider's liquidity in AMM
fn get_provider_liquidity(
    amm_account: &AccountInfo,
    provider: &Pubkey,
    amm_type: AMMType,
) -> Result<u64, ProgramError> {
    // Simplified - in production would check LP token balances
    Ok(100_000) // Placeholder
}

/// Calculate conversion ratio between AMM types
fn calculate_conversion_ratio(from: AMMType, to: AMMType) -> Result<u64, ProgramError> {
    // Return ratio in basis points (10000 = 1:1)
    match (from, to) {
        (AMMType::LMSR, AMMType::PMAMM) => Ok(9500), // 5% loss
        (AMMType::PMAMM, AMMType::L2AMM) => Ok(9800), // 2% loss
        (AMMType::L2AMM, AMMType::LMSR) => Ok(9000), // 10% loss
        _ => Ok(10000), // No loss
    }
}

/// Remove liquidity from source AMM
fn remove_liquidity_from_amm(
    amm_account: &AccountInfo,
    provider: &Pubkey,
    amount: u64,
    amm_type: AMMType,
) -> ProgramResult {
    msg!("Removing {} liquidity from {:?}", amount, amm_type);
    // Implementation would call appropriate AMM's remove liquidity function
    Ok(())
}

/// Add liquidity to target AMM
fn add_liquidity_to_amm(
    amm_account: &AccountInfo,
    provider: &Pubkey,
    amount: u64,
    amm_type: AMMType,
) -> ProgramResult {
    msg!("Adding {} liquidity to {:?}", amount, amm_type);
    // Implementation would call appropriate AMM's add liquidity function
    Ok(())
}