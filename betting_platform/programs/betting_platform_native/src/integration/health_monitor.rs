// Phase 20: System Health Monitoring
// Comprehensive health checks for all system components

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType, HealthCheckCompleteEvent, AutoRecoveryAttemptedEvent},
};

use super::coordinator::SystemStatus;

use crate::math::fixed_point::U64F64;

/// Health check thresholds
pub const WEBSOCKET_TIMEOUT_SLOTS: u64 = 150; // ~60s at 0.4s/slot
pub const POLYMARKET_TIMEOUT_SLOTS: u64 = 300; // ~120s
pub const MIN_KEEPER_COUNT: u32 = 3;
pub const MIN_COVERAGE_RATIO: u64 = 5000; // 0.5 in fixed point
pub const MAX_QUEUE_DEPTH: u32 = 1000;
pub const MAX_FAILED_TXS_PERCENT: u32 = 10;

/// Component health status
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Failed,
}

/// Individual component health
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ComponentHealth {
    pub component_name: [u8; 32], // Fixed size for serialization
    pub status: HealthStatus,
    pub last_check: u64,
    pub error_count: u32,
    pub latency_ms: u32,
    pub throughput: u32,
}

/// System-wide health monitor
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct SystemHealthMonitor {
    pub overall_status: SystemStatus,
    pub polymarket_health: ComponentHealth,
    pub websocket_health: ComponentHealth,
    pub amm_health: ComponentHealth,
    pub queue_health: ComponentHealth,
    pub keeper_health: ComponentHealth,
    pub vault_health: ComponentHealth,
    pub last_full_check: u64,
    pub consecutive_failures: u32,
    pub auto_recovery_enabled: bool,
    pub performance_metrics: PerformanceMetrics,
}

/// Performance tracking
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PerformanceMetrics {
    pub trades_per_second: u32,
    pub average_latency_ms: u32,
    pub success_rate_bps: u32, // basis points
    pub compute_units_used: u64,
    pub last_reset_slot: u64,
}

impl PerformanceMetrics {
    pub const SIZE: usize = 4 + 4 + 4 + 8 + 8; // 28 bytes total
}

impl SystemHealthMonitor {
    pub const SIZE: usize = 1 + // overall_status
        6 * ComponentHealth::SIZE + // 6 components
        8 + // last_full_check
        4 + // consecutive_failures
        1 + // auto_recovery_enabled
        32; // PerformanceMetrics size

    /// Initialize health monitor
    pub fn initialize(&mut self, current_slot: u64) -> ProgramResult {
        self.overall_status = SystemStatus::Initializing;
        self.last_full_check = current_slot;
        self.consecutive_failures = 0;
        self.auto_recovery_enabled = true;

        // Initialize component health
        self.polymarket_health = ComponentHealth::new(b"polymarket");
        self.websocket_health = ComponentHealth::new(b"websocket");
        self.amm_health = ComponentHealth::new(b"amm");
        self.queue_health = ComponentHealth::new(b"priority_queue");
        self.keeper_health = ComponentHealth::new(b"keeper_network");
        self.vault_health = ComponentHealth::new(b"vault");

        // Initialize performance metrics
        self.performance_metrics = PerformanceMetrics {
            trades_per_second: 0,
            average_latency_ms: 0,
            success_rate_bps: 10000, // 100%
            compute_units_used: 0,
            last_reset_slot: current_slot,
        };

        msg!("Health monitor initialized");
        Ok(())
    }

