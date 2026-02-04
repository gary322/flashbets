//! Time-weighted gradual (TWG) order implementation

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

pub fn process_place_twg_order(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _price_min: u64,
    _price_max: u64,
    _outcome: u8,
    _amount: u64,
    _duration: u64,
) -> ProgramResult {
    msg!("Placing TWG order");
    Err(BettingPlatformError::NotImplemented.into())
}