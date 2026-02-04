//! Flash Bets Migration Flags for Fused Leverage System
//!
//! Manages parallel operation of old and new leverage systems

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

/// Migration configuration for flash bets
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FlashMigrationConfig {
    /// Use fused leverage for flash bets
    pub use_fused_leverage: bool,
    
    /// Use legacy leverage calculation
    pub use_legacy_leverage: bool,
    
    /// Percentage of flash bets using fused (0-100)
    pub fused_percentage: u8,
    
    /// Max leverage with fused system
    pub fused_max_leverage: u16,
    
    /// Max leverage with legacy system  
    pub legacy_max_leverage: u16,
    
    /// Migration phase (0=legacy, 1=parallel, 2=fused-only)
    pub migration_phase: u8,
    
    /// Authority for migration updates
    pub migration_authority: Pubkey,
    
    /// Count of flash verses using fused
    pub fused_count: u64,
    
    /// Count of flash verses using legacy
    pub legacy_count: u64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Emergency fallback enabled
    pub fallback_enabled: bool,
}

impl FlashMigrationConfig {
    pub fn new(authority: Pubkey) -> Self {
        Self {
            use_fused_leverage: false,
            use_legacy_leverage: true,
            fused_percentage: 0,
            fused_max_leverage: 100,  // Will scale to 500x with chaining
            legacy_max_leverage: 75,   // Current max
            migration_phase: 0,
            migration_authority: authority,
            fused_count: 0,
            legacy_count: 0,
            last_update_slot: 0,
            fallback_enabled: true,
        }
    }
    
    /// Determine which leverage system to use
    pub fn select_leverage_system(&self, seed: u8) -> LeverageSystem {
        // Emergency fallback always uses legacy
        if self.fallback_enabled && !self.use_legacy_leverage {
            return LeverageSystem::Legacy;
        }
        
        match self.migration_phase {
            0 => LeverageSystem::Legacy,
            2 => LeverageSystem::Fused,
            1 => {
                // Parallel mode - use percentage
                let threshold = (self.fused_percentage as u32 * 255) / 100;
                if seed as u32 <= threshold {
                    LeverageSystem::Fused
                } else {
                    LeverageSystem::Legacy
                }
            }
            _ => LeverageSystem::Legacy, // Default to legacy for unknown phase
        }
    }
    
    /// Start migration to fused system
    pub fn start_migration(&mut self, slot: u64) {
        self.migration_phase = 1;
        self.use_fused_leverage = true;
        self.use_legacy_leverage = true;
        self.fused_percentage = 10; // Start with 10%
        self.last_update_slot = slot;
    }
    
    /// Increase fused percentage
    pub fn increase_fused(&mut self, increment: u8) {
        self.fused_percentage = (self.fused_percentage + increment).min(100);
    }
    
    /// Complete migration
    pub fn complete_migration(&mut self, slot: u64) {
        self.migration_phase = 2;
        self.use_fused_leverage = true;
        self.use_legacy_leverage = false;
        self.fused_percentage = 100;
        self.fallback_enabled = false;
        self.last_update_slot = slot;
    }
    
    /// Enable emergency fallback
    pub fn enable_fallback(&mut self) {
        self.fallback_enabled = true;
        self.use_legacy_leverage = true;
    }
    
    /// Record usage
    pub fn record_usage(&mut self, system: LeverageSystem) {
        match system {
            LeverageSystem::Fused => self.fused_count += 1,
            LeverageSystem::Legacy => self.legacy_count += 1,
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Must have at least one system enabled
        if !self.use_fused_leverage && !self.use_legacy_leverage {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Phase must be valid
        if self.migration_phase > 2 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Percentage must be valid
        if self.fused_percentage > 100 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Which leverage system to use
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum LeverageSystem {
    Legacy,
    Fused,
}

/// Calculate leverage based on selected system
pub fn calculate_leverage(
    config: &FlashMigrationConfig,
    system: LeverageSystem,
    tau: f64,
    sigma: f64,
    base_amount: u64,
) -> Result<u16, ProgramError> {
    match system {
        LeverageSystem::Legacy => {
            // Legacy calculation based on tau
            let base_leverage = config.legacy_max_leverage;
            let tau_factor = (1.0 + tau * 10.0).min(1.5);
            Ok((base_leverage as f64 * tau_factor) as u16)
        }
        LeverageSystem::Fused => {
            // Fused calculation using sigma and scalar
            let cap_fused = 20.0;
            let cap_vault = 30.0;
            let base_risk = 0.25;
            
            // Simplified scalar formula
            let scalar = (1.0 / sigma.max(0.01)) * (cap_fused * cap_vault / base_risk);
            let capped_scalar = scalar.min(1000.0);
            
            // Apply to base leverage
            let effective = (config.fused_max_leverage as f64 * capped_scalar / 100.0);
            Ok(effective.min(500.0) as u16) // Cap at 500x for flash
        }
    }
}

/// Migration statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct FlashMigrationStats {
    /// Total flash verses created
    pub total_verses: u64,
    
    /// Average leverage with fused
    pub avg_fused_leverage: f64,
    
    /// Average leverage with legacy
    pub avg_legacy_leverage: f64,
    
    /// Fused system errors
    pub fused_errors: u32,
    
    /// Legacy system errors
    pub legacy_errors: u32,
    
    /// Fallback triggers
    pub fallback_triggers: u32,
    
    /// Last stats update
    pub last_update: u64,
}

impl FlashMigrationStats {
    pub fn new() -> Self {
        Self {
            total_verses: 0,
            avg_fused_leverage: 0.0,
            avg_legacy_leverage: 0.0,
            fused_errors: 0,
            legacy_errors: 0,
            fallback_triggers: 0,
            last_update: 0,
        }
    }
    
    pub fn record_verse(&mut self, system: LeverageSystem, leverage: u16) {
        self.total_verses += 1;
        
        match system {
            LeverageSystem::Fused => {
                let n = self.total_verses as f64;
                self.avg_fused_leverage = 
                    ((n - 1.0) * self.avg_fused_leverage + leverage as f64) / n;
            }
            LeverageSystem::Legacy => {
                let n = self.total_verses as f64;
                self.avg_legacy_leverage = 
                    ((n - 1.0) * self.avg_legacy_leverage + leverage as f64) / n;
            }
        }
    }
    
    pub fn record_error(&mut self, system: LeverageSystem) {
        match system {
            LeverageSystem::Fused => self.fused_errors += 1,
            LeverageSystem::Legacy => self.legacy_errors += 1,
        }
    }
    
    pub fn record_fallback(&mut self) {
        self.fallback_triggers += 1;
    }
}