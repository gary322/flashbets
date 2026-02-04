//! Security Monitor
//!
//! Production-grade security monitoring and anomaly detection

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
    state::accounts::discriminators,
};

/// Security event types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventType {
    /// Suspicious transaction pattern
    SuspiciousTransaction,
    /// Rate limit violation
    RateLimitViolation,
    /// Invalid signature
    InvalidSignature,
    /// Unauthorized access attempt
    UnauthorizedAccess,
    /// Anomalous trading volume
    AnomalousVolume,
    /// Price manipulation attempt
    PriceManipulation,
    /// Flash loan detected
    FlashLoanDetected,
    /// Sandwich attack detected
    SandwichAttack,
    /// Reentrancy attempt
    ReentrancyAttempt,
    /// Circuit breaker triggered
    CircuitBreakerTriggered,
}

/// Security event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SecurityEvent {
    /// Event type
    pub event_type: SecurityEventType,
    /// Event timestamp (slot)
    pub slot: u64,
    /// Associated user (if applicable)
    pub user: Option<Pubkey>,
    /// Event details
    pub details: Vec<u8>,
    /// Severity (1-10)
    pub severity: u8,
    /// Action taken
    pub action_taken: SecurityAction,
}

/// Security actions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityAction {
    /// No action (logged only)
    None,
    /// Transaction rejected
    Rejected,
    /// User warned
    Warned,
    /// User suspended
    Suspended,
    /// User banned
    Banned,
    /// Protocol paused
    ProtocolPaused,
    /// Funds frozen
    FundsFrozen,
}

/// Security monitor state
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SecurityMonitor {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Monitor version
    pub version: u32,
    /// Authority
    pub authority: Pubkey,
    /// Recent events (circular buffer)
    pub recent_events: Vec<SecurityEvent>,
    /// Event buffer size
    pub buffer_size: usize,
    /// Current buffer index
    pub buffer_index: usize,
    /// Total events logged
    pub total_events: u64,
    /// Events by type
    pub events_by_type: [u64; 10], // One for each SecurityEventType
    /// Threat level (0-100)
    pub threat_level: u8,
    /// Last update slot
    pub last_update_slot: u64,
    /// Emergency contacts
    pub emergency_contacts: Vec<Pubkey>,
    /// Auto-response enabled
    pub auto_response_enabled: bool,
}

impl SecurityMonitor {
    pub const MAX_EVENTS: usize = 1000;
    pub const CRITICAL_THREAT_LEVEL: u8 = 80;
    
    pub fn new(authority: Pubkey) -> Self {
        Self {
            discriminator: discriminators::SECURITY_MONITOR,
            version: 1,
            authority,
            recent_events: Vec::with_capacity(Self::MAX_EVENTS),
            buffer_size: Self::MAX_EVENTS,
            buffer_index: 0,
            total_events: 0,
            events_by_type: [0; 10],
            threat_level: 0,
            last_update_slot: 0,
            emergency_contacts: Vec::new(),
            auto_response_enabled: true,
        }
    }
    
    /// Log security event
    pub fn log_event(
        &mut self,
        event_type: SecurityEventType,
        user: Option<Pubkey>,
        details: Vec<u8>,
        severity: u8,
    ) -> Result<SecurityAction, ProgramError> {
        let current_slot = Clock::get()?.slot;
        
        // Determine action based on severity and auto-response
        let action = if self.auto_response_enabled {
            self.determine_action(event_type, severity)
        } else {
            SecurityAction::None
        };
        
        // Create event
        let event = SecurityEvent {
            event_type,
            slot: current_slot,
            user,
            details,
            severity: severity.min(10),
            action_taken: action,
        };
        
        // Add to buffer
        if self.recent_events.len() < self.buffer_size {
            self.recent_events.push(event);
        } else {
            self.recent_events[self.buffer_index] = event;
        }
        self.buffer_index = (self.buffer_index + 1) % self.buffer_size;
        
        // Update counters
        self.total_events += 1;
        self.events_by_type[event_type as usize] += 1;
        
        // Update threat level
        self.update_threat_level(current_slot)?;
        
        // Alert if critical
        if self.threat_level >= Self::CRITICAL_THREAT_LEVEL {
            self.send_alerts()?;
        }
        
        self.last_update_slot = current_slot;
        
        msg!("Security event logged: {:?} (severity: {}, action: {:?})",
            event_type, severity, action);
        
        Ok(action)
    }
    
