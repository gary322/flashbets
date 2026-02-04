use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Health tracking for a high leverage position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionHealth {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Is initialized
    pub is_initialized: bool,
    
    /// Position ID
    pub position_id: [u8; 32],
    
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Trader pubkey
    pub trader: Pubkey,
    
    /// Entry price (fixed point 6 decimals)
    pub entry_price: u64,
    
    /// Current price (fixed point 6 decimals)
    pub current_price: u64,
    
    /// Position side (true = long, false = short)
    pub side: bool,
    
    /// Base leverage
    pub base_leverage: u64,
    
    /// Effective leverage (including chains)
    pub effective_leverage: u64,
    
    /// Chain steps applied
    pub chain_steps: Vec<ChainStep>,
    
    /// Health ratio (fixed point 6 decimals)
    pub health_ratio: u64,
    
    /// Liquidation price (fixed point 6 decimals)
    pub liquidation_price: u64,
    
    /// Time to estimated liquidation (seconds)
    pub time_to_liquidation: i64,
    
    /// Last health check slot
    pub last_check_slot: u64,
    
    /// Last health check timestamp
    pub last_check_timestamp: i64,
    
    /// Warning level
    pub warning_level: WarningLevel,
    
    /// In liquidation queue
    pub in_liquidation_queue: bool,
    
    /// Partial liquidations executed
    pub partial_liquidations: u32,
    
    /// Total liquidated amount
    pub total_liquidated: u64,
}

