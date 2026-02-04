#!/bin/bash

# Deployment workaround for Anchor v0.31.1 macro issue
# This script builds the program by temporarily modifying the code

echo "Starting deployment workaround..."

# Backup original lib.rs
cp programs/betting_platform/src/lib.rs programs/betting_platform/src/lib.rs.backup

# Create a temporary version with reduced imports in program module
cat > programs/betting_platform/src/lib_temp.rs << 'EOF'
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::{self, Token};
pub use spl_token;
use spl_token::instruction::AuthorityType;

declare_id!("Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4");

// Re-export all modules first
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

// Move all the program module implementations into a separate module
mod program_impl {
    use super::*;
    use crate::account_structs::*;
    use crate::contexts::*;
    use crate::errors::ErrorCode;
    use crate::events::*;
    use crate::advanced_orders::OrderSide;
    use crate::chain_execution::AutoChain;
    use crate::chain_state::ChainStepType;
    use crate::chain_unwind::UnwindChain;
    use crate::dark_pool::{InitializeDarkPool, PlaceDarkOrder, MatchDarkPool, TimeInForce};
    use crate::fees::DistributeFees;
    use crate::hybrid_amm::{InitializeHybridAMM, HybridTrade, AMMType};
    use crate::iceberg_orders::{PlaceIcebergOrder, ExecuteIcebergFill};
    use crate::instructions::attack_detection_instructions::{
        InitializeAttackDetector, ProcessTrade, UpdateVolumeBaseline, ResetDetector
    };
    use crate::instructions::circuit_breaker_instructions::{
        InitializeCircuitBreaker, CheckBreakers, EmergencyShutdown, UpdateBreakerConfig
    };
    use crate::instructions::liquidation_priority_instructions::{
        InitializeLiquidationQueue, UpdateAtRiskPosition, ProcessLiquidation, ClaimKeeperRewards
    };
    use crate::keeper_health::*;
    use crate::l2_amm::{InitializeL2AMM, L2AMMTrade, DistributionType};
    use crate::liquidation::PartialLiquidate;
    use crate::lmsr_amm::{InitializeLSMR, LSMRTrade};
    use crate::pm_amm::{InitializePMAMM, PMAMMTrade};
    use crate::price_cache::*;
    use crate::resolution::*;
    use crate::safety::{CheckCircuitBreakers, MonitorHealth};
    use crate::trading::{OpenPositionParams, OpenPosition, ClosePosition};
    use crate::twap_orders::{PlaceTWAPOrder, ExecuteTWAPInterval};

    // Re-export everything for the program module to use
    pub use super::*;
}

#[program]
pub mod betting_platform {
    use super::program_impl::*;

    pub fn initialize(ctx: Context<Initialize>, _seed: u128) -> Result<()> {
        let global_config = &mut ctx.accounts.global_config;
        global_config.epoch = 1;
        global_config.coverage = u128::MAX;
        global_config.vault = 0;
        global_config.total_oi = 0;
        global_config.halt_flag = false;
        global_config.fee_base = 300;
        global_config.fee_slope = 2500;
        Ok(())
    }

    // Include all other instruction handlers here...
    // (Copy from original lib.rs)
}

// Include all the account structs at the bottom
// (Copy from original lib.rs)
EOF

echo "Attempting build with workaround..."
anchor build

if [ $? -eq 0 ]; then
    echo "Build successful!"
    echo "Deploying program..."
    anchor deploy
else
    echo "Build failed. Trying alternative approach..."
    
    # Alternative: Use Solana tools directly
    echo "Building with cargo build-sbf..."
    cd programs/betting_platform
    cargo build-sbf --features no-entrypoint
    
    if [ $? -eq 0 ]; then
        echo "Direct build successful!"
        solana program deploy target/deploy/betting_platform.so
    else
        echo "All build attempts failed. Manual intervention required."
    fi
fi

# Restore original
mv programs/betting_platform/src/lib.rs.backup programs/betting_platform/src/lib.rs

echo "Deployment workaround complete."