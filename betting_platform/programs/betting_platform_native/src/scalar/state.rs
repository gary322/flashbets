//! Unified Scalar State Structures
//!
//! Defines the core state for unified pricing and risk calculations

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};

/// Unified scalar state for a market
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub struct UnifiedScalar {
    /// Market identifier
    pub market_id: u128,
    
    /// Base price from oracle (8 decimals)
    pub oracle_price: u64,
    
    /// Oracle confidence interval (basis points)
    pub oracle_confidence: u16,
    
    /// Synthetic token price adjustment factor (basis points)
    pub synthetic_adjustment: i16,
    
    /// CDP collateral factor (basis points, 10000 = 100%)
    pub cdp_collateral_factor: u16,
    
    /// Perpetual funding rate (per hour, basis points)
    pub perp_funding_rate: i16,
    
    /// Vault yield impact (basis points)
    pub vault_yield_impact: i16,
    
    /// Combined risk score (0-10000, higher = riskier)
    pub risk_score: u16,
    
    /// Liquidity depth (in base units)
    pub liquidity_depth: u128,
    
    /// 24h volume
    pub volume_24h: u128,
    
    /// Market volatility (standard deviation, basis points)
    pub volatility: u16,
    
    /// Last update timestamp
    pub last_update: UnixTimestamp,
    
    /// Update authority
    pub authority: Pubkey,
    
    /// Is market halted
    pub is_halted: bool,
    
    /// Reserved for future use
    pub _reserved: [u8; 32],
}

impl UnifiedScalar {
    pub const SIZE: usize = 16 + 8 + 2 + 2 + 2 + 2 + 2 + 2 + 16 + 16 + 2 + 8 + 32 + 1 + 32;
    
    /// Create new unified scalar state
    pub fn new(market_id: u128, authority: Pubkey) -> Self {
        Self {
            market_id,
            oracle_price: 0,
            oracle_confidence: 100, // 1% default
            synthetic_adjustment: 0,
            cdp_collateral_factor: 8000, // 80% default
            perp_funding_rate: 0,
            vault_yield_impact: 0,
            risk_score: 5000, // Medium risk default
            liquidity_depth: 0,
            volume_24h: 0,
            volatility: 500, // 5% default
            last_update: 0,
            authority,
            is_halted: false,
            _reserved: [0; 32],
        }
    }
    
    /// Calculate adjusted price incorporating all factors
    pub fn calculate_adjusted_price(&self) -> Result<u64, ProgramError> {
        // Start with oracle price
        let mut adjusted_price = self.oracle_price as u128;
        
        // Apply synthetic adjustment
        if self.synthetic_adjustment != 0 {
            let adjustment = (adjusted_price * self.synthetic_adjustment.abs() as u128) / 10000;
            if self.synthetic_adjustment > 0 {
                adjusted_price = adjusted_price.saturating_add(adjustment);
            } else {
                adjusted_price = adjusted_price.saturating_sub(adjustment);
            }
        }
        
        // Apply perpetual funding impact
        if self.perp_funding_rate != 0 {
            let funding_impact = (adjusted_price * self.perp_funding_rate.abs() as u128) / 10000;
            if self.perp_funding_rate > 0 {
                adjusted_price = adjusted_price.saturating_add(funding_impact);
            } else {
                adjusted_price = adjusted_price.saturating_sub(funding_impact);
            }
        }
        
        // Apply vault yield impact
        if self.vault_yield_impact != 0 {
            let yield_impact = (adjusted_price * self.vault_yield_impact.abs() as u128) / 10000;
            if self.vault_yield_impact > 0 {
                adjusted_price = adjusted_price.saturating_add(yield_impact);
            } else {
                adjusted_price = adjusted_price.saturating_sub(yield_impact);
            }
        }
        
        Ok(adjusted_price.min(u64::MAX as u128) as u64)
    }
    
