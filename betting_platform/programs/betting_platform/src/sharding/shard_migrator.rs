use anchor_lang::prelude::*;
use std::collections::HashMap;

use crate::sharding::{
    MigrationState, MigrationStatus, MarketSnapshot, PositionSnapshot, 
    OrderSnapshot, AMMStateSnapshot, MigrationError, MIGRATION_TIMEOUT_SLOTS
};

pub struct ShardMigrator {
    pub migration_buffer: HashMap<Pubkey, MigrationState>,
    pub migration_timeout: u64,
    pub paused_markets: HashMap<Pubkey, u64>, // market -> pause_start_slot
}

impl ShardMigrator {
    pub fn new() -> Self {
        Self {
            migration_buffer: HashMap::new(),
            migration_timeout: MIGRATION_TIMEOUT_SLOTS,
            paused_markets: HashMap::new(),
        }
    }

    /// Initiate market migration
    pub fn migrate_market(
        &mut self,
        market_id: Pubkey,
        from_shard: u8,
        to_shard: u8,
        current_slot: u64,
    ) -> Result<()> {
        // Check if migration already in progress
        if self.migration_buffer.contains_key(&market_id) {
            return Err(MigrationError::MigrationAlreadyInProgress.into());
        }

        // Take snapshot of current state
        let snapshot = self.take_market_snapshot(&market_id, from_shard)?;

        // Create migration state
        let migration = MigrationState {
            market_id,
            from_shard,
            to_shard,
            migration_started: current_slot,
            state_snapshot: snapshot,
            status: MigrationStatus::Pending,
        };

        // Buffer the migration
        self.migration_buffer.insert(market_id, migration);

        // Pause writes to market during migration
        self.pause_market_writes(&market_id, current_slot)?;

        Ok(())
    }

    /// Complete a pending migration
    pub fn complete_migration(
        &mut self,
        market_id: &Pubkey,
    ) -> Result<()> {
        // Extract the migration from the buffer to avoid borrowing issues
        let mut migration = self.migration_buffer.remove(market_id)
            .ok_or(MigrationError::MigrationNotFound)?;

        if migration.status != MigrationStatus::Pending {
            // Put it back if we can't process it
            self.migration_buffer.insert(*market_id, migration);
            return Err(MigrationError::InvalidMigrationState.into());
        }

        // Update status to in progress
        migration.status = MigrationStatus::InProgress;

        // Store snapshot data we need
        let snapshot = migration.state_snapshot.clone();
        let to_shard = migration.to_shard;

        // Atomic state transfer
        self.transfer_state_to_new_shard(&snapshot, to_shard)?;

        // Update shard assignment (this would be done through program state)
        self.update_shard_assignment(market_id, to_shard)?;

        // Resume writes on new shard
        self.resume_market_writes(market_id)?;

        // Migration completed successfully, it's already removed from buffer
        
        Ok(())
    }

    /// Take atomic snapshot of market state
    fn take_market_snapshot(
        &self,
        market_id: &Pubkey,
        _shard: u8,
    ) -> Result<MarketSnapshot> {
        // In production, this would read from actual on-chain accounts
        // For now, create a mock snapshot
        let positions = self.get_all_positions(market_id)?;
        let orders = self.get_all_orders(market_id)?;
        let amm_state = self.get_amm_state(market_id)?;

        Ok(MarketSnapshot {
            market_id: *market_id,
            positions,
            orders,
            amm_state,
            snapshot_slot: Clock::get()?.slot,
        })
    }

    /// Get all positions for a market (mock implementation)
    fn get_all_positions(&self, _market_id: &Pubkey) -> Result<Vec<PositionSnapshot>> {
        // In production, iterate through all position accounts for this market
        Ok(vec![])
    }

    /// Get all orders for a market (mock implementation)
    fn get_all_orders(&self, _market_id: &Pubkey) -> Result<Vec<OrderSnapshot>> {
        // In production, iterate through all order accounts for this market
        Ok(vec![])
    }

