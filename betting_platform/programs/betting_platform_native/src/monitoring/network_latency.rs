//! Network Latency Monitoring and Circuit Breaking
//!
//! Monitors network latency and triggers halt when latency exceeds 1.5ms threshold

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    error::BettingPlatformError,
    state::security_accounts::{CircuitBreaker, BreakerType},
    events::{emit_event, EventType},
};

/// Network latency tracking configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LatencyConfig {
    /// Latency threshold in microseconds (1.5ms = 1500μs)
    pub halt_threshold_micros: u64,
    /// Warning threshold in microseconds (1ms = 1000μs)
    pub warning_threshold_micros: u64,
    /// Number of samples to track
    pub sample_window_size: u32,
    /// Minimum samples before triggering halt
    pub min_samples_for_halt: u32,
}

impl Default for LatencyConfig {
    fn default() -> Self {
        Self {
            halt_threshold_micros: 1500,      // 1.5ms as per spec
            warning_threshold_micros: 1000,   // 1ms warning
            sample_window_size: 100,          // Track last 100 samples
            min_samples_for_halt: 10,         // Need 10 samples over threshold
        }
    }
}

/// Network latency monitor state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct NetworkLatencyMonitor {
    /// Configuration
    pub config: LatencyConfig,
    
    /// Recent latency samples (in microseconds)
    pub latency_samples: Vec<u64>,
    
    /// Current average latency
    pub avg_latency_micros: u64,
    
    /// Peak latency in current window
    pub peak_latency_micros: u64,
    
    /// Number of samples over halt threshold
    pub samples_over_threshold: u32,
    
    /// Last measurement timestamp
    pub last_measurement: i64,
    
    /// Total measurements
    pub total_measurements: u64,
    
    /// Is currently halted due to latency
    pub is_halted: bool,
    
    /// Halt triggered at
    pub halt_triggered_at: Option<i64>,
    
    /// Number of times halt was triggered
    pub halt_trigger_count: u32,
}

impl NetworkLatencyMonitor {
    pub const SIZE: usize = 8 + // config size marker
        8 + 8 + 4 + 4 + // config fields
        4 + (8 * 100) + // latency_samples vector
        8 + 8 + 4 + 8 + 8 + // metrics
        1 + 9 + 4; // halt state
        
    /// Create new latency monitor
    pub fn new() -> Self {
        Self {
            config: LatencyConfig::default(),
            latency_samples: Vec::with_capacity(100),
            avg_latency_micros: 0,
            peak_latency_micros: 0,
            samples_over_threshold: 0,
            last_measurement: 0,
            total_measurements: 0,
            is_halted: false,
            halt_triggered_at: None,
            halt_trigger_count: 0,
        }
    }
    
    /// Record a network latency measurement
    pub fn record_latency(
        &mut self,
        latency_micros: u64,
        current_timestamp: i64,
    ) -> Result<bool, ProgramError> {
        // Add to samples
        self.latency_samples.push(latency_micros);
        
        // Maintain window size
        if self.latency_samples.len() > self.config.sample_window_size as usize {
            self.latency_samples.remove(0);
        }
        
        // Update metrics
        self.total_measurements += 1;
        self.last_measurement = current_timestamp;
        
        // Update peak
        if latency_micros > self.peak_latency_micros {
            self.peak_latency_micros = latency_micros;
        }
        
        // Calculate average
        if !self.latency_samples.is_empty() {
            let sum: u64 = self.latency_samples.iter().sum();
            self.avg_latency_micros = sum / self.latency_samples.len() as u64;
        }
        
        // Count samples over threshold
        self.samples_over_threshold = self.latency_samples
            .iter()
            .filter(|&&l| l > self.config.halt_threshold_micros)
            .count() as u32;
        
        // Check if we should trigger halt
        let should_halt = self.check_halt_condition();
        
        if should_halt && !self.is_halted {
            self.trigger_halt(current_timestamp)?;
            return Ok(true);
        }
        
        // Log warnings
        if latency_micros > self.config.warning_threshold_micros {
            msg!("⚠️ Network latency warning: {}μs ({}ms) > {}μs threshold",
                latency_micros,
                latency_micros / 1000,
                self.config.warning_threshold_micros
            );
        }
        
        Ok(false)
    }
    
