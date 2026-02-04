//! Perpetual State Management
//!
//! Core state structures for perpetual positions

use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    cdp::CDPAccount,
};

/// Perpetual position state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PerpetualPosition {
    /// Position ID
    pub position_id: u128,
    
    /// Owner of the position
    pub owner: Pubkey,
    
    /// Market ID
    pub market_id: u128,
    
    /// Underlying CDP account
    pub cdp_account: Pubkey,
    
    /// Position type
    pub position_type: PositionType,
    
    /// Entry price
    pub entry_price: f64,
    
    /// Current mark price
    pub mark_price: f64,
    
    /// Position size (notional)
    pub size: u128,
    
    /// Leverage used
    pub leverage: u16,
    
    /// Collateral amount
    pub collateral: u128,
    
    /// Accumulated funding
    pub accumulated_funding: i128,
    
    /// Last funding payment
    pub last_funding_payment: UnixTimestamp,
    
    /// Unrealized PnL
    pub unrealized_pnl: i128,
    
    /// Realized PnL
    pub realized_pnl: i128,
    
    /// Auto-roll enabled
    pub auto_roll_enabled: bool,
    
    /// Roll parameters
    pub roll_params: RollParameters,
    
    /// Position status
    pub status: PositionStatus,
    
    /// Created timestamp
    pub created_at: UnixTimestamp,
    
    /// Last updated
    pub last_updated: UnixTimestamp,
    
    /// Liquidation price
    pub liquidation_price: f64,
    
    /// Stop loss price
    pub stop_loss: Option<f64>,
    
    /// Take profit price
    pub take_profit: Option<f64>,
    
    /// Funding rate at entry
    pub entry_funding_rate: f64,
    
    /// Oracle scalar at entry
    pub entry_oracle_scalar: f64,
    
    /// Maximum allowed leverage
    pub max_leverage: u16,
    
    /// Margin ratio
    pub margin_ratio: f64,
    
    /// Maintenance margin
    pub maintenance_margin: f64,
    
    /// Initial margin
    pub initial_margin: f64,
    
    /// Position expiry (for dated futures)
    pub expiry: Option<UnixTimestamp>,
    
    /// Roll count
    pub roll_count: u32,
    
    /// Total fees paid
    pub total_fees_paid: u128,
}

/// Position type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum PositionType {
    Long,
    Short,
}

/// Position status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum PositionStatus {
    Active,
    Closing,
    Closed,
    Liquidated,
    Expired,
    RollingOver,
}

/// Roll parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RollParameters {
    /// Auto-roll before expiry (slots)
    pub roll_before_expiry: u64,
    
    /// Maximum rolls allowed
    pub max_rolls: u32,
    
    /// Roll to next expiry
    pub roll_to_next: bool,
    
    /// Preferred expiry duration
    pub preferred_duration: u64,
    
    /// Maximum slippage allowed
    pub max_slippage: f64,
    
    /// Roll fee cap
    pub max_roll_fee: u128,
    
    /// Last roll slot
    pub last_roll_slot: u64,
}

/// Perpetual market state
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PerpetualMarket {
    /// Market ID
    pub market_id: u128,
    
    /// Base token
    pub base_token: Pubkey,
    
    /// Quote token
    pub quote_token: Pubkey,
    
    /// Oracle account
    pub oracle_account: Pubkey,
    
    /// Index price
    pub index_price: f64,
    
    /// Mark price
    pub mark_price: f64,
    
    /// Funding rate
    pub funding_rate: f64,
    
    /// Next funding time
    pub next_funding_time: UnixTimestamp,
    
    /// Funding interval (seconds)
    pub funding_interval: u64,
    
    /// Open interest (long)
    pub open_interest_long: u128,
    
    /// Open interest (short)
    pub open_interest_short: u128,
    
    /// Total collateral
    pub total_collateral: u128,
    
    /// Insurance fund
    pub insurance_fund: u128,
    
    /// Max leverage
    pub max_leverage: u16,
    
    /// Min leverage
    pub min_leverage: u16,
    
    /// Initial margin ratio
    pub initial_margin_ratio: f64,
    
    /// Maintenance margin ratio
    pub maintenance_margin_ratio: f64,
    
    /// Liquidation fee
    pub liquidation_fee: f64,
    
    /// Trading fee
    pub trading_fee: f64,
    
    /// Funding fee
    pub funding_fee: f64,
    
    /// Market status
    pub status: MarketStatus,
    
    /// Created at
    pub created_at: UnixTimestamp,
    
    /// Last update
    pub last_update: UnixTimestamp,
}

