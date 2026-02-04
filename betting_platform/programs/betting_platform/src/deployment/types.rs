use anchor_lang::prelude::*;

// GlobalConfig for deployment module - extends the base GlobalConfigPDA
#[derive(Clone, Debug, Default)]
pub struct GlobalConfig {
    // Base fields from GlobalConfigPDA
    pub epoch: u64,
    pub coverage: u128,
    pub vault: u64,
    pub total_oi: u64,
    pub halt_flag: bool,
    pub halt_until: u64,
    pub fee_base: u64,
    pub fee_slope: u64,
    pub season: u64,
    pub genesis_slot: u64,
    pub season_start_slot: u64,
    pub season_end_slot: u64,
    pub mmt_total_supply: u64,
    pub mmt_current_season: u64,
    pub mmt_emission_rate: u64,
    pub mmt_reward_pool: u64,
    
    // Bootstrap-specific fields
    pub bootstrap_mode: bool,
    pub bootstrap_trade_count: u64,
    pub bootstrap_max_trades: u64,
    pub maker_bonus_multiplier: f64,
    pub liquidity_mining_active: bool,
    pub liquidity_mining_rate: f64,
}

impl GlobalConfig {
    pub const INIT_SPACE: usize = 8 + // discriminator
        8 + // epoch
        16 + // coverage
        8 + // vault
        8 + // total_oi
        1 + // halt_flag
        8 + // halt_until
        8 + // fee_base
        8 + // fee_slope
        8 + // season
        8 + // genesis_slot
        8 + // season_start_slot
        8 + // season_end_slot
        8 + // mmt_total_supply
        8 + // mmt_current_season
        8 + // mmt_emission_rate
        8 + // mmt_reward_pool
        1 + // bootstrap_mode
        8 + // bootstrap_trade_count
        8 + // bootstrap_max_trades
        8 + // maker_bonus_multiplier (stored as u64)
        1 + // liquidity_mining_active
        8 + // liquidity_mining_rate (stored as u64)
        256; // Extra space for future upgrades
}