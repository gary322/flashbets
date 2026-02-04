//! AMM helper functions
//!
//! Common utilities for all AMM types

use solana_program::program_error::ProgramError;

use crate::{
    error::BettingPlatformError,
    math::U64F64,
    state::accounts::AMMType,
};

/// Calculate fee amount for a trade
pub fn calculate_fee(
    amount: u64,
    fee_bps: u16,
) -> Result<u64, ProgramError> {
    if fee_bps > 10000 {
        return Err(BettingPlatformError::FeeTooHigh.into());
    }

    Ok(amount
        .saturating_mul(fee_bps as u64)
        .saturating_div(10000))
}

/// Apply fee to amount (subtract fee)
pub fn apply_fee(
    amount: u64,
    fee_bps: u16,
) -> Result<u64, ProgramError> {
    let fee = calculate_fee(amount, fee_bps)?;
    Ok(amount.saturating_sub(fee))
}

/// Calculate price impact percentage
pub fn calculate_price_impact_percent(
    initial_price: u64,
    final_price: u64,
) -> u16 {
    if initial_price == 0 {
        return 10000; // 100% impact
    }

    let impact = if final_price > initial_price {
        ((final_price - initial_price) * 10000) / initial_price
    } else {
        ((initial_price - final_price) * 10000) / initial_price
    };

    impact.min(10000) as u16
}

