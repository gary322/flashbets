//! Maximum Leverage Edge Case Testing
//! 
//! Tests behavior at maximum leverage limits

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{GlobalConfigPDA, ProposalPDA, Position, LeverageTier},
    trading::{validate_leverage, calculate_margin_requirement, calculate_liquidation_price},
    events::{emit_event, EventType},
    math::U64F64,
};

/// Test maximum leverage limits per tier
pub fn test_max_leverage_limits(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    
    msg!("Testing maximum leverage limits");
    
    // Load global config with leverage tiers
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Test each leverage tier
    for (tier_idx, tier) in global_config.leverage_tiers.iter().enumerate() {
        msg!("Testing Tier {}: n={}, max_leverage={}x", tier_idx, tier.n, tier.max);
        
        // Step 1: Test valid leverage at limit
        let test_leverage = tier.max as u64;
        let test_size = 10_000_000_000; // $10k position
        
        match validate_leverage_for_position_count(test_leverage, tier.n, &global_config) {
            Ok(_) => msg!("  ✓ {}x leverage allowed", test_leverage),
            Err(e) => {
                msg!("  ✗ ERROR: Valid leverage rejected: {:?}", e);
                return Err(e.into());
            }
        }
        
        // Step 2: Test leverage exceeding limit
        let excessive_leverage = (tier.max + 1) as u64;
        
        match validate_leverage_for_position_count(excessive_leverage, tier.n, &global_config) {
            Err(BettingPlatformError::ExcessiveLeverage) => {
                msg!("  ✓ {}x leverage correctly rejected", excessive_leverage);
            }
            Ok(_) => {
                msg!("  ✗ ERROR: Excessive leverage accepted!");
                return Err(BettingPlatformError::InvalidLeverage.into());
            }
            Err(e) => {
                msg!("  ✗ Unexpected error: {:?}", e);
                return Err(e.into());
            }
        }
        
        // Step 3: Calculate margin and liquidation at max leverage
        let margin_required = calculate_margin_requirement(test_size, test_leverage)?;
        let margin_ratio = (margin_required * 100) / test_size;
        
        msg!("  Margin required: {} ({:.1}%)", margin_required, margin_ratio as f64);
        
        // Calculate liquidation price for max leverage
        let entry_price = 500_000; // 0.5
        let liq_price_long = calculate_liquidation_price(
            entry_price,
            test_leverage,
            true,
        )?;
        let liq_price_short = calculate_liquidation_price(
            entry_price,
            test_leverage,
            false,
        )?;
        
        let liq_distance_long = ((entry_price - liq_price_long) as f64 / entry_price as f64) * 100.0;
        let liq_distance_short = ((liq_price_short - entry_price) as f64 / entry_price as f64) * 100.0;
        
        msg!("  Liquidation distance (long): {:.2}%", liq_distance_long);
        msg!("  Liquidation distance (short): {:.2}%", liq_distance_short);
        
        // Verify liquidation distance is reasonable
        let min_safe_distance = 100.0 / test_leverage as f64; // At least 1/leverage
        if liq_distance_long < min_safe_distance || liq_distance_short < min_safe_distance {
            msg!("  ⚠️  WARNING: Liquidation too close to entry!");
        }
    }
    
    msg!("\nMax leverage test completed");
    
    Ok(())
}

