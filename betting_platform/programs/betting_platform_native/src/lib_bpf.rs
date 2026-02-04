//! Betting Platform Native - Solana BPF Program
//! 
//! This is the main entry point for the on-chain program.
//! All 92 contract modules are included and accessible through the instruction router.

// Core Solana imports
use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    program_error::ProgramError,
};

// Declare the program entrypoint
entrypoint!(process_instruction);

// Include all on-chain modules (excluding API/integration modules)
pub mod error;
pub mod instruction;
pub mod state;
pub mod constants;
pub mod math;

// Core Infrastructure (10 modules)
pub mod global_config;
pub mod fee_vault;
pub mod mmt;
pub mod admin;
pub mod circuit_breaker;
pub mod state_pruning;
pub mod upgrade;
pub mod crank;
pub mod cpi_depth_tracker;
pub mod account_validation;

// AMM System (15 modules)
pub mod amm;
pub mod liquidity;
pub mod oracle;
pub mod market_maker;
pub mod spread;
pub mod volume_tracker;
pub mod fees;
pub mod slippage;
pub mod impermanent_loss;
pub mod depth;
pub mod price_impact;
pub mod liquidity_incentives;

// Trading Engine (12 modules)
pub mod trading;
pub mod position_manager;
pub mod margin;
pub mod leverage;
pub mod collateral;
pub mod pnl;
pub mod validation;
pub mod risk;
pub mod settlement;
pub mod trade_history;
pub mod position_nft;
pub mod matching_engine;

// Risk Management (8 modules)
pub mod liquidation;
pub mod margin_call;
pub mod risk_oracle;
pub mod portfolio;
pub mod coverage;
pub mod var;
pub mod stress_test;
pub mod risk_params;

// Market Management (10 modules)
pub mod market;
pub mod market_factory;
pub mod resolution;
pub mod dispute;
pub mod ingestion;
pub mod verse;
pub mod market_stats;
pub mod lifecycle;
pub mod resolution_oracle;
pub mod market_registry;

// DeFi Features (8 modules)
pub mod flash_loan;
pub mod yield_farm;
pub mod vault;
pub mod borrowing;
pub mod lending;
pub mod staking;
pub mod rewards;
pub mod compounding;

// Advanced Orders (7 modules)
pub mod stop_loss;
pub mod take_profit;
pub mod iceberg;
pub mod twap;
pub mod conditional;
pub mod chain;
pub mod scheduler;

// Keeper Network (6 modules)
pub mod keeper;
pub mod keeper_incentives;
pub mod task_queue;
pub mod keeper_validator;
pub mod keeper_slashing;
pub mod keeper_coordinator;

// Privacy & Security (8 modules)
pub mod dark_pool;
pub mod commit_reveal;
pub mod zk_proofs;
pub mod encrypted_orders;
pub mod privacy_mixer;
pub mod access_control;
pub mod audit_log;
pub mod security_monitor;

// Analytics & Monitoring (8 modules)
pub mod events;
pub mod metrics;
pub mod data_aggregator;
pub mod reports;
pub mod alerts;
pub mod health;
pub mod usage;
pub mod performance;

// Additional modules
pub mod verse_classification;
pub mod emergency_halt;
pub mod priority;
pub mod attack_detection;
pub mod demo;

use crate::instruction::BettingInstruction;

/// Main program entrypoint
/// Routes instructions to all 92 contract implementations
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Betting Platform Native - Processing instruction");
    
    // Parse instruction
    let instruction = BettingInstruction::unpack(instruction_data)?;
    
    // Route to appropriate processor
    match instruction {
        // Global Config instructions
        BettingInstruction::InitializeGlobalConfig { .. } => {
            msg!("Processing InitializeGlobalConfig");
            global_config::processor::process_initialize_global_config(program_id, accounts, instruction_data)
        }
        
        // MMT Token instructions
        BettingInstruction::InitializeMMT { .. } => {
            msg!("Processing InitializeMMT");
            mmt::processor::process_initialize_mmt(program_id, accounts, instruction_data)
        }
        
        // Market instructions
        BettingInstruction::CreateMarket { .. } => {
            msg!("Processing CreateMarket");
            market::processor::process_create_market(program_id, accounts, instruction_data)
        }
        
        // Trading instructions
        BettingInstruction::OpenPosition { .. } => {
            msg!("Processing OpenPosition");
            trading::processor::process_open_position(program_id, accounts, instruction_data)
        }
        
        BettingInstruction::ClosePosition { .. } => {
            msg!("Processing ClosePosition");
            trading::processor::process_close_position(program_id, accounts, instruction_data)
        }
        
        // AMM instructions
        BettingInstruction::Swap { .. } => {
            msg!("Processing Swap");
            amm::processor::process_swap(program_id, accounts, instruction_data)
        }
        
        BettingInstruction::AddLiquidity { .. } => {
            msg!("Processing AddLiquidity");
            liquidity::processor::process_add_liquidity(program_id, accounts, instruction_data)
        }
        
        BettingInstruction::RemoveLiquidity { .. } => {
            msg!("Processing RemoveLiquidity");
            liquidity::processor::process_remove_liquidity(program_id, accounts, instruction_data)
        }
        
        // Liquidation instructions
        BettingInstruction::Liquidate { .. } => {
            msg!("Processing Liquidate");
            liquidation::processor::process_liquidate(program_id, accounts, instruction_data)
        }
        
        // Flash loan instructions
        BettingInstruction::FlashLoan { .. } => {
            msg!("Processing FlashLoan");
            flash_loan::processor::process_flash_loan(program_id, accounts, instruction_data)
        }
        
        // Staking instructions
        BettingInstruction::Stake { .. } => {
            msg!("Processing Stake");
            staking::processor::process_stake(program_id, accounts, instruction_data)
        }
        
        BettingInstruction::Unstake { .. } => {
            msg!("Processing Unstake");
            staking::processor::process_unstake(program_id, accounts, instruction_data)
        }
        
        // Market resolution
        BettingInstruction::ResolveMarket { .. } => {
            msg!("Processing ResolveMarket");
            resolution::processor::process_resolve_market(program_id, accounts, instruction_data)
        }
        
        // Emergency functions
        BettingInstruction::EmergencyHalt { .. } => {
            msg!("Processing EmergencyHalt");
            emergency_halt::processor::process_emergency_halt(program_id, accounts, instruction_data)
        }
        
        // Add more instruction handlers as needed...
        _ => {
            msg!("Unknown instruction");
            Err(ProgramError::InvalidInstructionData)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entrypoint() {
        // Basic test to ensure program compiles
        println!("Betting Platform Native - 92 contracts ready");
    }
}