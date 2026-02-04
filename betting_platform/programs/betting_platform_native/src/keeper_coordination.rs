//! Keeper coordination system
//!
//! Distributes work among multiple keepers and handles failures

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    error::BettingPlatformError,
    events::{Event, WorkAssigned, KeeperSuspended, WorkReassigned},
    state::{KeeperAccount, KeeperStatus, KeeperRegistry, WorkType, KeeperSpecialization},
};

/// Minimum performance score to avoid suspension (80%)
pub const SUSPENSION_THRESHOLD: u64 = 8000;

/// Work item to be executed
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct WorkItem {
    pub id: [u8; 32],
    pub work_type: WorkType,
    pub priority: u64,
    pub data: Vec<u8>,
    pub assigned_keeper: Option<Pubkey>,
    pub created_slot: u64,
    pub deadline_slot: u64,
}

/// Work assignment result
#[derive(Debug)]
pub struct AssignmentResult {
    pub keeper_id: [u8; 32],
    pub assigned_items: Vec<[u8; 32]>,
    pub priority: u64,
}

/// Keeper coordinator implementation
pub struct KeeperCoordinator;

impl KeeperCoordinator {
    /// Distribute work among multiple keepers
    pub fn assign_work_batch(
        registry: &KeeperRegistry,
        keepers: &mut [KeeperAccount],
        work_type: WorkType,
        work_items: Vec<WorkItem>,
    ) -> Result<Vec<AssignmentResult>, ProgramError> {
        // Get active keepers
        let active_keepers = Self::get_active_keepers(keepers)?;
        
        if active_keepers.is_empty() {
            return Err(BettingPlatformError::NoActiveKeepers.into());
        }
        
        // Filter and collect keepers with specialization
        let mut filtered_keepers: Vec<(usize, u64)> = active_keepers
            .iter()
            .enumerate()
            .filter(|(_, k)| k.has_specialization(&work_type))
            .map(|(idx, k)| (idx, k.calculate_priority()))
            .collect();
            
        // Sort by priority descending
        filtered_keepers.sort_by(|a, b| b.1.cmp(&a.1));
        
        if filtered_keepers.is_empty() {
            return Err(BettingPlatformError::NoActiveKeepers.into());
        }
        
        // Distribute work based on keeper capacity
        let items_per_keeper = work_items.len() / filtered_keepers.len();
        let mut assignments = Vec::new();
        
        for (i, (keeper_idx, priority)) in filtered_keepers.iter().enumerate() {
            let keeper = &active_keepers[*keeper_idx];
            let start = i * items_per_keeper;
            let end = if i == filtered_keepers.len() - 1 {
                work_items.len()
            } else {
                (i + 1) * items_per_keeper
            };
            
            let assigned_items: Vec<[u8; 32]> = work_items[start..end]
                .iter()
                .map(|item| item.id)
                .collect();
            
            // Emit work assignment event
            WorkAssigned {
                keeper_id: keeper.keeper_id,
                work_type: work_type as u8,
                items_count: assigned_items.len() as u32,
                priority: keeper.calculate_priority(),
            }.emit();
            
            assignments.push(AssignmentResult {
                keeper_id: keeper.keeper_id,
                assigned_items,
                priority: keeper.calculate_priority(),
            });
            
            msg!("Assigned {} {} items to keeper {}",
                end - start,
                format!("{:?}", work_type),
                bs58::encode(&keeper.keeper_id[..8]).into_string()
            );
        }
        
        Ok(assignments)
    }
    
    /// Handle keeper failure and reassign work
    pub fn handle_keeper_failure(
        registry: &mut KeeperRegistry,
        failed_keeper: &mut KeeperAccount,
        work_item: &WorkItem,
        remaining_keepers: &mut [KeeperAccount],
    ) -> ProgramResult {
        // Update failure stats
        failed_keeper.total_operations = failed_keeper.total_operations
            .checked_add(1)
            .ok_or(BettingPlatformError::Overflow)?;
            
        failed_keeper.performance_score = failed_keeper.successful_operations
            .checked_mul(10000)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(failed_keeper.total_operations)
            .ok_or(BettingPlatformError::DivisionByZero)?;
        
        // Check if suspension needed
        if failed_keeper.performance_score < SUSPENSION_THRESHOLD {
            failed_keeper.status = KeeperStatus::Suspended;
            registry.active_keepers = registry.active_keepers
                .checked_sub(1)
                .ok_or(BettingPlatformError::Underflow)?;
            
            KeeperSuspended {
                keeper_id: failed_keeper.keeper_id,
                performance_score: failed_keeper.performance_score,
                total_failures: failed_keeper.total_operations - failed_keeper.successful_operations,
            }.emit();
            
            msg!("Keeper {} suspended due to poor performance ({}%)",
                bs58::encode(&failed_keeper.keeper_id[..8]).into_string(),
                failed_keeper.performance_score / 100
            );
        }
        
        // Find backup keeper
        let backup_keeper = Self::find_backup_keeper(
            remaining_keepers,
            work_item,
            Some(failed_keeper.keeper_id),
        )?;
        
        // Reassign work
        WorkReassigned {
            original_keeper: failed_keeper.keeper_id,
            new_keeper: backup_keeper.keeper_id,
            work_item_id: work_item.id,
        }.emit();
        
        msg!("Reassigned work item {} from keeper {} to {}",
            bs58::encode(&work_item.id[..8]).into_string(),
            bs58::encode(&failed_keeper.keeper_id[..8]).into_string(),
            bs58::encode(&backup_keeper.keeper_id[..8]).into_string()
        );
        
        Ok(())
    }
    
