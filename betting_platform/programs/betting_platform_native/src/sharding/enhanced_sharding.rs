// Enhanced Sharding System - 4 shards per market for 4000+ TPS
// Each market gets dedicated sharding for parallel execution

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
    state::accounts::ProposalPDA,
};

/// Number of shards per market (as per specification)
pub const SHARDS_PER_MARKET: u8 = 4;

/// Target TPS per shard (increased for 5k+ total TPS)
pub const TARGET_TPS_PER_SHARD: u32 = 1250; // 1250 * 4 shards = 5000 TPS

/// Maximum markets per global shard
pub const MAX_MARKETS_PER_GLOBAL_SHARD: u32 = 100;

/// Shard types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ShardType {
    OrderBook,      // Handles order placement/cancellation
    Execution,      // Handles trade execution
    Settlement,     // Handles settlement and payouts
    Analytics,      // Handles analytics and aggregation
}

/// Market shard allocation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MarketShardAllocation {
    pub market_id: Pubkey,
    pub shard_assignments: [ShardAssignment; SHARDS_PER_MARKET as usize],
    pub creation_slot: u64,
    pub total_transactions: u64,
    pub peak_tps: u32,
}

impl MarketShardAllocation {
    pub const SIZE: usize = 32 + // market_id
        (SHARDS_PER_MARKET as usize * ShardAssignment::SIZE) + // shard_assignments
        8 + // creation_slot
        8 + // total_transactions
        4;  // peak_tps
    
    pub fn new(market_id: Pubkey, base_shard_id: u32) -> Self {
        let mut shard_assignments = [ShardAssignment::default(); SHARDS_PER_MARKET as usize];
        
        // Assign 4 shards per market with different types
        for (i, shard_type) in [
            ShardType::OrderBook,
            ShardType::Execution,
            ShardType::Settlement,
            ShardType::Analytics,
        ].iter().enumerate() {
            shard_assignments[i] = ShardAssignment {
                shard_id: base_shard_id + i as u32,
                shard_type: *shard_type,
                load_factor: 0,
                last_update_slot: 0,
            };
        }
        
        Self {
            market_id,
            shard_assignments,
            creation_slot: Clock::get().unwrap_or_default().slot,
            total_transactions: 0,
            peak_tps: 0,
        }
    }
    
    /// Get shard for specific operation type
    pub fn get_shard_for_operation(&self, operation: OperationType) -> &ShardAssignment {
        let shard_type = match operation {
            OperationType::PlaceOrder | OperationType::CancelOrder => ShardType::OrderBook,
            OperationType::ExecuteTrade => ShardType::Execution,
            OperationType::Settle | OperationType::ClaimPayout => ShardType::Settlement,
            OperationType::UpdateStats | OperationType::ReadData => ShardType::Analytics,
        };
        
        self.shard_assignments.iter()
            .find(|s| s.shard_type == shard_type)
            .unwrap_or(&self.shard_assignments[0])
    }
}

/// Individual shard assignment
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct ShardAssignment {
    pub shard_id: u32,
    pub shard_type: ShardType,
    pub load_factor: u16,         // 0-10000 basis points
    pub last_update_slot: u64,
}

impl ShardAssignment {
    pub const SIZE: usize = 4 + 1 + 2 + 8; // 15 bytes
}

impl Default for ShardAssignment {
    fn default() -> Self {
        Self {
            shard_id: 0,
            shard_type: ShardType::OrderBook,
            load_factor: 0,
            last_update_slot: 0,
        }
    }
}

/// Operation types for shard routing
#[derive(Debug, Clone, Copy)]
pub enum OperationType {
    PlaceOrder,
    CancelOrder,
    ExecuteTrade,
    Settle,
    ClaimPayout,
    UpdateStats,
    ReadData,
}

/// Enhanced shard manager with per-market sharding
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EnhancedShardManager {
    pub authority: Pubkey,
    pub total_shards: u32,
    pub markets_count: u32,
    pub global_tps: u32,
    pub last_rebalance_slot: u64,
    pub shard_allocations: Vec<MarketShardAllocation>,
}

