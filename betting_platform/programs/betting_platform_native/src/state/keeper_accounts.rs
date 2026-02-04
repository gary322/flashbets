//! Keeper network account structures
//!
//! Account types for the permissionless keeper system

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::account_validation::DISCRIMINATOR_SIZE;

/// Discriminators for keeper account types
pub mod discriminators {
    pub const KEEPER_REGISTRY: [u8; 8] = [89, 167, 23, 45, 201, 156, 78, 234];
    pub const KEEPER_ACCOUNT: [u8; 8] = [156, 78, 234, 89, 167, 23, 45, 201];
    pub const KEEPER_HEALTH: [u8; 8] = [234, 45, 201, 156, 78, 89, 167, 23];
    pub const PERFORMANCE_METRICS: [u8; 8] = [23, 201, 78, 156, 45, 89, 234, 167];
    pub const WEBSOCKET_STATE: [u8; 8] = [167, 89, 45, 23, 234, 201, 78, 156];
    pub const INGESTOR_STATE: [u8; 8] = [78, 156, 234, 201, 45, 23, 167, 89];
}

/// Keeper registry (global keeper state)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct KeeperRegistry {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Total registered keepers
    pub total_keepers: u32,
    
    /// Currently active keepers
    pub active_keepers: u32,
    
    /// Total rewards distributed
    pub total_rewards_distributed: u64,
    
    /// Minimum successful operations required
    pub performance_threshold: u64,
    
    /// Maximum failed operations allowed
    pub slash_threshold: u64,
    
    /// Active liquidation keepers
    pub active_liquidation_keepers: u32,
    
    /// Active order keepers
    pub active_order_keepers: u32,
    
    /// Active ingestor keepers
    pub active_ingestor_keepers: u32,
    
    /// Total MMT staked
    pub total_mmt_staked: u64,
    
    /// Number of slashing events
    pub slashing_events: u32,
}

