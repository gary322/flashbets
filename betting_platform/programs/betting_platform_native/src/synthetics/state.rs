//! State Management for Synthetic Tokens
//!
//! Tracks synthetic positions and collateral

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    account_validation::DISCRIMINATOR_SIZE,
};

/// Seed for synthetic state PDA
pub const SYNTHETIC_STATE_SEED: &[u8] = b"synthetic_state";

/// Discriminator for synthetic state
pub const SYNTHETIC_STATE_DISCRIMINATOR: [u8; 8] = [83, 89, 78, 84, 72, 83, 84, 84]; // "SYNTHSTT"

/// Main synthetic state account
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SyntheticState {
    /// Discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Owner of the synthetic position
    pub owner: Pubkey,
    
    /// Market ID
    pub market_id: u128,
    
    /// Synthetic token mint
    pub synthetic_mint: Pubkey,
    
    /// Collateral mint (USDC)
    pub collateral_mint: Pubkey,
    
    /// Total synthetic minted
    pub total_synthetic: u128,
    
    /// Total collateral locked
    pub total_collateral: u128,
    
    /// Current leverage
    pub current_leverage: u16,
    
    /// Max leverage allowed
    pub max_leverage: u16,
    
    /// Oracle account
    pub oracle_account: Pubkey,
    
    /// Last oracle scalar
    pub last_oracle_scalar: f64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Position health (0-100)
    pub health: u8,
    
    /// Liquidation price
    pub liquidation_price: f64,
    
    /// Is liquidated
    pub is_liquidated: bool,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// All positions under this state
    pub positions: Vec<SyntheticPosition>,
    
    /// Total profit/loss
    pub total_pnl: i64,
    
    /// Risk parameters
    pub risk_params: RiskParameters,
}

/// Individual synthetic position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SyntheticPosition {
    /// Position ID
    pub position_id: u128,
    
    /// Amount of synthetic tokens
    pub synthetic_amount: u128,
    
    /// Collateral backing this position
    pub collateral_amount: u128,
    
    /// Entry price (oracle value)
    pub entry_price: f64,
    
    /// Current price
    pub current_price: f64,
    
    /// Leverage at entry
    pub entry_leverage: u16,
    
    /// Position type
    pub position_type: PositionType,
    
    /// Opening slot
    pub open_slot: u64,
    
    /// Closing slot (0 if still open)
    pub close_slot: u64,
    
    /// Realized PnL
    pub realized_pnl: i64,
    
    /// Unrealized PnL
    pub unrealized_pnl: i64,
    
    /// Is active
    pub is_active: bool,
    
    /// Stop loss price
    pub stop_loss: Option<f64>,
    
    /// Take profit price
    pub take_profit: Option<f64>,
}

/// Position types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum PositionType {
    Long,
    Short,
    Neutral,
    Hedged,
}

/// Risk parameters for synthetic positions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RiskParameters {
    /// Maximum drawdown allowed
    pub max_drawdown: f64,
    
    /// Maintenance margin ratio
    pub maintenance_margin: f64,
    
    /// Initial margin ratio
    pub initial_margin: f64,
    
    /// Auto-deleverage threshold
    pub auto_deleverage_threshold: f64,
    
    /// Maximum position size
    pub max_position_size: u128,
    
    /// Risk tier (1-5)
    pub risk_tier: u8,
}

/// Collateral information
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CollateralInfo {
    /// Collateral token mint
    pub mint: Pubkey,
    
    /// Amount locked
    pub locked_amount: u128,
    
    /// Available amount
    pub available_amount: u128,
    
    /// Value in USD
    pub usd_value: u128,
    
    /// Last price update
    pub last_price_update: u64,
    
    /// Collateral ratio
    pub collateral_ratio: f64,
    
    /// Is accepted collateral
    pub is_accepted: bool,
}

impl SyntheticState {
    pub fn new(
        owner: Pubkey,
        market_id: u128,
        synthetic_mint: Pubkey,
        collateral_mint: Pubkey,
        oracle_account: Pubkey,
    ) -> Self {
        Self {
            discriminator: SYNTHETIC_STATE_DISCRIMINATOR,
            owner,
            market_id,
            synthetic_mint,
            collateral_mint,
            total_synthetic: 0,
            total_collateral: 0,
            current_leverage: 1,
            max_leverage: 100,
            oracle_account,
            last_oracle_scalar: 1.0,
            last_update_slot: 0,
            health: 100,
            liquidation_price: 0.0,
            is_liquidated: false,
            created_at: 0,
            positions: Vec::new(),
            total_pnl: 0,
            risk_params: RiskParameters::default(),
        }
    }
    