impl EnhancedShardManager {
    pub const SIZE: usize = 32 + // authority
        4 + // total_shards
        4 + // markets_count
        4 + // global_tps
        8 + // last_rebalance_slot
        4 + // vec length
        (1000 * MarketShardAllocation::SIZE); // Up to 1000 markets
    
    pub fn new(authority: Pubkey) -> Self {
        Self {
            authority,
            total_shards: 0,
            markets_count: 0,
            global_tps: 0,
            last_rebalance_slot: 0,
            shard_allocations: Vec::new(),
        }
    }
    
    /// Allocate shards for a new market
    pub fn allocate_market_shards(&mut self, market_id: &Pubkey) -> Result<u32, ProgramError> {
        // Check if market already has shards
        if self.shard_allocations.iter().any(|a| &a.market_id == market_id) {
            return Err(BettingPlatformError::MarketAlreadySharded.into());
        }
        
        // Allocate 4 new shards for this market
        let base_shard_id = self.total_shards;
        let allocation = MarketShardAllocation::new(*market_id, base_shard_id);
        
        self.shard_allocations.push(allocation);
        self.total_shards += SHARDS_PER_MARKET as u32;
        self.markets_count += 1;
        
        msg!("Allocated shards {}-{} for market {}", 
            base_shard_id, 
            base_shard_id + SHARDS_PER_MARKET as u32 - 1,
            market_id
        );
        
        Ok(base_shard_id)
    }
    
    /// Route operation to appropriate shard
    pub fn route_operation(
        &self,
        market_id: &Pubkey,
        operation: OperationType,
    ) -> Result<u32, ProgramError> {
        let allocation = self.shard_allocations.iter()
            .find(|a| &a.market_id == market_id)
            .ok_or(BettingPlatformError::MarketNotSharded)?;
        
        let shard = allocation.get_shard_for_operation(operation);
        
        msg!("Routing {:?} for market {} to shard {} (type: {:?})",
            operation, market_id, shard.shard_id, shard.shard_type
        );
        
        Ok(shard.shard_id)
    }
    
    /// Apply tau decay to reduce contention (spec: tau decay reduces contention)
    pub fn apply_tau_decay(&mut self, current_slot: u64) {
        const TAU_DECAY_RATE: u16 = 9900; // 0.99 decay factor (99% retention)
        const DECAY_INTERVAL_SLOTS: u64 = 100; // Apply decay every ~40 seconds
        
        // Only apply decay periodically
        if current_slot < self.last_rebalance_slot + DECAY_INTERVAL_SLOTS {
            return;
        }
        
        // Apply exponential decay to all shard load factors
        for allocation in &mut self.shard_allocations {
            for shard in &mut allocation.shard_assignments {
                // Apply tau decay: new_load = old_load * decay_rate / 10000
                shard.load_factor = ((shard.load_factor as u32 * TAU_DECAY_RATE as u32) / 10000) as u16;
            }
        }
        
        msg!("Applied tau decay at slot {}, reducing contention", current_slot);
    }
    
    /// Update shard metrics
    pub fn update_shard_metrics(
        &mut self,
        market_id: &Pubkey,
        shard_type: ShardType,
        transactions: u32,
        current_slot: u64,
    ) -> ProgramResult {
        let allocation = self.shard_allocations.iter_mut()
            .find(|a| &a.market_id == market_id)
            .ok_or(BettingPlatformError::MarketNotSharded)?;
        
        allocation.total_transactions += transactions as u64;
        
        // Update specific shard metrics
        if let Some(shard) = allocation.shard_assignments.iter_mut()
            .find(|s| s.shard_type == shard_type) {
            
            // Calculate load factor based on TPS
            let slot_diff = current_slot.saturating_sub(shard.last_update_slot).max(1);
            let tps = (transactions as u64 * 2) / slot_diff; // ~0.5s per slot
            
            shard.load_factor = ((tps * 10000) / TARGET_TPS_PER_SHARD as u64).min(10000) as u16;
            shard.last_update_slot = current_slot;
            
            // Update peak TPS
            if tps > allocation.peak_tps as u64 {
                allocation.peak_tps = tps as u32;
            }
        }
        
        // Update global TPS
        self.global_tps = self.calculate_global_tps();
        
        // Apply tau decay to reduce contention
        self.apply_tau_decay(current_slot);
        
        Ok(())
    }
    