/// Validate slippage tolerance
pub fn validate_slippage(
    expected_amount: u64,
    actual_amount: u64,
    max_slippage_bps: u16,
    is_buy: bool,
) -> Result<(), ProgramError> {
    if max_slippage_bps > 10000 {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    let slippage = if is_buy {
        // For buys, actual cost should not exceed expected by more than slippage
        if actual_amount > expected_amount {
            ((actual_amount - expected_amount) * 10000) / expected_amount
        } else {
            0
        }
    } else {
        // For sells, actual payout should not be less than expected by more than slippage
        if expected_amount > actual_amount {
            ((expected_amount - actual_amount) * 10000) / expected_amount
        } else {
            0
        }
    };

    if slippage > max_slippage_bps as u64 {
        return Err(BettingPlatformError::SlippageExceeded.into());
    }

    Ok(())
}

/// Calculate optimal trade size to minimize price impact
pub fn calculate_optimal_trade_size(
    total_liquidity: u64,
    target_impact_bps: u16,
) -> u64 {
    // Rule of thumb: trade size = liquidity * sqrt(target_impact)
    // For 1% impact (100 bps), trade ~10% of liquidity
    
    let impact_fraction = U64F64::from_num(target_impact_bps as u64) / U64F64::from_num(10000);
    let sqrt_impact = impact_fraction.sqrt().unwrap_or(U64F64::from_num(0));
    
    let optimal_size = U64F64::from_num(total_liquidity) * sqrt_impact;
    optimal_size.to_num()
}

/// Estimate gas cost for AMM operation
pub fn estimate_compute_units(
    amm_type: AMMType,
    operation: AMMOperation,
) -> u32 {
    match (amm_type, operation) {
        (AMMType::LMSR, AMMOperation::Trade) => 20_000,    // Optimized from 50k
        (AMMType::LMSR, AMMOperation::Initialize) => 15_000, // Optimized from 30k
        (AMMType::PMAMM, AMMOperation::Trade) => 35_000,   // Slightly optimized
        (AMMType::PMAMM, AMMOperation::AddLiquidity) => 45_000,
        (AMMType::PMAMM, AMMOperation::RemoveLiquidity) => 40_000,
        (AMMType::L2AMM, AMMOperation::Trade) => 25_000,   // Optimized from 70k
        (AMMType::L2AMM, AMMOperation::UpdateDistribution) => 30_000, // Optimized from 80k
        (AMMType::Hybrid, AMMOperation::Trade) => 30_000,  // Hybrid trade overhead
        (AMMType::Hybrid, AMMOperation::Initialize) => 20_000, // Hybrid initialization
        (AMMType::Hybrid, AMMOperation::AddLiquidity) => 50_000, // Hybrid liquidity ops
        (AMMType::Hybrid, AMMOperation::RemoveLiquidity) => 45_000,
        (AMMType::Hybrid, AMMOperation::UpdateDistribution) => 35_000,
        _ => 50_000, // Conservative estimate
    }
}

/// Estimate compute units with optimization flag
pub fn estimate_compute_units_optimized(
    amm_type: AMMType,
    operation: AMMOperation,
    use_optimized: bool,
) -> u32 {
    if use_optimized {
        match (amm_type, operation) {
            (AMMType::LMSR, AMMOperation::Trade) => 18_000,    // Using optimized math
            (AMMType::LMSR, AMMOperation::Initialize) => 12_000,
            (AMMType::PMAMM, AMMOperation::Trade) => 30_000,
            (AMMType::PMAMM, AMMOperation::AddLiquidity) => 40_000,
            (AMMType::PMAMM, AMMOperation::RemoveLiquidity) => 35_000,
            (AMMType::L2AMM, AMMOperation::Trade) => 20_000,   // Using optimized math
            (AMMType::L2AMM, AMMOperation::UpdateDistribution) => 25_000,
            (AMMType::Hybrid, AMMOperation::Trade) => 25_000,  // Optimized hybrid trade
            (AMMType::Hybrid, AMMOperation::Initialize) => 15_000,
            (AMMType::Hybrid, AMMOperation::AddLiquidity) => 45_000,
            (AMMType::Hybrid, AMMOperation::RemoveLiquidity) => 40_000,
            (AMMType::Hybrid, AMMOperation::UpdateDistribution) => 30_000,
            _ => 40_000,
        }
    } else {
        estimate_compute_units(amm_type, operation)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AMMOperation {
    Initialize,
    Trade,
    AddLiquidity,
    RemoveLiquidity,
    UpdateDistribution,
}

/// Calculate time-weighted average price (TWAP)
pub fn calculate_twap(
    price_history: &[(u64, i64)], // (price, timestamp)
    duration: i64,
) -> Result<u64, ProgramError> {
    if price_history.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    let current_time = price_history.last().unwrap().1;
    let start_time = current_time - duration;

    let mut weighted_sum = 0u128;
    let mut total_weight = 0u128;

    for window in price_history.windows(2) {
        let (price1, time1) = window[0];
        let (_, time2) = window[1];

        if time2 < start_time {
            continue;
        }

        let time_start = time1.max(start_time);
        let time_weight = (time2 - time_start) as u128;

        weighted_sum += price1 as u128 * time_weight;
        total_weight += time_weight;
    }

    if total_weight == 0 {
        return Ok(price_history.last().unwrap().0);
    }

    Ok((weighted_sum / total_weight) as u64)
}

/// Check if price is within acceptable bounds
pub fn validate_price_bounds(
    price: u64,
    min_price: u64,
    max_price: u64,
) -> Result<(), ProgramError> {
    if price < min_price || price > max_price {
        return Err(BettingPlatformError::PriceOutOfBounds.into());
    }
    Ok(())
}

/// Validate price movement per slot (2% clamp)
pub fn validate_price_movement_per_slot(
    old_price: u64,
    new_price: u64,
    slots_elapsed: u64,
) -> Result<(), ProgramError> {
    use crate::constants::PRICE_CLAMP_PER_SLOT_BPS;
    
    if old_price == 0 {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate max allowed change based on slots elapsed
    let max_change_bps = PRICE_CLAMP_PER_SLOT_BPS as u64 * slots_elapsed;
    
    // Calculate actual price change in basis points
    let price_change_bps = if new_price > old_price {
        ((new_price - old_price) * 10000) / old_price
    } else {
        ((old_price - new_price) * 10000) / old_price
    };
    
    // Check if price change exceeds allowed limit
    if price_change_bps > max_change_bps {
        return Err(BettingPlatformError::PriceManipulation.into());
    }
    
    Ok(())
}

/// Calculate liquidity provider share value
pub fn calculate_lp_share_value(
    lp_tokens: u64,
    total_lp_supply: u64,
    pool_value: u64,
) -> Result<u64, ProgramError> {
    if total_lp_supply == 0 {
        return Err(BettingPlatformError::DivisionByZero.into());
    }

    let share_value = (lp_tokens as u128 * pool_value as u128 / total_lp_supply as u128) as u64;
    Ok(share_value)
}

/// Calculate impermanent loss for liquidity providers
pub fn calculate_impermanent_loss(
    initial_prices: &[u64],
    current_prices: &[u64],
) -> Result<u16, ProgramError> {
    if initial_prices.len() != current_prices.len() || initial_prices.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }

    // Calculate price ratios
    let mut price_ratios = Vec::new();
    for (i, &initial) in initial_prices.iter().enumerate() {
        if initial == 0 {
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        let ratio = U64F64::from_num(current_prices[i]) / U64F64::from_num(initial);
        price_ratios.push(ratio);
    }

    // Calculate geometric mean of ratios
    let mut product = U64F64::from_num(1);
    for ratio in &price_ratios {
        product = product * ratio.sqrt()?;
    }
    let geometric_mean = product;

    // IL = 2 * sqrt(product of ratios) / (1 + sum of ratios) - 1
    let sum_ratios = price_ratios.iter().fold(U64F64::from_num(0), |acc, x| acc + *x);
    let n = price_ratios.len() as u64;
    
    let il_ratio = (U64F64::from_num(2) * geometric_mean) / 
                   (U64F64::from_num(1) + sum_ratios / U64F64::from_num(n));
    
    let il_percent = if il_ratio < U64F64::from_num(1) {
        (U64F64::from_num(1) - il_ratio) * U64F64::from_num(10000)
    } else {
        U64F64::from_num(0)
    };

    Ok(il_percent.to_num() as u16)
}

/// Calculate price impact for a given trade amount
pub fn calculate_price_impact(
    proposal_account: &[u8],
    outcome: u8,
    amount: u64,
    is_buy: bool,
) -> Result<u64, ProgramError> {
    use crate::state::ProposalPDA;
    use borsh::BorshDeserialize;
    
    // Deserialize proposal
    let proposal = ProposalPDA::try_from_slice(proposal_account)?;
    
    // Get current price based on AMM type
    let current_price = match proposal.amm_type {
        AMMType::LMSR => {
            use crate::amm::lmsr::LMSRAMMContext;
            let context = LMSRAMMContext::from_proposal(&proposal)?;
            context.price(outcome)?
        }
        AMMType::PMAMM => {
            use crate::amm::pmamm::price_discovery::PMAMMContext;
            let context = PMAMMContext::from_proposal(&proposal)?;
            context.current_price(outcome)?
        }
        AMMType::L2AMM => {
            use crate::amm::l2amm::types::L2AMMContext;
            let context = L2AMMContext::from_proposal(&proposal)?;
            context.calculate_price(outcome)?
        }
        AMMType::Hybrid => {
            use crate::amm::hybrid::calculate_hybrid_price;
            calculate_hybrid_price(&proposal, outcome)?
        }
    };
    
    // Simulate trade to get new price
    let mut proposal_copy = proposal.clone();
    if is_buy {
        proposal_copy.outcome_balances[outcome as usize] = 
            proposal_copy.outcome_balances[outcome as usize]
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
    } else {
        proposal_copy.outcome_balances[outcome as usize] = 
            proposal_copy.outcome_balances[outcome as usize]
                .checked_sub(amount)
                .ok_or(BettingPlatformError::InsufficientBalance)?;
    }
    
    // Get new price after trade
    let new_price = match proposal_copy.amm_type {
        AMMType::LMSR => {
            use crate::amm::lmsr::LMSRAMMContext;
            let context = LMSRAMMContext::from_proposal(&proposal_copy)?;
            context.price(outcome)?
        }
        AMMType::PMAMM => {
            use crate::amm::pmamm::price_discovery::PMAMMContext;
            let context = PMAMMContext::from_proposal(&proposal_copy)?;
            context.current_price(outcome)?
        }
        AMMType::L2AMM => {
            use crate::amm::l2amm::types::L2AMMContext;
            let context = L2AMMContext::from_proposal(&proposal_copy)?;
            context.calculate_price(outcome)?
        }
        AMMType::Hybrid => {
            use crate::amm::hybrid::calculate_hybrid_price;
            calculate_hybrid_price(&proposal_copy, outcome)?
        }
    };
    
    // Calculate impact as absolute difference
    let impact = if new_price > current_price {
        new_price - current_price
    } else {
        current_price - new_price
    };
    
    Ok(impact)
}

/// Execute a trade on the AMM
pub fn execute_trade(
    proposal_account: &mut [u8],
    outcome: u8,
    amount: u64,
    is_buy: bool,
) -> Result<u64, ProgramError> {
    use crate::state::ProposalPDA;
    use borsh::{BorshDeserialize, BorshSerialize};
    
    // Deserialize proposal
    let mut proposal = ProposalPDA::try_from_slice(proposal_account)?;
    
    // Validate inputs
    if outcome >= proposal.outcomes as u8 {
        return Err(BettingPlatformError::InvalidOutcome.into());
    }
    
    if amount == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    // Get current price
    let current_price = match proposal.amm_type {
        AMMType::LMSR => {
            use crate::amm::lmsr::LMSRAMMContext;
            let context = LMSRAMMContext::from_proposal(&proposal)?;
            context.price(outcome)?
        }
        AMMType::PMAMM => {
            use crate::amm::pmamm::price_discovery::PMAMMContext;
            let context = PMAMMContext::from_proposal(&proposal)?;
            context.current_price(outcome)?
        }
        AMMType::L2AMM => {
            use crate::amm::l2amm::types::L2AMMContext;
            let context = L2AMMContext::from_proposal(&proposal)?;
            context.calculate_price(outcome)?
        }
        AMMType::Hybrid => {
            use crate::amm::hybrid::calculate_hybrid_price;
            calculate_hybrid_price(&proposal, outcome)?
        }
    };
    
    // Update balances
    if is_buy {
        proposal.outcome_balances[outcome as usize] = 
            proposal.outcome_balances[outcome as usize]
                .checked_add(amount)
                .ok_or(BettingPlatformError::MathOverflow)?;
        proposal.total_volume = proposal.total_volume
            .checked_add(amount)
            .ok_or(BettingPlatformError::MathOverflow)?;
    } else {
        proposal.outcome_balances[outcome as usize] = 
            proposal.outcome_balances[outcome as usize]
                .checked_sub(amount)
                .ok_or(BettingPlatformError::InsufficientBalance)?;
    }
    
    // Serialize back
    proposal.serialize(&mut &mut proposal_account[..])?;
    
    // Return executed price
    Ok(current_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_fee() {
        assert_eq!(calculate_fee(10000, 30).unwrap(), 30); // 0.3%
        assert_eq!(calculate_fee(10000, 100).unwrap(), 100); // 1%
        assert_eq!(calculate_fee(10000, 0).unwrap(), 0); // 0%
    }

    #[test]
    fn test_validate_slippage() {
        // Buy with acceptable slippage
        assert!(validate_slippage(1000, 1050, 500, true).is_ok()); // 5% slippage, 5% allowed

        // Buy with excessive slippage
        assert!(validate_slippage(1000, 1100, 500, true).is_err()); // 10% slippage, 5% allowed

        // Sell with acceptable slippage
        assert!(validate_slippage(1000, 950, 500, false).is_ok()); // 5% slippage, 5% allowed
    }

    #[test]
    fn test_calculate_twap() {
        let price_history = vec![
            (100, 0),
            (110, 10),
            (120, 20),
            (115, 30),
        ];

        let twap = calculate_twap(&price_history, 30).unwrap();
        assert!(twap > 100 && twap < 120);
    }
    
    #[test]
    fn test_validate_price_movement_per_slot() {
        // 1% change in 1 slot - should pass (under 2% limit)
        assert!(validate_price_movement_per_slot(1000, 1010, 1).is_ok());
        
        // 3% change in 1 slot - should fail (over 2% limit)
        assert!(validate_price_movement_per_slot(1000, 1030, 1).is_err());
        
        // 5% change in 3 slots - should pass (under 6% limit)
        assert!(validate_price_movement_per_slot(1000, 1050, 3).is_ok());
        
        // 10% change in 3 slots - should fail (over 6% limit)
        assert!(validate_price_movement_per_slot(1000, 1100, 3).is_err());
        
        // Test with price decrease
        assert!(validate_price_movement_per_slot(1000, 970, 1).is_err()); // 3% drop
        assert!(validate_price_movement_per_slot(1000, 980, 1).is_ok());  // 2% drop
    }
}