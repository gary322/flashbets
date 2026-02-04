//! Tests for the migration module
//!
//! Verifies the 60-day parallel deployment framework

#[cfg(test)]
mod tests {
    use super::super::*;
    use solana_program::{
        account_info::AccountInfo,
        clock::Clock,
        instruction::AccountMeta,
        program_error::ProgramError,
        pubkey::Pubkey,
        rent::Rent,
        system_program,
    };
    use solana_program_test::{BanksClient, ProgramTest, ProgramTestContext};
    use solana_sdk::{
        account::Account,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };
    use borsh::BorshSerialize;

    /// Test account holder to manage lifetimes
    struct TestAccountData {
        lamports: u64,
        data: Vec<u8>,
    }
    
    /// Create test account
    fn create_account<'a>(
        pubkey: &'a Pubkey,
        account_data: &'a mut TestAccountData,
        owner: &'a Pubkey,
    ) -> AccountInfo<'a> {
        AccountInfo::new(
            pubkey,
            false,
            true,
            &mut account_data.lamports,
            &mut account_data.data,
            owner,
            false,
            0,
        )
    }

    #[test]
    fn test_parallel_deployment_new() {
        let old_program = Pubkey::new_unique();
        let new_program = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let current_slot = 1000;

        let deployment = ParallelDeployment::new(
            old_program,
            new_program,
            authority,
            current_slot,
        );

        assert_eq!(deployment.old_program_id, old_program);
        assert_eq!(deployment.new_program_id, new_program);
        assert_eq!(deployment.authority, authority);
        assert_eq!(deployment.start_slot, current_slot);
        assert_eq!(deployment.end_slot, current_slot + MIGRATION_PERIOD_SLOTS);
        assert_eq!(deployment.positions_migrated, 0);
        assert_eq!(deployment.mmt_rewards_distributed, 0);
        assert!(deployment.is_active);
    }

    #[test]
    fn test_migration_expiry() {
        let deployment = ParallelDeployment::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            1000,
        );

        // Not expired at start
        assert!(!deployment.is_expired(1000));
        
        // Not expired during period
        assert!(!deployment.is_expired(1000 + MIGRATION_PERIOD_SLOTS / 2));
        
        // Not expired at exact end
        assert!(!deployment.is_expired(1000 + MIGRATION_PERIOD_SLOTS));
        
        // Expired after end
        assert!(deployment.is_expired(1000 + MIGRATION_PERIOD_SLOTS + 1));
    }

    #[test]
    fn test_migration_progress() {
        let deployment = ParallelDeployment::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
        );

        // 0% at start
        assert_eq!(deployment.progress_percentage(0), 0);
        
        // 25% progress
        let quarter = MIGRATION_PERIOD_SLOTS / 4;
        assert_eq!(deployment.progress_percentage(quarter), 25);
        
        // 50% progress
        let half = MIGRATION_PERIOD_SLOTS / 2;
        assert_eq!(deployment.progress_percentage(half), 50);
        
        // 75% progress
        let three_quarters = (MIGRATION_PERIOD_SLOTS * 3) / 4;
        assert_eq!(deployment.progress_percentage(three_quarters), 75);
        
        // 100% at end
        assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS), 100);
        
        // Still 100% after end
        assert_eq!(deployment.progress_percentage(MIGRATION_PERIOD_SLOTS + 1000), 100);
    }

    #[test]
    fn test_remaining_slots() {
        let deployment = ParallelDeployment::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            1000,
        );

        // Full period remaining at start
        assert_eq!(deployment.remaining_slots(1000), MIGRATION_PERIOD_SLOTS);
        
        // Half remaining at midpoint
        let midpoint = 1000 + MIGRATION_PERIOD_SLOTS / 2;
        assert_eq!(deployment.remaining_slots(midpoint), MIGRATION_PERIOD_SLOTS / 2);
        
        // Zero remaining at end
        let end = 1000 + MIGRATION_PERIOD_SLOTS;
        assert_eq!(deployment.remaining_slots(end), 0);
        
        // Zero remaining after end
        assert_eq!(deployment.remaining_slots(end + 1000), 0);
    }

    #[test]
    fn test_migration_status() {
        let mut deployment = ParallelDeployment::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            0,
        );
        
        deployment.positions_migrated = 150;
        deployment.mmt_rewards_distributed = 15_000_000; // 15 MMT
        
        let current_slot = MIGRATION_PERIOD_SLOTS / 2; // 50% through
        let status = get_migration_status(&deployment, current_slot);
        
        assert!(status.is_active);
        assert_eq!(status.progress_pct, 50);
        assert_eq!(status.positions_migrated, 150);
        assert_eq!(status.mmt_distributed, 15_000_000);
        assert_eq!(status.slots_remaining, MIGRATION_PERIOD_SLOTS / 2);
        assert_eq!(status.days_remaining, 30); // Half of 60 days
    }

    #[test]
    fn test_migration_wizard_instructions() {
        let user = Pubkey::new_unique();
        let old_program = Pubkey::new_unique();
        let new_program = Pubkey::new_unique();
        
        let position_ids = vec![
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
        ];
        
        let instructions = create_migration_wizard_instructions(
            &user,
            position_ids.clone(),
            &old_program,
            &new_program,
        );
        
        assert_eq!(instructions.len(), 3);
        
        for (i, instruction) in instructions.iter().enumerate() {
            assert_eq!(instruction.position_id, position_ids[i]);
            assert_eq!(instruction.user, user);
            assert_eq!(instruction.old_program, old_program);
            assert_eq!(instruction.new_program, new_program);
            assert_eq!(instruction.estimated_reward, 1000); // Default estimate
        }
    }

    #[test]
    fn test_mmt_reward_calculation() {
        // Test the double MMT incentive calculation
        let notional = 1_000_000_000; // 1000 units
        let base_reward = notional / 1000; // 0.1%
        let migration_reward = base_reward * MIGRATION_MMT_MULTIPLIER;
        
        assert_eq!(base_reward, 1_000_000); // 1 unit base
        assert_eq!(migration_reward, 2_000_000); // 2 units with 2x multiplier
    }
}

// Export for use in integration tests
pub use tests::*;