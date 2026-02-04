//! Betting Platform - Native Solana Implementation
//! 
//! This module provides the native Solana entrypoint and instruction processing
//! for the betting platform, replacing the Anchor framework implementation.

use solana_program::{
    account_info::AccountInfo, 
    entrypoint, 
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};

// Re-export all modules
pub mod account_structs;
pub mod advanced_orders;
pub mod amm;
pub mod amm_verification;
pub mod attack_detection;
pub mod chain_execution;
pub mod chain_safety;
pub mod chain_state;
pub mod chain_unwind;
pub mod circuit_breaker;
pub mod contexts;
pub mod dark_pool;
pub mod deployment;
pub mod errors;
pub mod events;
pub mod fees;
pub mod fixed_math;
pub mod fixed_types;
pub mod hybrid_amm;
pub mod iceberg_orders;
pub mod instructions;
pub mod keeper_health;
pub mod keeper_network;
pub mod l2_amm;
pub mod liquidation;
pub mod liquidation_priority;
pub mod lmsr_amm;
pub mod math;
pub mod merkle;
pub mod performance;
pub mod pm_amm;
pub mod price_cache;
pub mod quantum;
pub mod resolution;
pub mod safety;
pub mod sharding;
pub mod state;
pub mod state_compression;
pub mod state_pruning;
pub mod state_traversal;
pub mod trading;
pub mod twap_orders;
pub mod validation;
pub mod verification;
pub mod verse_classifier;

#[cfg(test)]
pub mod test_runner;

#[cfg(test)]
pub mod tests;

// Native Solana entrypoint
entrypoint!(process_instruction);

/// Main entry point for the betting platform program
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Betting Platform Native: Processing instruction");
    
    // Delegate to instruction processor
    crate::instructions::process_instruction(program_id, accounts, instruction_data)
}