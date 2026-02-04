//! Raydium DEX Integration
//! 
//! Native Solana CPI integration with Raydium AMM for MMT token swaps
//! No Anchor dependencies - pure Native Solana implementation

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta},
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    cpi::depth_tracker::CPIDepthTracker,
};

/// Raydium AMM program ID on mainnet
pub const RAYDIUM_AMM_PROGRAM_ID: Pubkey = solana_program::pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

/// Raydium OpenBook Market program
pub const OPENBOOK_PROGRAM_ID: Pubkey = solana_program::pubkey!("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX");

/// Raydium AMM instructions
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum RaydiumInstruction {
    /// Initialize a new AMM pool
    Initialize {
        nonce: u8,
        open_time: u64,
    },
    /// Swap tokens
    SwapBaseIn {
        amount_in: u64,
        minimum_amount_out: u64,
    },
    /// Swap with exact output
    SwapBaseOut {
        max_amount_in: u64,
        amount_out: u64,
    },
}

/// Raydium pool state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AmmInfo {
    /// Status of the pool
    pub status: u64,
    /// Nonce used in program address
    pub nonce: u64,
    /// Max order count
    pub order_num: u64,
    /// Depth of the order book
    pub depth: u64,
    /// Base decimal
    pub coin_decimals: u64,
    /// Quote decimal  
    pub pc_decimals: u64,
    /// AMM state
    pub state: u64,
    /// Reset flag
    pub reset_flag: u64,
    /// Minimum size for a swap
    pub min_size: u64,
    /// Volume in PC
    pub vol_max_cut_ratio: u64,
    /// Amount wave
    pub amount_wave: u64,
    /// Base lot size
    pub coin_lot_size: u64,
    /// Quote lot size
    pub pc_lot_size: u64,
    /// Minimum price tick
    pub min_price_multiplier: u64,
    /// Maximum price tick
    pub max_price_multiplier: u64,
    /// System decimal value
    pub sys_decimal_value: u64,
    /// Fees
    pub fees: AmmFees,
}

/// Fee structure for Raydium AMM
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct AmmFees {
    /// Minimum fee for trades
    pub min_separate_numerator: u64,
    /// Minimum fee denominator
    pub min_separate_denominator: u64,
    /// Trade fee numerator
    pub trade_fee_numerator: u64,
    /// Trade fee denominator
    pub trade_fee_denominator: u64,
    /// PC trade fee numerator
    pub pnl_numerator: u64,
    /// PC trade fee denominator
    pub pnl_denominator: u64,
    /// Swap fee numerator
    pub swap_fee_numerator: u64,
    /// Swap fee denominator
    pub swap_fee_denominator: u64,
}

/// Swap MMT tokens on Raydium
pub fn swap_mmt_on_raydium<'a>(
    raydium_program: &AccountInfo<'a>,
    amm_id: &AccountInfo<'a>,
    amm_authority: &AccountInfo<'a>,
    amm_open_orders: &AccountInfo<'a>,
    amm_target_orders: &AccountInfo<'a>,
    pool_coin_token_account: &AccountInfo<'a>,
    pool_pc_token_account: &AccountInfo<'a>,
    serum_program: &AccountInfo<'a>,
    serum_market: &AccountInfo<'a>,
    serum_bids: &AccountInfo<'a>,
    serum_asks: &AccountInfo<'a>,
    serum_event_queue: &AccountInfo<'a>,
    serum_coin_vault_account: &AccountInfo<'a>,
    serum_pc_vault_account: &AccountInfo<'a>,
    serum_vault_signer: &AccountInfo<'a>,
    user_source_token_account: &AccountInfo<'a>,
    user_dest_token_account: &AccountInfo<'a>,
    user_owner: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount_in: u64,
    minimum_amount_out: u64,
) -> ProgramResult {
    msg!("Swapping {} tokens on Raydium AMM", amount_in);
    
    // Track CPI depth
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Validate inputs
    if amount_in == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Create swap instruction
    let instruction_data = RaydiumInstruction::SwapBaseIn {
        amount_in,
        minimum_amount_out,
    };
    
    let data = instruction_data.try_to_vec()?;
    
    // Build accounts array
    let accounts = vec![
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new(*amm_id.key, false),
        AccountMeta::new_readonly(*amm_authority.key, false),
        AccountMeta::new(*amm_open_orders.key, false),
        AccountMeta::new(*amm_target_orders.key, false),
        AccountMeta::new(*pool_coin_token_account.key, false),
        AccountMeta::new(*pool_pc_token_account.key, false),
        AccountMeta::new_readonly(*serum_program.key, false),
        AccountMeta::new(*serum_market.key, false),
        AccountMeta::new(*serum_bids.key, false),
        AccountMeta::new(*serum_asks.key, false),
        AccountMeta::new(*serum_event_queue.key, false),
        AccountMeta::new(*serum_coin_vault_account.key, false),
        AccountMeta::new(*serum_pc_vault_account.key, false),
        AccountMeta::new_readonly(*serum_vault_signer.key, false),
        AccountMeta::new(*user_source_token_account.key, false),
        AccountMeta::new(*user_dest_token_account.key, false),
        AccountMeta::new_readonly(*user_owner.key, true),
    ];
    
    let instruction = Instruction {
        program_id: *raydium_program.key,
        accounts,
        data,
    };
    
    // Invoke Raydium
    invoke(&instruction, &[
        token_program.clone(),
        amm_id.clone(),
        amm_authority.clone(),
        amm_open_orders.clone(),
        amm_target_orders.clone(),
        pool_coin_token_account.clone(),
        pool_pc_token_account.clone(),
        serum_program.clone(),
        serum_market.clone(),
        serum_bids.clone(),
        serum_asks.clone(),
        serum_event_queue.clone(),
        serum_coin_vault_account.clone(),
        serum_pc_vault_account.clone(),
        serum_vault_signer.clone(),
        user_source_token_account.clone(),
        user_dest_token_account.clone(),
        user_owner.clone(),
    ])?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully swapped tokens on Raydium");
    
    Ok(())
}

