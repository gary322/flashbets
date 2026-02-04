//! CPI Depth Tracking Module
//! 
//! Ensures CPI depth limits are enforced per specification:
//! - Maximum depth: 4
//! - Chain operations limited to depth 3 (borrow + liquidation + stake)

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    msg,
};
use crate::error::BettingPlatformError;

/// CPI Depth Tracker
pub struct CPIDepthTracker {
    current_depth: u8,
}

impl CPIDepthTracker {
    /// Maximum CPI depth allowed by Solana
    pub const MAX_CPI_DEPTH: u8 = 4;
    
    /// Maximum depth for chain operations (borrow + liquidation + stake)
    pub const CHAIN_MAX_DEPTH: u8 = 3;
    
    /// Create new depth tracker
    pub fn new() -> Self {
        Self {
            current_depth: 0,
        }
    }
    
    /// Get current depth from context
    pub fn from_account_info(accounts: &[AccountInfo]) -> Result<Self, ProgramError> {
        // In production, this would read from a PDA or context
        // For now, we initialize at depth 0
        Ok(Self::new())
    }
    
    /// Check if we can make another CPI call
    pub fn check_depth(&self) -> Result<(), ProgramError> {
        if self.current_depth >= Self::CHAIN_MAX_DEPTH {
            msg!("CPI depth limit exceeded: current={}, max={}", 
                self.current_depth, Self::CHAIN_MAX_DEPTH);
            return Err(BettingPlatformError::CPIDepthExceeded.into());
        }
        Ok(())
    }
    
    /// Check if we can make a CPI with specific depth requirement
    pub fn check_depth_for_operation(&self, required_depth: u8) -> Result<(), ProgramError> {
        if self.current_depth + required_depth > Self::MAX_CPI_DEPTH {
            msg!("CPI depth would exceed limit: current={}, required={}, max={}", 
                self.current_depth, required_depth, Self::MAX_CPI_DEPTH);
            return Err(BettingPlatformError::CPIDepthExceeded.into());
        }
        Ok(())
    }
    
    /// Increment depth for CPI call
    pub fn enter_cpi(&mut self) -> Result<(), ProgramError> {
        self.check_depth()?;
        self.current_depth += 1;
        msg!("Entering CPI, depth now: {}", self.current_depth);
        Ok(())
    }
    
    /// Decrement depth after CPI call
    pub fn exit_cpi(&mut self) {
        if self.current_depth > 0 {
            self.current_depth -= 1;
            msg!("Exiting CPI, depth now: {}", self.current_depth);
        }
    }
    
    /// Get current depth
    pub fn current_depth(&self) -> u8 {
        self.current_depth
    }
    
    /// Check if at maximum depth
    pub fn at_max_depth(&self) -> bool {
        self.current_depth >= Self::CHAIN_MAX_DEPTH
    }
}

/// Helper macro for CPI calls with depth tracking
#[macro_export]
macro_rules! invoke_with_depth_check {
    ($tracker:expr, $instruction:expr, $account_infos:expr) => {{
        $tracker.enter_cpi()?;
        let result = solana_program::program::invoke($instruction, $account_infos);
        $tracker.exit_cpi();
        result
    }};
    ($tracker:expr, $instruction:expr, $account_infos:expr, $seeds:expr) => {{
        $tracker.enter_cpi()?;
        let result = solana_program::program::invoke_signed($instruction, $account_infos, $seeds);
        $tracker.exit_cpi();
        result
    }};
}