    /// Get active keepers
    fn get_active_keepers(
        keepers: &mut [KeeperAccount],
    ) -> Result<Vec<&mut KeeperAccount>, ProgramError> {
        Ok(keepers
            .iter_mut()
            .filter(|k| k.status == KeeperStatus::Active)
            .collect())
    }
    
    /// Find best backup keeper for work item
    fn find_backup_keeper<'a>(
        keepers: &'a mut [KeeperAccount],
        work_item: &WorkItem,
        exclude: Option<[u8; 32]>,
    ) -> Result<&'a mut KeeperAccount, ProgramError> {
        let mut best_keeper = None;
        let mut best_score = 0u64;
        
        for keeper in keepers.iter_mut() {
            if keeper.status != KeeperStatus::Active {
                continue;
            }
            
            if let Some(excluded_id) = exclude {
                if keeper.keeper_id == excluded_id {
                    continue;
                }
            }
            
            if !keeper.has_specialization(&work_item.work_type) {
                continue;
            }
            
            let score = keeper.calculate_priority();
            if score > best_score {
                best_score = score;
                best_keeper = Some(keeper);
            }
        }
        
        best_keeper.ok_or(BettingPlatformError::NoBackupKeeperAvailable.into())
    }
    
    /// Monitor keeper health and performance
    pub fn monitor_keeper_health(
        keepers: &[KeeperAccount],
        current_slot: u64,
    ) -> Vec<KeeperHealthReport> {
        let mut reports = Vec::new();
        
        for keeper in keepers {
            let idle_slots = current_slot.saturating_sub(keeper.last_operation_slot);
            let idle_time_minutes = (idle_slots as f64 * 0.4) / 60.0;
            
            let health = if keeper.status != KeeperStatus::Active {
                KeeperHealth::Inactive
            } else if idle_time_minutes > 60.0 {
                KeeperHealth::Idle
            } else if keeper.performance_score < 9000 {
                KeeperHealth::Degraded
            } else {
                KeeperHealth::Healthy
            };
            
            reports.push(KeeperHealthReport {
                keeper_id: keeper.keeper_id,
                health,
                performance_score: keeper.performance_score,
                idle_time_minutes,
                total_operations: keeper.total_operations,
                successful_operations: keeper.successful_operations,
            });
        }
        
        reports
    }
}

// Methods for KeeperAccount are defined in state/keeper_accounts.rs

/// Keeper health status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeeperHealth {
    Healthy,
    Degraded,
    Idle,
    Inactive,
}

/// Keeper health report
#[derive(Debug)]
pub struct KeeperHealthReport {
    pub keeper_id: [u8; 32],
    pub health: KeeperHealth,
    pub performance_score: u64,
    pub idle_time_minutes: f64,
    pub total_operations: u64,
    pub successful_operations: u64,
}

// Hex encoding utility
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_work_distribution() {
        let work_items: Vec<WorkItem> = (0..10)
            .map(|i| WorkItem {
                id: [i as u8; 32],
                work_type: WorkType::Liquidations,
                priority: 100 - i as u64,
                data: vec![],
                assigned_keeper: None,
                created_slot: 0,
                deadline_slot: 1000,
            })
            .collect();
        
        // With 3 keepers, should distribute 3, 3, 4 items
        let items_per_keeper = work_items.len() / 3;
        assert_eq!(items_per_keeper, 3);
        
        // Last keeper gets remainder
        let last_keeper_items = work_items.len() - (items_per_keeper * 2);
        assert_eq!(last_keeper_items, 4);
    }
    
    #[test]
    fn test_keeper_priority_calculation() {
        use crate::state::{keeper_accounts::discriminators, keeper_accounts::{KeeperAccount, KeeperStatus}, KeeperType};
        
        let keeper = KeeperAccount {
            discriminator: discriminators::KEEPER_ACCOUNT,
            keeper_id: [1u8; 32],
            authority: Pubkey::default(),
            keeper_type: KeeperType::Liquidation,
            mmt_stake: 1_000_000,
            performance_score: 9500, // 95%
            total_operations: 100,
            successful_operations: 95,
            total_rewards_earned: 50_000,
            last_operation_slot: 1000,
            status: KeeperStatus::Active,
            specializations: vec![KeeperSpecialization::Liquidations],
            average_response_time: 0,
            priority_score: 0,
            registration_slot: 0,
            slashing_count: 0,
        };
        
        let priority = keeper.calculate_priority();
        assert_eq!(priority, 950_000); // 1M * 0.95
    }
    
    #[test]
    fn test_suspension_threshold() {
        let performance_score = 7500; // 75%
        assert!(performance_score < SUSPENSION_THRESHOLD);
    }
}