/// Calculate swap amounts using constant product formula
pub fn calculate_swap_amounts(
    input_amount: u64,
    input_pool_size: u64,
    output_pool_size: u64,
    trade_fee_numerator: u64,
    trade_fee_denominator: u64,
) -> Result<(u64, u64), ProgramError> {
    // Apply trading fee to input
    let fee_amount = input_amount
        .checked_mul(trade_fee_numerator)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(trade_fee_denominator)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    let input_after_fee = input_amount
        .checked_sub(fee_amount)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    // Calculate output using constant product formula
    // output = (input_after_fee * output_pool) / (input_pool + input_after_fee)
    let numerator = input_after_fee
        .checked_mul(output_pool_size)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    let denominator = input_pool_size
        .checked_add(input_after_fee)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    let output_amount = numerator
        .checked_div(denominator)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    Ok((output_amount, fee_amount))
}

/// Get pool information
pub fn get_pool_info(
    amm_data: &[u8],
) -> Result<AmmInfo, ProgramError> {
    // In production, would deserialize actual AMM state
    // For now, return mock data
    let mock_info = AmmInfo {
        status: 1, // Active
        nonce: 254,
        order_num: 10,
        depth: 3,
        coin_decimals: 9,
        pc_decimals: 6,
        state: 1,
        reset_flag: 0,
        min_size: 1,
        vol_max_cut_ratio: 0,
        amount_wave: 0,
        coin_lot_size: 1,
        pc_lot_size: 1,
        min_price_multiplier: 1,
        max_price_multiplier: 1000000000,
        sys_decimal_value: 1000000000,
        fees: AmmFees {
            min_separate_numerator: 0,
            min_separate_denominator: 1,
            trade_fee_numerator: 25,
            trade_fee_denominator: 10000, // 0.25% fee
            pnl_numerator: 12,
            pnl_denominator: 10000,
            swap_fee_numerator: 25,
            swap_fee_denominator: 10000,
        },
    };
    
    Ok(mock_info)
}

/// Create pool accounts for MMT/USDC pair
pub fn derive_pool_accounts(
    program_id: &Pubkey,
    coin_mint: &Pubkey,
    pc_mint: &Pubkey,
    market: &Pubkey,
) -> Result<RaydiumPoolAccounts, ProgramError> {
    // Derive AMM ID
    let (amm_id, _) = Pubkey::find_program_address(
        &[
            b"raydium_amm",
            coin_mint.as_ref(),
            pc_mint.as_ref(),
            market.as_ref(),
        ],
        program_id,
    );
    
    // Derive authority
    let (authority, nonce) = Pubkey::find_program_address(
        &[amm_id.as_ref()],
        program_id,
    );
    
    // Derive target orders
    let (target_orders, _) = Pubkey::find_program_address(
        &[
            b"target_orders",
            market.as_ref(),
        ],
        program_id,
    );
    
    // Derive coin vault
    let (coin_vault, _) = Pubkey::find_program_address(
        &[
            b"coin_vault",
            amm_id.as_ref(),
            coin_mint.as_ref(),
        ],
        program_id,
    );
    
    // Derive pc vault
    let (pc_vault, _) = Pubkey::find_program_address(
        &[
            b"pc_vault",
            amm_id.as_ref(),
            pc_mint.as_ref(),
        ],
        program_id,
    );
    
    Ok(RaydiumPoolAccounts {
        amm_id,
        authority,
        nonce,
        target_orders,
        coin_vault,
        pc_vault,
    })
}

