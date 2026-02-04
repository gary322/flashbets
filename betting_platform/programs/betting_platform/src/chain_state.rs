use anchor_lang::prelude::*;
use crate::fixed_math::FixedPoint;

// ============= Chain State Management =============

#[account]
pub struct ChainStatePDA {
    pub verse_id: u128,
    pub user: Pubkey,
    pub chain_id: u128,
    pub steps_completed: u8,
    pub max_steps: u8,
    pub initial_deposit: u64,
    pub current_value: u64,
    pub effective_leverage: FixedPoint,
    pub step_states: Vec<ChainStepState>,
    pub status: ChainStatus,
    pub created_slot: u64,
    pub last_update_slot: u64,
}

impl ChainStatePDA {
    pub const BASE_LEN: usize = 8 + // discriminator
        16 + // verse_id
        32 + // user
        16 + // chain_id
        1 + // steps_completed
        1 + // max_steps
        8 + // initial_deposit
        8 + // current_value
        16 + // effective_leverage (FixedPoint)
        1 + // status
        8 + // created_slot
        8; // last_update_slot
    
    pub const LEN: usize = Self::BASE_LEN + 4 + (5 * ChainStepState::LEN); // Max 5 steps
    
    pub fn space(max_steps: usize) -> usize {
        Self::BASE_LEN + 4 + (max_steps * ChainStepState::LEN)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ChainStepState {
    pub step_type: ChainStepType,
    pub input_amount: u64,
    pub output_amount: u64,
    pub leverage_multiplier: FixedPoint,
    pub position_id: Option<u128>,
    pub status: StepStatus,
    pub error_code: Option<u32>,
}

impl ChainStepState {
    pub const LEN: usize = 1 + // step_type
        8 + // input_amount
        8 + // output_amount
        16 + // leverage_multiplier
        17 + // position_id (Option)
        1 + // status
        5; // error_code (Option)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum ChainStepType {
    Borrow,
    Liquidity,
    Stake,
    Arbitrage,
}

// Re-export for use in other modules
pub use ChainStepType::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum ChainStatus {
    Active,
    Paused,
    Unwinding,
    Completed,
    Failed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Reverted,
}

// ============= Chain Result Structures =============

#[derive(Clone)]
pub struct ChainStepResult {
    pub step_state: ChainStepState,
    pub output_amount: u64,
    pub leverage_multiplier: FixedPoint,
}

#[derive(Clone)]
pub struct UnwindResult {
    pub recovered_amount: u64,
}

// ============= Supporting PDAs for Chaining =============

#[account]
pub struct VerseLiquidityPool {
    pub verse_id: u128,
    pub total_liquidity: u64,
    pub lp_token_supply: u64,
    pub fee_rate: u64, // basis points
    pub accumulated_fees: u64,
    pub bump: u8,
}

impl VerseLiquidityPool {
    pub const LEN: usize = 8 + 16 + 8 + 8 + 8 + 8 + 1;
}

#[account]
pub struct VerseStakingPool {
    pub verse_id: u128,
    pub total_staked: u64,
    pub reward_rate: u64, // per slot
    pub last_reward_slot: u64,
    pub accumulated_rewards: u64,
    pub bump: u8,
}

impl VerseStakingPool {
    pub const LEN: usize = 8 + 16 + 8 + 8 + 8 + 8 + 1;
}

// ============= Helper Structs =============

// Import Token type
use anchor_spl::token::Token;

// Helper structs for passing accounts during step execution
pub struct AutoChainAccounts<'info> {
    pub global_config: &'info Account<'info, crate::account_structs::GlobalConfigPDA>,
    pub verse_pda: &'info Account<'info, crate::account_structs::VersePDA>,
    pub verse_liquidity_pool: &'info Account<'info, VerseLiquidityPool>,
    pub verse_staking_pool: &'info Account<'info, VerseStakingPool>,
}