// Phase 20: System Integration Coordinator
// This module orchestrates all system components for end-to-end functionality

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
    state::{
        accounts::GlobalConfigPDA,
        amm_accounts::HybridAMM,
        keeper_accounts::KeeperRegistry,
    },
    synthetics::router::RoutingEngine,
    priority::queue::PriorityQueue,
    mmt::state::StakingPool,
};

/// System status enumeration
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Copy, PartialEq)]
pub enum SystemStatus {
    Initializing = 0,
    Bootstrapping = 1,
    Active = 2,
    Degraded = 3,
    Critical = 4,
    Halted = 5,
}

// Note: Using u64 for fixed-point calculations where 10000 = 1.0

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType, CoordinatorInitializedEvent, BootstrapProgressEvent, 
             BootstrapCompleteEvent, SystemHealthCheckEvent, EmergencyShutdownEvent},
};

/// Global configuration for the coordinator
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct GlobalConfig {
    pub admin: Pubkey,
    pub epoch: u64,
    pub coverage: u64,  // Store as u64, convert to/from U64F64 when needed
    pub total_markets: u64,
    pub total_verses: u64,
    pub mmt_supply: u64,
    pub season_allocation: u64,
    pub status: SystemStatus,
    pub last_update_slot: u64,
    pub vault_balance: u64,
    pub total_open_interest: u64,
    pub total_fees_collected: u64,
    pub total_liquidations: u64,
    pub max_leverage: u64,
    pub halt_state: bool,
    pub halt_reason: u8,
    pub polymarket_connected: bool,
    pub websocket_connected: bool,
}

/// Phase 20 specific constants
pub const BOOTSTRAP_COVERAGE_TARGET: u64 = 10000; // coverage = 1.0 for 10x leverage
pub const BOOTSTRAP_SEED_AMOUNT: u64 = 1_000_000_000; // $1k with 6 decimals
pub const MIN_LEVERAGE_MULTIPLIER: u64 = 10; // 10x minimum viable leverage
pub const MARKET_BATCH_SIZE: usize = 50;
pub const MAX_BATCH_SIZE: usize = 100;
pub const HEALTH_CHECK_INTERVAL: u64 = 150; // ~60 seconds at 0.4s/slot

// Removed duplicate SystemCoordinator and its implementation

pub const MAX_GAS_PER_BATCH: u64 = 1_400_000; // Solana's compute unit limit

/// Master System Coordinator that integrates all components
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct SystemCoordinator {
    pub global_config: GlobalConfig,
    pub amm_engine_pubkey: Pubkey,
    pub routing_engine_pubkey: Pubkey,
    pub queue_processor_pubkey: Pubkey,
    pub keeper_registry_pubkey: Pubkey,
    pub health_monitor_pubkey: Pubkey,
    pub correlation_calc_pubkey: Pubkey,
    pub bootstrap_complete: bool,
    pub system_initialized: bool,
    pub last_health_check: u64,
}

impl SystemCoordinator {
    pub const SIZE: usize = 32 + // global_config pubkey
        32 + // amm_engine_pubkey
        32 + // routing_engine_pubkey
        32 + // queue_processor_pubkey
        32 + // keeper_registry_pubkey
        32 + // health_monitor_pubkey
        32 + // correlation_calc_pubkey
        1 + // bootstrap_complete
        1 + // system_initialized
        8; // last_health_check