    /// Run comprehensive health check
    pub fn run_health_check(
        &mut self,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let clock = Clock::get()?;
        msg!("Running system health check at slot {}", clock.slot);

        // Check each component
        let polymarket_ok = self.check_polymarket_health(&clock)?;
        let websocket_ok = self.check_websocket_health(&clock)?;
        let amm_ok = self.check_amm_health(accounts)?;
        let queue_ok = self.check_queue_health(accounts)?;
        let keeper_ok = self.check_keeper_health(accounts)?;
        let vault_ok = self.check_vault_health(accounts)?;

        // Determine overall status
        let critical_count = [
            polymarket_ok,
            websocket_ok,
            amm_ok,
            queue_ok,
            keeper_ok,
            vault_ok,
        ].iter().filter(|&&ok| !ok).count();

        self.overall_status = match critical_count {
            0 => SystemStatus::Active,
            1..=2 => SystemStatus::Degraded,
            _ => SystemStatus::Critical,
        };

        // Track consecutive failures
        if critical_count > 0 {
            self.consecutive_failures += 1;
        } else {
            self.consecutive_failures = 0;
        }

        // Auto recovery if enabled
        if self.auto_recovery_enabled && self.consecutive_failures > 3 {
            self.attempt_auto_recovery()?;
        }

        self.last_full_check = clock.slot;

        emit_event(EventType::SystemHealthCheck, &HealthCheckCompleteEvent {
            status: self.overall_status as u8,
            components_healthy: (6 - critical_count) as u32,
            slot: clock.slot,
        });

        Ok(())
    }

    /// Check Polymarket integration health
    fn check_polymarket_health(&mut self, clock: &Clock) -> Result<bool, ProgramError> {
        let component = &mut self.polymarket_health;
        
        // Check if last update is recent
        let slots_since_update = clock.slot.saturating_sub(component.last_check);
        
        if slots_since_update > POLYMARKET_TIMEOUT_SLOTS {
            component.status = HealthStatus::Failed;
            component.error_count += 1;
            msg!("Polymarket health check failed: timeout");
            return Ok(false);
        }

        // Check error rate
        if component.error_count > 10 {
            component.status = HealthStatus::Critical;
            return Ok(false);
        }

        component.status = HealthStatus::Healthy;
        component.last_check = clock.slot;
        Ok(true)
    }

    /// Check WebSocket connection health
    fn check_websocket_health(&mut self, clock: &Clock) -> Result<bool, ProgramError> {
        let component = &mut self.websocket_health;
        
        // Check connection timeout
        let slots_since_update = clock.slot.saturating_sub(component.last_check);
        
        if slots_since_update > WEBSOCKET_TIMEOUT_SLOTS {
            component.status = HealthStatus::Degraded;
            msg!("WebSocket degraded: falling back to polling");
            // Not critical - we have polling fallback
            return Ok(true);
        }

        // Check latency
        if component.latency_ms > 1000 {
            component.status = HealthStatus::Degraded;
            return Ok(true);
        }

        component.status = HealthStatus::Healthy;
        component.last_check = clock.slot;
        Ok(true)
    }

    /// Check AMM health
    fn check_amm_health(&mut self, _accounts: &[AccountInfo]) -> Result<bool, ProgramError> {
        let component = &mut self.amm_health;
        
        // Check throughput
        if component.throughput < 10 {
            component.status = HealthStatus::Degraded;
            return Ok(true); // Degraded but not critical
        }

        component.status = HealthStatus::Healthy;
        Ok(true)
    }

    /// Check priority queue health
    fn check_queue_health(&mut self, _accounts: &[AccountInfo]) -> Result<bool, ProgramError> {
        let component = &mut self.queue_health;
        
        // In production, would check actual queue depth
        let queue_depth = 100; // Simulated
        
        if queue_depth > MAX_QUEUE_DEPTH {
            component.status = HealthStatus::Critical;
            msg!("Queue depth critical: {}", queue_depth);
            return Ok(false);
        }

        component.status = HealthStatus::Healthy;
        Ok(true)
    }