    /// Check if halt condition is met
    fn check_halt_condition(&self) -> bool {
        // Need minimum samples
        if self.latency_samples.len() < self.config.min_samples_for_halt as usize {
            return false;
        }
        
        // Check if enough samples are over threshold
        self.samples_over_threshold >= self.config.min_samples_for_halt
    }
    
    /// Trigger network latency halt
    fn trigger_halt(&mut self, current_timestamp: i64) -> ProgramResult {
        self.is_halted = true;
        self.halt_triggered_at = Some(current_timestamp);
        self.halt_trigger_count += 1;
        
        msg!("🚨 NETWORK LATENCY HALT TRIGGERED!");
        msg!("Average latency: {}μs ({}ms)", self.avg_latency_micros, self.avg_latency_micros / 1000);
        msg!("Peak latency: {}μs ({}ms)", self.peak_latency_micros, self.peak_latency_micros / 1000);
        msg!("Samples over threshold: {}/{}", self.samples_over_threshold, self.latency_samples.len());
        
        Ok(())
    }
    
    /// Reset halt state
    pub fn reset_halt(&mut self) -> ProgramResult {
        if !self.is_halted {
            return Err(BettingPlatformError::NotHalted.into());
        }
        
        self.is_halted = false;
        self.samples_over_threshold = 0;
        self.latency_samples.clear();
        
        msg!("Network latency halt reset");
        
        Ok(())
    }
    
    /// Get current status
    pub fn get_status(&self) -> LatencyStatus {
        if self.is_halted {
            LatencyStatus::Halted
        } else if self.avg_latency_micros > self.config.warning_threshold_micros {
            LatencyStatus::Warning
        } else {
            LatencyStatus::Normal
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LatencyStatus {
    Normal,
    Warning,
    Halted,
}

/// Measure network operation latency
pub fn measure_network_latency<F>(operation: F) -> Result<u64, ProgramError>
where
    F: FnOnce() -> ProgramResult,
{
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    // Execute operation
    operation()?;
    
    let end = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    // Calculate latency in microseconds
    let latency_micros = end
        .as_micros()
        .saturating_sub(start.as_micros()) as u64;
    
    Ok(latency_micros)
}

/// Process network latency update
pub fn process_network_latency_update(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    latency_micros: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let latency_monitor_account = next_account_info(account_info_iter)?;
    let circuit_breaker_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Load latency monitor
    let mut monitor = NetworkLatencyMonitor::try_from_slice(&latency_monitor_account.data.borrow())?;
    
    // Record latency
    let should_halt = monitor.record_latency(latency_micros, clock.unix_timestamp)?;
    
    // If halt triggered, update circuit breaker
    if should_halt {
        let mut breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
        
        // Activate congestion breaker
        breaker.congestion_breaker_active = true;
        breaker.congestion_activated_at = Some(clock.unix_timestamp);
        breaker.last_trigger_slot = clock.slot;
        breaker.total_triggers += 1;
        
        // Emit event
        emit_event(EventType::CircuitBreakerTriggered, &format!(
            "Network latency halt: {}ms > 1.5ms threshold",
            latency_micros / 1000
        ));
        
        // Save circuit breaker
        breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
    }
    
    // Save monitor state
    monitor.serialize(&mut &mut latency_monitor_account.data.borrow_mut()[..])?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_latency_monitoring() {
        let mut monitor = NetworkLatencyMonitor::new();
        
        // Record normal latencies
        for i in 0..5 {
            monitor.record_latency(500 + i * 100, i as i64).unwrap();
        }
        
        assert_eq!(monitor.get_status(), LatencyStatus::Normal);
        assert_eq!(monitor.samples_over_threshold, 0);
        
        // Record high latencies
        for i in 0..10 {
            monitor.record_latency(2000, 100 + i as i64).unwrap();
        }
        
        assert!(monitor.samples_over_threshold >= 10);
        assert!(monitor.is_halted);
        assert_eq!(monitor.get_status(), LatencyStatus::Halted);
    }
    
    #[test]
    fn test_halt_threshold() {
        let mut monitor = NetworkLatencyMonitor::new();
        
        // Just below threshold
        monitor.record_latency(1499, 1).unwrap();
        assert_eq!(monitor.samples_over_threshold, 0);
        
        // Just above threshold
        monitor.record_latency(1501, 2).unwrap();
        assert_eq!(monitor.samples_over_threshold, 1);
    }
}