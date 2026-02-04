// High Leverage Safety System for Betting Platform
// Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};

pub mod engine;
pub mod error;
pub mod instructions;
pub mod processor;
pub mod state;

use processor::process_instruction;

// Declare program ID
solana_program::declare_id!("LevSafety1111111111111111111111111111111111");

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);