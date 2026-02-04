use anchor_lang::prelude::*;
use fixed::types::U64F64;

#[account]
pub struct BootstrapState {
    /// Current bootstrap epoch (starts at 1)
    pub epoch: u64,
    
    /// Vault balance at bootstrap start
    pub initial_vault_balance: u64,
    
    /// Current vault balance
    pub current_vault_balance: u64,
    
    /// Total MMT rewards allocated for bootstrap
    pub bootstrap_mmt_allocation: u64, // 2M MMT from 10M season allocation
    
    /// MMT rewards distributed so far
    pub mmt_distributed: u64,
    
    /// Number of unique traders in bootstrap
    pub unique_traders: u64,
    
    /// Total volume during bootstrap
    pub total_volume: u64,
    
    /// Bootstrap phase status
    pub status: BootstrapStatus,
    
    /// Coverage at bootstrap start (0 initially)
    pub initial_coverage: U64F64,
    
    /// Current coverage ratio
    pub current_coverage: U64F64,
    
    /// Target coverage to end bootstrap (1.0 = 100%)
    pub target_coverage: U64F64,
    
    /// Slot when bootstrap started
    pub start_slot: u64,
    
    /// Expected end slot (6 months = 38,880,000 slots)
    pub expected_end_slot: u64,
    
    /// Early trader bonus multiplier (2x for first 100)
    pub early_bonus_multiplier: U64F64,
    
    /// Number of early traders who got bonus
    pub early_traders_count: u64,
    
    /// Max early traders for bonus
    pub max_early_traders: u64, // 100
    
    /// Minimum trade size for rewards
    pub min_trade_size: u64, // $10 equivalent
    
    /// Fee structure during bootstrap
    pub bootstrap_fee_bps: u16, // 28 bps max
    
    /// Padding for future upgrades
    pub _padding: [u8; 256],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum BootstrapStatus {
    /// Bootstrap not started yet
    NotStarted,
    
    /// Active bootstrap phase
    Active,
    
    /// Bootstrap paused due to low activity
    Paused,
    
    /// Bootstrap completed successfully
    Completed,
    
    /// Bootstrap failed (shouldn't happen with proper incentives)
    Failed,
}

#[account]
pub struct BootstrapTrader {
    /// Trader pubkey
    pub trader: Pubkey,
    
    /// Total volume traded during bootstrap
    pub volume_traded: u64,
    
    /// MMT rewards earned
    pub mmt_earned: u64,
    
    /// Number of trades
    pub trade_count: u64,
    
    /// Is early trader (first 100)
    pub is_early_trader: bool,
    
    /// First trade slot
    pub first_trade_slot: u64,
    
    /// Average leverage used
    pub avg_leverage: U64F64,
    
    /// Contribution to vault growth
    pub vault_contribution: u64,
    
    /// Referral bonus earned
    pub referral_bonus: u64,
    
    /// Referred traders count
    pub referred_count: u64,
}

#[account]
pub struct BootstrapMilestone {
    /// Milestone index
    pub index: u64,
    
    /// Required vault balance
    pub vault_target: u64,
    
    /// Required coverage ratio
    pub coverage_target: U64F64,
    
    /// Required unique traders
    pub traders_target: u64,
    
    /// MMT bonus pool for this milestone
    pub mmt_bonus_pool: u64,
    
    /// Is milestone achieved
    pub achieved: bool,
    
    /// Slot when achieved
    pub achieved_slot: u64,
    