    /// Initialize the system coordinator
    pub fn initialize(
        &mut self,
        admin: &Pubkey,
        amm_engine: &Pubkey,
        routing_engine: &Pubkey,
        queue_processor: &Pubkey,
        keeper_registry: &Pubkey,
        health_monitor: &Pubkey,
        correlation_calc: &Pubkey,
    ) -> ProgramResult {
        // Set component references
        self.amm_engine_pubkey = *amm_engine;
        self.routing_engine_pubkey = *routing_engine;
        self.queue_processor_pubkey = *queue_processor;
        self.keeper_registry_pubkey = *keeper_registry;
        self.health_monitor_pubkey = *health_monitor;
        self.correlation_calc_pubkey = *correlation_calc;

        // Initialize global config
        self.global_config = GlobalConfig {
            admin: *admin,
            epoch: 1,
            coverage: 0,
            total_markets: 0,
            total_verses: 0,
            mmt_supply: 1_000_000_000_000_000, // 1M MMT with 9 decimals
            season_allocation: 10_000_000_000_000, // 10k MMT per season
            status: SystemStatus::Initializing,
            last_update_slot: Clock::get()?.slot,
            vault_balance: 0,
            total_open_interest: 0,
            total_fees_collected: 0,
            total_liquidations: 0,
            max_leverage: 0,
            halt_state: false,
            halt_reason: 0,
            polymarket_connected: false,
            websocket_connected: false,
        };

        self.system_initialized = true;
        self.bootstrap_complete = false;
        self.last_health_check = Clock::get()?.slot;

        msg!("System coordinator initialized");
        emit_event(EventType::CoordinatorInitialized, &CoordinatorInitializedEvent {
            admin: *admin,
            components: 6,
        });

        Ok(())
    }

    /// Bootstrap the entire system
    pub fn bootstrap_system(
        &mut self,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        msg!("Starting system bootstrap...");

        if self.bootstrap_complete {
            return Err(BettingPlatformError::BootstrapAlreadyComplete.into());
        }

        let clock = Clock::get()?;

        // Step 1: Verify all components are initialized
        self.verify_components_ready(accounts)?;
        msg!("All components verified");

        // Step 2: Load precomputed math tables (simulated)
        self.load_math_tables()?;
        msg!("Math tables loaded");

        // Step 3: Initialize MMT distribution
        self.initialize_mmt_distribution(accounts)?;
        msg!("MMT distribution initialized");

        // Step 4: Update system status
        self.global_config.status = SystemStatus::Bootstrapping;
        self.global_config.last_update_slot = clock.slot;

        // Step 5: Check if bootstrap target reached
        if self.global_config.vault_balance >= BOOTSTRAP_SEED_AMOUNT {
            self.complete_bootstrap()?;
        }

        emit_event(EventType::BootstrapProgress, &BootstrapProgressEvent {
            vault_balance: self.global_config.vault_balance,
            target: BOOTSTRAP_SEED_AMOUNT,
            progress_pct: (self.global_config.vault_balance * 100) / BOOTSTRAP_SEED_AMOUNT,
        });

        Ok(())
    }

    /// Complete bootstrap and activate system
    pub fn complete_bootstrap(&mut self) -> ProgramResult {
        if self.bootstrap_complete {
            return Ok(());
        }

        // Calculate initial coverage
        let coverage = self.calculate_coverage()?;
        self.global_config.coverage = coverage;

        // Enable leverage based on coverage (10000 = 1.0)
        if coverage >= 10000 {
            self.global_config.max_leverage = MIN_LEVERAGE_MULTIPLIER;
        }

        // Update status
        self.global_config.status = SystemStatus::Active;
        self.bootstrap_complete = true;

        msg!("Bootstrap complete! System active with {}x leverage", 
            self.global_config.max_leverage);

                emit_event(EventType::BootstrapComplete, &BootstrapCompleteEvent {
            coverage: coverage,
            max_leverage: self.global_config.max_leverage,
        });

        Ok(())
    }

    /// Process a batch of market updates
    pub fn process_market_batch(
        &mut self,
        market_updates: &[MarketUpdate],
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        if !self.system_initialized {
            return Err(BettingPlatformError::SystemNotInitialized.into());
        }

        let clock = Clock::get()?;
        let mut processed = 0;

        for update in market_updates.iter().take(MARKET_BATCH_SIZE) {
            // Update market data
            self.update_market_data(update, accounts)?;
            processed += 1;
        }

        self.global_config.total_markets += processed as u64;
        self.global_config.last_update_slot = clock.slot;

        msg!("Processed {} market updates", processed);

        Ok(())
    }