/// Market status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum MarketStatus {
    Active,
    Paused,
    SettlementOnly,
    Delisted,
}

impl PerpetualPosition {
    /// Create new perpetual position
    pub fn new(
        position_id: u128,
        owner: Pubkey,
        market_id: u128,
        cdp_account: Pubkey,
        position_type: PositionType,
        entry_price: f64,
        size: u128,
        leverage: u16,
        collateral: u128,
    ) -> Self {
        let liquidation_price = Self::calculate_liquidation_price(
            entry_price,
            leverage,
            &position_type,
        );
        
        Self {
            position_id,
            owner,
            market_id,
            cdp_account,
            position_type,
            entry_price,
            mark_price: entry_price,
            size,
            leverage,
            collateral,
            accumulated_funding: 0,
            last_funding_payment: 0,
            unrealized_pnl: 0,
            realized_pnl: 0,
            auto_roll_enabled: true,
            roll_params: RollParameters::default(),
            status: PositionStatus::Active,
            created_at: 0,
            last_updated: 0,
            liquidation_price,
            stop_loss: None,
            take_profit: None,
            entry_funding_rate: 0.0,
            entry_oracle_scalar: 1.0,
            max_leverage: 1000,
            margin_ratio: 1.0 / leverage as f64,
            maintenance_margin: 0.005, // 0.5%
            initial_margin: 1.0 / leverage as f64,
            expiry: None,
            roll_count: 0,
            total_fees_paid: 0,
        }
    }
    
    /// Calculate liquidation price
    fn calculate_liquidation_price(
        entry_price: f64,
        leverage: u16,
        position_type: &PositionType,
    ) -> f64 {
        let maintenance_margin = 0.005; // 0.5%
        let margin_ratio = 1.0 / leverage as f64;
        
        match position_type {
            PositionType::Long => {
                entry_price * (1.0 - margin_ratio + maintenance_margin)
            },
            PositionType::Short => {
                entry_price * (1.0 + margin_ratio - maintenance_margin)
            }
        }
    }
    
    /// Update mark price and PnL
    pub fn update_mark_price(&mut self, new_mark_price: f64) {
        self.mark_price = new_mark_price;
        self.calculate_unrealized_pnl();
        self.last_updated = solana_program::clock::Clock::get()
            .map(|c| c.unix_timestamp)
            .unwrap_or(0);
    }
    
    /// Calculate unrealized PnL
    fn calculate_unrealized_pnl(&mut self) {
        let price_diff = self.mark_price - self.entry_price;
        
        let pnl = match self.position_type {
            PositionType::Long => (self.size as f64) * price_diff / self.entry_price,
            PositionType::Short => -(self.size as f64) * price_diff / self.entry_price,
        };
        
        self.unrealized_pnl = pnl as i128;
    }
    
    /// Check if position needs liquidation
    pub fn is_liquidatable(&self) -> bool {
        match self.position_type {
            PositionType::Long => self.mark_price <= self.liquidation_price,
            PositionType::Short => self.mark_price >= self.liquidation_price,
        }
    }
    
    /// Check if position should be rolled
    pub fn should_roll(&self, current_slot: u64) -> bool {
        if !self.auto_roll_enabled {
            return false;
        }
        
        if self.roll_count >= self.roll_params.max_rolls {
            return false;
        }
        
        // Check if near expiry
        if let Some(expiry) = self.expiry {
            let slots_to_expiry = (expiry as u64).saturating_sub(current_slot);
            return slots_to_expiry <= self.roll_params.roll_before_expiry;
        }
        
        false
    }
    
