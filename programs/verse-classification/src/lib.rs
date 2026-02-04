use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};

pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod normalization;
pub mod classification;

use crate::processor::Processor;

// Declare program ID - using a valid base58 string
solana_program::declare_id!("11111111111111111111111111111111");

// Program entrypoint
entrypoint!(process);

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Verse Classification Program entrypoint");
    Processor::process(program_id, accounts, instruction_data)
}