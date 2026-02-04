//! Circuit breaker module
//!
//! Advanced safety mechanisms

pub mod initialize;
pub mod check;
pub mod shutdown;
pub mod config;

use borsh::{BorshDeserialize, BorshSerialize};

// Re-export from state
pub use crate::state::security_accounts::CircuitBreaker;

// Type alias for compatibility
pub type BreakerType = CircuitBreakerType;

/// Circuit breaker trigger types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum CircuitBreakerType {
    /// Triggered by excessive price movement
    PriceMovement,
    
    /// Triggered by low coverage ratio
    LowCoverage,
    
    /// Triggered by high volume spike
    VolumeSpike,
    
    /// Triggered by position concentration
    ConcentrationRisk,
    
    /// Triggered by cascading liquidations
    LiquidationCascade,
    
    /// Manual emergency halt
    EmergencyHalt,
    
    /// Oracle failure
    OracleFailure,
    
    /// For compatibility with BreakerType
    Coverage,
    Price, 
    Volume,
    Liquidation,
    Congestion,
}