    /// Calculate effective collateral value for CDP
    pub fn calculate_cdp_value(&self, amount: u64) -> u64 {
        let adjusted_price = self.calculate_adjusted_price().unwrap_or(self.oracle_price);
        let value = (amount as u128 * adjusted_price as u128) / 10_u128.pow(8);
        let collateral_value = (value * self.cdp_collateral_factor as u128) / 10000;
        collateral_value.min(u64::MAX as u128) as u64
    }
    
    /// Check if market should be halted based on risk
    pub fn should_halt(&self) -> bool {
        self.is_halted || 
        self.risk_score > 9500 || // Extreme risk
        self.volatility > 5000 || // >50% volatility
        self.oracle_confidence > 1000 // >10% confidence interval
    }
}

/// Risk parameters for the platform
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct RiskParameters {
    /// Maximum allowed leverage (in basis points, 10000 = 1x)
    pub max_leverage: u16,
    
    /// Minimum collateral ratio (basis points)
    pub min_collateral_ratio: u16,
    
    /// Liquidation threshold (basis points)
    pub liquidation_threshold: u16,
    
    /// Maximum position size as % of liquidity (basis points)
    pub max_position_size_ratio: u16,
    
    /// Minimum liquidity for trading (in base units)
    pub min_liquidity: u64,
    
    /// Maximum volatility allowed (basis points)
    pub max_volatility: u16,
    
    /// Circuit breaker threshold (% price movement, basis points)
    pub circuit_breaker_threshold: u16,
    
    /// Cooldown period after circuit breaker (seconds)
    pub circuit_breaker_cooldown: u32,
    
    /// Authority for updating parameters
    pub authority: Pubkey,
    
    /// Last update timestamp
    pub last_update: UnixTimestamp,
}

impl RiskParameters {
    pub const SIZE: usize = 2 + 2 + 2 + 2 + 8 + 2 + 2 + 4 + 32 + 8;
    
    /// Create default risk parameters
    pub fn default(authority: Pubkey) -> Self {
        Self {
            max_leverage: 1000000, // 100x max
            min_collateral_ratio: 11000, // 110%
            liquidation_threshold: 10500, // 105%
            max_position_size_ratio: 1000, // 10% of liquidity
            min_liquidity: 10000 * 10_u64.pow(6), // 10k USDC minimum
            max_volatility: 10000, // 100% max volatility
            circuit_breaker_threshold: 2000, // 20% price movement
            circuit_breaker_cooldown: 300, // 5 minutes
            authority,
            last_update: 0,
        }
    }
    
    /// Validate leverage against maximum allowed
    pub fn validate_leverage(&self, leverage: u64) -> bool {
        leverage <= self.max_leverage as u64
    }
    
    /// Check if collateral ratio is sufficient
    pub fn is_collateral_sufficient(&self, collateral_ratio: u16) -> bool {
        collateral_ratio >= self.min_collateral_ratio
    }
    
    /// Check if position should be liquidated
    pub fn should_liquidate(&self, collateral_ratio: u16) -> bool {
        collateral_ratio < self.liquidation_threshold
    }
    
    /// Validate position size against liquidity
    pub fn validate_position_size(&self, position_size: u64, liquidity: u64) -> bool {
        if liquidity == 0 {
            return false;
        }
        let ratio = (position_size as u128 * 10000) / liquidity as u128;
        ratio <= self.max_position_size_ratio as u128
    }
}

/// Aggregated metrics across all markets
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct PlatformMetrics {
    /// Total value locked across all vaults
    pub total_tvl: u128,
    
    /// Total CDP collateral
    pub total_cdp_collateral: u128,
    
    /// Total perpetual open interest
    pub total_perp_open_interest: u128,
    
    /// Total synthetic tokens minted
    pub total_synthetics_minted: u128,
    
    /// Total trading volume (24h)
    pub total_volume_24h: u128,
    
    /// Number of active markets
    pub active_markets: u32,
    
    /// Number of active users
    pub active_users: u32,
    
    /// Platform-wide risk score (0-10000)
    pub platform_risk_score: u16,
    
    /// Last update timestamp
    pub last_update: UnixTimestamp,
}

