//! Minimum Viable Vault Implementation
//!
//! Manages the $10k minimum viable vault size requirement and the
//! transition from bootstrap phase to full operational status.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
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
    events::{emit_event, EventType},
    integration::{
        bootstrap_coordinator::BootstrapCoordinator,
        bootstrap_vault_initialization::BootstrapVaultState,
    },
    constants::BOOTSTRAP_TARGET_VAULT,
};

/// Minimum viable vault constants
pub const MINIMUM_VIABLE_VAULT_SIZE: u64 = 10_000_000_000; // $10k with 6 decimals
pub const LEVERAGE_UNLOCK_THRESHOLD: u64 = 1_000_000_000; // $1k to unlock any leverage
pub const MAX_LEVERAGE: u8 = 10; // Maximum 10x leverage
pub const VIABILITY_CHECK_INTERVAL_SLOTS: u64 = 150; // Check every ~60 seconds

/// Vault viability states
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum VaultViabilityState {
    /// Bootstrap phase - building towards minimum
    Bootstrap,
    /// Approaching viability (90%+ of target)
    NearingViability,
    /// Minimum viable - basic operations enabled
    MinimumViable,
    /// Fully operational - all features enabled
    FullyOperational,
    /// Degraded - fell below minimum after being viable
    Degraded,
}

/// Vault viability tracker
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VaultViabilityTracker {
    /// Current viability state
    pub state: VaultViabilityState,
    /// Current vault balance
    pub current_balance: u64,
    /// Minimum required balance
    pub minimum_required: u64,
    /// Timestamp when minimum was first reached
    pub viability_reached_at: Option<i64>,
    /// Number of times degraded below minimum
    pub degradation_count: u32,
    /// Last viability check slot
    pub last_check_slot: u64,
    /// Features currently enabled
    pub enabled_features: EnabledFeatures,
}

/// Features that can be enabled based on vault size
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct EnabledFeatures {
    /// Basic trading enabled
    pub trading_enabled: bool,
    /// Leverage trading enabled
    pub leverage_enabled: bool,
    /// Maximum leverage available
    pub max_leverage: u8,
    /// Liquidations enabled
    pub liquidations_enabled: bool,
    /// Chain positions enabled
    pub chain_positions_enabled: bool,
    /// Advanced orders enabled
    pub advanced_orders_enabled: bool,
    /// Fee distribution enabled
    pub fee_distribution_enabled: bool,
}

impl Default for EnabledFeatures {
    fn default() -> Self {
        Self {
            trading_enabled: false,
            leverage_enabled: false,
            max_leverage: 0,
            liquidations_enabled: false,
            chain_positions_enabled: false,
            advanced_orders_enabled: false,
            fee_distribution_enabled: false,
        }
    }
}

