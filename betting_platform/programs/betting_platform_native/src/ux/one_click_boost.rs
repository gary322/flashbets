//! One-Click Boost Feature
//! 
//! Provides simplified interface for users to boost their positions with a single click,
//! showing preview calculations of efficiency gains and savings.

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    chain_execution::auto_chain::ChainConfig,
    constants::{BASIS_POINTS_DIVISOR, LEVERAGE_PRECISION},
    error::BettingPlatformError,
    math::leverage::calculate_effective_leverage,
    state::accounts::{GlobalConfigPDA, Position},
    validation::validate_account_owner,
};

/// Maximum boost multiplier (500x as per spec)
pub const MAX_BOOST_MULTIPLIER: u64 = 500;

/// Default boost multiplier for one-click (200x as per spec)
pub const DEFAULT_BOOST_MULTIPLIER: u64 = 200;

/// Scale precision for USD calculations
pub const SCALE_PRECISION: u64 = 1_000_000;

/// Boost preview information shown to users
#[derive(Debug, Clone, Copy)]
pub struct BoostPreview {
    /// Current effective leverage
    pub current_leverage: u64,
    /// Boosted effective leverage
    pub boosted_leverage: u64,
    /// Efficiency gain percentage (e.g., 400 for +400%)
    pub efficiency_gain_percentage: u64,
    /// Estimated savings in USD (scaled by SCALE_PRECISION)
    pub estimated_savings_usd: u128,
    /// Risk level after boost
    pub risk_level: RiskLevel,
    /// Required collateral for boost
    pub required_collateral: u64,
    /// Liquidation price after boost
    pub liquidation_price_after: u64,
}

/// Risk levels for user education
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low,      // < 10x
    Medium,   // 10x - 50x
    High,     // 50x - 100x
    VeryHigh, // 100x - 200x
    Extreme,  // > 200x
}

impl RiskLevel {
    pub fn from_leverage(leverage: u64) -> Self {
        match leverage {
            0..=10 => RiskLevel::Low,
            11..=50 => RiskLevel::Medium,
            51..=100 => RiskLevel::High,
            101..=200 => RiskLevel::VeryHigh,
            _ => RiskLevel::Extreme,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Low Risk",
            RiskLevel::Medium => "Medium Risk",
            RiskLevel::High => "High Risk",
            RiskLevel::VeryHigh => "Very High Risk",
            RiskLevel::Extreme => "EXTREME RISK - Total loss possible on 0.2% move",
        }
    }
}

/// Calculate boost preview for user interface
pub fn calculate_boost_preview(
    current_position: &Position,
    boost_multiplier: u64,
    current_price: u64,
    global_config: &GlobalConfigPDA,
    chain_depth: u8,
) -> Result<BoostPreview, ProgramError> {
    // Validate boost multiplier
    if boost_multiplier > MAX_BOOST_MULTIPLIER {
        return Err(BettingPlatformError::InvalidLeverage.into());
    }

    // Calculate current effective leverage
    let current_leverage = calculate_effective_leverage(
        current_position.leverage,
        chain_depth as u64 * BASIS_POINTS_DIVISOR / 100, // Convert chain depth to basis points
    );

    // Calculate boosted leverage
    let boosted_leverage = current_leverage
        .checked_mul(boost_multiplier)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(LEVERAGE_PRECISION)
        .ok_or(BettingPlatformError::MathOverflow)?;

    // Ensure boosted leverage doesn't exceed maximum
    let boosted_leverage = boosted_leverage.min(MAX_BOOST_MULTIPLIER * LEVERAGE_PRECISION);

    // Calculate efficiency gain percentage
    let efficiency_gain_percentage = boost_multiplier
        .checked_sub(100)
        .unwrap_or(0)
        .checked_mul(4) // +400% as per spec
        .ok_or(BettingPlatformError::MathOverflow)?;

    // Calculate estimated savings based on reduced fees with higher volume
    let volume_multiplier = boost_multiplier / 100; // Higher leverage = higher volume
    let fee_reduction_bps = volume_multiplier
        .checked_mul(5) // 5bp reduction per 100x leverage
        .unwrap_or(0)
        .min(25); // Cap at 25bp reduction
    
    let estimated_savings_usd = calculate_fee_savings(
        current_position.size,
        fee_reduction_bps,
        global_config.fee_base as u64,
    )? as u64;

    // Calculate liquidation price after boost
    let liquidation_price_after = calculate_liquidation_price(
        current_position.entry_price,
        boosted_leverage,
        current_position.is_long,
    )?;

    // Determine risk level
    let risk_level = RiskLevel::from_leverage(boosted_leverage / LEVERAGE_PRECISION);

    // Calculate required collateral for boost
    let required_collateral = calculate_required_collateral(
        current_position.size,
        current_leverage,
        boosted_leverage,
    )?;

    Ok(BoostPreview {
        current_leverage: current_leverage / LEVERAGE_PRECISION,
        boosted_leverage: boosted_leverage / LEVERAGE_PRECISION,
        efficiency_gain_percentage,
        estimated_savings_usd: estimated_savings_usd as u128,
        risk_level,
        required_collateral,
        liquidation_price_after,
    })
}