impl PlatformMetrics {
    pub const SIZE: usize = 16 + 16 + 16 + 16 + 16 + 4 + 4 + 2 + 8;
    
    /// Create new platform metrics
    pub fn new() -> Self {
        Self {
            total_tvl: 0,
            total_cdp_collateral: 0,
            total_perp_open_interest: 0,
            total_synthetics_minted: 0,
            total_volume_24h: 0,
            active_markets: 0,
            active_users: 0,
            platform_risk_score: 5000,
            last_update: 0,
        }
    }
    
    /// Calculate overall platform health score (0-100)
    pub fn calculate_health_score(&self) -> u8 {
        // Higher TVL and volume = healthier
        let tvl_score = if self.total_tvl > 100_000_000 * 10_u128.pow(6) {
            30 // Max 30 points for TVL > $100M
        } else {
            (self.total_tvl / (3_333_333 * 10_u128.pow(6))).min(30) as u8
        };
        
        let volume_score = if self.total_volume_24h > 10_000_000 * 10_u128.pow(6) {
            30 // Max 30 points for volume > $10M
        } else {
            (self.total_volume_24h / (333_333 * 10_u128.pow(6))).min(30) as u8
        };
        
        // Lower risk = healthier
        let risk_score = if self.platform_risk_score < 3000 {
            30 // Low risk
        } else if self.platform_risk_score < 7000 {
            20 // Medium risk
        } else {
            10 // High risk
        };
        
        // Active users bonus
        let user_score = (self.active_users / 100).min(10) as u8;
        
        tvl_score + volume_score + risk_score + user_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unified_scalar_creation() {
        let market_id = 12345u128;
        let authority = Pubkey::new_unique();
        let scalar = UnifiedScalar::new(market_id, authority);
        
        assert_eq!(scalar.market_id, market_id);
        assert_eq!(scalar.authority, authority);
        assert_eq!(scalar.risk_score, 5000);
        assert!(!scalar.is_halted);
    }
    
    #[test]
    fn test_adjusted_price_calculation() {
        let mut scalar = UnifiedScalar::new(1, Pubkey::new_unique());
        scalar.oracle_price = 100_000_000; // $100
        scalar.synthetic_adjustment = 100; // +1%
        scalar.perp_funding_rate = -50; // -0.5%
        scalar.vault_yield_impact = 25; // +0.25%
        
        let adjusted = scalar.calculate_adjusted_price().unwrap();
        // 100 * 1.01 * 0.995 * 1.0025 = 100.75
        assert!(adjusted > 100_000_000 && adjusted < 101_000_000);
    }
    
    #[test]
    fn test_risk_parameters() {
        let params = RiskParameters::default(Pubkey::new_unique());
        
        assert!(params.validate_leverage(100_000)); // 10x ok
        assert!(!params.validate_leverage(2_000_000)); // 200x too high
        
        assert!(params.is_collateral_sufficient(12000)); // 120% ok
        assert!(!params.is_collateral_sufficient(10000)); // 100% too low
        
        assert!(params.should_liquidate(10000)); // 100% should liquidate
        assert!(!params.should_liquidate(11000)); // 110% should not
    }
    
    #[test]
    fn test_platform_metrics() {
        let mut metrics = PlatformMetrics::new();
        metrics.total_tvl = 50_000_000 * 10_u128.pow(6); // $50M
        metrics.total_volume_24h = 5_000_000 * 10_u128.pow(6); // $5M
        metrics.platform_risk_score = 4000; // Medium risk
        metrics.active_users = 500;
        
        let health = metrics.calculate_health_score();
        assert!(health > 50 && health < 80); // Good but not excellent health
    }
}