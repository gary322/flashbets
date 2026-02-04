//! Sustainability Model for Post-MMT Era
//!
//! Implements fee-based revenue model for long-term platform sustainability
//! after 5-year MMT emission period ends

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    math::fixed_point::U64F64,
    state::accounts::discriminators,
};

/// Sustainability configuration constants
pub const MMT_EMISSION_DURATION_YEARS: u64 = 5;
pub const MMT_EMISSION_END_SLOT: u64 = 788_400_000; // ~5 years at 0.4s/slot
pub const BASE_FEE_BPS: u16 = 30; // 0.3% base trading fee
pub const MAX_FEE_BPS: u16 = 100; // 1% maximum fee
pub const FEE_TO_VAULT_PERCENTAGE: u8 = 50; // 50% to insurance vault
pub const FEE_TO_REBATES_PERCENTAGE: u8 = 30; // 30% to user rebates
pub const FEE_TO_TREASURY_PERCENTAGE: u8 = 20; // 20% to treasury

/// Sustainability model state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SustainabilityModel {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Model activation slot (when MMT emissions end)
    pub activation_slot: u64,
    
    /// Current fee tier
    pub current_fee_tier: FeeTier,
    
    /// Total fees collected
    pub total_fees_collected: u64,
    
    /// Fees distributed to vault
    pub fees_to_vault: u64,
    
    /// Fees distributed as rebates
    pub fees_to_rebates: u64,
    
    /// Fees to treasury
    pub fees_to_treasury: u64,
    
    /// Volume-based fee discount enabled
    pub volume_discount_enabled: bool,
    
    /// Staker fee discount enabled
    pub staker_discount_enabled: bool,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Model version
    pub version: u8,
}

impl SustainabilityModel {
    pub const SIZE: usize = 8 + // discriminator
        8 + // activation_slot
        1 + // current_fee_tier
        8 + // total_fees_collected
        8 + // fees_to_vault
        8 + // fees_to_rebates
        8 + // fees_to_treasury
        1 + // volume_discount_enabled
        1 + // staker_discount_enabled
        8 + // last_update_slot
        1; // version
        
    /// Initialize sustainability model
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::SUSTAINABILITY_MODEL,
            activation_slot: MMT_EMISSION_END_SLOT,
            current_fee_tier: FeeTier::Standard,
            total_fees_collected: 0,
            fees_to_vault: 0,
            fees_to_rebates: 0,
            fees_to_treasury: 0,
            volume_discount_enabled: true,
            staker_discount_enabled: true,
            last_update_slot: 0,
            version: 1,
        }
    }
    
    /// Check if model is active (post-MMT era)
    pub fn is_active(&self) -> Result<bool, ProgramError> {
        let current_slot = Clock::get()?.slot;
        Ok(current_slot >= self.activation_slot)
    }
    
    /// Calculate trading fee for a user
    pub fn calculate_fee(
        &self,
        base_amount: u64,
        user_volume_30d: u64,
        user_mmt_staked: u64,
    ) -> Result<FeeCalculation, ProgramError> {
        // Start with base fee
        let mut fee_bps = self.current_fee_tier.base_fee_bps();
        
        // Apply volume discount
        if self.volume_discount_enabled {
            let volume_discount = VolumeDiscount::from_volume(user_volume_30d);
            fee_bps = fee_bps.saturating_sub(volume_discount.discount_bps());
        }
        
        // Apply staker discount
        if self.staker_discount_enabled {
            let staker_discount = StakerDiscount::from_stake(user_mmt_staked);
            fee_bps = fee_bps.saturating_sub(staker_discount.discount_bps());
        }
        
        // Ensure minimum fee
        fee_bps = fee_bps.max(5); // 0.05% minimum
        
        // Calculate fee amount
        let fee_amount = (base_amount as u128)
            .checked_mul(fee_bps as u128)
            .ok_or(BettingPlatformError::NumericalOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::NumericalOverflow)? as u64;
            
        Ok(FeeCalculation {
            fee_amount,
            fee_bps,
            volume_discount_applied: self.volume_discount_enabled && user_volume_30d > 0,
            staker_discount_applied: self.staker_discount_enabled && user_mmt_staked > 0,
        })
    }
    
    /// Distribute collected fees according to model
    pub fn distribute_fees(&mut self, fee_amount: u64) -> Result<FeeDistribution, ProgramError> {
        // Calculate distributions
        let to_vault = fee_amount
            .checked_mul(FEE_TO_VAULT_PERCENTAGE as u64)
            .ok_or(BettingPlatformError::NumericalOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
            
        let to_rebates = fee_amount
            .checked_mul(FEE_TO_REBATES_PERCENTAGE as u64)
            .ok_or(BettingPlatformError::NumericalOverflow)?
            .checked_div(100)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
            
        let to_treasury = fee_amount
            .saturating_sub(to_vault)
            .saturating_sub(to_rebates);
        
        // Update tracking
        self.total_fees_collected = self.total_fees_collected
            .checked_add(fee_amount)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
        self.fees_to_vault = self.fees_to_vault
            .checked_add(to_vault)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
        self.fees_to_rebates = self.fees_to_rebates
            .checked_add(to_rebates)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
        self.fees_to_treasury = self.fees_to_treasury
            .checked_add(to_treasury)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
            
        self.last_update_slot = Clock::get()?.slot;
        
        Ok(FeeDistribution {
            to_vault,
            to_rebates,
            to_treasury,
        })
    }
    
    /// Update fee tier based on market conditions
    pub fn update_fee_tier(&mut self, new_tier: FeeTier) -> ProgramResult {
        msg!("Updating fee tier from {:?} to {:?}", self.current_fee_tier, new_tier);
        self.current_fee_tier = new_tier;
        self.last_update_slot = Clock::get()?.slot;
        Ok(())
    }
    
    /// Get projected annual revenue
    pub fn projected_annual_revenue(&self, daily_volume: u64) -> Result<u64, ProgramError> {
        let avg_fee_bps = self.current_fee_tier.base_fee_bps() as u64;
        let daily_fees = daily_volume
            .checked_mul(avg_fee_bps)
            .ok_or(BettingPlatformError::NumericalOverflow)?
            .checked_div(10000)
            .ok_or(BettingPlatformError::NumericalOverflow)?;
            
        Ok(daily_fees
            .checked_mul(365)
            .ok_or(BettingPlatformError::NumericalOverflow)?)
    }
}

