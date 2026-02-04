//! Iceberg order placement

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

pub fn process_place_iceberg(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _market_id: u128,
    _side: crate::instruction::OrderSide,
    _total_size: u64,
    _visible_size: u64,
    _price: u64,
) -> ProgramResult {
    Err(BettingPlatformError::NotImplemented.into())
}