//! Keeper performance tracking
//!
//! Tracks detailed performance metrics for keeper optimization

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
    clock::Clock,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::keeper_accounts::{
        PerformanceMetrics, KeeperAccount, discriminators,
    },
};

pub mod initialize {
    use super::*;
    
    /// Initialize performance metrics account
    pub fn process_initialize_metrics(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        msg!("Initializing performance metrics");
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let keeper_authority = next_account_info(account_info_iter)?;
        let keeper_account = next_account_info(account_info_iter)?;
        let metrics_account = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = next_account_info(account_info_iter)?;
        
        // Verify keeper authority is signer
        if !keeper_authority.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Load and validate keeper
        let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
        keeper.validate()?;
        
        // Verify keeper authority
        if keeper.authority != *keeper_authority.key {
            msg!("Keeper authority mismatch");
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Derive metrics account PDA
        let (metrics_pda, bump_seed) = Pubkey::find_program_address(
            &[b"keeper_metrics", &keeper.keeper_id],
            program_id,
        );
        
        // Verify PDA matches
        if metrics_pda != *metrics_account.key {
            msg!("Invalid metrics account PDA");
            return Err(ProgramError::InvalidSeeds);
        }
        
        // Check if already initialized
        if metrics_account.data_len() > 0 {
            msg!("Metrics account already initialized");
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        
        // Calculate required space (including space for latency samples)
        let metrics_size = std::mem::size_of::<PerformanceMetrics>() + 8000; // Extra for Vec
        
        // Create account
        let rent_lamports = Rent::from_account_info(rent)?
            .minimum_balance(metrics_size);
        
        invoke_signed(
            &system_instruction::create_account(
                keeper_authority.key,
                metrics_account.key,
                rent_lamports,
                metrics_size as u64,
                program_id,
            ),
            &[
                keeper_authority.clone(),
                metrics_account.clone(),
                system_program.clone(),
            ],
            &[&[b"keeper_metrics", &keeper.keeper_id, &[bump_seed]]],
        )?;
        
        // Initialize metrics
        let metrics = PerformanceMetrics::new();
        
        // Log initialization
        msg!("Performance metrics initialized:");
        msg!("  Keeper ID: {:?}", keeper.keeper_id);
        msg!("  Latency sample capacity: 1000");
        
        // Serialize and save
        metrics.serialize(&mut &mut metrics_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

pub mod update {
    use super::*;
    
    /// Update performance metrics
    pub fn process_update_metrics(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        request_count: u64,
        success_count: u64,
        fail_count: u64,
        latencies: Vec<u64>,
    ) -> ProgramResult {
        msg!("Updating performance metrics");
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let keeper_authority = next_account_info(account_info_iter)?;
        let keeper_account = next_account_info(account_info_iter)?;
        let metrics_account = next_account_info(account_info_iter)?;
        let clock = next_account_info(account_info_iter)?;
        
        // Verify keeper authority is signer
        if !keeper_authority.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Validate input
        if success_count + fail_count != request_count {
            msg!("Success + fail count must equal request count");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        if latencies.len() != request_count as usize {
            msg!("Latency samples must match request count");
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Load accounts
        let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
        let mut metrics = PerformanceMetrics::try_from_slice(&metrics_account.data.borrow())?;
        
        // Validate accounts
        keeper.validate()?;
        metrics.validate()?;
        
        // Verify keeper authority
        if keeper.authority != *keeper_authority.key {
            msg!("Keeper authority mismatch");
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Get current slot
        let clock = Clock::from_account_info(clock)?;
        
        // Update counters
        metrics.total_requests += request_count;
        metrics.successful_requests += success_count;
        metrics.failed_requests += fail_count;
        
        // Update latencies
        metrics.update_latencies(latencies.clone());
        
        // Update timestamp
        metrics.last_update_slot = clock.slot;
        
        // Calculate success rate
        let success_rate = if metrics.total_requests > 0 {
            (metrics.successful_requests * 100) / metrics.total_requests
        } else {
            0
        };
        
        // Log update
        msg!("Performance metrics updated:");
        msg!("  New requests: {}", request_count);
        msg!("  Successes: {}", success_count);
        msg!("  Failures: {}", fail_count);
        msg!("  Total requests: {}", metrics.total_requests);
        msg!("  Success rate: {}%", success_rate);
        msg!("  Avg latency: {}ms", metrics.avg_latency);
        msg!("  P95 latency: {}ms", metrics.p95_latency);
        msg!("  P99 latency: {}ms", metrics.p99_latency);
        
        // Serialize and save
        metrics.serialize(&mut &mut metrics_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

/// Analyze keeper performance and generate recommendations
pub fn process_analyze_performance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Analyzing keeper performance");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let keeper_account = next_account_info(account_info_iter)?;
    let metrics_account = next_account_info(account_info_iter)?;
    let health_account = next_account_info(account_info_iter)?;
    
    // Load accounts
    let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    let metrics = PerformanceMetrics::try_from_slice(&metrics_account.data.borrow())?;
    let health = crate::state::keeper_accounts::KeeperHealth::try_from_slice(&health_account.data.borrow())?;
    
    // Validate accounts
    keeper.validate()?;
    metrics.validate()?;
    health.validate()?;
    
    // Analyze performance
    let success_rate = if metrics.total_requests > 0 {
        (metrics.successful_requests * 100) / metrics.total_requests
    } else {
        0
    };
    
    // Generate recommendations
    msg!("Performance Analysis:");
    msg!("  Keeper ID: {:?}", keeper.keeper_id);
    msg!("  Success rate: {}%", success_rate);
    msg!("  Performance score: {}%", keeper.performance_score / 100);
    msg!("  Average latency: {}ms", metrics.avg_latency);
    msg!("  Uptime: {}%", health.uptime_percentage / 100);
    
    // Recommendations based on metrics
    if success_rate < 90 {
        msg!("RECOMMENDATION: Success rate below 90%, investigate failure causes");
    }
    
    if metrics.avg_latency > 200 {
        msg!("RECOMMENDATION: High average latency, consider optimizing network or compute");
    }
    
    if metrics.p99_latency > metrics.avg_latency * 5 {
        msg!("RECOMMENDATION: High P99 latency variance, investigate outliers");
    }
    
    if health.uptime_percentage < 9500 {
        msg!("RECOMMENDATION: Uptime below 95%, improve reliability");
    }
    
    if keeper.average_response_time > 10 {
        msg!("RECOMMENDATION: Slow response time, optimize keeper logic");
    }
    
    // Performance tier
    let tier = match (success_rate, health.uptime_percentage / 100, metrics.avg_latency) {
        (95.., 98.., 0..=100) => "ELITE",
        (90.., 95.., 0..=200) => "PROFESSIONAL",
        (85.., 90.., 0..=300) => "STANDARD",
        _ => "NEEDS IMPROVEMENT",
    };
    
    msg!("Performance Tier: {}", tier);
    
    Ok(())
}

/// Get keeper leaderboard
pub fn process_get_leaderboard(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    limit: u8,
) -> ProgramResult {
    msg!("Getting keeper leaderboard (top {})", limit);
    
    // In a real implementation, this would:
    // 1. Query all keeper accounts
    // 2. Sort by performance metrics
    // 3. Return top N keepers
    
    // For now, just validate the request
    if limit == 0 || limit > 100 {
        msg!("Invalid limit: must be between 1 and 100");
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    msg!("Leaderboard query completed");
    
    Ok(())
}