/// Check and update vault viability status
pub fn check_vault_viability(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let vault_account = next_account_info(account_info_iter)?;
    let viability_tracker_account = next_account_info(account_info_iter)?;
    let bootstrap_coordinator_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    
    let clock = Clock::get()?;
    
    // Load accounts
    let vault = BootstrapVaultState::deserialize(&mut &vault_account.data.borrow()[..])?;
    let mut tracker = VaultViabilityTracker::deserialize(&mut &viability_tracker_account.data.borrow()[..])?;
    let mut bootstrap = BootstrapCoordinator::deserialize(&mut &bootstrap_coordinator_account.data.borrow()[..])?;
    
    // Check if it's time for a viability check
    if clock.slot < tracker.last_check_slot + VIABILITY_CHECK_INTERVAL_SLOTS {
        return Ok(()); // Not time yet
    }
    
    // Update tracker
    tracker.current_balance = vault.total_deposits;
    tracker.last_check_slot = clock.slot;
    
    // Determine new state
    let previous_state = tracker.state.clone();
    let new_state = determine_viability_state(
        vault.total_deposits,
        &tracker,
        vault.is_bootstrap_phase,
    );
    
    // Handle state transitions
    if previous_state != new_state {
        handle_state_transition(
            &previous_state,
            &new_state,
            &mut tracker,
            &mut bootstrap,
            vault.total_deposits,
            clock.unix_timestamp,
        )?;
    }
    
    // Update enabled features based on new state
    update_enabled_features(&mut tracker, vault.total_deposits);
    
    // Save updated states
    tracker.serialize(&mut &mut viability_tracker_account.data.borrow_mut()[..])?;
    bootstrap.serialize(&mut &mut bootstrap_coordinator_account.data.borrow_mut()[..])?;
    
    // Emit status event
    emit_event(EventType::VaultViabilityChecked, &VaultViabilityCheckedEvent {
        state: new_state as u8,
        current_balance: vault.total_deposits,
        minimum_required: MINIMUM_VIABLE_VAULT_SIZE,
        enabled_features_bitmap: encode_features_bitmap(&tracker.enabled_features),
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}

/// Determine vault viability state based on balance
fn determine_viability_state(
    balance: u64,
    tracker: &VaultViabilityTracker,
    is_bootstrap: bool,
) -> VaultViabilityState {
    let minimum = MINIMUM_VIABLE_VAULT_SIZE;
    let ninety_percent = (minimum * 9) / 10;
    
    match (balance, tracker.viability_reached_at.is_some()) {
        // Never been viable before
        (b, false) if b < ninety_percent => VaultViabilityState::Bootstrap,
        (b, false) if b < minimum => VaultViabilityState::NearingViability,
        (b, false) if b >= minimum => VaultViabilityState::MinimumViable,
        
        // Has been viable before
        (b, true) if b < minimum => VaultViabilityState::Degraded,
        (b, true) if b < minimum * 2 => VaultViabilityState::MinimumViable,
        (_, true) => VaultViabilityState::FullyOperational,
        
        _ => VaultViabilityState::Bootstrap,
    }
}

/// Handle state transitions
fn handle_state_transition(
    from: &VaultViabilityState,
    to: &VaultViabilityState,
    tracker: &mut VaultViabilityTracker,
    bootstrap: &mut BootstrapCoordinator,
    balance: u64,
    timestamp: i64,
) -> ProgramResult {
    tracker.state = to.clone();
    
    match (from, to) {
        // First time reaching minimum viable
        (VaultViabilityState::Bootstrap | VaultViabilityState::NearingViability, 
         VaultViabilityState::MinimumViable) => {
            tracker.viability_reached_at = Some(timestamp);
            bootstrap.bootstrap_complete = true;
            msg!("🎉 Vault reached minimum viable size of $10k!");
            
            emit_event(EventType::VaultViabilityReached, &VaultViabilityReachedEvent {
                balance,
                timestamp,
                bootstrap_duration_slots: Clock::get()?.slot - bootstrap.bootstrap_start_slot,
            });
        }
        
        // Degraded below minimum
        (VaultViabilityState::MinimumViable | VaultViabilityState::FullyOperational,
         VaultViabilityState::Degraded) => {
            tracker.degradation_count += 1;
            msg!("⚠️ Vault degraded below minimum viable size");
            
            emit_event(EventType::VaultDegraded, &VaultDegradedEvent {
                balance,
                minimum_required: MINIMUM_VIABLE_VAULT_SIZE,
                degradation_count: tracker.degradation_count,
                timestamp,
            });
        }
        
        // Recovered from degradation
        (VaultViabilityState::Degraded,
         VaultViabilityState::MinimumViable | VaultViabilityState::FullyOperational) => {
            msg!("✅ Vault recovered to viable state");
            
            emit_event(EventType::VaultRecovered, &VaultRecoveredEvent {
                balance,
                timestamp,
            });
        }
        
        // Approaching viability
        (VaultViabilityState::Bootstrap,
         VaultViabilityState::NearingViability) => {
            msg!("📈 Vault approaching minimum viable size (90%+)");
            
            emit_event(EventType::VaultNearingViability, &VaultNearingViabilityEvent {
                balance,
                target: MINIMUM_VIABLE_VAULT_SIZE,
                percent_complete: (balance * 100) / MINIMUM_VIABLE_VAULT_SIZE,
            });
        }
        
        _ => {} // Other transitions don't need special handling
    }
    
    Ok(())
}

/// Update enabled features based on vault balance and state
fn update_enabled_features(tracker: &mut VaultViabilityTracker, balance: u64) {
    let features = &mut tracker.enabled_features;
    
    match tracker.state {
        VaultViabilityState::Bootstrap => {
            // Minimal features during bootstrap
            features.trading_enabled = false;
            features.leverage_enabled = false;
            features.max_leverage = 0;
            features.liquidations_enabled = false;
            features.chain_positions_enabled = false;
            features.advanced_orders_enabled = false;
            features.fee_distribution_enabled = false;
        }
        
        VaultViabilityState::NearingViability => {
            // Enable basic features as we approach viability
            features.trading_enabled = balance >= LEVERAGE_UNLOCK_THRESHOLD;
            features.leverage_enabled = false;
            features.max_leverage = 0;
            features.liquidations_enabled = false;
            features.chain_positions_enabled = false;
            features.advanced_orders_enabled = false;
            features.fee_distribution_enabled = false;
        }
        
        VaultViabilityState::MinimumViable => {
            // Core features enabled
            features.trading_enabled = true;
            features.leverage_enabled = true;
            features.max_leverage = calculate_max_leverage(balance);
            features.liquidations_enabled = true;
            features.chain_positions_enabled = false; // Not yet
            features.advanced_orders_enabled = false; // Not yet
            features.fee_distribution_enabled = true;
        }
        
        VaultViabilityState::FullyOperational => {
            // All features enabled
            features.trading_enabled = true;
            features.leverage_enabled = true;
            features.max_leverage = MAX_LEVERAGE;
            features.liquidations_enabled = true;
            features.chain_positions_enabled = true;
            features.advanced_orders_enabled = true;
            features.fee_distribution_enabled = true;
        }
        
        VaultViabilityState::Degraded => {
            // Limited features in degraded state
            features.trading_enabled = true; // Allow closing positions
            features.leverage_enabled = false; // No new leveraged positions
            features.max_leverage = 0;
            features.liquidations_enabled = true; // Still need liquidations
            features.chain_positions_enabled = false;
            features.advanced_orders_enabled = false;
            features.fee_distribution_enabled = true;
        }
    }
}

/// Calculate maximum leverage based on vault balance
fn calculate_max_leverage(balance: u64) -> u8 {
    if balance < LEVERAGE_UNLOCK_THRESHOLD {
        0
    } else if balance < MINIMUM_VIABLE_VAULT_SIZE {
        // Linear scaling from 1x to 10x between $1k and $10k
        let progress = balance.saturating_sub(LEVERAGE_UNLOCK_THRESHOLD);
        let range = MINIMUM_VIABLE_VAULT_SIZE.saturating_sub(LEVERAGE_UNLOCK_THRESHOLD);
        let leverage = 1 + ((progress * 9) / range);
        leverage.min(MAX_LEVERAGE as u64) as u8
    } else {
        MAX_LEVERAGE
    }
}

/// Initialize viability tracker
pub fn initialize_viability_tracker(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let viability_tracker_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let rent_sysvar = next_account_info(account_info_iter)?;
    
    // Verify authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Initialize tracker
    let tracker = VaultViabilityTracker {
        state: VaultViabilityState::Bootstrap,
        current_balance: 0,
        minimum_required: MINIMUM_VIABLE_VAULT_SIZE,
        viability_reached_at: None,
        degradation_count: 0,
        last_check_slot: 0,
        enabled_features: EnabledFeatures::default(),
    };
    
    // Save state
    tracker.serialize(&mut &mut viability_tracker_account.data.borrow_mut()[..])?;
    
    msg!("Vault viability tracker initialized");
    msg!("Minimum viable size: ${}", MINIMUM_VIABLE_VAULT_SIZE / 1_000_000);
    
    Ok(())
}

/// Check if a specific feature is enabled
pub fn is_feature_enabled(
    vault_account: &AccountInfo,
    feature: VaultFeature,
) -> Result<bool, ProgramError> {
    let tracker = VaultViabilityTracker::deserialize(&mut &vault_account.data.borrow()[..])?;
    
    let enabled = match feature {
        VaultFeature::Trading => tracker.enabled_features.trading_enabled,
        VaultFeature::Leverage => tracker.enabled_features.leverage_enabled,
        VaultFeature::Liquidations => tracker.enabled_features.liquidations_enabled,
        VaultFeature::ChainPositions => tracker.enabled_features.chain_positions_enabled,
        VaultFeature::AdvancedOrders => tracker.enabled_features.advanced_orders_enabled,
        VaultFeature::FeeDistribution => tracker.enabled_features.fee_distribution_enabled,
    };
    
    Ok(enabled)
}

/// Features that can be checked
#[derive(Debug, Clone, Copy)]
pub enum VaultFeature {
    Trading,
    Leverage,
    Liquidations,
    ChainPositions,
    AdvancedOrders,
    FeeDistribution,
}

// Event definitions are in events.rs, import them
use crate::events::{
    VaultViabilityCheckedEvent, VaultViabilityReachedEvent,
    VaultDegradedEvent, VaultRecoveredEvent, VaultNearingViabilityEvent,
};

/// Encode enabled features as a bitmap for events
fn encode_features_bitmap(features: &EnabledFeatures) -> u8 {
    let mut bitmap = 0u8;
    if features.trading_enabled { bitmap |= 1 << 0; }
    if features.leverage_enabled { bitmap |= 1 << 1; }
    if features.liquidations_enabled { bitmap |= 1 << 2; }
    if features.chain_positions_enabled { bitmap |= 1 << 3; }
    if features.advanced_orders_enabled { bitmap |= 1 << 4; }
    if features.fee_distribution_enabled { bitmap |= 1 << 5; }
    bitmap
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_viability_state_determination() {
        let mut tracker = VaultViabilityTracker {
            state: VaultViabilityState::Bootstrap,
            current_balance: 0,
            minimum_required: MINIMUM_VIABLE_VAULT_SIZE,
            viability_reached_at: None,
            degradation_count: 0,
            last_check_slot: 0,
            enabled_features: EnabledFeatures::default(),
        };
        
        // Test bootstrap state
        assert_eq!(
            determine_viability_state(0, &tracker, true),
            VaultViabilityState::Bootstrap
        );
        
        // Test nearing viability (90%)
        assert_eq!(
            determine_viability_state(9_000_000_000, &tracker, true),
            VaultViabilityState::NearingViability
        );
        
        // Test minimum viable
        assert_eq!(
            determine_viability_state(10_000_000_000, &tracker, true),
            VaultViabilityState::MinimumViable
        );
        
        // Test degradation
        tracker.viability_reached_at = Some(1234567890);
        assert_eq!(
            determine_viability_state(8_000_000_000, &tracker, false),
            VaultViabilityState::Degraded
        );
        
        // Test fully operational
        assert_eq!(
            determine_viability_state(25_000_000_000, &tracker, false),
            VaultViabilityState::FullyOperational
        );
    }
    
    #[test]
    fn test_leverage_calculation() {
        // No leverage below $1k
        assert_eq!(calculate_max_leverage(500_000_000), 0);
        
        // 1x at exactly $1k
        assert_eq!(calculate_max_leverage(1_000_000_000), 1);
        
        // Linear scaling
        assert_eq!(calculate_max_leverage(5_500_000_000), 5); // $5.5k ≈ 5x
        
        // Max 10x at $10k+
        assert_eq!(calculate_max_leverage(10_000_000_000), 10);
        assert_eq!(calculate_max_leverage(20_000_000_000), 10);
    }
    
    #[test]
    fn test_feature_enablement() {
        let mut tracker = VaultViabilityTracker {
            state: VaultViabilityState::Bootstrap,
            current_balance: 0,
            minimum_required: MINIMUM_VIABLE_VAULT_SIZE,
            viability_reached_at: None,
            degradation_count: 0,
            last_check_slot: 0,
            enabled_features: EnabledFeatures::default(),
        };
        
        // Bootstrap: no features
        update_enabled_features(&mut tracker, 500_000_000);
        assert!(!tracker.enabled_features.trading_enabled);
        assert_eq!(tracker.enabled_features.max_leverage, 0);
        
        // Minimum viable: core features
        tracker.state = VaultViabilityState::MinimumViable;
        update_enabled_features(&mut tracker, 10_000_000_000);
        assert!(tracker.enabled_features.trading_enabled);
        assert!(tracker.enabled_features.leverage_enabled);
        assert!(tracker.enabled_features.liquidations_enabled);
        assert!(!tracker.enabled_features.chain_positions_enabled);
        
        // Fully operational: all features
        tracker.state = VaultViabilityState::FullyOperational;
        update_enabled_features(&mut tracker, 20_000_000_000);
        assert!(tracker.enabled_features.trading_enabled);
        assert!(tracker.enabled_features.chain_positions_enabled);
        assert!(tracker.enabled_features.advanced_orders_enabled);
        assert_eq!(tracker.enabled_features.max_leverage, 10);
    }
}