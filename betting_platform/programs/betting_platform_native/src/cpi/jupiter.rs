//! Jupiter DEX Integration
//! 
//! Native Solana CPI integration with Jupiter aggregator for MMT token swaps
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

/// Jupiter program ID on mainnet
pub const JUPITER_PROGRAM_ID: Pubkey = solana_program::pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

/// Jupiter V6 Swap instruction discriminator
pub const SWAP_DISCRIMINATOR: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];

/// Jupiter swap modes
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

/// Jupiter swap instruction data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SwapInstructionData {
    /// Discriminator for the swap instruction
    pub discriminator: [u8; 8],
    /// Amount to swap (input or output based on mode)
    pub amount: u64,
    /// Minimum/Maximum amount (based on mode)
    pub other_amount_threshold: u64,
    /// Slippage in basis points
    pub slippage_bps: u16,
    /// Platform fee in basis points (optional)
    pub platform_fee_bps: u16,
    /// Swap mode
    pub mode: SwapMode,
}

/// Route info for Jupiter swap
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RouteInfo {
    /// Input mint
    pub input_mint: Pubkey,
    /// Output mint
    pub output_mint: Pubkey,
    /// Market infos for the route
    pub market_infos: Vec<MarketInfo>,
    /// Expected input amount
    pub in_amount: u64,
    /// Expected output amount
    pub out_amount: u64,
    /// Price impact percentage
    pub price_impact_pct: f64,
}

/// Market info for a single hop
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketInfo {
    /// Market ID
    pub id: Pubkey,
    /// Label (e.g., "Raydium", "Orca")
    pub label: String,
    /// Input mint for this hop
    pub input_mint: Pubkey,
    /// Output mint for this hop
    pub output_mint: Pubkey,
    /// Is this market not whitelisted
    pub not_whitelisted: bool,
}

/// Swap MMT tokens using Jupiter
pub fn swap_mmt_tokens<'a>(
    jupiter_program: &AccountInfo<'a>,
    user_source_token_account: &AccountInfo<'a>,
    user_destination_token_account: &AccountInfo<'a>,
    user_authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    route_accounts: &[AccountInfo<'a>],
    amount: u64,
    minimum_amount_out: u64,
    platform_fee_bps: u16,
) -> ProgramResult {
    msg!("Swapping {} MMT tokens via Jupiter", amount);
    
    // Track CPI depth
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Validate inputs
    if amount == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Calculate slippage (50 bps = 0.5%)
    let slippage_bps = 50;
    
    // Create swap instruction data
    let instruction_data = SwapInstructionData {
        discriminator: SWAP_DISCRIMINATOR,
        amount,
        other_amount_threshold: minimum_amount_out,
        slippage_bps,
        platform_fee_bps,
        mode: SwapMode::ExactIn,
    };
    
    let data = instruction_data.try_to_vec()?;
    
    // Build accounts for Jupiter swap
    let mut accounts = vec![
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new_readonly(*user_authority.key, true),
        AccountMeta::new(*user_source_token_account.key, false),
        AccountMeta::new(*user_destination_token_account.key, false),
    ];
    
    // Add route-specific accounts
    for account in route_accounts {
        accounts.push(AccountMeta::new(*account.key, false));
    }
    
    let instruction = Instruction {
        program_id: *jupiter_program.key,
        accounts,
        data,
    };
    
    // Invoke Jupiter
    let mut invoke_accounts = vec![
        token_program.clone(),
        user_authority.clone(),
        user_source_token_account.clone(),
        user_destination_token_account.clone(),
    ];
    
    for account in route_accounts {
        invoke_accounts.push(account.clone());
    }
    
    invoke(&instruction, &invoke_accounts)?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully swapped MMT tokens via Jupiter");
    
    Ok(())
}

/// Get best route for MMT swap
/// In production, this would query Jupiter's route API
pub fn get_best_route(
    input_mint: &Pubkey,
    output_mint: &Pubkey,
    amount: u64,
    slippage_bps: u16,
) -> Result<RouteInfo, ProgramError> {
    msg!("Getting best route for swap: {} -> {}", input_mint, output_mint);
    
    // In production, would call Jupiter's route API
    // For now, return a mock route
    let route = RouteInfo {
        input_mint: *input_mint,
        output_mint: *output_mint,
        market_infos: vec![
            MarketInfo {
                id: Pubkey::new_unique(),
                label: "Raydium".to_string(),
                input_mint: *input_mint,
                output_mint: *output_mint,
                not_whitelisted: false,
            }
        ],
        in_amount: amount,
        out_amount: calculate_output_amount(amount, slippage_bps)?,
        price_impact_pct: 0.1, // 0.1% price impact
    };
    
    Ok(route)
}

/// Calculate output amount with slippage
fn calculate_output_amount(input_amount: u64, slippage_bps: u16) -> Result<u64, ProgramError> {
    // Mock calculation - in production would use actual route pricing
    let base_output = input_amount; // 1:1 for simplicity
    
    // Apply slippage
    let slippage_factor = 10000u64.saturating_sub(slippage_bps as u64);
    let output_with_slippage = base_output
        .checked_mul(slippage_factor)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(10000)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    Ok(output_with_slippage)
}

