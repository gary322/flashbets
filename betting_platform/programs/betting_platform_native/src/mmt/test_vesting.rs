//! Tests for MMT Vesting Implementation

#[cfg(test)]
mod tests {
    use super::super::*;
    use solana_program::clock::Clock;
    
    #[test]
    fn test_vesting_allocations() {
        // Verify total allocations match 90M tokens
        let total = VestingAllocations::TEAM_ALLOCATION +
                   VestingAllocations::ADVISORS_ALLOCATION +
                   VestingAllocations::STRATEGIC_ALLOCATION +
                   VestingAllocations::ECOSYSTEM_ALLOCATION +
                   VestingAllocations::RESERVE_ALLOCATION;
        
        assert_eq!(total, 90_000_000 * 10u64.pow(MMT_DECIMALS as u32), 
            "Total vesting allocation should be 90M tokens");
        
        // Verify individual allocations
        assert_eq!(VestingAllocations::TEAM_ALLOCATION, 20_000_000 * 10u64.pow(MMT_DECIMALS as u32));
        assert_eq!(VestingAllocations::ADVISORS_ALLOCATION, 5_000_000 * 10u64.pow(MMT_DECIMALS as u32));
        assert_eq!(VestingAllocations::STRATEGIC_ALLOCATION, 15_000_000 * 10u64.pow(MMT_DECIMALS as u32));
        assert_eq!(VestingAllocations::ECOSYSTEM_ALLOCATION, 30_000_000 * 10u64.pow(MMT_DECIMALS as u32));
        assert_eq!(VestingAllocations::RESERVE_ALLOCATION, 20_000_000 * 10u64.pow(MMT_DECIMALS as u32));
    }
    
    #[test]
    fn test_team_vesting_schedule() {
        let mut schedule = VestingSchedule::new(
            VestingScheduleType::Team,
            solana_program::pubkey::Pubkey::default(),
            VestingAllocations::TEAM_ALLOCATION,
            0, // start_time
        );
        
        // Before cliff (1 year), nothing should be claimable
        assert_eq!(schedule.calculate_claimable(SECONDS_PER_YEAR - 1).unwrap(), 0);
        
        // After cliff, 25% should be claimable
        let after_cliff = schedule.calculate_claimable(SECONDS_PER_YEAR).unwrap();
        let expected_cliff = VestingAllocations::TEAM_ALLOCATION / 4; // 25%
        assert_eq!(after_cliff, expected_cliff);
        
        // After 2 years, 50% should be claimable
        let after_2_years = schedule.calculate_claimable(SECONDS_PER_YEAR * 2).unwrap();
        let expected_2_years = VestingAllocations::TEAM_ALLOCATION / 2; // 50%
        assert_eq!(after_2_years, expected_2_years);
        
        // After 4 years, 100% should be claimable
        let after_4_years = schedule.calculate_claimable(SECONDS_PER_YEAR * 4).unwrap();
        assert_eq!(after_4_years, VestingAllocations::TEAM_ALLOCATION);
    }
    
    #[test]
    fn test_advisors_vesting_schedule() {
        let mut schedule = VestingSchedule::new(
            VestingScheduleType::Advisors,
            solana_program::pubkey::Pubkey::default(),
            VestingAllocations::ADVISORS_ALLOCATION,
            0,
        );
        
        // Before 6 month cliff, nothing claimable
        assert_eq!(schedule.calculate_claimable(SECONDS_PER_YEAR / 2 - 1).unwrap(), 0);
        
        // After cliff, vesting should start
        let after_cliff = schedule.calculate_claimable(SECONDS_PER_YEAR / 2).unwrap();
        assert!(after_cliff > 0, "Should have claimable tokens after cliff");
        
        // After 2 years, 100% should be claimable
        let after_2_years = schedule.calculate_claimable(SECONDS_PER_YEAR * 2).unwrap();
        assert_eq!(after_2_years, VestingAllocations::ADVISORS_ALLOCATION);
    }
    
    #[test]
    fn test_reserve_vesting_schedule() {
        let mut schedule = VestingSchedule::new(
            VestingScheduleType::Reserve,
            solana_program::pubkey::Pubkey::default(),
            VestingAllocations::RESERVE_ALLOCATION,
            0,
        );
        
        // Before year 3, nothing claimable
        assert_eq!(schedule.calculate_claimable(SECONDS_PER_YEAR * 3 - 1).unwrap(), 0);
        
        // After year 3, vesting should start
        let after_3_years = schedule.calculate_claimable(SECONDS_PER_YEAR * 3).unwrap();
        assert!(after_3_years > 0, "Should have claimable tokens after 3 years");
        
        // After 10 years, 100% should be claimable
        let after_10_years = schedule.calculate_claimable(SECONDS_PER_YEAR * 10).unwrap();
        assert_eq!(after_10_years, VestingAllocations::RESERVE_ALLOCATION);
    }
    
    #[test]
    fn test_vesting_with_claims() {
        let mut schedule = VestingSchedule::new(
            VestingScheduleType::Ecosystem,
            solana_program::pubkey::Pubkey::default(),
            VestingAllocations::ECOSYSTEM_ALLOCATION,
            0,
        );
        
        // Claim after 1 year
        let claimable_1 = schedule.calculate_claimable(SECONDS_PER_YEAR).unwrap();
        schedule.claimed_amount = claimable_1;
        
        // Try to claim again at same time - should be 0
        assert_eq!(schedule.calculate_claimable(SECONDS_PER_YEAR).unwrap(), 0);
        
        // Claim after 2 years - should only get the difference
        let claimable_2 = schedule.calculate_claimable(SECONDS_PER_YEAR * 2).unwrap();
        assert!(claimable_2 > 0, "Should have more tokens to claim after year 2");
        assert!(claimable_2 < VestingAllocations::ECOSYSTEM_ALLOCATION, "Should not be able to claim all tokens yet");
    }
}