/// Raydium pool accounts structure
#[derive(Debug, Clone)]
pub struct RaydiumPoolAccounts {
    pub amm_id: Pubkey,
    pub authority: Pubkey,
    pub nonce: u8,
    pub target_orders: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
}

/// Swap with exact output amount
pub fn swap_exact_out<'a>(
    raydium_program: &AccountInfo<'a>,
    amm_accounts: &[AccountInfo<'a>],
    user_source_token_account: &AccountInfo<'a>,
    user_dest_token_account: &AccountInfo<'a>,
    user_owner: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    max_amount_in: u64,
    amount_out: u64,
) -> ProgramResult {
    msg!("Swapping for exactly {} output tokens on Raydium", amount_out);
    
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    let instruction_data = RaydiumInstruction::SwapBaseOut {
        max_amount_in,
        amount_out,
    };
    
    let data = instruction_data.try_to_vec()?;
    
    // Build accounts (similar structure to swap_base_in)
    let mut accounts = vec![AccountMeta::new_readonly(*token_program.key, false)];
    
    // Add AMM accounts
    for (i, account) in amm_accounts.iter().enumerate() {
        let is_writable = i != 2 && i != 7; // Authority and serum_program are readonly
        accounts.push(if is_writable {
            AccountMeta::new(*account.key, false)
        } else {
            AccountMeta::new_readonly(*account.key, false)
        });
    }
    
    // Add user accounts
    accounts.push(AccountMeta::new(*user_source_token_account.key, false));
    accounts.push(AccountMeta::new(*user_dest_token_account.key, false));
    accounts.push(AccountMeta::new_readonly(*user_owner.key, true));
    
    let instruction = Instruction {
        program_id: *raydium_program.key,
        accounts,
        data,
    };
    
    let mut invoke_accounts = vec![token_program.clone()];
    for account in amm_accounts {
        invoke_accounts.push(account.clone());
    }
    invoke_accounts.push(user_source_token_account.clone());
    invoke_accounts.push(user_dest_token_account.clone());
    invoke_accounts.push(user_owner.clone());
    
    invoke(&instruction, &invoke_accounts)?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully swapped for exact output on Raydium");
    
    Ok(())
}

/// Validate Raydium pool is active
pub fn validate_pool_active(
    amm_info: &AmmInfo,
) -> Result<(), ProgramError> {
    if amm_info.status != 1 {
        msg!("Pool is not active, status: {}", amm_info.status);
        return Err(BettingPlatformError::PoolNotActive.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_swap_calculation() {
        // Test constant product formula
        let input = 1000000; // 1M tokens
        let input_pool = 10000000; // 10M in pool
        let output_pool = 5000000; // 5M output tokens
        let fee_num = 25;
        let fee_denom = 10000; // 0.25% fee
        
        let (output, fee) = calculate_swap_amounts(
            input,
            input_pool,
            output_pool,
            fee_num,
            fee_denom
        ).unwrap();
        
        // Fee should be 2500 (0.25% of 1M)
        assert_eq!(fee, 2500);
        
        // Output should follow constant product after fee
        // (1M - 2500) * 5M / (10M + 1M - 2500) ≈ 451,590
        assert!(output > 450000 && output < 452000);
    }
    
    #[test]
    fn test_raydium_constants() {
        // Verify program IDs are valid format
        assert_eq!(RAYDIUM_AMM_PROGRAM_ID.to_string().len(), 44);
        assert_eq!(OPENBOOK_PROGRAM_ID.to_string().len(), 44);
    }
    
    #[test]
    fn test_pool_validation() {
        let active_pool = AmmInfo {
            status: 1,
            nonce: 1,
            order_num: 0,
            depth: 0,
            coin_decimals: 9,
            pc_decimals: 6,
            state: 1,
            reset_flag: 0,
            min_size: 1,
            vol_max_cut_ratio: 0,
            amount_wave: 0,
            coin_lot_size: 1,
            pc_lot_size: 1,
            min_price_multiplier: 1,
            max_price_multiplier: 1000000000,
            sys_decimal_value: 1000000000,
            fees: AmmFees {
                min_separate_numerator: 0,
                min_separate_denominator: 1,
                trade_fee_numerator: 25,
                trade_fee_denominator: 10000,
                pnl_numerator: 12,
                pnl_denominator: 10000,
                swap_fee_numerator: 25,
                swap_fee_denominator: 10000,
            },
        };
        
        assert!(validate_pool_active(&active_pool).is_ok());
        
        let inactive_pool = AmmInfo {
            status: 0, // Inactive
            ..active_pool
        };
        
        assert!(validate_pool_active(&inactive_pool).is_err());
    }
}