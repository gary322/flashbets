use anchor_lang::prelude::*;
use crate::types::U64F64;
use crate::state::{BootstrapState, BootstrapMilestone};
use crate::errors::ErrorCode;

pub struct MilestoneManager;

impl MilestoneManager {
    /// Process milestone achievement
    pub fn check_and_process_milestone(
        bootstrap_state: &mut BootstrapState,
        milestone: &mut BootstrapMilestone,
        top_traders: Vec<(Pubkey, u64)>, // (trader, contribution)
        clock: &Clock,
    ) -> Result<bool> {
        if milestone.achieved {
            return Ok(false);
        }
        
        let vault_reached = bootstrap_state.current_vault_balance >= milestone.vault_target;
        let coverage_reached = bootstrap_state.current_coverage >= milestone.coverage_target;
        let traders_reached = bootstrap_state.unique_traders >= milestone.traders_target;
        
        if vault_reached && coverage_reached && traders_reached {
            milestone.achieved = true;
            milestone.achieved_slot = clock.slot;
            
            // Store top 10 contributors
            milestone.top_contributors = top_traders
                .into_iter()
                .take(10)
                .map(|(pubkey, _)| pubkey)
                .collect();
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Get bootstrap milestones for progressive rewards
    pub fn get_bootstrap_milestones() -> Vec<(u64, U64F64, u64, u64)> {
        // (vault_target, coverage_target, traders_target, mmt_bonus_pool)
        vec![
            (1_000 * 10u64.pow(6), U64F64::from_num(1u32) / U64F64::from_num(10u32), 10, 10_000 * 10u64.pow(6)), // 0.1
            (10_000 * 10u64.pow(6), U64F64::from_num(1u32) / U64F64::from_num(4u32), 50, 50_000 * 10u64.pow(6)), // 0.25
            (50_000 * 10u64.pow(6), U64F64::from_num(1u32) / U64F64::from_num(2u32), 100, 100_000 * 10u64.pow(6)), // 0.5
            (100_000 * 10u64.pow(6), U64F64::from_num(3u32) / U64F64::from_num(4u32), 500, 200_000 * 10u64.pow(6)), // 0.75
            (500_000 * 10u64.pow(6), U64F64::one(), 1000, 500_000 * 10u64.pow(6)), // 1.0
        ]
    }
    
    /// Initialize milestone
    pub fn initialize_milestone(
        milestone: &mut BootstrapMilestone,
        index: u64,
        vault_target: u64,
        coverage_target: U64F64,
        traders_target: u64,
        mmt_bonus_pool: u64,
    ) -> Result<()> {
        milestone.index = index;
        milestone.vault_target = vault_target;
        milestone.coverage_target = coverage_target;
        milestone.traders_target = traders_target;
        milestone.mmt_bonus_pool = mmt_bonus_pool;
        milestone.achieved = false;
        milestone.achieved_slot = 0;
        milestone.top_contributors = vec![];
        
        Ok(())
    }
}