    /// Traders who contributed to milestone
    pub top_contributors: Vec<Pubkey>,
}

/// Bootstrap incentive tiers based on contribution
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct IncentiveTier {
    /// Minimum volume for tier
    pub min_volume: u64,
    
    /// MMT reward multiplier
    pub reward_multiplier: U64F64,
    
    /// Fee rebate in bps
    pub fee_rebate_bps: u16,
    
    /// Priority in liquidation queue
    pub liquidation_priority: u8,
    
    /// Access to advanced features
    pub advanced_features: bool,
}

impl BootstrapState {
    pub const LEN: usize = 8 + // discriminator
        8 + // epoch
        8 + // initial_vault_balance
        8 + // current_vault_balance
        8 + // bootstrap_mmt_allocation
        8 + // mmt_distributed
        8 + // unique_traders
        8 + // total_volume
        1 + // status
        8 + // initial_coverage
        8 + // current_coverage
        8 + // target_coverage
        8 + // start_slot
        8 + // expected_end_slot
        8 + // early_bonus_multiplier
        8 + // early_traders_count
        8 + // max_early_traders
        8 + // min_trade_size
        2 + // bootstrap_fee_bps
        256; // padding
    
    pub fn init(&mut self, clock: &Clock) -> Result<()> {
        self.epoch = 1;
        self.initial_vault_balance = 0;
        self.current_vault_balance = 0;
        self.bootstrap_mmt_allocation = 2_000_000 * 10u64.pow(6); // 2M MMT with 6 decimals
        self.mmt_distributed = 0;
        self.unique_traders = 0;
        self.total_volume = 0;
        self.status = BootstrapStatus::Active;
        self.initial_coverage = U64F64::from_num(0);
        self.current_coverage = U64F64::from_num(0);
        self.target_coverage = U64F64::from_num(1); // 1.0 = 100% coverage
        self.start_slot = clock.slot;
        self.expected_end_slot = clock.slot + 38_880_000; // 6 months
        self.early_bonus_multiplier = U64F64::from_num(2); // 2x for early traders
        self.early_traders_count = 0;
        self.max_early_traders = 100;
        self.min_trade_size = 10 * 10u64.pow(6); // $10 USDC
        self.bootstrap_fee_bps = 28; // Start at max to build vault quickly
        
        Ok(())
    }
    
    /// Calculate dynamic fee based on coverage progress
    pub fn calculate_bootstrap_fee(&self) -> u16 {
        // Fee decreases as coverage increases
        // 28 bps at 0% coverage, 3 bps at 100% coverage
        let coverage_ratio = self.current_coverage.min(U64F64::from_num(1));
        let fee_reduction = coverage_ratio * U64F64::from_num(25);
        let base_fee = U64F64::from_num(3);
        let total_fee = base_fee + (U64F64::from_num(25) - fee_reduction);
        
        total_fee.to_num::<u16>()
    }
    
    /// Check if bootstrap should end
    pub fn should_end_bootstrap(&self, clock: &Clock) -> bool {
        // End if target coverage reached or time expired
        self.current_coverage >= self.target_coverage ||
        clock.slot >= self.expected_end_slot
    }
    
    /// Calculate MMT rewards for a trade
    pub fn calculate_mmt_reward(
        &self,
        trade_volume: u64,
        is_early_trader: bool,
        trader_tier: &IncentiveTier,
    ) -> u64 {
        let base_reward = (trade_volume as u128 * 100) / 10_000; // 1% base
        
        let multiplier = if is_early_trader {
            self.early_bonus_multiplier * trader_tier.reward_multiplier
        } else {
            trader_tier.reward_multiplier
        };
        
        let total_reward = U64F64::from_num(base_reward) * multiplier;
        total_reward.to_num::<u64>()
    }
}

impl BootstrapTrader {
    pub const LEN: usize = 8 + // discriminator
        32 + // trader
        8 + // volume_traded
        8 + // mmt_earned
        8 + // trade_count
        1 + // is_early_trader
        8 + // first_trade_slot
        8 + // avg_leverage
        8 + // vault_contribution
        8 + // referral_bonus
        8; // referred_count
}

impl BootstrapMilestone {
    pub const LEN: usize = 8 + // discriminator
        8 + // index
        8 + // vault_target
        8 + // coverage_target
        8 + // traders_target
        8 + // mmt_bonus_pool
        1 + // achieved
        8 + // achieved_slot
        4 + 32 * 10; // top_contributors (max 10)
}