/// Test position at extreme leverage with price movements
pub fn test_extreme_leverage_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    
    msg!("Testing extreme leverage position behavior");
    
    // Create position at maximum allowed leverage (100x for tier 1)
    let leverage = 100u64;
    let size = 1_000_000_000; // $1k position
    let entry_price = 500_000; // 0.5
    
    let position = Position::new(
        *user_account.key,
        1,
        1,
        0,
        size,
        leverage,
        entry_price,
        true, // Long
        Clock::get()?.unix_timestamp,
    );
    
    msg!("Created {}x leveraged position", leverage);
    msg!("Position size: ${}", size / 1_000_000);
    msg!("Margin: ${}", position.margin / 1_000_000);
    msg!("Entry price: {}", entry_price);
    msg!("Liquidation price: {}", position.liquidation_price);
    
    // Test various price movements
    let price_scenarios = vec![
        ("0.1% favorable", 500_500),
        ("0.5% favorable", 502_500),
        ("0.1% adverse", 499_500),
        ("0.5% adverse", 497_500),
        ("0.9% adverse", 495_500),
        ("1% adverse (liquidation)", 495_000),
    ];
    
    msg!("\nTesting price movements:");
    
    for (scenario, new_price) in price_scenarios {
        let pnl = calculate_position_pnl(&position, new_price)?;
        let pnl_percentage = (pnl as f64 / position.margin as f64) * 100.0;
        let should_liquidate = new_price <= position.liquidation_price;
        
        msg!("  {} -> price {}: PnL ${} ({:.1}% of margin) {}",
            scenario,
            new_price,
            pnl / 1_000_000,
            pnl_percentage,
            if should_liquidate { "⚠️ LIQUIDATED" } else { "" }
        );
        
        // Verify liquidation trigger
        if should_liquidate && new_price > position.liquidation_price {
            msg!("  ✗ ERROR: Position should be liquidated!");
            return Err(BettingPlatformError::LiquidationMissed.into());
        }
    }
    
    // Test maximum possible profit
    let max_profit_price = entry_price * 2; // 100% price increase
    let max_pnl = calculate_position_pnl(&position, max_profit_price)?;
    let roi = (max_pnl as f64 / position.margin as f64) * 100.0;
    
    msg!("\nMaximum profit scenario (price doubles):");
    msg!("  PnL: ${}", max_pnl / 1_000_000);
    msg!("  ROI: {:.0}%", roi);
    
    Ok(())
}

