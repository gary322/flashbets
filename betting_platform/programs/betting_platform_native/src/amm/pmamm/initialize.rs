//! PM-AMM pool initialization

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
    events::{Event, PoolCreated},
    pda::PmammPoolPDA,
    state::amm_accounts::PMAMMMarket as PMAMMPool,
};

/// Initialize a new PM-AMM pool
pub fn process_initialize_pmamm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    pool_id: u128,
    num_outcomes: u8,
    initial_amounts: Vec<u64>,
) -> ProgramResult {
    msg!("Initializing PM-AMM pool");

    // Validate parameters
    if num_outcomes < 2 || num_outcomes > MAX_OUTCOMES {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    if initial_amounts.len() != num_outcomes as usize {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Validate all amounts are non-zero and above minimum
    for &amount in &initial_amounts {
        if amount < MIN_LIQUIDITY {
            return Err(BettingPlatformError::InsufficientBalance.into());
        }
    }

    // Get accounts
    let account_info_iter = &mut accounts.iter();
    
    let initializer = next_account_info(account_info_iter)?;
    let pool_account = next_account_info(account_info_iter)?;
    let lp_mint = next_account_info(account_info_iter)?;
    let lp_token_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;

    // Validate accounts
    validate_signer(initializer)?;
    validate_writable(pool_account)?;
    validate_writable(lp_mint)?;
    validate_writable(lp_token_account)?;

    // Validate PDA
    let (pool_pda, bump) = PmammPoolPDA::derive(program_id, pool_id);
    if pool_account.key != &pool_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    // Calculate pool account size
    let pool_size = 8 + // discriminator
        16 + // pool_id
        1 + // num_outcomes  
        4 + (num_outcomes as usize * 8) + // reserves vector
        8 + // total_lp_supply
        2 + // fee_bps
        4 + 256 + // liquidity_providers vector (max 32 providers * 8 bytes)
        8 + // total_volume
        8 + // created_at
        8 + // last_update
        1 + // state
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

    // Calculate total initial liquidity
    let total_liquidity: u64 = initial_amounts.iter().sum();

    // Check initializer has enough funds
    if **initializer.lamports.borrow() < total_liquidity + required_lamports {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }

    // Transfer liquidity to pool
    **initializer.lamports.borrow_mut() -= total_liquidity;
    **pool_account.lamports.borrow_mut() += total_liquidity;

    // Calculate initial LP tokens
    let lp_tokens = calculate_initial_lp_tokens(&initial_amounts)?;

    // Initialize pool
    let clock = Clock::get()?;
    let pool = PMAMMPool::new(
        pool_id,
        total_liquidity,
        clock.unix_timestamp + 30 * 24 * 60 * 60, // 30 days expiry
        num_outcomes,
        5000, // 50% initial price
        Pubkey::default(), // Oracle will be set later if needed
    );
    
    // Update with actual values
    let mut pool_mut = pool;
    pool_mut.reserves = initial_amounts.clone();
    pool_mut.total_lp_supply = lp_tokens;
    pool_mut.liquidity_providers = 1;
    pool_mut.created_at = clock.unix_timestamp;
    pool_mut.last_update = clock.unix_timestamp;

    // Mint LP tokens to initializer
    mint_lp_tokens(
        lp_mint,
        lp_token_account,
        pool_account,
        token_program,
        lp_tokens,
        &[b"pmamm_pool", &pool_id.to_le_bytes(), &[bump]],
    )?;

    // Write pool data
    pool_mut.serialize(&mut &mut pool_account.data.borrow_mut()[..])?;

    // Emit event
    PoolCreated {
        pool_id,
        amm_type: "PM-AMM".to_string(),
        num_outcomes,
        initial_reserves: initial_amounts,
        initial_lp_supply: lp_tokens,
        fee_bps: DEFAULT_FEE_BPS,
    }
    .emit();

    msg!("PM-AMM pool initialized successfully");
    Ok(())
}

/// Calculate initial LP tokens using geometric mean
fn calculate_initial_lp_tokens(amounts: &[u64]) -> Result<u64, ProgramError> {
    use crate::math::U128F128;

    let mut product = U128F128::from_num(1u128);
    let n = amounts.len() as u32;

    for &amount in amounts {
        if amount == 0 {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Calculate nth root by: exp(ln(amount) / n)
        let amount_f = U128F128::from_num(amount as u128);
        let ln_amount = amount_f.ln()?;
        let scaled_ln = ln_amount.checked_div(U128F128::from_num(n as u128))
            .ok_or(BettingPlatformError::DivisionByZero)?;
        let nth_root = scaled_ln.exp()?;
        
        product = product.checked_mul(nth_root)
            .ok_or(BettingPlatformError::MathOverflow)?;
    }

    // Scale up for precision
    let scaled = product.checked_mul(U128F128::from_num(INITIAL_LP_SCALE as u128))
        .ok_or(BettingPlatformError::MathOverflow)?;
    Ok(scaled.to_num() as u64)
}

/// Mint LP tokens using CPI to SPL Token program
fn mint_lp_tokens<'a>(
    mint: &AccountInfo<'a>,
    token_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    use crate::cpi::spl_token;
    
    msg!("Minting {} LP tokens", amount);
    
    // Use the CPI helper to mint tokens
    spl_token::mint_to(
        mint,
        token_account,
        authority,
        amount,
        token_program,
        &[signer_seeds],
    )
}

#[derive(BorshSerialize, Clone)]
struct LiquidityProvider {
    provider: Pubkey,
    lp_tokens: u64,
    initial_investment: u64,
}

const INITIAL_LP_SCALE: u64 = 1_000_000;