/// Fee tiers based on market conditions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum FeeTier {
    /// Promotional tier (lowest fees)
    Promotional,
    /// Standard tier
    Standard,
    /// Premium tier (higher fees, more features)
    Premium,
    /// Dynamic tier (market-based)
    Dynamic,
}

impl FeeTier {
    pub fn base_fee_bps(&self) -> u16 {
        match self {
            FeeTier::Promotional => 20, // 0.2%
            FeeTier::Standard => BASE_FEE_BPS, // 0.3%
            FeeTier::Premium => 50, // 0.5%
            FeeTier::Dynamic => BASE_FEE_BPS, // Adjusted by algorithm
        }
    }
}

/// Volume-based fee discount tiers
#[derive(Debug, Clone, Copy)]
pub enum VolumeDiscount {
    None,
    Bronze,  // $100k+ monthly
    Silver,  // $1M+ monthly
    Gold,    // $10M+ monthly
    Platinum, // $100M+ monthly
}

impl VolumeDiscount {
    pub fn from_volume(volume_30d: u64) -> Self {
        match volume_30d {
            0..=99_999_999_999 => VolumeDiscount::None,
            100_000_000_000..=999_999_999_999 => VolumeDiscount::Bronze,
            1_000_000_000_000..=9_999_999_999_999 => VolumeDiscount::Silver,
            10_000_000_000_000..=99_999_999_999_999 => VolumeDiscount::Gold,
            _ => VolumeDiscount::Platinum,
        }
    }
    
    pub fn discount_bps(&self) -> u16 {
        match self {
            VolumeDiscount::None => 0,
            VolumeDiscount::Bronze => 5,   // 0.05% discount
            VolumeDiscount::Silver => 10,  // 0.10% discount
            VolumeDiscount::Gold => 15,    // 0.15% discount
            VolumeDiscount::Platinum => 20, // 0.20% discount
        }
    }
}

/// MMT staker fee discount tiers
#[derive(Debug, Clone, Copy)]
pub enum StakerDiscount {
    None,
    Basic,    // 1k+ MMT staked
    Advanced, // 10k+ MMT staked
    Pro,      // 100k+ MMT staked
    Whale,    // 1M+ MMT staked
}

impl StakerDiscount {
    pub fn from_stake(mmt_staked: u64) -> Self {
        match mmt_staked {
            0..=999_999_999 => StakerDiscount::None,
            1_000_000_000..=9_999_999_999 => StakerDiscount::Basic,
            10_000_000_000..=99_999_999_999 => StakerDiscount::Advanced,
            100_000_000_000..=999_999_999_999 => StakerDiscount::Pro,
            _ => StakerDiscount::Whale,
        }
    }
    
    pub fn discount_bps(&self) -> u16 {
        match self {
            StakerDiscount::None => 0,
            StakerDiscount::Basic => 5,     // 0.05% discount
            StakerDiscount::Advanced => 10, // 0.10% discount
            StakerDiscount::Pro => 15,      // 0.15% discount
            StakerDiscount::Whale => 25,    // 0.25% discount
        }
    }
}

