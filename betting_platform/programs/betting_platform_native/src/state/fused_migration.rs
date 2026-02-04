//! Fused Leverage System Migration State
//!
//! Migration flags and state for transitioning from coverage-based to oracle-based leverage

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::Clock,
    program_error::ProgramError,
    sysvar::Sysvar,
};

use crate::account_validation::DISCRIMINATOR_SIZE;

/// Discriminator for fused migration state
pub const FUSED_MIGRATION: [u8; 8] = [70, 85, 83, 69, 68, 77, 73, 71]; // "FUSEDMIG"

/// Fused leverage migration flags
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct FusedMigrationFlags {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Whether the fused leverage system is enabled
    pub fused_enabled: bool,
    
    /// Whether to use legacy coverage-based leverage
    pub legacy_enabled: bool,
    
    /// Parallel mode - both systems running
    pub parallel_mode: bool,
    
    /// Oracle-only mode (no fallback)
    pub oracle_only: bool,
    
    /// Migration started slot
    pub migration_start_slot: u64,
    
    /// Migration end slot (when legacy will be disabled)
    pub migration_end_slot: u64,
    
    /// Percentage of orders using fused system (0-100)
    pub fused_percentage: u8,
    
    /// Number of positions migrated
    pub positions_migrated: u64,
    
    /// Number of positions remaining
    pub positions_remaining: u64,
    
    /// Authority that can update migration flags
    pub migration_authority: Pubkey,
    
    /// Emergency pause for fused system
    pub fused_paused: bool,
    
    /// Fallback triggered count
    pub fallback_count: u32,
    
    /// Last fallback slot
    pub last_fallback_slot: u64,
}

impl FusedMigrationFlags {
    pub fn new(authority: Pubkey) -> Self {
        Self {
            discriminator: FUSED_MIGRATION,
            fused_enabled: false,
            legacy_enabled: true,
            parallel_mode: false,
            oracle_only: false,
            migration_start_slot: 0,
            migration_end_slot: 0,
            fused_percentage: 0,
            positions_migrated: 0,
            positions_remaining: 0,
            migration_authority: authority,
            fused_paused: false,
            fallback_count: 0,
            last_fallback_slot: 0,
        }
    }
    
    /// Check if should use fused system for this order
    pub fn should_use_fused(&self, random_seed: u8) -> bool {
        if self.fused_paused || !self.fused_enabled {
            return false;
        }
        
        if self.oracle_only {
            return true;
        }
        
        if self.parallel_mode {
            // Use random seed to determine which system
            let threshold = (self.fused_percentage as u32 * 255) / 100;
            return random_seed as u32 <= threshold;
        }
        
        self.fused_enabled && !self.legacy_enabled
    }
    
    /// Check if should fallback to legacy
    pub fn should_fallback(&self) -> bool {
        self.legacy_enabled && !self.oracle_only
    }
    
    /// Update migration progress
    pub fn update_progress(&mut self, migrated: u64, remaining: u64) {
        self.positions_migrated = migrated;
        self.positions_remaining = remaining;
    }
    
    /// Trigger fallback
    pub fn trigger_fallback(&mut self, slot: u64) {
        self.fallback_count += 1;
        self.last_fallback_slot = slot;
    }
    
    /// Start migration
    pub fn start_migration(&mut self, slot: u64, duration_slots: u64) {
        self.migration_start_slot = slot;
        self.migration_end_slot = slot + duration_slots;
        self.parallel_mode = true;
        self.fused_enabled = true;
        self.legacy_enabled = true;
        self.fused_percentage = 10; // Start with 10% traffic
    }
    
    /// Increase fused percentage
    pub fn increase_fused_percentage(&mut self, increment: u8) {
        self.fused_percentage = (self.fused_percentage + increment).min(100);
    }
    
    /// Complete migration
    pub fn complete_migration(&mut self) {
        self.parallel_mode = false;
        self.oracle_only = true;
        self.legacy_enabled = false;
        self.fused_percentage = 100;
    }
    
    /// Emergency pause fused system
    pub fn emergency_pause(&mut self) {
        self.fused_paused = true;
        self.oracle_only = false;
        self.legacy_enabled = true;
    }
    
    /// Resume fused system
    pub fn resume_fused(&mut self) {
        self.fused_paused = false;
    }
    
    /// Validate migration state
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != FUSED_MIGRATION {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Can't have both disabled
        if !self.fused_enabled && !self.legacy_enabled {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Oracle-only implies fused enabled
        if self.oracle_only && !self.fused_enabled {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Oracle-only implies legacy disabled
        if self.oracle_only && self.legacy_enabled {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Migration statistics for monitoring
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationStats {
    /// Orders processed with fused
    pub fused_orders: u64,
    
    /// Orders processed with legacy
    pub legacy_orders: u64,
    
    /// Average leverage with fused
    pub avg_fused_leverage: f64,
    
    /// Average leverage with legacy
    pub avg_legacy_leverage: f64,
    
    /// Errors in fused system
    pub fused_errors: u32,
    
    /// Errors in legacy system
    pub legacy_errors: u32,
    
    /// Last update slot
    pub last_update_slot: u64,
}

impl MigrationStats {
    pub fn new() -> Self {
        Self {
            fused_orders: 0,
            legacy_orders: 0,
            avg_fused_leverage: 0.0,
            avg_legacy_leverage: 0.0,
            fused_errors: 0,
            legacy_errors: 0,
            last_update_slot: 0,
        }
    }
    
    pub fn record_fused_order(&mut self, leverage: f64) {
        self.fused_orders += 1;
        // Update running average
        let n = self.fused_orders as f64;
        self.avg_fused_leverage = ((n - 1.0) * self.avg_fused_leverage + leverage) / n;
    }
    
    pub fn record_legacy_order(&mut self, leverage: f64) {
        self.legacy_orders += 1;
        // Update running average
        let n = self.legacy_orders as f64;
        self.avg_legacy_leverage = ((n - 1.0) * self.avg_legacy_leverage + leverage) / n;
    }
    
    pub fn record_fused_error(&mut self) {
        self.fused_errors += 1;
    }
    
    pub fn record_legacy_error(&mut self) {
        self.legacy_errors += 1;
    }
    
    pub fn update(&mut self, slot: u64) {
        self.last_update_slot = slot;
    }
}