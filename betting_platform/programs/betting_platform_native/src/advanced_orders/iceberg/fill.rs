//! Iceberg order fill execution

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;

pub fn process_iceberg_fill(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _fill_size: u64,
) -> ProgramResult {
    Err(BettingPlatformError::NotImplemented.into())
}