    /// Apply funding payment
    pub fn apply_funding(&mut self, funding_payment: i128) {
        self.accumulated_funding += funding_payment;
        self.last_funding_payment = solana_program::clock::Clock::get()
            .map(|c| c.unix_timestamp)
            .unwrap_or(0);
    }
    
    /// Close position
    pub fn close(&mut self, final_price: f64) -> Result<i128, ProgramError> {
        if self.status != PositionStatus::Active {
            return Err(BettingPlatformError::InvalidPositionStatus.into());
        }
        
        self.mark_price = final_price;
        self.calculate_unrealized_pnl();
        
        let total_pnl = self.unrealized_pnl + self.accumulated_funding;
        self.realized_pnl = total_pnl;
        self.status = PositionStatus::Closed;
        
        Ok(total_pnl)
    }
}

impl Default for RollParameters {
    fn default() -> Self {
        Self {
            roll_before_expiry: 432000, // ~3 days at 2 slots/sec
            max_rolls: 12, // Monthly rolls for a year
            roll_to_next: true,
            preferred_duration: 2592000, // ~30 days
            max_slippage: 0.01, // 1%
            max_roll_fee: 1000000, // 1 USDC
            last_roll_slot: 0,
        }
    }
}

impl PerpetualMarket {
    /// Create new perpetual market
    pub fn new(
        market_id: u128,
        base_token: Pubkey,
        quote_token: Pubkey,
        oracle_account: Pubkey,
    ) -> Self {
        Self {
            market_id,
            base_token,
            quote_token,
            oracle_account,
            index_price: 0.0,
            mark_price: 0.0,
            funding_rate: 0.0,
            next_funding_time: 0,
            funding_interval: 3600, // 1 hour
            open_interest_long: 0,
            open_interest_short: 0,
            total_collateral: 0,
            insurance_fund: 0,
            max_leverage: 1000,
            min_leverage: 1,
            initial_margin_ratio: 0.01, // 1%
            maintenance_margin_ratio: 0.005, // 0.5%
            liquidation_fee: 0.001, // 0.1%
            trading_fee: 0.0005, // 0.05%
            funding_fee: 0.0001, // 0.01%
            status: MarketStatus::Active,
            created_at: 0,
            last_update: 0,
        }
    }
    
    /// Update market prices
    pub fn update_prices(&mut self, index_price: f64, mark_price: f64) {
        self.index_price = index_price;
        self.mark_price = mark_price;
        self.last_update = solana_program::clock::Clock::get()
            .map(|c| c.unix_timestamp)
            .unwrap_or(0);
    }
    
    /// Calculate funding rate
    pub fn calculate_funding_rate(&self) -> f64 {
        // Premium = (Mark - Index) / Index
        let premium = (self.mark_price - self.index_price) / self.index_price;
        
        // Funding rate = Premium * funding_interval / 86400
        let funding_rate = premium * (self.funding_interval as f64) / 86400.0;
        
        // Cap at ±0.75% per funding period
        funding_rate.max(-0.0075).min(0.0075)
    }
    
    /// Update open interest
    pub fn update_open_interest(
        &mut self,
        position_type: &PositionType,
        size: u128,
        is_increase: bool,
    ) {
        match position_type {
            PositionType::Long => {
                if is_increase {
                    self.open_interest_long += size;
                } else {
                    self.open_interest_long = self.open_interest_long.saturating_sub(size);
                }
            },
            PositionType::Short => {
                if is_increase {
                    self.open_interest_short += size;
                } else {
                    self.open_interest_short = self.open_interest_short.saturating_sub(size);
                }
            }
        }
    }
}

/// Derive perpetual position PDA
pub fn derive_perpetual_position_pda(
    program_id: &Pubkey,
    owner: &Pubkey,
    position_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"perpetual_position",
            owner.as_ref(),
            &position_id.to_le_bytes(),
        ],
        program_id,
    )
}

/// Derive perpetual market PDA
pub fn derive_perpetual_market_pda(
    program_id: &Pubkey,
    market_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"perpetual_market",
            &market_id.to_le_bytes(),
        ],
        program_id,
    )
}