    /// Determine action based on event type and severity
    fn determine_action(&self, event_type: SecurityEventType, severity: u8) -> SecurityAction {
        match (event_type, severity) {
            // Critical events (severity 8-10)
            (SecurityEventType::FlashLoanDetected, s) if s >= 8 => SecurityAction::ProtocolPaused,
            (SecurityEventType::PriceManipulation, s) if s >= 8 => SecurityAction::ProtocolPaused,
            (SecurityEventType::ReentrancyAttempt, s) if s >= 8 => SecurityAction::Rejected,
            
            // High severity (6-7)
            (SecurityEventType::SandwichAttack, s) if s >= 6 => SecurityAction::Rejected,
            (SecurityEventType::UnauthorizedAccess, s) if s >= 6 => SecurityAction::Suspended,
            (SecurityEventType::AnomalousVolume, s) if s >= 6 => SecurityAction::FundsFrozen,
            
            // Medium severity (4-5)
            (SecurityEventType::RateLimitViolation, s) if s >= 4 => SecurityAction::Warned,
            (SecurityEventType::InvalidSignature, s) if s >= 4 => SecurityAction::Rejected,
            
            // Low severity or default
            _ => SecurityAction::None,
        }
    }
    
    /// Update threat level based on recent events
    fn update_threat_level(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        // Time window for threat calculation (100 slots)
        let window_start = current_slot.saturating_sub(100);
        
        // Count recent events
        let recent_count = self.recent_events.iter()
            .filter(|e| e.slot >= window_start)
            .count();
        
        // Calculate weighted severity
        let weighted_severity: u64 = self.recent_events.iter()
            .filter(|e| e.slot >= window_start)
            .map(|e| e.severity as u64)
            .sum();
        
        // Calculate threat level (0-100)
        let event_factor = (recent_count as u64 * 100 / 50).min(50); // 50% weight
        let severity_factor = (weighted_severity * 100 / (recent_count.max(1) * 10) as u64).min(50); // 50% weight
        
        self.threat_level = (event_factor + severity_factor) as u8;
        
        Ok(())
    }
    
    /// Send alerts to emergency contacts
    fn send_alerts(&self) -> Result<(), ProgramError> {
        msg!("CRITICAL ALERT: Threat level {} reached!", self.threat_level);
        
        for contact in &self.emergency_contacts {
            msg!("Alerting emergency contact: {}", contact);
            // In production, this would trigger off-chain notifications
        }
        
        Ok(())
    }
    
    /// Get recent events of specific type
    pub fn get_events_by_type(&self, event_type: SecurityEventType, max_count: usize) -> Vec<&SecurityEvent> {
        self.recent_events.iter()
            .filter(|e| e.event_type == event_type)
            .take(max_count)
            .collect()
    }
    
    /// Get security statistics
    pub fn get_stats(&self) -> SecurityStats {
        let current_slot = Clock::get().unwrap_or_default().slot;
        let window_start = current_slot.saturating_sub(1000);
        
        let recent_events = self.recent_events.iter()
            .filter(|e| e.slot >= window_start)
            .count();
        
        let actions_taken: Vec<SecurityAction> = self.recent_events.iter()
            .filter(|e| e.slot >= window_start && e.action_taken != SecurityAction::None)
            .map(|e| e.action_taken)
            .collect();
        
        SecurityStats {
            total_events: self.total_events,
            recent_events: recent_events as u64,
            threat_level: self.threat_level,
            events_by_type: self.events_by_type,
            actions_taken: actions_taken.len() as u64,
            protocol_paused: actions_taken.contains(&SecurityAction::ProtocolPaused),
        }
    }
}

/// Security statistics
#[derive(Debug)]
pub struct SecurityStats {
    pub total_events: u64,
    pub recent_events: u64,
    pub threat_level: u8,
    pub events_by_type: [u64; 10],
    pub actions_taken: u64,
    pub protocol_paused: bool,
}

/// Anomaly detector for pattern recognition
pub struct AnomalyDetector {
    /// Normal trading volume (rolling average)
    pub normal_volume: u64,
    /// Normal price volatility
    pub normal_volatility: u64,
    /// Normal transaction frequency
    pub normal_frequency: u64,
    /// Detection sensitivity (1-10)
    pub sensitivity: u8,
}

impl AnomalyDetector {
    pub fn new(sensitivity: u8) -> Self {
        Self {
            normal_volume: 0,
            normal_volatility: 0,
            normal_frequency: 0,
            sensitivity: sensitivity.min(10).max(1),
        }
    }
    
    /// Update baselines
    pub fn update_baselines(
        &mut self,
        volume: u64,
        volatility: u64,
        frequency: u64,
    ) {
        // Exponential moving average
        let alpha = 100; // 0.01 as fixed point
        
        self.normal_volume = (self.normal_volume * (10000 - alpha) + volume * alpha) / 10000;
        self.normal_volatility = (self.normal_volatility * (10000 - alpha) + volatility * alpha) / 10000;
        self.normal_frequency = (self.normal_frequency * (10000 - alpha) + frequency * alpha) / 10000;
    }
    
