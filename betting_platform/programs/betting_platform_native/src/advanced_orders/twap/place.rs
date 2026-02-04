//! TWAP order placement

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

pub fn process_place_twap(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _market_id: u128,
    _outcome: u8,
    _total_size: u64,
    _duration: u64,
    _intervals: u8,
    _side: crate::instruction::OrderSide,
) -> ProgramResult {
    Err(BettingPlatformError::NotImplemented.into())
}