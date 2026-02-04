use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::synthetics::{SyntheticWrapper, router::{RouteRequest, ExecutionReceipt, ExecutionStatus}};
use borsh::{BorshDeserialize, BorshSerialize};

/// Route synthetic trade through Polymarket
pub fn process_route_synthetic_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    synthetic_id: u128,
    is_buy: bool,
    amount: u64,
    leverage: U64F64,
    min_received: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let wrapper_account = next_account_info(account_info_iter)?;
    let user = next_account_info(account_info_iter)?;
    let receipt_account = next_account_info(account_info_iter)?;
    let keeper = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify user is signer
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify PDA
    let (wrapper_pda, _) = Pubkey::find_program_address(
        &[b"synthetic", synthetic_id.to_le_bytes().as_ref()],
        program_id,
    );

    if wrapper_pda != *wrapper_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Unpack wrapper
    let wrapper = SyntheticWrapper::unpack(&wrapper_account.data.borrow())?;

    // Verify wrapper is active
    if wrapper.status != crate::synthetics::WrapperStatus::Active {
        return Err(BettingPlatformError::WrapperNotActive.into());
    }

    // Create route request
    let route_request = RouteRequest {
        synthetic_id,
        is_buy,
        amount,
        leverage,
        user: *user.key,
    };

    // Calculate order distribution
    let mut total_distributed = 0u64;
    for weight in &wrapper.weights {
        let market_amount = U64F64::from_num(amount).checked_mul(*weight)?.to_num();
        total_distributed += market_amount;
    }

    // Verify minimum received
    if total_distributed < min_received {
        return Err(BettingPlatformError::SlippageExceeded.into());
    }

    // Get clock
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Create execution receipt (to be filled by keeper)
    let receipt = ExecutionReceipt {
        synthetic_id,
        user: *user.key,
        timestamp: clock.unix_timestamp,
        polymarket_orders: vec![], // Will be filled by keeper
        signatures: vec![],         // Will be filled by keeper
        total_executed: amount,
        average_price: wrapper.derived_probability,
        status: ExecutionStatus::Pending,
    };

    // Serialize receipt to receipt_account
    receipt.serialize(&mut &mut receipt_account.data.borrow_mut()[..])?;

    msg!("Routed synthetic trade {} for {} tokens across {} markets", 
        synthetic_id, 
        amount,
        wrapper.polymarket_markets.len()
    );

    Ok(())
}

/// Cancel pending synthetic trade
pub fn process_cancel_synthetic_trade(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    receipt_id: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let receipt_account = next_account_info(account_info_iter)?;
    let user = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify user is signer
    if !user.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load and verify receipt
    let mut receipt = ExecutionReceipt::try_from_slice(&receipt_account.data.borrow())?;
    
    // Verify user owns the order
    if receipt.user != *user.key {
        msg!("User mismatch: receipt owner {} vs signer {}", receipt.user, user.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Verify order is still pending
    if receipt.status != ExecutionStatus::Pending {
        msg!("Cannot cancel non-pending order with status: {:?}", receipt.status);
        return Err(BettingPlatformError::InvalidOrderStatus.into());
    }
    
    // Get clock for cancellation timestamp
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Check cancellation window (max 5 minutes)
    const CANCEL_WINDOW_SECONDS: i64 = 300;
    if clock.unix_timestamp > receipt.timestamp + CANCEL_WINDOW_SECONDS {
        msg!("Cancellation window expired. Order submitted at {}, current time {}", 
            receipt.timestamp, clock.unix_timestamp);
        return Err(BettingPlatformError::CancellationWindowExpired.into());
    }
    
    // Mark as cancelled
    receipt.status = ExecutionStatus::Failed;
    receipt.serialize(&mut &mut receipt_account.data.borrow_mut()[..])?;

    msg!("Cancelled synthetic trade receipt {}", receipt_id);

    Ok(())
}