    /// Check keeper network health
    fn check_keeper_health(&mut self, _accounts: &[AccountInfo]) -> Result<bool, ProgramError> {
        let component = &mut self.keeper_health;
        
        // In production, would check actual keeper count
        let active_keepers = 5; // Simulated
        
        if active_keepers < MIN_KEEPER_COUNT {
            component.status = HealthStatus::Critical;
            msg!("Insufficient keepers: {}", active_keepers);
            return Ok(false);
        }

        component.status = HealthStatus::Healthy;
        Ok(true)
    }

    /// Check vault health and coverage
    fn check_vault_health(&mut self, _accounts: &[AccountInfo]) -> Result<bool, ProgramError> {
        let component = &mut self.vault_health;
        
        // In production, would check actual coverage ratio
        let coverage_ratio = 8000; // 0.8 in fixed point
        
        if coverage_ratio < MIN_COVERAGE_RATIO {
            component.status = HealthStatus::Critical;
            msg!("Coverage ratio critical: {}", coverage_ratio);
            return Ok(false);
        }

        component.status = HealthStatus::Healthy;
        Ok(true)
    }

    /// Attempt automatic recovery
    fn attempt_auto_recovery(&mut self) -> ProgramResult {
        msg!("Attempting auto-recovery after {} consecutive failures", 
            self.consecutive_failures);

        // Reset error counts
        self.polymarket_health.error_count = 0;
        self.websocket_health.error_count = 0;
        self.amm_health.error_count = 0;
        self.queue_health.error_count = 0;
        self.keeper_health.error_count = 0;
        self.vault_health.error_count = 0;

        // Reset consecutive failures
        self.consecutive_failures = 0;

        emit_event(EventType::AutoRecoveryTriggered, &AutoRecoveryAttemptedEvent {
            components_reset: 6,
        });

        Ok(())
    }

    /// Update performance metrics
    pub fn update_performance_metrics(
        &mut self,
        trades: u32,
        latency_ms: u32,
        success: bool,
        compute_units: u64,
    ) -> ProgramResult {
        let metrics = &mut self.performance_metrics;
        
        // Update TPS (simple moving average)
        metrics.trades_per_second = (metrics.trades_per_second + trades) / 2;
        
        // Update latency (simple moving average)
        metrics.average_latency_ms = (metrics.average_latency_ms + latency_ms) / 2;
        
        // Update success rate
        if success {
            metrics.success_rate_bps = ((metrics.success_rate_bps as u64 * 99 + 10000) / 100) as u32;
        } else {
            metrics.success_rate_bps = ((metrics.success_rate_bps as u64 * 99) / 100) as u32;
        }
        
        // Track compute units
        metrics.compute_units_used += compute_units;

        Ok(())
    }

    /// Get health summary
    pub fn get_health_summary(&self) -> HealthSummary {
        HealthSummary {
            overall_status: self.overall_status,
            healthy_components: self.count_healthy_components(),
            total_components: 6,
            uptime_percent: self.calculate_uptime(),
            performance_score: self.calculate_performance_score(),
        }
    }

    fn count_healthy_components(&self) -> u32 {
        let mut count = 0;
        if self.polymarket_health.status == HealthStatus::Healthy { count += 1; }
        if self.websocket_health.status == HealthStatus::Healthy { count += 1; }
        if self.amm_health.status == HealthStatus::Healthy { count += 1; }
        if self.queue_health.status == HealthStatus::Healthy { count += 1; }
        if self.keeper_health.status == HealthStatus::Healthy { count += 1; }
        if self.vault_health.status == HealthStatus::Healthy { count += 1; }
        count
    }

    fn calculate_uptime(&self) -> u32 {
        // Simplified - in production would track actual uptime
        if self.consecutive_failures == 0 {
            10000 // 100% in basis points
        } else {
            (10000_u32).saturating_sub(self.consecutive_failures * 1000)
        }
    }