impl KeeperRegistry {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 4 + 4 + 8 + 8 + 8 + 4 + 4 + 4 + 8 + 4;
    
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::KEEPER_REGISTRY,
            total_keepers: 0,
            active_keepers: 0,
            total_rewards_distributed: 0,
            performance_threshold: 95, // 95% success rate minimum
            slash_threshold: 100,      // Max 100 failed operations
            active_liquidation_keepers: 0,
            active_order_keepers: 0,
            active_ingestor_keepers: 0,
            total_mmt_staked: 0,
            slashing_events: 0,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::KEEPER_REGISTRY {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.active_keepers > self.total_keepers {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Individual keeper account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct KeeperAccount {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Unique keeper identifier
    pub keeper_id: [u8; 32],
    
    /// Keeper authority
    pub authority: Pubkey,
    
    /// Staked MMT for priority
    pub mmt_stake: u64,
    
    /// Performance score (0-10000 = 0-100%)
    pub performance_score: u64,
    
    /// Total operations performed
    pub total_operations: u64,
    
    /// Successful operations
    pub successful_operations: u64,
    
    /// Total rewards earned
    pub total_rewards_earned: u64,
    
    /// Last operation slot
    pub last_operation_slot: u64,
    
    /// Keeper status
    pub status: KeeperStatus,
    
    /// Keeper type
    pub keeper_type: KeeperType,
    
    /// Specializations
    pub specializations: Vec<KeeperSpecialization>,
    
    /// Average response time in slots
    pub average_response_time: u64,
    
    /// Priority score
    pub priority_score: u128,
    
    /// Registration slot
    pub registration_slot: u64,
    
    /// Slashing count
    pub slashing_count: u32,
}

impl KeeperAccount {
    pub const BASE_LEN: usize = DISCRIMINATOR_SIZE + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 4 + 8 + 16 + 8 + 4;
    
    pub fn space(specializations: usize) -> usize {
        Self::BASE_LEN + specializations
    }
    
    pub fn new(keeper_id: [u8; 32], authority: Pubkey, keeper_type: KeeperType) -> Self {
        Self {
            discriminator: discriminators::KEEPER_ACCOUNT,
            keeper_id,
            authority,
            mmt_stake: 0,
            performance_score: 10000, // Start at 100%
            total_operations: 0,
            successful_operations: 0,
            total_rewards_earned: 0,
            last_operation_slot: 0,
            status: KeeperStatus::Active,
            keeper_type,
            specializations: vec![],
            average_response_time: 0,
            priority_score: 0,
            registration_slot: 0,
            slashing_count: 0,
        }
    }
    
    pub fn calculate_priority(&self) -> u64 {
        // Priority = stake * performance_score / 10000
        self.mmt_stake
            .saturating_mul(self.performance_score)
            .saturating_div(10000)
    }
    
    pub fn has_specialization(&self, work_type: &WorkType) -> bool {
        let required_spec = match work_type {
            WorkType::Liquidations => KeeperSpecialization::Liquidations,
            WorkType::StopOrders => KeeperSpecialization::StopLosses,
            WorkType::PriceUpdates => KeeperSpecialization::PriceUpdates,
            WorkType::Resolutions => KeeperSpecialization::MarketResolution,
        };
        
        self.specializations.contains(&required_spec)
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::KEEPER_ACCOUNT {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.successful_operations > self.total_operations {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Keeper status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum KeeperStatus {
    Active,
    Suspended,     // Failed operations exceeded threshold
    Slashed,       // Malicious behavior detected
    Inactive,      // Voluntary pause
    Deactivated,   // Removed from network
}

/// Keeper type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum KeeperType {
    Liquidation,
    Order,
    Ingestor,
    General,
}

/// Keeper specialization
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum KeeperSpecialization {
    Liquidations,
    StopLosses,
    PriceUpdates,
    MarketResolution,
    ChainExecution,
    CircuitBreakers,
}

/// Work type for task assignment
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum WorkType {
    Liquidations,
    StopOrders,
    PriceUpdates,
    Resolutions,
}

/// Keeper health monitoring
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct KeeperHealth {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Total markets being monitored
    pub total_markets: u64,
    
    /// Markets processed in last hour
    pub markets_processed_hour: u64,
    
    /// Total errors in last hour
    pub errors_hour: u64,
    
    /// Average latency in milliseconds
    pub avg_latency_ms: u64,
    
    /// Uptime percentage (0-10000)
    pub uptime_percentage: u16,
    
    /// Last health check slot
    pub last_check_slot: u64,
    
    /// WebSocket connection status
    pub websocket_status: WebSocketHealth,
    
    /// Failed health checks
    pub failed_checks: u32,
}

impl KeeperHealth {
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::KEEPER_HEALTH,
            total_markets: 0,
            markets_processed_hour: 0,
            errors_hour: 0,
            avg_latency_ms: 0,
            uptime_percentage: 10000,
            last_check_slot: 0,
            websocket_status: WebSocketHealth::Healthy,
            failed_checks: 0,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::KEEPER_HEALTH {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// WebSocket health status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum WebSocketHealth {
    Healthy,
    Degraded,
    Failed,
}

/// Performance metrics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct PerformanceMetrics {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Total requests
    pub total_requests: u64,
    
    /// Successful requests
    pub successful_requests: u64,
    
    /// Failed requests
    pub failed_requests: u64,
    
    /// Average latency
    pub avg_latency: u64,
    
    /// P95 latency
    pub p95_latency: u64,
    
    /// P99 latency
    pub p99_latency: u64,
    
    /// Latency samples
    pub latency_samples: Vec<u64>,
    
    /// Last update slot
    pub last_update_slot: u64,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::PERFORMANCE_METRICS,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_latency: 0,
            p95_latency: 0,
            p99_latency: 0,
            latency_samples: Vec::with_capacity(1000),
            last_update_slot: 0,
        }
    }
    
    pub fn update_latencies(&mut self, new_samples: Vec<u64>) {
        // Keep last 1000 samples
        self.latency_samples.extend(new_samples);
        if self.latency_samples.len() > 1000 {
            self.latency_samples.drain(0..self.latency_samples.len() - 1000);
        }
        
        // Calculate percentiles
        if !self.latency_samples.is_empty() {
            let mut sorted = self.latency_samples.clone();
            sorted.sort_unstable();
            
            self.avg_latency = sorted.iter().sum::<u64>() / sorted.len() as u64;
            self.p95_latency = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
            self.p99_latency = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)];
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::PERFORMANCE_METRICS {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// WebSocket state for price feeds
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct WebSocketState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Connection status
    pub status: WebSocketHealth,
    
    /// Markets subscribed
    pub subscribed_markets: u32,
    
    /// Messages received
    pub messages_received: u64,
    
    /// Errors encountered
    pub errors: u32,
    
    /// Reconnection attempts
    pub reconnection_attempts: u32,
}

impl WebSocketState {
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::WEBSOCKET_STATE,
            last_update_slot: 0,
            status: WebSocketHealth::Healthy,
            subscribed_markets: 0,
            messages_received: 0,
            errors: 0,
            reconnection_attempts: 0,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::WEBSOCKET_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Ingestor keeper state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IngestorState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Keeper ID
    pub keeper_id: [u8; 32],
    
    /// Assigned market range
    pub market_range_start: u32,
    pub market_range_end: u32,
    
    /// Last successful batch
    pub last_successful_batch: u64,
    
    /// Error count
    pub error_count: u32,
    
    /// Backoff until timestamp
    pub backoff_until: i64,
    
    /// Total markets ingested
    pub total_ingested: u64,
}

impl IngestorState {
    pub fn new(keeper_id: [u8; 32], market_range_start: u32, market_range_end: u32) -> Self {
        Self {
            discriminator: discriminators::INGESTOR_STATE,
            keeper_id,
            market_range_start,
            market_range_end,
            last_successful_batch: 0,
            error_count: 0,
            backoff_until: 0,
            total_ingested: 0,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::INGESTOR_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.market_range_end < self.market_range_start {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}