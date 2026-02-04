// Phase 20: Enhanced System State with Circuit Breakers
// Implements comprehensive system state management with safety mechanisms

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
// Note: Using u64 for fixed-point calculations where 10000 = 1.0

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
};

/// Circuit breaker configuration
pub const PRICE_DEVIATION_BREAKER_BPS: u16 = 2000; // 20% price movement
pub const VOLUME_SPIKE_BREAKER_MULTIPLIER: u64 = 10; // 10x normal volume
pub const LIQUIDATION_CASCADE_THRESHOLD: u32 = 50; // 50 liquidations in 1 minute
pub const VAULT_DRAWDOWN_BREAKER_BPS: u16 = 1000; // 10% vault loss
pub const SYSTEM_LOAD_BREAKER_THRESHOLD: u32 = 90; // 90% capacity
pub const BREAKER_COOLDOWN_SLOTS: u64 = 900; // ~6 minutes

/// System state tracking
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EnhancedSystemState {
    pub current_state: SystemState,
    pub circuit_breakers: CircuitBreakers,
    pub performance_metrics: SystemPerformance,
    pub risk_metrics: RiskMetrics,
    pub operational_health: OperationalHealth,
    pub last_state_transition: StateTransition,
    pub emergency_contacts: Vec<Pubkey>,
    pub auto_recovery_enabled: bool,
}

impl EnhancedSystemState {
    pub const SIZE: usize = 1 + // current_state
        CircuitBreakers::SIZE +
        SystemPerformance::SIZE +
        RiskMetrics::SIZE +
        OperationalHealth::SIZE +
        StateTransition::SIZE +
        4 + 32 * 3 + // emergency_contacts
        1; // auto_recovery_enabled

    /// Initialize system state
    pub fn initialize(&mut self, admin: &Pubkey) -> ProgramResult {
        self.current_state = SystemState::Initializing;
        self.circuit_breakers = CircuitBreakers::new();
        self.performance_metrics = SystemPerformance::new();
        self.risk_metrics = RiskMetrics::new();
        self.operational_health = OperationalHealth::new();
        self.last_state_transition = StateTransition::default();
        self.emergency_contacts = vec![*admin];
        self.auto_recovery_enabled = true;

        msg!("Enhanced system state initialized");
        Ok(())
    }

    /// Update system state based on conditions
    pub fn update_state(&mut self, current_slot: u64) -> ProgramResult {
        let previous_state = self.current_state.clone();
        
        // Check all circuit breakers
        let breaker_status = self.circuit_breakers.check_all_breakers(
            &self.performance_metrics,
            &self.risk_metrics,
            current_slot,
        )?;

        // Determine new state
        self.current_state = match (self.current_state, breaker_status) {
            (SystemState::Active, BreakerStatus::AllClear) => SystemState::Active,
            (SystemState::Active, BreakerStatus::Warning) => SystemState::Degraded,
            (SystemState::Active, BreakerStatus::Triggered) => SystemState::Emergency,
            (SystemState::Degraded, BreakerStatus::AllClear) => {
                if self.can_recover(current_slot) {
                    SystemState::Active
                } else {
                    SystemState::Degraded
                }
            },
            (SystemState::Degraded, BreakerStatus::Triggered) => SystemState::Emergency,
            (SystemState::Emergency, BreakerStatus::AllClear) => {
                if self.can_recover(current_slot) {
                    SystemState::Degraded
                } else {
                    SystemState::Emergency
                }
            },
            (SystemState::Halted, _) => SystemState::Halted, // Manual intervention required
            (current, _) => current,
        };

        // Record state transition
        if self.current_state != previous_state {
            self.record_state_transition(previous_state, current_slot)?;
        }

        Ok(())
    }

    /// Check if system can auto-recover
    fn can_recover(&self, current_slot: u64) -> bool {
        self.auto_recovery_enabled &&
        current_slot >= self.last_state_transition.slot + BREAKER_COOLDOWN_SLOTS &&
        self.operational_health.uptime_percentage > 95
    }

    /// Record state transition
    fn record_state_transition(
        &mut self,
        from_state: SystemState,
        current_slot: u64,
    ) -> ProgramResult {
        self.last_state_transition = StateTransition {
            from_state,
            to_state: self.current_state.clone(),
            slot: current_slot,
            timestamp: Clock::get()?.unix_timestamp,
            triggered_by: self.circuit_breakers.get_triggered_breakers(),
        };

        msg!("State transition: {:?} -> {:?}", from_state, self.current_state);
        Ok(())
    }
}

