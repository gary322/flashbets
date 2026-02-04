//! Order cancellation

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

pub fn process_cancel_advanced_order(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _order_id: u128,
) -> ProgramResult {
    msg!("Cancelling advanced order");
    Err(BettingPlatformError::NotImplemented.into())
}