    /// Calculate global TPS across all shards
    fn calculate_global_tps(&self) -> u32 {
        let mut total_tps = 0u32;
        
        for allocation in &self.shard_allocations {
            for shard in &allocation.shard_assignments {
                let shard_tps = (shard.load_factor as u32 * TARGET_TPS_PER_SHARD) / 10000;
                total_tps += shard_tps;
            }
        }
        
        total_tps
    }
    
    /// Check if system is meeting 5000+ TPS target (spec: 5k+ TPS)
    pub fn is_meeting_tps_target(&self) -> bool {
        self.global_tps >= 5000
    }
    
    /// Get shard statistics for monitoring
    pub fn get_shard_stats(&self) -> ShardStatistics {
        let total_load: u32 = self.shard_allocations.iter()
            .flat_map(|a| &a.shard_assignments)
            .map(|s| s.load_factor as u32)
            .sum();
        
        let avg_load = if self.total_shards > 0 {
            total_load / self.total_shards
        } else {
            0
        };
        
        let hottest_shard = self.shard_allocations.iter()
            .flat_map(|a| &a.shard_assignments)
            .max_by_key(|s| s.load_factor)
            .copied();
        
        ShardStatistics {
            total_shards: self.total_shards,
            active_markets: self.markets_count,
            global_tps: self.global_tps,
            average_load_factor: avg_load as u16,
            hottest_shard,
            meeting_target: self.is_meeting_tps_target(),
        }
    }
    
    /// Rebalance shards if needed (for hot markets)
    pub fn rebalance_if_needed(&mut self, current_slot: u64) -> Result<bool, ProgramError> {
        // Only rebalance every 1000 slots (~400 seconds)
        if current_slot < self.last_rebalance_slot + 1000 {
            return Ok(false);
        }
        
        let mut rebalanced = false;
        let mut migration_plan = Vec::new();
        
        // Find overloaded shards (>90% load)
        for allocation in &self.shard_allocations {
            for (shard_idx, shard) in allocation.shard_assignments.iter().enumerate() {
                if shard.load_factor > 9000 {
                    // Find least loaded shard of same type
                    let mut target_shard_id = None;
                    let mut min_load = u16::MAX;
                    
                    for other_alloc in &self.shard_allocations {
                        for other_shard in &other_alloc.shard_assignments {
                            if other_shard.shard_type == shard.shard_type 
                                && other_shard.load_factor < min_load 
                                && other_shard.shard_id != shard.shard_id {
                                min_load = other_shard.load_factor;
                                target_shard_id = Some(other_shard.shard_id);
                            }
                        }
                    }
                    
                    if let Some(target_id) = target_shard_id {
                        migration_plan.push((
                            allocation.market_id,
                            shard.shard_id,
                            target_id,
                            shard.shard_type,
                            shard.load_factor - 5000, // Migrate 50% of load
                        ));
                    }
                }
            }
        }
        
        // Execute migration plan
        for (market_id, source_shard, target_shard, shard_type, load_to_migrate) in migration_plan {
            msg!("Migrating load from shard {} to {} for market {}",
                source_shard, target_shard, market_id);
            
            // Update source shard
            if let Some(alloc) = self.shard_allocations.iter_mut()
                .find(|a| a.market_id == market_id) {
                if let Some(shard) = alloc.shard_assignments.iter_mut()
                    .find(|s| s.shard_id == source_shard) {
                    shard.load_factor = shard.load_factor.saturating_sub(load_to_migrate);
                    rebalanced = true;
                }
            }
            
            // Update target shard
            for alloc in &mut self.shard_allocations {
                if let Some(shard) = alloc.shard_assignments.iter_mut()
                    .find(|s| s.shard_id == target_shard) {
                    shard.load_factor = shard.load_factor.saturating_add(load_to_migrate / 2);
                    shard.last_update_slot = current_slot;
                }
            }
            
            // Emit rebalance event
            msg!("Rebalanced: {} load moved from shard {} to {}",
                load_to_migrate, source_shard, target_shard);
        }
        
        if rebalanced {
            self.last_rebalance_slot = current_slot;
            
            // Verify no shard is still overloaded
            let still_overloaded = self.shard_allocations.iter()
                .any(|a| a.shard_assignments.iter().any(|s| s.load_factor > 9000));
            
            if still_overloaded {
                msg!("Warning: Some shards still overloaded after rebalance");
            }
        }
        
        Ok(rebalanced)
    }
}

