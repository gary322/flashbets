//! L2-AMM pool initialization

use borsh::BorshSerialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

use crate::{
    account_validation::{validate_signer, validate_writable},
    amm::constants::*,
    error::BettingPlatformError,
    events::{Event, L2PoolCreated},
    pda::L2ammPoolPDA,
    state::amm_accounts::{L2AMMPool, DistributionBin, PoolState, DistributionType},
};

/// Initialize parameters for L2-AMM pool
#[derive(Debug, Clone)]
pub struct L2InitParams {
    pub pool_id: u128,
    pub min_value: u64,
    pub max_value: u64,
    pub num_bins: u8,
    pub initial_distribution: Option<Vec<u64>>,
    pub liquidity_parameter: u64,
}

/// Initialize a new L2-AMM pool for continuous distributions
pub fn process_initialize_l2amm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    params: L2InitParams,
) -> ProgramResult {
    msg!("Initializing L2-AMM pool");

    // Validate parameters
    if params.num_bins < 10 || params.num_bins > 100 {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    if params.min_value >= params.max_value {
        return Err(BettingPlatformError::InvalidRange.into());
    }

    if params.liquidity_parameter < MIN_LIQUIDITY {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let initializer = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let oracle = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(initializer)?;
    validate_writable(pool_account)?;

    // Validate PDA
    let (pool_pda, _) = L2ammPoolPDA::derive(program_id, params.pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Create distribution bins
    let distribution = create_distribution_bins(&params)?;

    // Calculate pool account size
    let pool_size = 8 + // discriminator
        16 + // pool_id
        8 + // min_value
        8 + // max_value
        1 + // num_bins
        4 + (params.num_bins as usize * 24) + // distribution vector (8+8+8 per bin)
        8 + // liquidity_parameter
        8 + // total_shares
        1 + // state
        32 + // oracle
        8 + // created_at
        8 + // last_update
        8 + // total_volume
        2 + // fee_bps
        64; // padding

    // Create pool account
    let rent = Rent::from_account_info(rent_sysvar)?;
    let required_lamports = rent.minimum_balance(pool_size);

    invoke(
        &solana_program::system_instruction::create_account(
            initializer.key,
            pool_account.key,
            required_lamports,
            pool_size as u64,
            program_id,
        ),
        &[
            initializer.clone(),
            pool_account.clone(),
            system_program.clone(),
        ],
    )?;

    // Check initializer has enough funds
    let total_cost = params.liquidity_parameter + required_lamports;
    if **initializer.lamports.borrow() < total_cost {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Transfer liquidity
    **initializer.lamports.borrow_mut() -= params.liquidity_parameter;
    **pool_account.lamports.borrow_mut() += params.liquidity_parameter;

    // Calculate total initial shares
    let total_shares: u64 = distribution.iter().map(|b| b.weight).sum();

    // Initialize pool
    let clock = Clock::get()?;
    let pool = L2AMMPool {
        discriminator: *b"L2AMM_PL",
        market_id: params.pool_id,
        pool_id: params.pool_id,
        k_parameter: params.liquidity_parameter,
        liquidity_parameter: params.liquidity_parameter,
        b_bound: 1000, // Default bound
        distribution_type: DistributionType::Normal,
        discretization_points: params.num_bins as u16,
        range_min: params.min_value,
        min_value: params.min_value,
        range_max: params.max_value,
        max_value: params.max_value,
        positions: vec![0; params.num_bins as usize],
        distribution,
        total_liquidity: 0,
        total_shares,
        total_volume: 0,
        state: PoolState::Active,
        fee_bps: DEFAULT_FEE_BPS,
        oracle: *oracle.key,
        last_update: clock.unix_timestamp,
    };

    // Write pool data
    pool.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Emit event
    L2PoolCreated {
        pool_id: params.pool_id,
        min_value: params.min_value,
        max_value: params.max_value,
        num_bins: params.num_bins,
        liquidity_parameter: params.liquidity_parameter,
        oracle: *oracle.key,
    }
    .emit();

    msg!("L2-AMM pool initialized successfully");
    Ok(())
}

/// Create distribution bins with initial weights
fn create_distribution_bins(params: &L2InitParams) -> Result<Vec<DistributionBin>, ProgramError> {
    let range = params.max_value - params.min_value;
    let bin_width = range / params.num_bins as u64;
    
    let mut bins = Vec::with_capacity(params.num_bins as usize);
    
    for i in 0..params.num_bins {
        let lower_bound = params.min_value + (i as u64 * bin_width);
        let upper_bound = if i == params.num_bins - 1 {
            params.max_value
        } else {
            lower_bound + bin_width
        };
        
        // Set initial weight based on provided distribution or uniform
        let weight = if let Some(ref dist) = params.initial_distribution {
            if dist.len() != params.num_bins as usize {
                return Err(BettingPlatformError::InvalidInput.into());
            }
            dist[i as usize]
        } else {
            // Default to normal distribution centered at middle
            calculate_normal_weight(i, params.num_bins)
        };
        
        bins.push(DistributionBin {
            lower_bound,
            upper_bound,
            weight,
        });
    }
    
    Ok(bins)
}

/// Calculate weight for normal distribution initialization
fn calculate_normal_weight(bin_index: u8, total_bins: u8) -> u64 {
    // Center of distribution
    let center = total_bins as f64 / 2.0;
    
    // Standard deviation (1/4 of range)
    let std_dev = total_bins as f64 / 4.0;
    
    // Calculate normal distribution weight
    let x = bin_index as f64 + 0.5; // Use bin center
    let exponent = -((x - center) * (x - center)) / (2.0 * std_dev * std_dev);
    let weight = (exponent.exp() * 1000.0) as u64;
    
    weight.max(1) // Ensure minimum weight of 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_distribution_bins() {
        let params = L2InitParams {
            pool_id: 1,
            min_value: 0,
            max_value: 1000,
            num_bins: 10,
            initial_distribution: None,
            liquidity_parameter: 1_000_000,
        };

        let bins = create_distribution_bins(&params).unwrap();
        
        assert_eq!(bins.len(), 10);
        assert_eq!(bins[0].lower_bound, 0);
        assert_eq!(bins[0].upper_bound, 100);
        assert_eq!(bins[9].lower_bound, 900);
        assert_eq!(bins[9].upper_bound, 1000);
        
        // Check weights are highest in middle (normal distribution)
        let middle_weight = bins[5].weight;
        assert!(bins[0].weight < middle_weight);
        assert!(bins[9].weight < middle_weight);
    }

    #[test]
    fn test_normal_weight_calculation() {
        let weight_center = calculate_normal_weight(5, 10);
        let weight_edge = calculate_normal_weight(0, 10);
        
        // Center should have higher weight than edge
        assert!(weight_center > weight_edge);
    }
}