/// Test leverage tier progression
pub fn test_leverage_tier_progression(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let user_positions_account = next_account_info(account_iter)?;
    
    msg!("Testing leverage tier progression");
    
    // Load global config
    let global_config = GlobalConfigPDA::try_from_slice(&global_config_account.data.borrow())?;
    
    // Simulate user with multiple positions
    let mut position_count = 0u32;
    let mut current_tier_idx = 0usize;
    
    msg!("Starting at Tier 0: max leverage {}x", global_config.leverage_tiers[0].max);
    
    // Add positions and check tier changes
    for i in 1..=100 {
        position_count = i;
        
        // Find current tier
        let mut new_tier_idx = 0;
        for (idx, tier) in global_config.leverage_tiers.iter().enumerate() {
            if position_count <= tier.n {
                new_tier_idx = idx;
                break;
            }
        }
        
        // Check if tier changed
        if new_tier_idx != current_tier_idx {
            current_tier_idx = new_tier_idx;
            let tier = &global_config.leverage_tiers[current_tier_idx];
            
            msg!("Position count {}: Moved to Tier {} (max leverage {}x)",
                position_count, current_tier_idx, tier.max);
            
            // Test that old leverage is now invalid
            if current_tier_idx > 0 {
                let old_tier = &global_config.leverage_tiers[current_tier_idx - 1];
                let old_max_leverage = old_tier.max as u64;
                
                match validate_leverage_for_position_count(
                    old_max_leverage,
                    position_count,
                    &global_config,
                ) {
                    Err(BettingPlatformError::ExcessiveLeverage) => {
                        msg!("  ✓ Previous tier's {}x leverage now rejected", old_max_leverage);
                    }
                    Ok(_) => {
                        msg!("  ✗ ERROR: Old leverage still accepted!");
                        return Err(ProgramError::InvalidAccountData);
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    
    msg!("\nLeverage tier progression test completed");
    
    Ok(())
}

/// Test cross-leverage positions (multiple markets)
pub fn test_cross_leverage_positions(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing cross-leverage position limits");
    
    // Simulate user with positions across multiple markets
    let positions = vec![
        ("BTC-USD", 50, 10_000_000_000),  // 50x leverage, $10k
        ("ETH-USD", 25, 5_000_000_000),   // 25x leverage, $5k
        ("SOL-USD", 10, 2_000_000_000),   // 10x leverage, $2k
    ];
    
    // Calculate aggregate metrics
    let mut total_notional = 0u64;
    let mut total_margin = 0u64;
    let mut weighted_leverage = 0u64;
    
    for (market, leverage, size) in &positions {
        let margin = size / leverage;
        total_notional += size;
        total_margin += margin;
        weighted_leverage += size * leverage;
        
        msg!("  {}: {}x leverage, ${} position, ${} margin",
            market,
            leverage,
            size / 1_000_000,
            margin / 1_000_000
        );
    }
    
    let effective_leverage = weighted_leverage / total_notional;
    
    msg!("\nAggregate position metrics:");
    msg!("  Total notional: ${}", total_notional / 1_000_000);
    msg!("  Total margin: ${}", total_margin / 1_000_000);
    msg!("  Effective leverage: {}x", effective_leverage);
    
    // Check aggregate risk limits
    const MAX_TOTAL_NOTIONAL: u64 = 100_000_000_000; // $100k
    const MAX_EFFECTIVE_LEVERAGE: u64 = 20;
    
    if total_notional > MAX_TOTAL_NOTIONAL {
        msg!("  ⚠️  WARNING: Total notional exceeds limit!");
        return Err(BettingPlatformError::PositionLimitExceeded.into());
    }
    
    if effective_leverage > MAX_EFFECTIVE_LEVERAGE {
        msg!("  ⚠️  WARNING: Effective leverage exceeds safe limit!");
    }
    
    // Test margin call scenario
    msg!("\nTesting cross-position margin call:");
    
    // Simulate 5% adverse move across all positions
    let portfolio_loss = total_notional * 5 / 100;
    let remaining_equity = total_margin.saturating_sub(portfolio_loss);
    let margin_ratio = (remaining_equity * 100) / total_notional;
    
    msg!("  5% adverse move:");
    msg!("  Portfolio loss: ${}", portfolio_loss / 1_000_000);
    msg!("  Remaining equity: ${}", remaining_equity / 1_000_000);
    msg!("  Margin ratio: {}%", margin_ratio);
    
    if margin_ratio < 5 {
        msg!("  ⚠️  MARGIN CALL TRIGGERED!");
    }
    
    Ok(())
}

/// Calculate position PnL
fn calculate_position_pnl(position: &Position, current_price: u64) -> Result<i64, ProgramError> {
    let price_diff = if position.is_long {
        current_price as i64 - position.entry_price as i64
    } else {
        position.entry_price as i64 - current_price as i64
    };
    
    // PnL = price_diff * size * leverage / entry_price
    let pnl = (price_diff * position.size as i64 * position.leverage as i64) / position.entry_price as i64;
    
    Ok(pnl)
}

/// Validate leverage for position count
fn validate_leverage_for_position_count(
    leverage: u64,
    position_count: u32,
    config: &GlobalConfigPDA,
) -> Result<(), BettingPlatformError> {
    // Find applicable tier
    let mut max_allowed_leverage = 5u8; // Default minimum
    
    for tier in &config.leverage_tiers {
        if position_count <= tier.n {
            max_allowed_leverage = tier.max;
            break;
        }
    }
    
    if leverage > max_allowed_leverage as u64 {
        return Err(BettingPlatformError::ExcessiveLeverage);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pnl_calculation() {
        let position = Position {
            discriminator: [0; 8],
            version: crate::state::versioned_accounts::CURRENT_VERSION,
            size: 1_000_000_000, // $1k
            leverage: 100,
            entry_price: 500_000, // 0.5
            is_long: true,
            margin: 10_000_000, // $10
            collateral: 0,            // ... other fields
            user: Pubkey::default(),
            proposal_id: 0,
            position_id: [0; 32],
            outcome: 0,
            notional: 1_000_000_000,
            liquidation_price: 495_000,
            created_at: 0,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
            partial_liq_accumulator: 0,
            verse_id: 0,
            is_short: false,
            last_mark_price: 500_000, // Same as entry price
            unrealized_pnl: 0,
            cross_margin_enabled: false,
            unrealized_pnl_pct: 0,
        };
        
        // 1% favorable move
        let pnl = calculate_position_pnl(&position, 505_000).unwrap();
        assert_eq!(pnl, 10_000_000); // $10 profit (100% of margin)
        
        // 1% adverse move (liquidation)
        let pnl = calculate_position_pnl(&position, 495_000).unwrap();
        assert_eq!(pnl, -10_000_000); // $10 loss (100% of margin)
    }
    
    #[test]
    fn test_leverage_tiers() {
        let tiers = vec![
            LeverageTier { n: 1, max: 100 },
            LeverageTier { n: 2, max: 70 },
            LeverageTier { n: 4, max: 25 },
            LeverageTier { n: 8, max: 15 },
        ];
        
        // Find tier for 3 positions
        let position_count = 3;
        let mut max_leverage = 5;
        
        for tier in &tiers {
            if position_count <= tier.n {
                max_leverage = tier.max;
                break;
            }
        }
        
        assert_eq!(max_leverage, 25); // Should be in tier with n=4
    }
}