/// Calculate fee savings from volume-based discounts
fn calculate_fee_savings(
    position_size: u64,
    fee_reduction_bps: u64,
    base_fee_bps: u64,
) -> Result<u128, ProgramError> {
    // Calculate the fee savings
    let savings_bps = fee_reduction_bps.min(base_fee_bps);
    
    let savings = (position_size as u128)
        .checked_mul(savings_bps as u128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(BASIS_POINTS_DIVISOR as u128)
        .ok_or(BettingPlatformError::MathOverflow)?;

    Ok(savings)
}

/// Calculate liquidation price based on leverage
fn calculate_liquidation_price(
    entry_price: u64,
    leverage: u64,
    is_long: bool,
) -> Result<u64, ProgramError> {
    // Liquidation occurs when loss reaches 100% / leverage
    // For 500x leverage, liquidation at 0.2% move (100/500 = 0.2%)
    let liquidation_percentage = LEVERAGE_PRECISION
        .checked_mul(100)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(leverage)
        .ok_or(BettingPlatformError::MathOverflow)?;

    Ok(if is_long {
        // Long position liquidates when price drops
        entry_price
            .checked_mul(LEVERAGE_PRECISION.saturating_sub(liquidation_percentage))
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(LEVERAGE_PRECISION)
            .ok_or(BettingPlatformError::MathOverflow)?
    } else {
        // Short position liquidates when price rises
        entry_price
            .checked_mul(LEVERAGE_PRECISION.saturating_add(liquidation_percentage))
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(LEVERAGE_PRECISION)
            .ok_or(BettingPlatformError::MathOverflow)?
    })
}

/// Calculate additional collateral required for boost
fn calculate_required_collateral(
    position_size: u64,
    current_leverage: u64,
    target_leverage: u64,
) -> Result<u64, ProgramError> {
    // Current collateral = position_size / current_leverage
    let current_collateral = position_size
        .checked_mul(LEVERAGE_PRECISION)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(current_leverage)
        .ok_or(BettingPlatformError::MathOverflow)?;

    // Target collateral = position_size / target_leverage
    let target_collateral = position_size
        .checked_mul(LEVERAGE_PRECISION)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(target_leverage)
        .ok_or(BettingPlatformError::MathOverflow)?;

    // Additional required = current - target (negative means we need less collateral)
    if current_collateral > target_collateral {
        Ok(0) // No additional collateral needed when increasing leverage
    } else {
        target_collateral
            .checked_sub(current_collateral)
            .ok_or(BettingPlatformError::MathOverflow.into())
    }
}

/// Execute one-click boost
pub fn execute_one_click_boost(
    user_position_account: &AccountInfo,
    global_config_account: &AccountInfo,
    user_account: &AccountInfo,
    boost_multiplier: Option<u64>,
    chain_depth: u8,
) -> ProgramResult {
    // Validate accounts
    validate_account_owner(user_position_account, &crate::ID)?;
    validate_account_owner(global_config_account, &crate::ID)?;

    // Deserialize accounts
    let mut user_position = Position::deserialize(&mut &user_position_account.data.borrow()[..])?;
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;

    // Check if user is signer
    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Use default boost if not specified
    let boost_multiplier = boost_multiplier.unwrap_or(DEFAULT_BOOST_MULTIPLIER);

    // Get current price (in production, this would come from oracle)
    let current_price = user_position.entry_price; // Simplified for now

    // Calculate boost preview
    let preview = calculate_boost_preview(
        &user_position,
        boost_multiplier,
        current_price,
        &global_config,
        chain_depth,
    )?;

    msg!(
        "One-Click Boost Preview: {}x → {}x leverage, +{}% efficiency, ${} saved",
        preview.current_leverage,
        preview.boosted_leverage,
        preview.efficiency_gain_percentage,
        preview.estimated_savings_usd / SCALE_PRECISION as u128
    );

    msg!("Risk Level: {}", preview.risk_level.to_string());

    // Update position with boosted leverage
    user_position.leverage = preview.boosted_leverage * LEVERAGE_PRECISION;

    // Serialize updated position
    user_position.serialize(&mut &mut user_position_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Format boost preview for display
pub fn format_boost_preview(preview: &BoostPreview) -> String {
    format!(
        "+{}x eff, ${} saved",
        preview.boosted_leverage,
        preview.estimated_savings_usd / SCALE_PRECISION as u128
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_categorization() {
        assert_eq!(RiskLevel::from_leverage(5), RiskLevel::Low);
        assert_eq!(RiskLevel::from_leverage(25), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_leverage(75), RiskLevel::High);
        assert_eq!(RiskLevel::from_leverage(150), RiskLevel::VeryHigh);
        assert_eq!(RiskLevel::from_leverage(300), RiskLevel::Extreme);
        assert_eq!(RiskLevel::from_leverage(500), RiskLevel::Extreme);
    }

    #[test]
    fn test_liquidation_price_calculation() {
        // Test long position with 500x leverage
        let entry_price = 100_000_000; // $100
        let leverage = 500 * LEVERAGE_PRECISION;
        let is_long = true;

        let liq_price = calculate_liquidation_price(entry_price, leverage, is_long).unwrap();
        
        // Should liquidate at ~0.2% drop
        let expected = 99_800_000; // $99.80
        assert!((liq_price as i64 - expected as i64).abs() < 100_000); // Allow small rounding

        // Test short position
        let liq_price_short = calculate_liquidation_price(entry_price, leverage, false).unwrap();
        let expected_short = 100_200_000; // $100.20
        assert!((liq_price_short as i64 - expected_short as i64).abs() < 100_000);
    }

    #[test]
    fn test_boost_preview_calculation() {
        let position = Position::new(
            solana_program::pubkey::Pubkey::new_unique(),
            12345u128,
            67890u128,
            0,
            10_000_000_000, // $10,000
            10 * LEVERAGE_PRECISION, // 10x
            100_000_000, // $100
            true,
            0,
        );

        let mut global_config = GlobalConfigPDA::new();
        global_config.fee_base = 30; // 30bp

        let preview = calculate_boost_preview(
            &position,
            200, // 200x boost
            100_000_000,
            &global_config,
            0, // chain_depth
        ).unwrap();

        assert_eq!(preview.current_leverage, 10);
        assert_eq!(preview.boosted_leverage, 20); // 10x * 2 = 20x
        assert_eq!(preview.efficiency_gain_percentage, 400); // +400%
        assert_eq!(preview.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_format_boost_preview() {
        let preview = BoostPreview {
            current_leverage: 10,
            boosted_leverage: 200,
            efficiency_gain_percentage: 400,
            estimated_savings_usd: 5_000_000_000, // $5 scaled
            risk_level: RiskLevel::VeryHigh,
            required_collateral: 0,
            liquidation_price_after: 99_500_000,
        };

        let formatted = format_boost_preview(&preview);
        assert_eq!(formatted, "+200x eff, $5 saved");
    }
}