/// System states
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum SystemState {
    Initializing,
    Bootstrapping,
    Active,
    Degraded,
    Emergency,
    Maintenance,
    Halted,
}

/// Circuit breakers
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CircuitBreakers {
    pub price_deviation_breaker: CircuitBreaker,
    pub volume_spike_breaker: CircuitBreaker,
    pub liquidation_cascade_breaker: CircuitBreaker,
    pub vault_drawdown_breaker: CircuitBreaker,
    pub system_load_breaker: CircuitBreaker,
    pub global_kill_switch: bool,
}

impl CircuitBreakers {
    pub const SIZE: usize = 5 * CircuitBreaker::SIZE + 1;

    pub fn new() -> Self {
        Self {
            price_deviation_breaker: CircuitBreaker::new("Price Deviation", PRICE_DEVIATION_BREAKER_BPS),
            volume_spike_breaker: CircuitBreaker::new("Volume Spike", VOLUME_SPIKE_BREAKER_MULTIPLIER as u16),
            liquidation_cascade_breaker: CircuitBreaker::new("Liquidation Cascade", LIQUIDATION_CASCADE_THRESHOLD as u16),
            vault_drawdown_breaker: CircuitBreaker::new("Vault Drawdown", VAULT_DRAWDOWN_BREAKER_BPS),
            system_load_breaker: CircuitBreaker::new("System Load", SYSTEM_LOAD_BREAKER_THRESHOLD as u16),
            global_kill_switch: false,
        }
    }

    /// Check all breakers
    pub fn check_all_breakers(
        &mut self,
        performance: &SystemPerformance,
        risk: &RiskMetrics,
        current_slot: u64,
    ) -> Result<BreakerStatus, ProgramError> {
        if self.global_kill_switch {
            return Ok(BreakerStatus::Triggered);
        }

        let mut any_triggered = false;
        let mut any_warning = false;

        // Price deviation check
        if risk.max_price_deviation_bps > PRICE_DEVIATION_BREAKER_BPS {
            self.price_deviation_breaker.trigger(current_slot)?;
            any_triggered = true;
        } else if risk.max_price_deviation_bps > (PRICE_DEVIATION_BREAKER_BPS as u32 * 70 / 100) as u16 {
            any_warning = true;
        }

        // Volume spike check
        if performance.current_volume > performance.average_volume * VOLUME_SPIKE_BREAKER_MULTIPLIER {
            self.volume_spike_breaker.trigger(current_slot)?;
            any_triggered = true;
        }

        // Liquidation cascade check
        if risk.liquidations_per_minute > LIQUIDATION_CASCADE_THRESHOLD {
            self.liquidation_cascade_breaker.trigger(current_slot)?;
            any_triggered = true;
        }

        // Vault drawdown check
        if risk.vault_drawdown_bps > VAULT_DRAWDOWN_BREAKER_BPS {
            self.vault_drawdown_breaker.trigger(current_slot)?;
            any_triggered = true;
        }

        // System load check
        if performance.system_load_percentage > SYSTEM_LOAD_BREAKER_THRESHOLD {
            self.system_load_breaker.trigger(current_slot)?;
            any_triggered = true;
        }

        Ok(if any_triggered {
            BreakerStatus::Triggered
        } else if any_warning {
            BreakerStatus::Warning
        } else {
            BreakerStatus::AllClear
        })
    }

    /// Get list of triggered breakers
    pub fn get_triggered_breakers(&self) -> Vec<String> {
        let mut triggered = Vec::new();
        
        if self.price_deviation_breaker.is_triggered {
            triggered.push(self.price_deviation_breaker.name.clone());
        }
        if self.volume_spike_breaker.is_triggered {
            triggered.push(self.volume_spike_breaker.name.clone());
        }
        if self.liquidation_cascade_breaker.is_triggered {
            triggered.push(self.liquidation_cascade_breaker.name.clone());
        }
        if self.vault_drawdown_breaker.is_triggered {
            triggered.push(self.vault_drawdown_breaker.name.clone());
        }
        if self.system_load_breaker.is_triggered {
            triggered.push(self.system_load_breaker.name.clone());
        }
        
        triggered
    }