/// Shard statistics for monitoring
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ShardStatistics {
    pub total_shards: u32,
    pub active_markets: u32,
    pub global_tps: u32,
    pub average_load_factor: u16,
    pub hottest_shard: Option<ShardAssignment>,
    pub meeting_target: bool,
}

/// Result of parallel shard read operation
#[derive(Debug, Clone)]
pub struct ShardReadResult {
    pub market_id: Pubkey,
    pub shard_data: Vec<ShardData>,
    pub read_latency_us: u64,
    pub success_count: u32,
    pub error_count: u32,
}

/// Data from individual shard
#[derive(Debug, Clone)]
pub struct ShardData {
    pub shard_id: u32,
    pub shard_type: ShardType,
    pub data: Vec<u8>,
    pub read_success: bool,
}

/// Result of parallel shard write operation
#[derive(Debug, Clone)]
pub struct ShardWriteResult {
    pub market_id: Pubkey,
    pub operation: OperationType,
    pub success_count: u32,
    pub error_count: u32,
    pub write_latency_us: u64,
}

/// Shard coordinator for parallel execution
pub struct ShardCoordinator;

impl ShardCoordinator {
    /// Execute operation on appropriate shard
    pub fn execute_on_shard(
        manager: &EnhancedShardManager,
        market_id: &Pubkey,
        operation: OperationType,
        operation_fn: impl FnOnce(u32) -> ProgramResult,
    ) -> ProgramResult {
        // Route to appropriate shard
        let shard_id = manager.route_operation(market_id, operation)?;
        
        // Execute operation
        operation_fn(shard_id)?;
        
        Ok(())
    }
    
    /// Parallel read from multiple shards
    pub fn parallel_read(
        manager: &EnhancedShardManager,
        market_id: &Pubkey,
        read_fn: impl Fn(u32, ShardType) -> Result<Vec<u8>, ProgramError>,
    ) -> Result<ShardReadResult, ProgramError> {
        let allocation = manager.shard_allocations.iter()
            .find(|a| &a.market_id == market_id)
            .ok_or(BettingPlatformError::MarketNotSharded)?;
        
        let mut results = ShardReadResult {
            market_id: *market_id,
            shard_data: Vec::new(),
            read_latency_us: 0,
            success_count: 0,
            error_count: 0,
        };
        
        let start_time = Clock::get()?.unix_timestamp;
        
        // Read from each shard in parallel (simulated via sequential reads)
        // In production with Solana's parallel execution, these would be concurrent
        for shard in &allocation.shard_assignments {
            match read_fn(shard.shard_id, shard.shard_type) {
                Ok(data) => {
                    results.shard_data.push(ShardData {
                        shard_id: shard.shard_id,
                        shard_type: shard.shard_type,
                        data,
                        read_success: true,
                    });
                    results.success_count += 1;
                }
                Err(e) => {
                    msg!("Failed to read from shard {}: {:?}", shard.shard_id, e);
                    results.shard_data.push(ShardData {
                        shard_id: shard.shard_id,
                        shard_type: shard.shard_type,
                        data: vec![],
                        read_success: false,
                    });
                    results.error_count += 1;
                }
            }
        }
        
        let end_time = Clock::get()?.unix_timestamp;
        results.read_latency_us = ((end_time - start_time) * 1_000_000) as u64;
        
        // Ensure we have at least one successful read
        if results.success_count == 0 {
            return Err(BettingPlatformError::AllShardsUnavailable.into());
        }
        
        Ok(results)
    }
    