    /// Get AMM state for a market (mock implementation)
    fn get_amm_state(&self, _market_id: &Pubkey) -> Result<AMMStateSnapshot> {
        // In production, read from AMM account
        Ok(AMMStateSnapshot {
            liquidity: 1_000_000,
            yes_shares: 500_000,
            no_shares: 500_000,
            k_constant: 250_000_000_000,
        })
    }

    /// Transfer state to new shard atomically
    fn transfer_state_to_new_shard(
        &self,
        snapshot: &MarketSnapshot,
        to_shard: u8,
    ) -> Result<()> {
        // In production, this would:
        // 1. Create new accounts on target shard
        // 2. Copy all state from snapshot
        // 3. Verify checksums match
        // 4. Mark old shard data for cleanup
        
        msg!("Transferring market {} to shard {}", snapshot.market_id, to_shard);
        msg!("Positions to transfer: {}", snapshot.positions.len());
        msg!("Orders to transfer: {}", snapshot.orders.len());
        
        Ok(())
    }

    /// Update shard assignment in global state
    fn update_shard_assignment(&self, market_id: &Pubkey, to_shard: u8) -> Result<()> {
        // In production, update global shard assignment map
        msg!("Updated shard assignment for market {} to shard {}", market_id, to_shard);
        Ok(())
    }

    /// Pause writes to a market
    fn pause_market_writes(&mut self, market_id: &Pubkey, current_slot: u64) -> Result<()> {
        self.paused_markets.insert(*market_id, current_slot);
        msg!("Paused writes to market {}", market_id);
        Ok(())
    }

    /// Resume writes to a market
    fn resume_market_writes(&mut self, market_id: &Pubkey) -> Result<()> {
        self.paused_markets.remove(market_id);
        msg!("Resumed writes to market {}", market_id);
        Ok(())
    }

    /// Check if market writes are paused
    pub fn is_market_paused(&self, market_id: &Pubkey) -> bool {
        self.paused_markets.contains_key(market_id)
    }

    /// Handle migration timeouts
    pub fn check_migration_timeouts(&mut self, current_slot: u64) -> Vec<Pubkey> {
        let mut timed_out = vec![];

        for (market_id, migration) in &mut self.migration_buffer {
            if current_slot > migration.migration_started + self.migration_timeout {
                migration.status = MigrationStatus::Failed;
                timed_out.push(*market_id);
                
                // Resume writes on timeout
                self.paused_markets.remove(market_id);
            }
        }

        // Clean up failed migrations
        for market_id in &timed_out {
            self.migration_buffer.remove(market_id);
        }

        timed_out
    }

    /// Retry failed migration
    pub fn retry_migration(&mut self, market_id: &Pubkey, current_slot: u64) -> Result<()> {
        let migration = self.migration_buffer.get_mut(market_id)
            .ok_or(MigrationError::MigrationNotFound)?;

        if migration.status != MigrationStatus::Failed {
            return Err(MigrationError::InvalidMigrationState.into());
        }

        // Reset migration
        migration.status = MigrationStatus::Pending;
        migration.migration_started = current_slot;
        
        // Re-pause market
        self.pause_market_writes(market_id, current_slot)?;

        Ok(())
    }

    /// Get migration progress
    pub fn get_migration_progress(&self, market_id: &Pubkey) -> Option<MigrationProgress> {
        self.migration_buffer.get(market_id).map(|migration| {
            MigrationProgress {
                status: migration.status.clone(),
                from_shard: migration.from_shard,
                to_shard: migration.to_shard,
                started_slot: migration.migration_started,
                positions_count: migration.state_snapshot.positions.len(),
                orders_count: migration.state_snapshot.orders.len(),
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct MigrationProgress {
    pub status: MigrationStatus,
    pub from_shard: u8,
    pub to_shard: u8,
    pub started_slot: u64,
    pub positions_count: usize,
    pub orders_count: usize,
}