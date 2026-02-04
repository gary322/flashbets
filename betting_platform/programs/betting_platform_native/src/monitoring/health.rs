//! System health monitoring
//!
//! Tracks TPS, CU usage, coverage, and service health

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;
use crate::math::U64F64;

/// System health state account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SystemHealth {
    pub status: SystemStatus,
    pub last_update_slot: u64,
    pub epoch_start_slot: u64,
    
    // Performance metrics
    pub current_tps: u32,
    pub average_tps: u32,
    pub peak_tps: u32,
    pub total_transactions: u64,
    
    // Compute unit tracking
    pub average_cu_per_tx: u32,
    pub peak_cu_usage: u32,
    pub cu_violations: u16,  // Count of >20k CU transactions
    
    // Protocol health
    pub coverage_ratio: U64F64,
    pub lowest_coverage: U64F64,
    pub api_response_time_ms: u32,
    pub api_failures: u16,
    pub api_price_deviation_pct: u8,  // CLAUDE.md: Track API deviation %
    
    // Service states
    pub keeper_network: ServiceStatus,
    pub polymarket_api: ServiceStatus,
    pub price_feeds: ServiceStatus,
    pub liquidation_engine: ServiceStatus,
    
    // Circuit breaker
    pub circuit_breaker_active: bool,
    pub circuit_breaker_trigger_slot: Option<u64>,
    pub circuit_breaker_reason: Option<CircuitBreakerReason>,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum SystemStatus {
    Healthy,
    Degraded,
    Critical,
    Emergency,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum ServiceStatus {
    Online,
    Degraded,
    Offline,
    Unknown,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum CircuitBreakerReason {
    LowCoverage,
    HighAPIDeviation,
    NetworkCongestion,
    PolymarketOutage,
    SolanaOutage,
    ManualTrigger,
}

/// Health monitor for updating system health
pub struct HealthMonitor;

impl HealthMonitor {
    /// Update system health metrics
    pub fn update_health(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        new_tps: u32,
        new_cu_usage: u32,
        coverage: U64F64,
        api_deviation_pct: u8,  // CLAUDE.md: Track API price deviation
    ) -> ProgramResult {
        // Account layout:
        // 0. System health account (mut)
        // 1. Authority (signer)
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let health_account = &accounts[0];
        let authority = &accounts[1];
        let clock = Clock::get()?;
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize health state
        let mut health_data = health_account.try_borrow_mut_data()?;
        let mut health = SystemHealth::try_from_slice(&health_data)?;
        
        // Update TPS metrics
        health.current_tps = new_tps;
        health.total_transactions += new_tps as u64;
        
        // Calculate average TPS
        let elapsed_slots = clock.slot.saturating_sub(health.epoch_start_slot).max(1);
        health.average_tps = (health.total_transactions / elapsed_slots) as u32;
        
        // Update peak TPS if necessary
        if new_tps > health.peak_tps {
            health.peak_tps = new_tps;
        }
        
        // Update CU metrics
        if new_cu_usage > 20_000 {
            health.cu_violations += 1;
        }
        
        if new_cu_usage > health.peak_cu_usage {
            health.peak_cu_usage = new_cu_usage;
        }
        
        // Update coverage
        health.coverage_ratio = coverage;
        if coverage < health.lowest_coverage {
            health.lowest_coverage = coverage;
        }
        
        // Update API deviation (CLAUDE.md: Track >5% deviation)
        health.api_price_deviation_pct = api_deviation_pct;
        
        // Check for alerts
        let new_status = Self::calculate_system_status(&health)?;
        
        if new_status != health.status {
            msg!(
                "SystemStatusChange - old_status: {:?}, new_status: {:?}, tps: {}, cu: {}, coverage: {}",
                health.status,
                new_status,
                new_tps,
                new_cu_usage,
                coverage.to_num()
            );
        }
        
        health.status = new_status;
        health.last_update_slot = clock.slot;
        
        // Serialize updated health
        health.serialize(&mut *health_data)?;
        
        Ok(())
    }
    
    /// Calculate system status based on metrics (CLAUDE.md thresholds)
    fn calculate_system_status(health: &SystemHealth) -> Result<SystemStatus, ProgramError> {
        // Coverage check (critical if < 1 as per CLAUDE.md)
        if health.coverage_ratio < U64F64::from_num(1) {
            msg!("CRITICAL: Coverage ratio {} is below 1.0 threshold", health.coverage_ratio.to_num());
            return Ok(SystemStatus::Critical);
        }
        
        // API health check (CLAUDE.md: >5% deviation is critical)
        if health.api_price_deviation_pct > 5 {
            msg!("CRITICAL: API price deviation {}% exceeds 5% threshold", health.api_price_deviation_pct);
            return Ok(SystemStatus::Critical);
        }
        
        if health.api_failures > 10 || health.api_response_time_ms > 5000 {
            return Ok(SystemStatus::Degraded);
        }
        
        // Service health check
        let offline_services = [
            health.keeper_network,
            health.polymarket_api,
            health.price_feeds,
            health.liquidation_engine,
        ]
        .iter()
        .filter(|&&s| s == ServiceStatus::Offline)
        .count();
        
        if offline_services > 0 {
            return Ok(SystemStatus::Degraded);
        }
        
        // CU violations check
        if health.cu_violations > 100 {
            return Ok(SystemStatus::Degraded);
        }
        
        Ok(SystemStatus::Healthy)
    }
    
    /// Trigger circuit breaker
    pub fn trigger_circuit_breaker(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        reason: CircuitBreakerReason,
    ) -> ProgramResult {
        // Account layout:
        // 0. System health account (mut)
        // 1. Authority (signer)
        // 2. Clock sysvar
        
        if accounts.len() < 3 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let health_account = &accounts[0];
        let authority = &accounts[1];
        let clock = Clock::get()?;
        
        if !authority.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Deserialize health state
        let mut health_data = health_account.try_borrow_mut_data()?;
        let mut health = SystemHealth::try_from_slice(&health_data)?;
        
        // Activate circuit breaker
        health.circuit_breaker_active = true;
        health.circuit_breaker_trigger_slot = Some(clock.slot);
        health.circuit_breaker_reason = Some(reason);
        health.status = SystemStatus::Emergency;
        
        msg!(
            "CircuitBreakerTriggered - reason: {:?}, slot: {}, coverage: {}",
            reason,
            clock.slot,
            health.coverage_ratio.to_num()
        );
        
        // Serialize updated health
        health.serialize(&mut *health_data)?;
        
        Ok(())
    }
    
    /// Check if operation should be allowed based on health
    pub fn check_operation_allowed(
        health: &SystemHealth,
        operation_type: &str,
    ) -> Result<bool, ProgramError> {
        // Circuit breaker check
        if health.circuit_breaker_active {
            msg!("Circuit breaker active, operation {} blocked", operation_type);
            return Ok(false);
        }
        
        // Emergency status check
        if health.status == SystemStatus::Emergency {
            msg!("System in emergency status, operation {} blocked", operation_type);
            return Ok(false);
        }
        
        // Critical operations blocked in degraded state
        if health.status == SystemStatus::Critical {
            match operation_type {
                "open_position" | "increase_leverage" => {
                    msg!("Critical operations blocked in degraded state");
                    return Ok(false);
                }
                _ => {}
            }
        }
        
        Ok(true)
    }
}

// Helper functions for health checks
impl SystemHealth {
    pub const SIZE: usize = 1 + // status
        8 + // last_update_slot
        8 + // epoch_start_slot
        4 + // current_tps
        4 + // average_tps
        4 + // peak_tps
        8 + // total_transactions
        4 + // average_cu_per_tx
        4 + // peak_cu_usage
        2 + // cu_violations
        16 + // coverage_ratio (U64F64)
        16 + // lowest_coverage
        4 + // api_response_time_ms
        2 + // api_failures
        1 + // api_price_deviation_pct
        1 + // keeper_network
        1 + // polymarket_api
        1 + // price_feeds
        1 + // liquidation_engine
        1 + // circuit_breaker_active
        1 + 8 + // circuit_breaker_trigger_slot Option
        1 + 1; // circuit_breaker_reason Option
    
    /// Check if Polymarket is healthy
    pub fn is_polymarket_healthy(&self) -> bool {
        self.polymarket_api == ServiceStatus::Online &&
        self.api_response_time_ms < 5000 &&
        self.api_failures < 5
    }
    
    /// Get health score (0-100)
    pub fn get_health_score(&self) -> u8 {
        let mut score = 100u8;
        
        // Deduct for status
        match self.status {
            SystemStatus::Healthy => {},
            SystemStatus::Degraded => score = score.saturating_sub(20),
            SystemStatus::Critical => score = score.saturating_sub(50),
            SystemStatus::Emergency => return 0,
        }
        
        // Deduct for low coverage
        if self.coverage_ratio < U64F64::from_num(2) {
            score = score.saturating_sub(10);
        }
        
        // Deduct for service issues
        let service_deduction = match self.keeper_network {
            ServiceStatus::Offline => 10,
            ServiceStatus::Degraded => 5,
            _ => 0,
        };
        score = score.saturating_sub(service_deduction);
        
        score
    }
}