/// Swap with exact output amount
pub fn swap_mmt_exact_out<'a>(
    jupiter_program: &AccountInfo<'a>,
    user_source_token_account: &AccountInfo<'a>,
    user_destination_token_account: &AccountInfo<'a>,
    user_authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    route_accounts: &[AccountInfo<'a>],
    exact_amount_out: u64,
    maximum_amount_in: u64,
    platform_fee_bps: u16,
) -> ProgramResult {
    msg!("Swapping for exactly {} MMT tokens via Jupiter", exact_amount_out);
    
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Create swap instruction for exact output
    let instruction_data = SwapInstructionData {
        discriminator: SWAP_DISCRIMINATOR,
        amount: exact_amount_out,
        other_amount_threshold: maximum_amount_in,
        slippage_bps: 50, // 0.5% slippage
        platform_fee_bps,
        mode: SwapMode::ExactOut,
    };
    
    let data = instruction_data.try_to_vec()?;
    
    // Build and invoke instruction (similar to exact_in)
    let mut accounts = vec![
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new_readonly(*user_authority.key, true),
        AccountMeta::new(*user_source_token_account.key, false),
        AccountMeta::new(*user_destination_token_account.key, false),
    ];
    
    for account in route_accounts {
        accounts.push(AccountMeta::new(*account.key, false));
    }
    
    let instruction = Instruction {
        program_id: *jupiter_program.key,
        accounts,
        data,
    };
    
    let mut invoke_accounts = vec![
        token_program.clone(),
        user_authority.clone(),
        user_source_token_account.clone(),
        user_destination_token_account.clone(),
    ];
    
    for account in route_accounts {
        invoke_accounts.push(account.clone());
    }
    
    invoke(&instruction, &invoke_accounts)?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully swapped for exact output via Jupiter");
    
    Ok(())
}

/// Swap with program authority (for protocol-owned liquidity)
pub fn swap_with_authority<'a>(
    jupiter_program: &AccountInfo<'a>,
    source_token_account: &AccountInfo<'a>,
    destination_token_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    route_accounts: &[AccountInfo<'a>],
    amount: u64,
    minimum_amount_out: u64,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    msg!("Swapping with program authority");
    
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    let instruction_data = SwapInstructionData {
        discriminator: SWAP_DISCRIMINATOR,
        amount,
        other_amount_threshold: minimum_amount_out,
        slippage_bps: 50,
        platform_fee_bps: 0, // No platform fee for protocol swaps
        mode: SwapMode::ExactIn,
    };
    
    let data = instruction_data.try_to_vec()?;
    
    let mut accounts = vec![
        AccountMeta::new_readonly(*token_program.key, false),
        AccountMeta::new_readonly(*authority.key, true),
        AccountMeta::new(*source_token_account.key, false),
        AccountMeta::new(*destination_token_account.key, false),
    ];
    
    for account in route_accounts {
        accounts.push(AccountMeta::new(*account.key, false));
    }
    
    let instruction = Instruction {
        program_id: *jupiter_program.key,
        accounts,
        data,
    };
    
    let mut invoke_accounts = vec![
        token_program.clone(),
        authority.clone(),
        source_token_account.clone(),
        destination_token_account.clone(),
    ];
    
    for account in route_accounts {
        invoke_accounts.push(account.clone());
    }
    
    invoke_signed(&instruction, &invoke_accounts, signer_seeds)?;
    
    cpi_tracker.exit_cpi();
    msg!("Successfully swapped with program authority");
    
    Ok(())
}

/// Validate Jupiter accounts
pub fn validate_jupiter_accounts<'a>(
    jupiter_program: &AccountInfo<'a>,
    expected_program_id: &Pubkey,
) -> Result<(), ProgramError> {
    if jupiter_program.key != expected_program_id {
        return Err(BettingPlatformError::InvalidProgram.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_output_amount() {
        // Test with 50 bps (0.5%) slippage
        let input = 1000000; // 1M tokens
        let slippage = 50;
        
        let output = calculate_output_amount(input, slippage).unwrap();
        
        // Expected: 1M * 0.995 = 995,000
        assert_eq!(output, 995000);
    }
    
    #[test]
    fn test_swap_modes() {
        let exact_in = SwapMode::ExactIn;
        let exact_out = SwapMode::ExactOut;
        
        // Ensure they serialize differently
        let in_bytes = borsh::to_vec(&exact_in).unwrap();
        let out_bytes = borsh::to_vec(&exact_out).unwrap();
        
        assert_ne!(in_bytes, out_bytes);
    }
    
    #[test]
    fn test_jupiter_constants() {
        // Verify Jupiter program ID is valid format
        assert_eq!(JUPITER_PROGRAM_ID.to_string().len(), 44);
        
        // Verify discriminator is 8 bytes
        assert_eq!(SWAP_DISCRIMINATOR.len(), 8);
    }
}