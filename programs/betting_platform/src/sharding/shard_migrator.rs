use anchor_lang::prelude::*;
use std::collections::HashMap;

use crate::sharding::types::*;
use crate::sharding::errors::ShardingError;

pub const MIGRATION_TIMEOUT_SLOTS: u64 = 100; // ~40 seconds

#[derive(Clone, Debug)]
pub struct MigrationState {
    pub market_id: Pubkey,
    pub from_shard: u8,
    pub to_shard: u8,
    pub migration_started: u64,
    pub state_snapshot: MarketSnapshot,
    pub status: MigrationStatus,
}

pub struct ShardMigrator {
    pub migration_buffer: HashMap<Pubkey, MigrationState>,
    pub migration_timeout: u64, // slots
    pub paused_markets: HashMap<Pubkey, u64>, // market -> pause start slot
}

impl ShardMigrator {
    pub fn new() -> Self {
        Self {
            migration_buffer: HashMap::new(),
            migration_timeout: MIGRATION_TIMEOUT_SLOTS,
            paused_markets: HashMap::new(),
        }
    }

    pub fn migrate_market(
        &mut self,
        market_id: Pubkey,
        from_shard: u8,
        to_shard: u8,
        current_slot: u64,
    ) -> Result<()> {
        // Check if migration already in progress
        if self.migration_buffer.contains_key(&market_id) {
            return Err(ShardingError::RebalanceInProgress.into());
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

    pub fn complete_migration(
        &mut self,
        market_id: &Pubkey,
    ) -> Result<()> {
        let migration = self.migration_buffer.get_mut(market_id)
            .ok_or(ShardingError::MigrationNotFound)?;

        // Update status
        migration.status = MigrationStatus::InProgress;

        // Atomic state transfer
        self.transfer_state_to_new_shard(
            &migration.state_snapshot,
            migration.to_shard,
        )?;

        // Update shard assignment
        self.update_shard_assignment(market_id, migration.to_shard)?;

        // Resume writes on new shard
        self.resume_market_writes(market_id)?;

        migration.status = MigrationStatus::Completed;
        
        // Remove from buffer after successful migration
        self.migration_buffer.remove(market_id);
        
        Ok(())
    }

    pub fn check_migration_timeouts(&mut self, current_slot: u64) -> Vec<Pubkey> {
        let mut timed_out = Vec::new();

        for (market_id, migration) in &mut self.migration_buffer {
            if current_slot > migration.migration_started + self.migration_timeout {
                migration.status = MigrationStatus::Failed;
                timed_out.push(*market_id);
            }
        }

        // Clean up timed out migrations
        for market_id in &timed_out {
            self.migration_buffer.remove(market_id);
            // Attempt to resume writes for timed out markets
            let _ = self.resume_market_writes(market_id);
        }

        timed_out
    }

    fn take_market_snapshot(
        &self,
        market_id: &Pubkey,
        shard: u8,
    ) -> Result<MarketSnapshot> {
        // Capture all market state atomically
        let positions = self.get_all_positions(market_id, shard)?;
        let orders = self.get_all_orders(market_id, shard)?;
        let amm_state = self.get_amm_state(market_id, shard)?;

        Ok(MarketSnapshot {
            market_id: *market_id,
            positions,
            orders,
            amm_state,
            snapshot_slot: Clock::get()?.slot,
        })
    }

    fn pause_market_writes(&mut self, market_id: &Pubkey, current_slot: u64) -> Result<()> {
        // In a real implementation, this would interact with the trading engine
        // to prevent new writes to this market
        self.paused_markets.insert(*market_id, current_slot);
        Ok(())
    }

    fn resume_market_writes(&mut self, market_id: &Pubkey) -> Result<()> {
        // In a real implementation, this would interact with the trading engine
        // to resume writes to this market
        self.paused_markets.remove(market_id);
        Ok(())
    }

    fn transfer_state_to_new_shard(
        &self,
        snapshot: &MarketSnapshot,
        to_shard: u8,
    ) -> Result<()> {
        // In a real implementation, this would:
        // 1. Write snapshot data to the new shard
        // 2. Verify data integrity
        // 3. Update routing tables
        
        // For now, we'll just validate the shard ID
        if to_shard >= crate::sharding::shard_manager::SHARD_COUNT_DEFAULT {
            return Err(ShardingError::InvalidShardId.into());
        }

        Ok(())
    }

    fn update_shard_assignment(&self, _market_id: &Pubkey, _to_shard: u8) -> Result<()> {
        // In a real implementation, this would update the global shard assignment
        // table that routers use to direct traffic
        Ok(())
    }

    fn get_all_positions(&self, _market_id: &Pubkey, _shard: u8) -> Result<Vec<Position>> {
        // In a real implementation, this would fetch all positions from the shard
        Ok(Vec::new())
    }

    fn get_all_orders(&self, _market_id: &Pubkey, _shard: u8) -> Result<Vec<Order>> {
        // In a real implementation, this would fetch all orders from the shard
        Ok(Vec::new())
    }

    fn get_amm_state(&self, _market_id: &Pubkey, _shard: u8) -> Result<AmmState> {
        // In a real implementation, this would fetch the AMM state from the shard
        Ok(AmmState {
            liquidity: 0,
            fees_collected: 0,
        })
    }

    pub fn is_market_paused(&self, market_id: &Pubkey) -> bool {
        self.paused_markets.contains_key(market_id)
    }

    pub fn get_migration_status(&self, market_id: &Pubkey) -> Option<&MigrationStatus> {
        self.migration_buffer.get(market_id).map(|m| &m.status)
    }

    pub fn get_active_migrations_count(&self) -> usize {
        self.migration_buffer.len()
    }

    pub fn cancel_migration(&mut self, market_id: &Pubkey) -> Result<()> {
        if let Some(migration) = self.migration_buffer.remove(market_id) {
            // Only allow cancellation if not yet in progress
            match migration.status {
                MigrationStatus::Pending => {
                    self.resume_market_writes(market_id)?;
                    Ok(())
                }
                _ => {
                    // Put it back if we can't cancel
                    self.migration_buffer.insert(*market_id, migration);
                    Err(ShardingError::RebalanceInProgress.into())
                }
            }
        } else {
            Err(ShardingError::MigrationNotFound.into())
        }
    }
}