    fn calculate_performance_score(&self) -> u32 {
        let metrics = &self.performance_metrics;
        
        // Weight: 40% success rate, 30% latency, 30% throughput
        let success_score = metrics.success_rate_bps * 4 / 10;
        let latency_score = (10000_u32).saturating_sub(metrics.average_latency_ms.min(10000)) * 3 / 10;
        let tps_score = (metrics.trades_per_second.min(100) * 100) * 3 / 10;
        
        success_score + latency_score + tps_score
    }
}

impl ComponentHealth {
    pub const SIZE: usize = 32 + // component_name
        1 + // status
        8 + // last_check
        4 + // error_count
        4 + // latency_ms
        4; // throughput

    fn new(name: &[u8]) -> Self {
        let mut component_name = [0u8; 32];
        component_name[..name.len().min(32)].copy_from_slice(&name[..name.len().min(32)]);
        
        Self {
            component_name,
            status: HealthStatus::Healthy,
            last_check: 0,
            error_count: 0,
            latency_ms: 0,
            throughput: 0,
        }
    }
}

/// Health summary for external monitoring
#[derive(BorshSerialize, BorshDeserialize)]
pub struct HealthSummary {
    pub overall_status: SystemStatus,
    pub healthy_components: u32,
    pub total_components: u32,
    pub uptime_percent: u32,
    pub performance_score: u32,
}

/// Health check alerts
#[derive(BorshSerialize, BorshDeserialize)]
pub enum HealthAlert {
    ComponentFailure {
        component: [u8; 32],
        error_count: u32,
    },
    PerformanceDegradation {
        metric: [u8; 32],
        current_value: u32,
        threshold: u32,
    },
    SystemCritical {
        failed_components: u32,
    },
    RecoveryInitiated {
        trigger: [u8; 32],
    },
}

/// Process health monitoring instruction
pub fn process_health_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_monitor(program_id, accounts),
        1 => process_run_health_check(program_id, accounts),
        2 => process_update_component_health(program_id, accounts, &instruction_data[1..]),
        3 => process_toggle_auto_recovery(program_id, accounts, instruction_data[1] != 0),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_monitor(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let monitor_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut monitor = SystemHealthMonitor::try_from_slice(&monitor_account.data.borrow())?;
    let clock = Clock::get()?;

    monitor.initialize(clock.slot)?;

    monitor.serialize(&mut &mut monitor_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_run_health_check(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let monitor_account = next_account_info(account_iter)?;

    let mut monitor = SystemHealthMonitor::try_from_slice(&monitor_account.data.borrow())?;

    monitor.run_health_check(accounts)?;

    monitor.serialize(&mut &mut monitor_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_update_component_health(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let monitor_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;

    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut monitor = SystemHealthMonitor::try_from_slice(&monitor_account.data.borrow())?;
    
    // Parse update data
    let component_id = data[0];
    let status = data[1];
    let latency_ms = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
    
    // Update appropriate component
    let component = match component_id {
        0 => &mut monitor.polymarket_health,
        1 => &mut monitor.websocket_health,
        2 => &mut monitor.amm_health,
        3 => &mut monitor.queue_health,
        4 => &mut monitor.keeper_health,
        5 => &mut monitor.vault_health,
        _ => return Err(BettingPlatformError::InvalidComponent.into()),
    };

    component.status = match status {
        0 => HealthStatus::Healthy,
        1 => HealthStatus::Degraded,
        2 => HealthStatus::Critical,
        3 => HealthStatus::Failed,
        _ => return Err(BettingPlatformError::InvalidHealthStatus.into()),
    };
    
    component.latency_ms = latency_ms;
    component.last_check = Clock::get()?.slot;

    monitor.serialize(&mut &mut monitor_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_toggle_auto_recovery(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    enabled: bool,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let monitor_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut monitor = SystemHealthMonitor::try_from_slice(&monitor_account.data.borrow())?;

    monitor.auto_recovery_enabled = enabled;

    msg!("Auto-recovery {}", if enabled { "enabled" } else { "disabled" });

    monitor.serialize(&mut &mut monitor_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;