use anchor_lang::prelude::*;
use crate::account_structs::*;
use crate::errors::ErrorCode;
use crate::trading::calculate_coverage;

// Position Validator

pub fn validate_user_positions(
    user_map: &MapEntryPDA,
    price_cache: &PriceCachePDA,
) -> Result<()> {
    // Verify health calculation
    let calculated_health = calculate_health_off_chain(user_map, price_cache.last_price);
    
    require!(
        (user_map.health_factor as i64 - calculated_health as i64).abs() < 100,
        ErrorCode::InconsistentCoverage
    );
    
    // Verify no duplicate positions
    let position_ids: Vec<u128> = user_map.positions.iter()
        .map(|p| p.proposal_id)
        .collect();
    let unique_ids: std::collections::HashSet<_> = position_ids.iter().collect();
    
    require!(
        position_ids.len() == unique_ids.len(),
        ErrorCode::InvalidPosition
    );
    
    // Verify collateral sum
    let total_collateral = user_map.positions.iter()
        .map(|pos| calculate_position_collateral(pos))
        .sum::<u64>();
    
    require!(
        user_map.total_collateral == total_collateral,
        ErrorCode::InconsistentCoverage
    );
    
    // Verify position limits
    require!(
        user_map.positions.len() <= 50,
        ErrorCode::InvalidPosition
    );
    
    // Verify each position
    for position in &user_map.positions {
        validate_position(position)?;
    }
    
    Ok(())
}

pub fn validate_position(position: &Position) -> Result<()> {
    // Check minimum size
    require!(
        position.size >= 10_000_000, // 0.01 SOL minimum
        ErrorCode::InvalidPosition
    );
    
    // Check leverage limits
    require!(
        position.leverage > 0 && position.leverage <= 500,
        ErrorCode::ExcessiveLeverage
    );
    
    // Check price sanity
    require!(
        position.entry_price > 0,
        ErrorCode::InvalidPosition
    );
    
    require!(
        position.liquidation_price > 0,
        ErrorCode::InvalidPosition
    );
    
    // For long positions, liquidation price should be below entry
    if position.is_long {
        require!(
            position.liquidation_price < position.entry_price,
            ErrorCode::InvalidPosition
        );
    } else {
        // For short positions, liquidation price should be above entry
        require!(
            position.liquidation_price > position.entry_price,
            ErrorCode::InvalidPosition
        );
    }
    
    Ok(())
}

pub fn calculate_health_off_chain(
    user_map: &MapEntryPDA,
    current_price: u64,
) -> u64 {
    if user_map.total_collateral == 0 {
        return 0;
    }

    let mut total_value = user_map.total_collateral;
    let mut total_risk = 0u64;

    for (_i, position) in user_map.positions.iter().enumerate() {
        // Use the single current_price for all positions in this simplified version
        let price_for_position = current_price;
        let price_delta = if price_for_position > position.entry_price {
            price_for_position - position.entry_price
        } else {
            position.entry_price - price_for_position
        };

        let position_risk = (price_delta * position.size * position.leverage) / PRICE_PRECISION;
        total_risk = total_risk.saturating_add(position_risk);
    }

    if total_risk == 0 {
        u64::MAX
    } else {
        (total_value * HEALTH_PRECISION) / total_risk
    }
}

pub fn calculate_position_collateral(position: &Position) -> u64 {
    position.size / position.leverage
}

// Validate global state consistency
pub fn validate_global_state(
    global_config: &GlobalConfigPDA,
    all_positions: &[Position],
) -> Result<()> {
    // Calculate total OI from all positions
    let calculated_oi: u64 = all_positions.iter()
        .map(|p| p.size)
        .sum();
    
    // Allow small discrepancy due to rounding
    require!(
        (global_config.total_oi as i64 - calculated_oi as i64).abs() < 1000,
        ErrorCode::InconsistentCoverage
    );
    
    // Verify coverage calculation
    let expected_coverage = calculate_coverage(
        global_config.vault,
        global_config.total_oi,
        1, // Simplified
    );
    
    require!(
        (global_config.coverage as i128 - expected_coverage as i128).abs() < 1000,
        ErrorCode::InconsistentCoverage
    );
    
    // Verify leverage tiers
    validate_leverage_tiers(&global_config.leverage_tiers)?;
    
    Ok(())
}

