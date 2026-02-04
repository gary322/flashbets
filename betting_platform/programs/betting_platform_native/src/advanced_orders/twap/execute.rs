//! TWAP interval execution

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

pub fn process_twap_interval(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
) -> ProgramResult {
    Err(BettingPlatformError::NotImplemented.into())
}