    /// Reset all breakers
    pub fn reset_all(&mut self, current_slot: u64) -> ProgramResult {
        self.price_deviation_breaker.reset(current_slot)?;
        self.volume_spike_breaker.reset(current_slot)?;
        self.liquidation_cascade_breaker.reset(current_slot)?;
        self.vault_drawdown_breaker.reset(current_slot)?;
        self.system_load_breaker.reset(current_slot)?;
        
        msg!("All circuit breakers reset");
        Ok(())
    }
}

/// Individual circuit breaker
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CircuitBreaker {
    pub name: String,
    pub threshold: u16,
    pub is_triggered: bool,
    pub trigger_count: u32,
    pub last_triggered_slot: u64,
    pub cooldown_until_slot: u64,
}

impl CircuitBreaker {
    pub const SIZE: usize = 32 + // name (max 32 chars)
        2 + // threshold
        1 + // is_triggered
        4 + // trigger_count
        8 + // last_triggered_slot
        8; // cooldown_until_slot

    pub fn new(name: &str, threshold: u16) -> Self {
        Self {
            name: name.to_string(),
            threshold,
            is_triggered: false,
            trigger_count: 0,
            last_triggered_slot: 0,
            cooldown_until_slot: 0,
        }
    }

    /// Trigger the breaker
    pub fn trigger(&mut self, current_slot: u64) -> ProgramResult {
        if !self.is_triggered {
            self.is_triggered = true;
            self.trigger_count += 1;
            self.last_triggered_slot = current_slot;
            self.cooldown_until_slot = current_slot + BREAKER_COOLDOWN_SLOTS;
            
            msg!("Circuit breaker '{}' TRIGGERED!", self.name);
        }
        Ok(())
    }

    /// Reset the breaker
    pub fn reset(&mut self, current_slot: u64) -> ProgramResult {
        if self.is_triggered && current_slot >= self.cooldown_until_slot {
            self.is_triggered = false;
            msg!("Circuit breaker '{}' reset", self.name);
        }
        Ok(())
    }
}

/// Breaker status
#[derive(Debug, PartialEq)]
pub enum BreakerStatus {
    AllClear,
    Warning,
    Triggered,
}

/// System performance metrics
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct SystemPerformance {
    pub transactions_per_second: u32,
    pub average_latency_ms: u32,
    pub system_load_percentage: u32,
    pub current_volume: u64,
    pub average_volume: u64,
    pub success_rate_bps: u16,
}

impl SystemPerformance {
    pub const SIZE: usize = 4 + 4 + 4 + 8 + 8 + 2;

    pub fn new() -> Self {
        Self {
            transactions_per_second: 0,
            average_latency_ms: 0,
            system_load_percentage: 0,
            current_volume: 0,
            average_volume: 0,
            success_rate_bps: 10000, // 100%
        }
    }
}

/// Risk metrics
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RiskMetrics {
    pub max_price_deviation_bps: u16,
    pub liquidations_per_minute: u32,
    pub vault_drawdown_bps: u16,
    pub open_interest_ratio: u64,  // In basis points where 10000 = 1.0
    pub concentration_risk_score: u32,
}

impl RiskMetrics {
    pub const SIZE: usize = 2 + 4 + 2 + 8 + 4;

    pub fn new() -> Self {
        Self {
            max_price_deviation_bps: 0,
            liquidations_per_minute: 0,
            vault_drawdown_bps: 0,
            open_interest_ratio: 0,
            concentration_risk_score: 0,
        }
    }
}

/// Operational health
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OperationalHealth {
    pub uptime_percentage: u32,
    pub keeper_availability: u32,
    pub oracle_reliability: u32,
    pub websocket_stability: u32,
    pub last_health_check_slot: u64,
}

impl OperationalHealth {
    pub const SIZE: usize = 4 + 4 + 4 + 4 + 8;

    pub fn new() -> Self {
        Self {
            uptime_percentage: 100,
            keeper_availability: 100,
            oracle_reliability: 100,
            websocket_stability: 100,
            last_health_check_slot: 0,
        }
    }

    /// Calculate overall health score
    pub fn overall_score(&self) -> u32 {
        (self.uptime_percentage + 
         self.keeper_availability + 
         self.oracle_reliability + 
         self.websocket_stability) / 4
    }
}

/// State transition record
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct StateTransition {
    pub from_state: SystemState,
    pub to_state: SystemState,
    pub slot: u64,
    pub timestamp: i64,
    pub triggered_by: Vec<String>,
}

impl StateTransition {
    pub const SIZE: usize = 1 + 1 + 8 + 8 + 4 + 32 * 5; // up to 5 trigger reasons
}

