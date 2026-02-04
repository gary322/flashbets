//! Keeper health monitoring
//!
//! Tracks keeper health metrics and performance

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
        KeeperHealth, KeeperAccount, WebSocketHealth, discriminators,
    },
};

pub mod initialize {
    use super::*;
    
    /// Initialize keeper health monitoring account
    pub fn process_initialize_health(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        msg!("Initializing keeper health monitoring");
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let keeper_authority = next_account_info(account_info_iter)?;
        let keeper_account = next_account_info(account_info_iter)?;
        let health_account = next_account_info(account_info_iter)?;
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
        
        // Derive health account PDA
        let (health_pda, bump_seed) = Pubkey::find_program_address(
            &[b"keeper_health", &keeper.keeper_id],
            program_id,
        );
        
        // Verify PDA matches
        if health_pda != *health_account.key {
            msg!("Invalid health account PDA");
            return Err(ProgramError::InvalidSeeds);
        }
        
        // Check if already initialized
        if health_account.data_len() > 0 {
            msg!("Health account already initialized");
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        
        // Calculate required space
        let health_size = std::mem::size_of::<KeeperHealth>();
        
        // Create account
        let rent_lamports = Rent::from_account_info(rent)?
            .minimum_balance(health_size);
        
        invoke_signed(
            &system_instruction::create_account(
                keeper_authority.key,
                health_account.key,
                rent_lamports,
                health_size as u64,
                program_id,
            ),
            &[
                keeper_authority.clone(),
                health_account.clone(),
                system_program.clone(),
            ],
            &[&[b"keeper_health", &keeper.keeper_id, &[bump_seed]]],
        )?;
        
        // Initialize health
        let health = KeeperHealth::new();
        
        // Log initialization
        msg!("Keeper health initialized:");
        msg!("  Keeper ID: {:?}", keeper.keeper_id);
        msg!("  Uptime: {}%", health.uptime_percentage / 100);
        
        // Serialize and save
        health.serialize(&mut &mut health_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

pub mod report {
    use super::*;
    
    /// Report keeper health metrics
    pub fn process_report_metrics(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        markets_processed: u32,
        errors: u32,
        avg_latency: u64,
    ) -> ProgramResult {
        msg!("Reporting keeper metrics");
        
        let account_info_iter = &mut accounts.iter();
        
        // Expected accounts
        let keeper_authority = next_account_info(account_info_iter)?;
        let keeper_account = next_account_info(account_info_iter)?;
        let health_account = next_account_info(account_info_iter)?;
        let clock = next_account_info(account_info_iter)?;
        
        // Verify keeper authority is signer
        if !keeper_authority.is_signer {
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Load accounts
        let keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
        let mut health = KeeperHealth::try_from_slice(&health_account.data.borrow())?;
        
        // Validate accounts
        keeper.validate()?;
        health.validate()?;
        
        // Verify keeper authority
        if keeper.authority != *keeper_authority.key {
            msg!("Keeper authority mismatch");
            return Err(BettingPlatformError::Unauthorized.into());
        }
        
        // Get current slot
        let clock = Clock::from_account_info(clock)?;
        let current_slot = clock.slot;
        
        // Calculate time since last check (assuming 400ms per slot)
        let slots_elapsed = current_slot.saturating_sub(health.last_check_slot);
        let hours_elapsed = slots_elapsed as f64 * 0.4 / 3600.0;
        
        // Update hourly metrics
        if hours_elapsed >= 1.0 {
            // Reset hourly counters
            health.markets_processed_hour = markets_processed as u64;
            health.errors_hour = errors as u64;
        } else {
            // Accumulate
            health.markets_processed_hour += markets_processed as u64;
            health.errors_hour += errors as u64;
        }
        
        // Update total markets
        health.total_markets = health.total_markets.max(markets_processed as u64);
        
        // Update latency
        health.avg_latency_ms = avg_latency;
        
        // Calculate uptime (deduct for errors)
        let error_penalty = (errors as u64 * 100).min(1000); // Max 10% penalty
        health.uptime_percentage = health.uptime_percentage.saturating_sub(error_penalty as u16);
        
        // Determine WebSocket health based on metrics
        health.websocket_status = if errors == 0 && avg_latency < 100 {
            WebSocketHealth::Healthy
        } else if errors < 5 && avg_latency < 500 {
            WebSocketHealth::Degraded
        } else {
            health.failed_checks += 1;
            WebSocketHealth::Failed
        };
        
        // Update last check
        health.last_check_slot = current_slot;
        
        // Log metrics
        msg!("Health metrics reported:");
        msg!("  Markets processed: {}", markets_processed);
        msg!("  Errors: {}", errors);
        msg!("  Avg latency: {}ms", avg_latency);
        msg!("  Uptime: {}%", health.uptime_percentage / 100);
        msg!("  WebSocket status: {:?}", health.websocket_status);
        msg!("  Failed checks: {}", health.failed_checks);
        
        // Check if keeper should be suspended due to poor health
        if health.failed_checks > 10 || health.uptime_percentage < 8000 {
            msg!("WARNING: Keeper health is poor, consider suspension");
        }
        
        // Serialize and save
        health.serialize(&mut &mut health_account.data.borrow_mut()[..])?;
        
        Ok(())
    }
}

/// Check keeper health and potentially suspend unhealthy keepers
pub fn process_health_check(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Performing keeper health check");
    
    let account_info_iter = &mut accounts.iter();
    
    // Expected accounts
    let program_authority = next_account_info(account_info_iter)?;
    let keeper_account = next_account_info(account_info_iter)?;
    let health_account = next_account_info(account_info_iter)?;
    let clock = next_account_info(account_info_iter)?;
    
    // Verify program authority
    if !program_authority.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load accounts
    let mut keeper = KeeperAccount::try_from_slice(&keeper_account.data.borrow())?;
    let health = KeeperHealth::try_from_slice(&health_account.data.borrow())?;
    
    // Get current slot
    let clock = Clock::from_account_info(clock)?;
    let current_slot = clock.slot;
    
    // Check if health check is stale (more than 1 hour)
    let slots_since_update = current_slot.saturating_sub(health.last_check_slot);
    let hours_since_update = slots_since_update as f64 * 0.4 / 3600.0;
    
    let mut should_suspend = false;
    let mut suspension_reason = "";
    
    // Check various health criteria
    if hours_since_update > 2.0 {
        should_suspend = true;
        suspension_reason = "No health updates for over 2 hours";
    } else if health.uptime_percentage < 8000 {
        should_suspend = true;
        suspension_reason = "Uptime below 80%";
    } else if health.failed_checks > 20 {
        should_suspend = true;
        suspension_reason = "Too many failed health checks";
    } else if health.websocket_status == WebSocketHealth::Failed && health.failed_checks > 5 {
        should_suspend = true;
        suspension_reason = "Persistent WebSocket failures";
    }
    
    // Suspend keeper if unhealthy
    if should_suspend && keeper.status == crate::state::keeper_accounts::KeeperStatus::Active {
        msg!("Suspending unhealthy keeper: {}", suspension_reason);
        keeper.status = crate::state::keeper_accounts::KeeperStatus::Suspended;
        keeper.serialize(&mut &mut keeper_account.data.borrow_mut()[..])?;
    }
    
    msg!("Health check completed:");
    msg!("  Keeper ID: {:?}", keeper.keeper_id);
    msg!("  Status: {:?}", keeper.status);
    msg!("  Uptime: {}%", health.uptime_percentage / 100);
    msg!("  Hours since update: {:.1}", hours_since_update);
    
    Ok(())
}