/// Chain step that multiplies leverage
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct ChainStep {
    pub step_type: ChainStepType,
    pub multiplier: u64, // Fixed point 6 decimals
    pub applied_at_slot: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ChainStepType {
    Borrow,    // 1.5x multiplier
    Liquidity, // 1.2x multiplier
    Stake,     // 1.1x multiplier
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum WarningLevel {
    None,
    Low,      // Health ratio < 1.5
    Medium,   // Health ratio < 1.3
    High,     // Health ratio < 1.1
    Critical, // Health ratio < 1.05
}

impl PositionHealth {
    pub const DISCRIMINATOR: [u8; 8] = [80, 79, 83, 95, 72, 76, 84, 72]; // "POS_HLTH"
    
    pub const LEN: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // position_id
        32 + // market_id
        32 + // trader
        8 + // entry_price
        8 + // current_price
        1 + // side
        8 + // base_leverage
        8 + // effective_leverage
        4 + (10 * 17) + // chain_steps vec (max 10 steps)
        8 + // health_ratio
        8 + // liquidation_price
        8 + // time_to_liquidation
        8 + // last_check_slot
        8 + // last_check_timestamp
        1 + // warning_level
        1 + // in_liquidation_queue
        4 + // partial_liquidations
        8 + // total_liquidated
        64; // padding
    
    /// Create new position health tracker
    pub fn new(
        position_id: [u8; 32],
        market_id: [u8; 32],
        trader: Pubkey,
        entry_price: u64,
        side: bool,
        base_leverage: u64,
    ) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            position_id,
            market_id,
            trader,
            entry_price,
            current_price: entry_price,
            side,
            base_leverage,
            effective_leverage: base_leverage,
            chain_steps: Vec::new(),
            health_ratio: 10_000_000, // 10.0 (very healthy)
            liquidation_price: 0,
            time_to_liquidation: i64::MAX,
            last_check_slot: 0,
            last_check_timestamp: 0,
            warning_level: WarningLevel::None,
            in_liquidation_queue: false,
            partial_liquidations: 0,
            total_liquidated: 0,
        }
    }
    
    /// Add chain step and recalculate effective leverage
    pub fn add_chain_step(&mut self, step_type: ChainStepType, slot: u64) -> Result<(), ProgramError> {
        if self.chain_steps.len() >= 10 {
            return Err(crate::error::LeverageSafetyError::DepthLimitExceeded.into());
        }
        
        let multiplier = match step_type {
            ChainStepType::Borrow => 1_500_000,    // 1.5x
            ChainStepType::Liquidity => 1_200_000, // 1.2x
            ChainStepType::Stake => 1_100_000,     // 1.1x
        };
        
        self.chain_steps.push(ChainStep {
            step_type,
            multiplier,
            applied_at_slot: slot,
        });
        
        // Recalculate effective leverage
        self.recalculate_effective_leverage()?;
        
        Ok(())
    }
    
    /// Recalculate effective leverage from chain steps
    pub fn recalculate_effective_leverage(&mut self) -> Result<(), ProgramError> {
        let mut effective = self.base_leverage as u128;
        
        for step in &self.chain_steps {
            effective = effective
                .checked_mul(step.multiplier as u128)
                .ok_or(crate::error::LeverageSafetyError::ArithmeticOverflow)?
                / 1_000_000; // Divide by decimals
        }
        
        // Cap at max allowed
        if effective > 500_000_000 { // 500x with 6 decimals
            effective = 500_000_000;
        }
        
        self.effective_leverage = effective as u64;
        Ok(())
    }
    
    /// Calculate current PnL percentage
    pub fn calculate_pnl_percent(&self) -> Result<i64, ProgramError> {
        let price_diff = if self.current_price > self.entry_price {
            (self.current_price - self.entry_price) as i128
        } else {
            -((self.entry_price - self.current_price) as i128)
        };
        
        let pnl_percent = if self.side {
            // Long position
            price_diff * 1_000_000 / self.entry_price as i128
        } else {
            // Short position
            -price_diff * 1_000_000 / self.entry_price as i128
        };
        
        Ok(pnl_percent as i64)
    }
    
    /// Calculate health ratio
    pub fn calculate_health_ratio(&mut self) -> Result<(), ProgramError> {
        let pnl_percent = self.calculate_pnl_percent()?;
        
        // Health ratio = 1 + (pnl% * leverage)
        // At 100x leverage, +1% move = +100% gain on position
        // pnl_percent is in fixed point (10_000 = 1%), effective_leverage is raw (100 = 100x)
        let leverage_adjusted_pnl = (pnl_percent as i128)
            .checked_mul(self.effective_leverage as i128)
            .ok_or(crate::error::LeverageSafetyError::ArithmeticOverflow)?
            / 1_000_000;
        
        let health_ratio = 1_000_000i128 + leverage_adjusted_pnl;
        
        if health_ratio < 0 {
            self.health_ratio = 0;
        } else {
            self.health_ratio = health_ratio as u64;
        }
        
        // Update warning level
        self.warning_level = match self.health_ratio {
            0..=1_050_000 => WarningLevel::Critical,      // <= 1.05
            1_050_001..=1_100_000 => WarningLevel::High, // 1.05 - 1.1
            1_100_001..=1_300_000 => WarningLevel::Medium, // 1.1 - 1.3
            1_300_001..=1_500_000 => WarningLevel::Low,  // 1.3 - 1.5
            _ => WarningLevel::None,
        };
        
        Ok(())
    }
    
    /// Calculate liquidation price
    pub fn calculate_liquidation_price(&mut self) -> Result<(), ProgramError> {
        // At liquidation, health ratio = 0
        // Health ratio = 1 + (pnl% * leverage) / 1_000_000
        // 0 = 1 + (pnl% * leverage) / 1_000_000
        // pnl% = -1_000_000 / leverage
        let max_loss_percent = -1_000_000i128 / self.effective_leverage as i128;
        
        if self.side {
            // Long: liq_price = entry_price * (1 + max_loss%)
            let multiplier = 1_000_000 + max_loss_percent;
            self.liquidation_price = (self.entry_price as i128 * multiplier / 1_000_000) as u64;
        } else {
            // Short: liq_price = entry_price * (1 - max_loss%)
            let multiplier = 1_000_000 - max_loss_percent;
            self.liquidation_price = (self.entry_price as i128 * multiplier / 1_000_000) as u64;
        }
        
        Ok(())
    }
    
    /// Check if position should be liquidated
    pub fn should_liquidate(&self) -> bool {
        self.health_ratio < 1_000_000 // Health ratio < 1.0
    }
    
    /// Check if position should be added to liquidation queue
    pub fn should_queue_for_liquidation(&self) -> bool {
        self.warning_level == WarningLevel::Critical && !self.in_liquidation_queue
    }
}