impl Default for SystemState {
    fn default() -> Self {
        SystemState::Initializing
    }
}

/// Emergency action handler
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EmergencyActionHandler {
    pub allowed_actions: Vec<EmergencyAction>,
    pub executed_actions: Vec<ExecutedAction>,
    pub multi_sig_threshold: u8,
}

/// Emergency actions
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum EmergencyAction {
    HaltTrading,
    DisableLiquidations,
    FreezeNewPositions,
    EnableWithdrawOnly,
    ResetCircuitBreakers,
    ForceStateTransition(SystemState),
}

/// Executed action record
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ExecutedAction {
    pub action: EmergencyAction,
    pub executed_by: Vec<Pubkey>,
    pub executed_at_slot: u64,
}

/// State monitoring service
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct StateMonitor {
    pub monitoring_interval: u64,
    pub alert_thresholds: AlertThresholds,
    pub recent_alerts: Vec<SystemAlert>,
}

/// Alert thresholds
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AlertThresholds {
    pub price_deviation_warning_bps: u16,
    pub volume_spike_warning_multiplier: u64,
    pub liquidation_rate_warning: u32,
    pub vault_drawdown_warning_bps: u16,
    pub system_load_warning_percentage: u32,
}

/// System alert
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct SystemAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub triggered_at_slot: u64,
    pub message: String,
}

/// Alert types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum AlertType {
    PriceDeviation,
    VolumeSpike,
    LiquidationCascade,
    VaultDrawdown,
    SystemOverload,
    ComponentFailure,
}

/// Alert severity
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Process system state instructions
pub fn process_system_state_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_state(program_id, accounts),
        1 => process_update_state(program_id, accounts),
        2 => process_trigger_emergency_action(program_id, accounts, &instruction_data[1..]),
        3 => process_reset_circuit_breakers(program_id, accounts),
        4 => process_update_metrics(program_id, accounts, &instruction_data[1..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_state(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let state_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut state = EnhancedSystemState::try_from_slice(&state_account.data.borrow())?;
    state.initialize(admin_account.key)?;
    state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_update_state(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let state_account = next_account_info(account_iter)?;

    let mut state = EnhancedSystemState::try_from_slice(&state_account.data.borrow())?;
    state.update_state(Clock::get()?.slot)?;
    state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_trigger_emergency_action(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let state_account = next_account_info(account_iter)?;

    // Collect emergency signers
    let mut signers = Vec::new();
    for account in account_iter {
        if account.is_signer {
            signers.push(*account.key);
        }
    }

    let mut state = EnhancedSystemState::try_from_slice(&state_account.data.borrow())?;

    // Verify enough emergency contacts signed
    let valid_signers: Vec<_> = signers.iter()
        .filter(|s| state.emergency_contacts.contains(s))
        .collect();

    if valid_signers.len() < 2 {
        return Err(BettingPlatformError::InsufficientEmergencySigners.into());
    }

    // Parse and execute action
    let action = match data[0] {
        0 => EmergencyAction::HaltTrading,
        1 => EmergencyAction::DisableLiquidations,
        2 => EmergencyAction::FreezeNewPositions,
        3 => EmergencyAction::EnableWithdrawOnly,
        4 => EmergencyAction::ResetCircuitBreakers,
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    msg!("Emergency action executed: {:?}", action);

    match action {
        EmergencyAction::HaltTrading => {
            state.current_state = SystemState::Halted;
            state.circuit_breakers.global_kill_switch = true;
        },
        EmergencyAction::ResetCircuitBreakers => {
            state.circuit_breakers.reset_all(Clock::get()?.slot)?;
        },
        _ => {
            // Other actions would be implemented based on specific requirements
        }
    }

    state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_reset_circuit_breakers(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let state_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut state = EnhancedSystemState::try_from_slice(&state_account.data.borrow())?;

    if !state.emergency_contacts.contains(admin_account.key) {
        return Err(BettingPlatformError::UnauthorizedAdmin.into());
    }

    state.circuit_breakers.reset_all(Clock::get()?.slot)?;
    state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_update_metrics(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let state_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;

    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut state = EnhancedSystemState::try_from_slice(&state_account.data.borrow())?;

    // Parse metrics update
    // In production, would deserialize full metrics data
    let tps = u32::from_le_bytes(data[0..4].try_into().unwrap());
    state.performance_metrics.transactions_per_second = tps;

    state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;