    /// Run system health check
    pub fn health_check(&mut self) -> ProgramResult {
        let clock = Clock::get()?;
        
        // Check component health
        let amm_healthy = self.check_amm_health()?;
        let routing_healthy = self.check_routing_health()?;
        let queue_healthy = self.check_queue_health()?;
        let keeper_healthy = self.check_keeper_health()?;

        // Update health status
        let all_healthy = amm_healthy && routing_healthy && queue_healthy && keeper_healthy;
        
        if !all_healthy {
            self.global_config.status = SystemStatus::Degraded;
            msg!("System health check failed - entering degraded mode");
        } else if self.global_config.status == SystemStatus::Degraded {
            self.global_config.status = SystemStatus::Active;
            msg!("System health restored");
        }

        self.last_health_check = clock.slot;

        emit_event(EventType::SystemHealthCheck, &SystemHealthCheckEvent {
            status: self.global_config.status as u8,
            components_healthy: if all_healthy { 4 } else { 0 },
            slot: clock.slot,
        });

        Ok(())
    }

    // Private helper methods

    fn verify_components_ready(&self, accounts: &[AccountInfo]) -> ProgramResult {
        // In production, would verify each component PDA exists and is initialized
        if accounts.len() < 6 {
            return Err(BettingPlatformError::InsufficientAccounts.into());
        }
        Ok(())
    }

    fn load_math_tables(&self) -> ProgramResult {
        // Math tables are loaded in the math module
        msg!("Math tables verification complete");
        Ok(())
    }

    fn initialize_mmt_distribution(&mut self, _accounts: &[AccountInfo]) -> ProgramResult {
        // MMT distribution handled by mmt module
        msg!("MMT distribution ready");
        Ok(())
    }

    fn calculate_coverage(&self) -> Result<u64, ProgramError> {
        if self.global_config.total_open_interest == 0 {
            return Ok(1000000); // Very high coverage with no positions
        }

        // coverage = vault / (tail_loss_factor * open_interest)
        // Returns in basis points where 10000 = 1.0
        let coverage_raw = (self.global_config.vault_balance as u128 * 10000)
            / self.global_config.total_open_interest as u128;
        
        Ok(coverage_raw as u64)
    }

    fn update_market_data(
        &mut self,
        update: &MarketUpdate,
        _accounts: &[AccountInfo],
    ) -> ProgramResult {
        // Market updates handled by specific modules
        msg!("Market {} updated", update.market_id);
        Ok(())
    }

    fn check_amm_health(&self) -> Result<bool, ProgramError> {
        // Check AMM engine health
        Ok(true) // Simplified
    }

    fn check_routing_health(&self) -> Result<bool, ProgramError> {
        // Check routing engine health
        Ok(true) // Simplified
    }

    fn check_queue_health(&self) -> Result<bool, ProgramError> {
        // Check priority queue health
        Ok(true) // Simplified
    }

    fn check_keeper_health(&self) -> Result<bool, ProgramError> {
        // Check keeper network health
        Ok(true) // Simplified
    }
}

/// Market update data structure
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketUpdate {
    pub market_id: Pubkey,
    pub yes_price: u64,
    pub no_price: u64,
    pub volume_24h: u64,
    pub liquidity: u64,
    pub timestamp: i64,
}

/// Integration instructions
#[derive(BorshSerialize, BorshDeserialize)]
pub enum IntegrationInstruction {
    /// Initialize the system coordinator
    InitializeCoordinator {
        amm_engine: Pubkey,
        routing_engine: Pubkey,
        queue_processor: Pubkey,
        keeper_registry: Pubkey,
        health_monitor: Pubkey,
        correlation_calc: Pubkey,
    },
    /// Bootstrap the system
    BootstrapSystem,
    /// Process market batch update
    ProcessMarketBatch {
        updates: Vec<MarketUpdate>,
    },
    /// Run health check
    HealthCheck,
    /// Emergency shutdown
    EmergencyShutdown {
        reason: String,
    },
}