    /// Validate state
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != SYNTHETIC_STATE_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.is_liquidated {
            msg!("Position is liquidated");
            return Err(BettingPlatformError::PositionLiquidated.into());
        }
        
        if self.health == 0 {
            msg!("Position health is zero");
            return Err(BettingPlatformError::UnhealthyPosition.into());
        }
        
        Ok(())
    }
    
    /// Add a new position
    pub fn add_position(
        &mut self,
        position_id: u128,
        synthetic_amount: u128,
        collateral_amount: u128,
        entry_price: f64,
        leverage: u16,
        position_type: PositionType,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check max positions
        if self.positions.len() >= 100 {
            return Err(BettingPlatformError::MaxPositionsReached.into());
        }
        
        let position = SyntheticPosition {
            position_id,
            synthetic_amount,
            collateral_amount,
            entry_price,
            current_price: entry_price,
            entry_leverage: leverage,
            position_type,
            open_slot: current_slot,
            close_slot: 0,
            realized_pnl: 0,
            unrealized_pnl: 0,
            is_active: true,
            stop_loss: None,
            take_profit: None,
        };
        
        self.positions.push(position);
        
        // Update totals
        self.total_synthetic = self.total_synthetic
            .checked_add(synthetic_amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        self.total_collateral = self.total_collateral
            .checked_add(collateral_amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        
        // Update leverage
        if self.total_collateral > 0 {
            self.current_leverage = ((self.total_synthetic / self.total_collateral) as u16)
                .min(self.max_leverage);
        }
        
        Ok(())
    }
    
    /// Close a position
    pub fn close_position(
        &mut self,
        position_id: u128,
        exit_price: f64,
        current_slot: u64,
    ) -> Result<i64, ProgramError> {
        let position = self.positions
            .iter_mut()
            .find(|p| p.position_id == position_id && p.is_active)
            .ok_or(BettingPlatformError::PositionNotFound)?;
        
        // Calculate PnL
        let price_change = exit_price - position.entry_price;
        let pnl_percent = price_change / position.entry_price;
        let base_pnl = (position.collateral_amount as f64 * pnl_percent) as i64;
        
        // Apply leverage
        let leveraged_pnl = base_pnl * position.entry_leverage as i64;
        
        // Update position
        position.current_price = exit_price;
        position.close_slot = current_slot;
        position.realized_pnl = leveraged_pnl;
        position.is_active = false;
        
        // Update state totals
        self.total_synthetic = self.total_synthetic
            .saturating_sub(position.synthetic_amount);
        
        self.total_collateral = self.total_collateral
            .saturating_sub(position.collateral_amount);
        
        self.total_pnl += leveraged_pnl;
        
        Ok(leveraged_pnl)
    }
    
    /// Update position prices
    pub fn update_prices(&mut self, current_price: f64) {
        for position in self.positions.iter_mut() {
            if position.is_active {
                position.current_price = current_price;
                
                // Calculate unrealized PnL
                let price_change = current_price - position.entry_price;
                let pnl_percent = price_change / position.entry_price;
                let base_pnl = (position.collateral_amount as f64 * pnl_percent) as i64;
                position.unrealized_pnl = base_pnl * position.entry_leverage as i64;
                
                // Check stop loss
                if let Some(stop_loss) = position.stop_loss {
                    if current_price <= stop_loss {
                        position.is_active = false;
                        position.realized_pnl = position.unrealized_pnl;
                    }
                }
                
                // Check take profit
                if let Some(take_profit) = position.take_profit {
                    if current_price >= take_profit {
                        position.is_active = false;
                        position.realized_pnl = position.unrealized_pnl;
                    }
                }
            }
        }
    }
    
    /// Calculate position health
    pub fn calculate_health(&mut self, oracle_scalar: f64) -> u8 {
        if self.total_collateral == 0 {
            return 0;
        }
        
        // Calculate total value
        let synthetic_value = (self.total_synthetic as f64 / oracle_scalar) as u128;
        
        // Calculate health ratio
        let health_ratio = self.total_collateral as f64 / synthetic_value as f64;
        
        // Convert to 0-100 scale
        let health = (health_ratio * 100.0).min(100.0).max(0.0) as u8;
        
        self.health = health;
        
        // Calculate liquidation price
        self.liquidation_price = self.risk_params.maintenance_margin / health_ratio;
        
        health
    }
    
    /// Check if should liquidate
    pub fn should_liquidate(&self, current_price: f64) -> bool {
        if self.health < 20 {
            return true;
        }
        
        if current_price <= self.liquidation_price {
            return true;
        }
        
        // Check max drawdown
        let total_unrealized: i64 = self.positions
            .iter()
            .filter(|p| p.is_active)
            .map(|p| p.unrealized_pnl)
            .sum();
        
        let total_value = self.total_collateral as i64 + total_unrealized;
        let drawdown = if self.total_collateral > 0 {
            1.0 - (total_value as f64 / self.total_collateral as f64)
        } else {
            0.0
        };
        
        drawdown > self.risk_params.max_drawdown
    }
    
    /// Liquidate all positions
    pub fn liquidate(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        for position in self.positions.iter_mut() {
            if position.is_active {
                position.is_active = false;
                position.close_slot = current_slot;
                position.realized_pnl = position.unrealized_pnl;
            }
        }
        
        self.is_liquidated = true;
        self.health = 0;
        
        Ok(())
    }
}

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            max_drawdown: 0.5, // 50%
            maintenance_margin: 0.05, // 5%
            initial_margin: 0.1, // 10%
            auto_deleverage_threshold: 0.02, // 2%
            max_position_size: 10_000_000 * 10u128.pow(9), // 10M tokens
            risk_tier: 3, // Medium risk
        }
    }
}