/// Fee calculation result
#[derive(Debug, Clone)]
pub struct FeeCalculation {
    pub fee_amount: u64,
    pub fee_bps: u16,
    pub volume_discount_applied: bool,
    pub staker_discount_applied: bool,
}

/// Fee distribution result
#[derive(Debug, Clone)]
pub struct FeeDistribution {
    pub to_vault: u64,
    pub to_rebates: u64,
    pub to_treasury: u64,
}

/// Treasury management for sustainability
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TreasuryManagement {
    /// Total treasury balance
    pub balance: u64,
    
    /// Reserved for operations
    pub operations_reserve: u64,
    
    /// Reserved for insurance
    pub insurance_reserve: u64,
    
    /// Available for governance
    pub governance_available: u64,
    
    /// Burn rate (monthly)
    pub monthly_burn_rate: u64,
    
    /// Runway in months
    pub runway_months: u16,
}

impl TreasuryManagement {
    /// Update treasury stats
    pub fn update_stats(&mut self) -> ProgramResult {
        // Calculate reserves (40% operations, 40% insurance, 20% governance)
        self.operations_reserve = self.balance * 40 / 100;
        self.insurance_reserve = self.balance * 40 / 100;
        self.governance_available = self.balance * 20 / 100;
        
        // Calculate runway
        if self.monthly_burn_rate > 0 {
            self.runway_months = (self.balance / self.monthly_burn_rate) as u16;
        } else {
            self.runway_months = u16::MAX;
        }
        
        Ok(())
    }
    
    /// Check if treasury is healthy
    pub fn is_healthy(&self) -> bool {
        self.runway_months >= 24 // At least 2 years runway
    }
}

/// Revenue optimization engine
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RevenueOptimizer {
    /// Target daily revenue
    pub target_daily_revenue: u64,
    
    /// Current daily revenue (7-day average)
    pub current_daily_revenue: u64,
    
    /// Optimization strategy
    pub strategy: OptimizationStrategy,
    
    /// Last optimization timestamp
    pub last_optimization: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OptimizationStrategy {
    /// Maximize volume through lower fees
    VolumeMaximization,
    /// Maximize revenue through optimal fees
    RevenueMaximization,
    /// Balance volume and revenue
    Balanced,
    /// Respond to competition
    Competitive,
}

impl RevenueOptimizer {
    /// Recommend fee adjustment
    pub fn recommend_fee_adjustment(&self) -> FeeTier {
        let revenue_ratio = self.current_daily_revenue as f64 / self.target_daily_revenue as f64;
        
        match self.strategy {
            OptimizationStrategy::VolumeMaximization => {
                if revenue_ratio < 0.8 {
                    FeeTier::Promotional
                } else {
                    FeeTier::Standard
                }
            }
            OptimizationStrategy::RevenueMaximization => {
                if revenue_ratio < 0.9 {
                    FeeTier::Premium
                } else if revenue_ratio > 1.1 {
                    FeeTier::Standard
                } else {
                    FeeTier::Premium
                }
            }
            OptimizationStrategy::Balanced => {
                if revenue_ratio < 0.85 {
                    FeeTier::Standard
                } else if revenue_ratio > 1.15 {
                    FeeTier::Promotional
                } else {
                    FeeTier::Standard
                }
            }
            OptimizationStrategy::Competitive => {
                // Would check competitor fees in production
                FeeTier::Dynamic
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fee_calculation() {
        let model = SustainabilityModel::new();
        
        // Test base fee
        let fee = model.calculate_fee(1_000_000_000, 0, 0).unwrap();
        assert_eq!(fee.fee_bps, 30); // 0.3%
        assert_eq!(fee.fee_amount, 3_000_000); // 0.3% of 1B
        
        // Test with volume discount
        let fee = model.calculate_fee(
            1_000_000_000,
            1_000_000_000_000, // $1M volume
            0
        ).unwrap();
        assert_eq!(fee.fee_bps, 20); // 0.3% - 0.1% = 0.2%
        
        // Test with staker discount
        let fee = model.calculate_fee(
            1_000_000_000,
            0,
            10_000_000_000, // 10k MMT
        ).unwrap();
        assert_eq!(fee.fee_bps, 20); // 0.3% - 0.1% = 0.2%
    }
    
    #[test]
    fn test_fee_distribution() {
        let mut model = SustainabilityModel::new();
        
        let dist = model.distribute_fees(1_000_000).unwrap();
        assert_eq!(dist.to_vault, 500_000); // 50%
        assert_eq!(dist.to_rebates, 300_000); // 30%
        assert_eq!(dist.to_treasury, 200_000); // 20%
        
        assert_eq!(model.total_fees_collected, 1_000_000);
    }
}