    /// Parallel write to multiple shards
    pub fn parallel_write(
        manager: &mut EnhancedShardManager,
        market_id: &Pubkey,
        operation: OperationType,
        write_fn: impl Fn(u32, ShardType, &[u8]) -> Result<(), ProgramError>,
        data: &[u8],
    ) -> Result<ShardWriteResult, ProgramError> {
        let allocation = manager.shard_allocations.iter()
            .find(|a| &a.market_id == market_id)
            .ok_or(BettingPlatformError::MarketNotSharded)?
            .clone(); // Clone to avoid borrow issues
        
        let mut results = ShardWriteResult {
            market_id: *market_id,
            operation,
            success_count: 0,
            error_count: 0,
            write_latency_us: 0,
        };
        
        let start_time = Clock::get()?.unix_timestamp;
        
        // Get appropriate shard for operation
        let target_shard = allocation.get_shard_for_operation(operation);
        
        // Write to primary shard
        match write_fn(target_shard.shard_id, target_shard.shard_type, data) {
            Ok(()) => {
                results.success_count += 1;
                
                // Update metrics
                manager.update_shard_metrics(
                    market_id,
                    target_shard.shard_type,
                    1,
                    Clock::get()?.slot,
                )?;
            }
            Err(e) => {
                msg!("Failed to write to shard {}: {:?}", target_shard.shard_id, e);
                results.error_count += 1;
                return Err(e);
            }
        }
        
        // For critical operations, replicate to backup shard
        if matches!(operation, OperationType::ExecuteTrade | OperationType::Settle) {
            // Find backup shard (analytics shard can serve as backup)
            if let Some(backup_shard) = allocation.shard_assignments.iter()
                .find(|s| s.shard_type == ShardType::Analytics) {
                
                let _ = write_fn(backup_shard.shard_id, backup_shard.shard_type, data);
                // Ignore backup errors, primary write succeeded
            }
        }
        
        let end_time = Clock::get()?.unix_timestamp;
        results.write_latency_us = ((end_time - start_time) * 1_000_000) as u64;
        
        Ok(results)
    }
}

// From<BettingPlatformError> for ProgramError is already implemented in error.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_market_shard_allocation() {
        let mut manager = EnhancedShardManager::new(Pubkey::new_unique());
        let market_id = Pubkey::new_unique();
        
        // Allocate shards for market
        let base_shard = manager.allocate_market_shards(&market_id).unwrap();
        assert_eq!(manager.total_shards, 4);
        assert_eq!(manager.markets_count, 1);
        
        // Verify 4 shards allocated
        let allocation = &manager.shard_allocations[0];
        assert_eq!(allocation.shard_assignments.len(), 4);
        
        // Verify shard types
        assert_eq!(allocation.shard_assignments[0].shard_type, ShardType::OrderBook);
        assert_eq!(allocation.shard_assignments[1].shard_type, ShardType::Execution);
        assert_eq!(allocation.shard_assignments[2].shard_type, ShardType::Settlement);
        assert_eq!(allocation.shard_assignments[3].shard_type, ShardType::Analytics);
    }
    
    #[test]
    fn test_operation_routing() {
        let mut manager = EnhancedShardManager::new(Pubkey::new_unique());
        let market_id = Pubkey::new_unique();
        
        manager.allocate_market_shards(&market_id).unwrap();
        
        // Test routing different operations
        let order_shard = manager.route_operation(&market_id, OperationType::PlaceOrder).unwrap();
        let exec_shard = manager.route_operation(&market_id, OperationType::ExecuteTrade).unwrap();
        
        assert_ne!(order_shard, exec_shard);
    }
    
    #[test]
    fn test_tps_calculation() {
        let mut manager = EnhancedShardManager::new(Pubkey::new_unique());
        
        // Add 10 markets (40 shards total)
        for _ in 0..10 {
            let market_id = Pubkey::new_unique();
            manager.allocate_market_shards(&market_id).unwrap();
        }
        
        // Update metrics to simulate load
        for allocation in &mut manager.shard_allocations {
            for shard in &mut allocation.shard_assignments {
                shard.load_factor = 2500; // 25% load
            }
        }
        
        let tps = manager.calculate_global_tps();
        // 40 shards * 1000 TPS/shard * 0.25 load = 10,000 TPS
        assert_eq!(tps, 10_000);
        assert!(manager.is_meeting_tps_target());
    }
}