impl SyntheticPosition {
    /// Set stop loss
    pub fn set_stop_loss(&mut self, price: f64) {
        self.stop_loss = Some(price);
    }
    
    /// Set take profit
    pub fn set_take_profit(&mut self, price: f64) {
        self.take_profit = Some(price);
    }
    
    /// Get position value
    pub fn get_value(&self) -> u128 {
        let price_ratio = self.current_price / self.entry_price;
        (self.collateral_amount as f64 * price_ratio) as u128
    }
    
    /// Get margin ratio
    pub fn get_margin_ratio(&self) -> f64 {
        if self.synthetic_amount == 0 {
            return 1.0;
        }
        
        self.collateral_amount as f64 / self.synthetic_amount as f64
    }
}

/// Derive synthetic state PDA
pub fn derive_synthetic_state_pda(
    program_id: &Pubkey,
    owner: &Pubkey,
    market_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SYNTHETIC_STATE_SEED,
            owner.as_ref(),
            &market_id.to_le_bytes(),
        ],
        program_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_synthetic_state() {
        let mut state = SyntheticState::new(
            Pubkey::default(),
            12345,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        
        // Test adding position
        assert!(state.add_position(
            1,
            1000,
            100,
            0.5,
            10,
            PositionType::Long,
            1000,
        ).is_ok());
        
        assert_eq!(state.positions.len(), 1);
        assert_eq!(state.total_synthetic, 1000);
        assert_eq!(state.total_collateral, 100);
    }
    
    #[test]
    fn test_position_pnl() {
        let mut state = SyntheticState::new(
            Pubkey::default(),
            12345,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        
        // Add position
        state.add_position(
            1,
            1000,
            100,
            0.5,
            10,
            PositionType::Long,
            1000,
        ).unwrap();
        
        // Close with profit
        let pnl = state.close_position(1, 0.6, 2000).unwrap();
        assert!(pnl > 0); // Should be profitable
        
        // Check position is closed
        assert!(!state.positions[0].is_active);
    }
    
    #[test]
    fn test_health_calculation() {
        let mut state = SyntheticState::new(
            Pubkey::default(),
            12345,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        
        state.total_synthetic = 1000;
        state.total_collateral = 150;
        
        let health = state.calculate_health(2.0);
        assert!(health > 0);
        assert!(health <= 100);
    }
}