    /// Detect volume anomaly
    pub fn detect_volume_anomaly(&self, current_volume: u64) -> Option<u8> {
        if self.normal_volume == 0 {
            return None;
        }
        
        let threshold = 200 + (10 - self.sensitivity) as u64 * 50; // 200-700%
        let ratio = current_volume * 100 / self.normal_volume;
        
        if ratio > threshold {
            let severity = ((ratio - threshold) / 100).min(10) as u8;
            Some(severity)
        } else {
            None
        }
    }
    
    /// Detect price anomaly
    pub fn detect_price_anomaly(&self, current_volatility: u64) -> Option<u8> {
        if self.normal_volatility == 0 {
            return None;
        }
        
        let threshold = 150 + (10 - self.sensitivity) as u64 * 30; // 150-420%
        let ratio = current_volatility * 100 / self.normal_volatility;
        
        if ratio > threshold {
            let severity = ((ratio - threshold) / 50).min(10) as u8;
            Some(severity)
        } else {
            None
        }
    }
    
    /// Detect frequency anomaly
    pub fn detect_frequency_anomaly(&self, current_frequency: u64) -> Option<u8> {
        if self.normal_frequency == 0 {
            return None;
        }
        
        let threshold = 300 + (10 - self.sensitivity) as u64 * 100; // 300-1300%
        let ratio = current_frequency * 100 / self.normal_frequency;
        
        if ratio > threshold {
            let severity = ((ratio - threshold) / 200).min(10) as u8;
            Some(severity)
        } else {
            None
        }
    }
}

/// Initialize security monitor
pub fn initialize_security_monitor<'a>(
    monitor_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    // Verify account is uninitialized
    if !monitor_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Calculate space needed
    let monitor = SecurityMonitor::new(*authority.key);
    let space = monitor.try_to_vec()?.len() + 10000; // Extra space for events
    
    // Create account
    let rent = solana_program::rent::Rent::get()?;
    let rent_lamports = rent.minimum_balance(space);
    
    solana_program::program::invoke(
        &solana_program::system_instruction::create_account(
            payer.key,
            monitor_account.key,
            rent_lamports,
            space as u64,
            &crate::ID,
        ),
        &[payer.clone(), monitor_account.clone(), system_program.clone()],
    )?;
    
    // Initialize monitor
    monitor.serialize(&mut &mut monitor_account.data.borrow_mut()[..])?;
    
    msg!("Security monitor initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_event_logging() {
        let authority = Pubkey::new_unique();
        let mut monitor = SecurityMonitor::new(authority);
        
        // Log events
        let action = monitor.log_event(
            SecurityEventType::SuspiciousTransaction,
            Some(Pubkey::new_unique()),
            b"High frequency trading detected".to_vec(),
            5,
        ).unwrap();
        
        assert_eq!(action, SecurityAction::None); // Medium severity
        assert_eq!(monitor.total_events, 1);
        assert_eq!(monitor.events_by_type[0], 1);
    }

    #[test]
    fn test_auto_response() {
        let authority = Pubkey::new_unique();
        let mut monitor = SecurityMonitor::new(authority);
        
        // High severity flash loan
        let action = monitor.log_event(
            SecurityEventType::FlashLoanDetected,
            None,
            b"Large flash loan detected".to_vec(),
            9,
        ).unwrap();
        
        assert_eq!(action, SecurityAction::ProtocolPaused);
    }

    #[test]
    fn test_anomaly_detection() {
        let mut detector = AnomalyDetector::new(5);
        
        // Set baselines
        detector.update_baselines(1_000_000, 1000, 100);
        detector.update_baselines(1_100_000, 1100, 110);
        detector.update_baselines(900_000, 900, 90);
        
        // Normal volume (no anomaly)
        assert!(detector.detect_volume_anomaly(1_200_000).is_none());
        
        // Anomalous volume (5x normal)
        let severity = detector.detect_volume_anomaly(5_000_000);
        assert!(severity.is_some());
        assert!(severity.unwrap() > 0);
    }

    #[test]
    fn test_threat_level_calculation() {
        let authority = Pubkey::new_unique();
        let mut monitor = SecurityMonitor::new(authority);
        
        // Log multiple events
        for i in 0..10 {
            monitor.log_event(
                SecurityEventType::RateLimitViolation,
                None,
                vec![],
                i % 5 + 1,
            ).unwrap();
        }
        
        // Threat level should increase
        assert!(monitor.threat_level > 0);
    }
}