pub fn validate_leverage_tiers(tiers: &[LeverageTier]) -> Result<()> {
    // Should have exactly 7 tiers
    require!(
        tiers.len() == 7,
        ErrorCode::InvalidLeverageTier
    );
    
    // Verify tier values match specification
    let expected_tiers = vec![
        (1, 100),
        (2, 70),
        (4, 25),
        (8, 15),
        (16, 12),
        (64, 10),
        (u32::MAX, 5),
    ];
    
    for (i, tier) in tiers.iter().enumerate() {
        let (expected_n, expected_max) = expected_tiers[i];
        require!(
            tier.n == expected_n || (i == 6 && tier.n > 64),
            ErrorCode::InvalidLeverageTier
        );
        require!(
            tier.max == expected_max,
            ErrorCode::InvalidLeverageTier
        );
    }
    
    Ok(())
}

// Validate verse hierarchy
pub fn validate_verse_hierarchy(
    verse: &VersePDA,
    all_verses: &[VersePDA],
) -> Result<()> {
    // Check depth limit
    require!(
        verse.depth <= 32,
        ErrorCode::MaxDepthExceeded
    );
    
    // If has parent, verify parent exists and depth is correct
    if let Some(parent_id) = verse.parent_id {
        let parent = all_verses.iter()
            .find(|v| v.verse_id == parent_id)
            .ok_or(ErrorCode::VerseNotFound)?;
        
        require!(
            verse.depth == parent.depth + 1,
            ErrorCode::InvalidVerseHierarchy
        );
        
        // Check no circular references
        let mut current_parent = parent.parent_id;
        let mut depth_check = 0;
        
        while let Some(pid) = current_parent {
            require!(
                pid != verse.verse_id,
                ErrorCode::CircularHierarchy
            );
            
            depth_check += 1;
            require!(
                depth_check <= 32,
                ErrorCode::MaxDepthExceeded
            );
            
            let p = all_verses.iter()
                .find(|v| v.verse_id == pid)
                .ok_or(ErrorCode::VerseNotFound)?;
            
            current_parent = p.parent_id;
        }
    } else {
        // Root verse should have depth 0
        require!(
            verse.depth == 0,
            ErrorCode::InvalidVerseHierarchy
        );
    }
    
    Ok(())
}

// Helper function to get position price (synchronous)
pub fn get_position_price(position: &Position, price_cache: &PriceCachePDA) -> u64 {
    // In a real implementation, this would look up the specific price for the position's proposal
    // For now, return the cached price
    price_cache.last_price
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_validation() {
        let valid_position = Position {
            proposal_id: 1,
            outcome: 0,
            size: 100_000_000,
            leverage: 10,
            entry_price: 1_000_000,
            liquidation_price: 950_000,
            is_long: true,
            created_at: 0,
        };
        
        assert!(validate_position(&valid_position).is_ok());
        
        // Test invalid size
        let mut invalid_position = valid_position.clone();
        invalid_position.size = 1_000_000; // Too small
        assert!(validate_position(&invalid_position).is_err());
        
        // Test invalid leverage
        invalid_position = valid_position.clone();
        invalid_position.leverage = 600; // Too high
        assert!(validate_position(&invalid_position).is_err());
        
        // Test invalid liquidation price for long
        invalid_position = valid_position.clone();
        invalid_position.liquidation_price = 1_100_000; // Above entry for long
        assert!(validate_position(&invalid_position).is_err());
    }

    #[test]
    fn test_leverage_tier_validation() {
        let valid_tiers = vec![
            LeverageTier { n: 1, max: 100 },
            LeverageTier { n: 2, max: 70 },
            LeverageTier { n: 4, max: 25 },
            LeverageTier { n: 8, max: 15 },
            LeverageTier { n: 16, max: 12 },
            LeverageTier { n: 64, max: 10 },
            LeverageTier { n: 100, max: 5 },
        ];
        
        assert!(validate_leverage_tiers(&valid_tiers).is_ok());
        
        // Test wrong number of tiers
        let invalid_tiers = valid_tiers[..6].to_vec();
        assert!(validate_leverage_tiers(&invalid_tiers).is_err());
        
        // Test wrong max value
        let mut invalid_tiers = valid_tiers.clone();
        invalid_tiers[0].max = 50; // Should be 100
        assert!(validate_leverage_tiers(&invalid_tiers).is_err());
    }
}