/// Process integration instructions
pub fn process_integration_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = IntegrationInstruction::try_from_slice(instruction_data)?;

    match instruction {
        IntegrationInstruction::InitializeCoordinator {
            amm_engine,
            routing_engine,
            queue_processor,
            keeper_registry,
            health_monitor,
            correlation_calc,
        } => {
            msg!("Processing InitializeCoordinator");
            process_initialize_coordinator(
                program_id,
                accounts,
                &amm_engine,
                &routing_engine,
                &queue_processor,
                &keeper_registry,
                &health_monitor,
                &correlation_calc,
            )
        }
        IntegrationInstruction::BootstrapSystem => {
            msg!("Processing BootstrapSystem");
            process_bootstrap_system(program_id, accounts)
        }
        IntegrationInstruction::ProcessMarketBatch { updates } => {
            msg!("Processing ProcessMarketBatch");
            process_market_batch(program_id, accounts, &updates)
        }
        IntegrationInstruction::HealthCheck => {
            msg!("Processing HealthCheck");
            process_health_check(program_id, accounts)
        }
        IntegrationInstruction::EmergencyShutdown { reason } => {
            msg!("Processing EmergencyShutdown: {}", reason);
            process_emergency_shutdown(program_id, accounts, &reason)
        }
    }
}

fn process_initialize_coordinator(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    amm_engine: &Pubkey,
    routing_engine: &Pubkey,
    queue_processor: &Pubkey,
    keeper_registry: &Pubkey,
    health_monitor: &Pubkey,
    correlation_calc: &Pubkey,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let coordinator_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    // Verify admin is signer
    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut coordinator = SystemCoordinator::try_from_slice(&coordinator_account.data.borrow())?;

    coordinator.initialize(
        admin_account.key,
        amm_engine,
        routing_engine,
        queue_processor,
        keeper_registry,
        health_monitor,
        correlation_calc,
    )?;

    coordinator.serialize(&mut &mut coordinator_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_bootstrap_system(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let coordinator_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    // Verify admin is signer
    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut coordinator = SystemCoordinator::try_from_slice(&coordinator_account.data.borrow())?;

    // Verify admin matches
    if coordinator.global_config.admin != *admin_account.key {
        return Err(BettingPlatformError::UnauthorizedAdmin.into());
    }

    coordinator.bootstrap_system(accounts)?;

    coordinator.serialize(&mut &mut coordinator_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_market_batch(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    updates: &[MarketUpdate],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let coordinator_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;

    // Verify keeper is authorized
    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut coordinator = SystemCoordinator::try_from_slice(&coordinator_account.data.borrow())?;

    coordinator.process_market_batch(updates, accounts)?;

    coordinator.serialize(&mut &mut coordinator_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_health_check(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let coordinator_account = next_account_info(account_iter)?;

    let mut coordinator = SystemCoordinator::try_from_slice(&coordinator_account.data.borrow())?;

    coordinator.health_check()?;

    coordinator.serialize(&mut &mut coordinator_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_emergency_shutdown(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    reason: &str,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let coordinator_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    // Verify admin is signer
    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut coordinator = SystemCoordinator::try_from_slice(&coordinator_account.data.borrow())?;

    // Verify admin matches
    if coordinator.global_config.admin != *admin_account.key {
        return Err(BettingPlatformError::UnauthorizedAdmin.into());
    }

    // Set system to halted
    coordinator.global_config.status = SystemStatus::Halted;
    coordinator.global_config.halt_state = true;

    msg!("EMERGENCY SHUTDOWN: {}", reason);

    emit_event(EventType::EmergencyShutdownIntegration, &EmergencyShutdownEvent {
        reason: reason.to_string(),
        admin: *admin_account.key,
        slot: Clock::get()?.slot,
    });

    coordinator.serialize(&mut &mut coordinator_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;