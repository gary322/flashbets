use anchor_lang::prelude::*;
use fixed::types::U64F64;
use crate::state::IncentiveTier;

#[derive(Debug, Clone)]
pub struct BootstrapTradeResult {
    pub mmt_reward: u64,
    pub fee_rebate: u64,
    pub net_fee: u64,
    pub new_coverage: U64F64,
    pub is_early_trader: bool,
    pub tier: IncentiveTier,
}