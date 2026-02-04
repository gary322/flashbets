use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Main configuration for the leverage safety system
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LeverageSafetyConfig {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Is initialized flag
    pub is_initialized: bool,
    
    /// Authority that can update config
    pub authority: Pubkey,
    
    /// Maximum base leverage (default: 100x)
    pub max_base_leverage: u64,
    
    /// Maximum effective leverage through chaining (default: 500x)
    pub max_effective_leverage: u64,
    
    /// Chain depth multiplier (default: 0.1 per depth level)
    /// Stored as fixed point with 6 decimals (100_000 = 0.1)
    pub chain_depth_multiplier: u64,
    
    /// Minimum coverage for emergency halt (default: 0.5)
    /// Stored as fixed point with 6 decimals (500_000 = 0.5)
    pub coverage_minimum: u64,
    
    /// Liquidation parameters
    pub liquidation_params: LiquidationParameters,
    
    /// Risk tier configurations
    pub tier_caps: Vec<TierCap>,
    
    /// Volatility adjustment enabled
    pub volatility_adjustment: bool,
    
    /// Correlation penalty factor
    /// Stored as fixed point with 6 decimals
    pub correlation_penalty: u64,
    
    /// Emergency halt active
    pub emergency_halt: bool,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Stats
    pub total_positions_monitored: u64,
    pub total_liquidations: u64,
    pub total_warnings_issued: u64,
}

/// Liquidation parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LiquidationParameters {
    /// Maximum liquidation per slot as percentage (default: 8%)
    /// Stored as basis points (800 = 8%)
    pub partial_liq_percent: u16,
    
    /// Liquidation buffer in basis points (default: 200 = 2%)
    pub liq_buffer_bps: u16,
    
    /// Minimum health ratio (default: 1.1)
    /// Stored as fixed point with 6 decimals (1_100_000 = 1.1)
    pub min_health_ratio: u64,
    
    /// Liquidation fee in basis points
    pub liquidation_fee_bps: u16,
    
    /// Cooldown slots between liquidations
    pub liquidation_cooldown: u64,
}

/// Tier cap configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TierCap {
    /// Minimum outcome count for this tier (inclusive)
    pub min_outcomes: u8,
    
    /// Maximum outcome count for this tier (inclusive) 
    pub max_outcomes: u8,
    
    /// Maximum leverage for this tier
    pub max_leverage: u64,
}

impl LeverageSafetyConfig {
    pub const DISCRIMINATOR: [u8; 8] = [76, 69, 86, 95, 83, 65, 70, 69]; // "LEV_SAFE"
    
    pub const LEN: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // authority
        8 + // max_base_leverage
        8 + // max_effective_leverage
        8 + // chain_depth_multiplier
        8 + // coverage_minimum
        (2 + 2 + 8 + 2 + 8) + // liquidation_params: 22 bytes
        4 + (7 * 10) + // tier_caps vec (7 tiers max, 10 bytes each: u8+u8+u64)
        1 + // volatility_adjustment
        8 + // correlation_penalty
        1 + // emergency_halt
        8 + // last_update
        8 + // total_positions_monitored
        8 + // total_liquidations
        8 + // total_warnings_issued
        128; // padding for growth
    
    /// Create default configuration
    pub fn default(authority: Pubkey) -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            is_initialized: true,
            authority,
            max_base_leverage: 100,
            max_effective_leverage: 500,
            chain_depth_multiplier: 100_000, // 0.1
            coverage_minimum: 500_000, // 0.5
            liquidation_params: LiquidationParameters {
                partial_liq_percent: 800, // 8%
                liq_buffer_bps: 200, // 2%
                min_health_ratio: 1_100_000, // 1.1
                liquidation_fee_bps: 50, // 0.5%
                liquidation_cooldown: 10, // 10 slots
            },
            tier_caps: Self::default_tier_caps(),
            volatility_adjustment: true,
            correlation_penalty: 500_000, // 0.5
            emergency_halt: false,
            last_update: 0,
            total_positions_monitored: 0,
            total_liquidations: 0,
            total_warnings_issued: 0,
        }
    }
    
    /// Get default tier caps
    pub fn default_tier_caps() -> Vec<TierCap> {
        vec![
            TierCap { min_outcomes: 1, max_outcomes: 1, max_leverage: 100 },
            TierCap { min_outcomes: 2, max_outcomes: 2, max_leverage: 70 },
            TierCap { min_outcomes: 3, max_outcomes: 4, max_leverage: 25 },
            TierCap { min_outcomes: 5, max_outcomes: 8, max_leverage: 15 },
            TierCap { min_outcomes: 9, max_outcomes: 16, max_leverage: 12 },
            TierCap { min_outcomes: 17, max_outcomes: 64, max_leverage: 10 },
            TierCap { min_outcomes: 65, max_outcomes: 255, max_leverage: 5 },
        ]
    }
    
    /// Get tier cap for outcome count
    pub fn get_tier_cap(&self, outcome_count: u8) -> Result<u64, ProgramError> {
        for tier in &self.tier_caps {
            if outcome_count >= tier.min_outcomes && outcome_count <= tier.max_outcomes {
                return Ok(tier.max_leverage);
            }
        }
        
        // Default to lowest tier if not found
        Ok(5)
    }
    
    /// Check if emergency halt should be triggered
    pub fn should_emergency_halt(&self, current_coverage: u64) -> bool {
        current_coverage < self.coverage_minimum
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check discriminator
        if self.discriminator != Self::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        // Check initialized
        if !self.is_initialized {
            return Err(ProgramError::UninitializedAccount);
        }
        
        // Validate leverage limits
        if self.max_base_leverage == 0 || self.max_base_leverage > 1000 {
            return Err(crate::error::LeverageSafetyError::InvalidTierConfiguration.into());
        }
        
        if self.max_effective_leverage < self.max_base_leverage {
            return Err(crate::error::LeverageSafetyError::InvalidTierConfiguration.into());
        }
        
        // Validate tier caps are in order
        let mut last_max = 0;
        for tier in &self.tier_caps {
            if tier.min_outcomes <= last_max {
                return Err(crate::error::LeverageSafetyError::InvalidTierConfiguration.into());
            }
            last_max = tier.max_outcomes;
            
            if tier.max_leverage == 0 || tier.max_leverage > self.max_base_leverage {
                return Err(crate::error::LeverageSafetyError::InvalidTierConfiguration.into());
            }
        }
        
        Ok(())
    }
}