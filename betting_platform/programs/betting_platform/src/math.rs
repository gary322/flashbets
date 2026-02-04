use anchor_lang::prelude::*;
use crate::errors::ErrorCode;
use crate::fixed_math::FixedPoint;

// AMM Price Calculation Functions

pub fn calculate_lmsr_price(
    q_values: &[i64],
    liquidity_parameter: u64,
    outcome: usize,
) -> Result<u64> {
    // LMSR (Logarithmic Market Scoring Rule) price calculation
    // Price = e^(q_i/b) / Σ(e^(q_j/b)) for all j
    
    if outcome >= q_values.len() {
        return Err(ErrorCode::InvalidOutcome.into());
    }
    
    let b = liquidity_parameter as f64;
    let q_i = q_values[outcome] as f64;
    
    // Calculate numerator: e^(q_i/b)
    let numerator = (q_i / b).exp();
    
    // Calculate denominator: sum of e^(q_j/b) for all outcomes
    let denominator: f64 = q_values.iter()
        .map(|&q| (q as f64 / b).exp())
        .sum();
    
    // Price = numerator / denominator
    let price = numerator / denominator;
    
    // Convert to fixed point (scale by 10^9 for precision)
    Ok((price * 1_000_000_000.0) as u64)
}

pub fn calculate_pmamm_price(
    prices: &[u64],
    amount: u64,
    liquidity_parameter: u64,
    time_elapsed: u64,
    total_time: u64,
) -> Result<u64> {
    // PM-AMM (Prediction Market AMM) price calculation with time decay
    
    if prices.is_empty() {
        return Err(ErrorCode::InvalidOutcome.into());
    }
    
    // Base price from current prices
    let base_price = prices[0];
    
    // Time decay factor (0 to 1)
    let time_factor = if total_time > 0 {
        1.0 - (time_elapsed as f64 / total_time as f64).min(1.0)
    } else {
        1.0
    };
    
    // Liquidity impact
    let liquidity_impact = (amount as f64) / (liquidity_parameter as f64);
    
    // Price impact = base_price * (1 + liquidity_impact * time_factor)
    let price_impact = 1.0 + (liquidity_impact * time_factor);
    let final_price = (base_price as f64 * price_impact) as u64;
    
    Ok(final_price)
}

pub fn calculate_l2_price(
    prices: &[u64],
    outcome: usize,
    amount: u64,
) -> Result<u64> {
    // L2 (Layer 2) continuous distribution price calculation
    
    if outcome >= prices.len() {
        return Err(ErrorCode::InvalidOutcome.into());
    }
    
    // Get base price for outcome
    let base_price = prices[outcome];
    
    // Apply square root market impact for continuous distributions
    let impact_factor = ((amount as f64).sqrt() / 1000.0).min(0.1); // Cap at 10% impact
    
    // Calculate final price with impact
    let final_price = (base_price as f64 * (1.0 + impact_factor)) as u64;
    
    Ok(final_price)
}

pub fn calculate_exp_positive(x: u128) -> u64 {
    // Taylor series approximation for e^x
    // e^x ≈ 1 + x + x²/2! + x³/3! + x⁴/4! + ...
    
    let precision = 1_000_000_000u128; // 10^9 for fixed point
    
    if x == 0 {
        return precision as u64;
    }
    
    // Limit x to prevent overflow
    let x_capped = x.min(5 * precision); // Cap at e^5 ≈ 148
    
    let mut result = precision; // Start with 1.0
    let mut term = x_capped; // First term: x
    result = result.saturating_add(term);
    
    // x²/2!
    term = term.saturating_mul(x_capped) / precision / 2;
    result = result.saturating_add(term);
    
    // x³/3!
    term = term.saturating_mul(x_capped) / precision / 3;
    result = result.saturating_add(term);
    
    // x⁴/4!
    term = term.saturating_mul(x_capped) / precision / 4;
    result = result.saturating_add(term);
    
    (result / precision) as u64
}

// Liquidation price calculation
pub fn calculate_liquidation_price(
    entry_price: u64,
    leverage: u64,
    is_long: bool,
    coverage: u128,
) -> u64 {
    // Calculate maintenance margin based on coverage
    let maintenance_margin = if coverage > 1_000_000_000 {
        200 // 2% for high coverage
    } else {
        500 // 5% for low coverage
    };
    
    // Liquidation occurs when loss reaches (100% - maintenance_margin) of collateral
    let max_loss_percentage = 10000u64.saturating_sub(maintenance_margin);
    
    if is_long {
        // Long position: liquidated when price drops
        // Liquidation Price = Entry Price × (1 - max_loss% / leverage)
        let price_drop = max_loss_percentage.saturating_div(leverage);
        entry_price.saturating_mul(10000u64.saturating_sub(price_drop)) / 10000
    } else {
        // Short position: liquidated when price rises
        // Liquidation Price = Entry Price × (1 + max_loss% / leverage)
        let price_rise = max_loss_percentage.saturating_div(leverage);
        entry_price.saturating_mul(10000u64.saturating_add(price_rise)) / 10000
    }
}

// Volatility calculation for liquidation
pub fn calculate_volatility(price_history: &PriceHistory) -> u64 {
    // Simple volatility calculation based on price movements
    let movements = &price_history.movements;
    
    if movements.is_empty() {
        return 0;
    }
    
    // Calculate standard deviation of price movements
    let sum: i64 = movements.iter().sum();
    let mean = sum / movements.len() as i64;
    
    let variance: u64 = movements.iter()
        .map(|&x| {
            let diff = (x - mean).abs() as u64;
            diff.saturating_mul(diff)
        })
        .sum::<u64>() / movements.len() as u64;
    
    // Return square root as volatility (simplified)
    (variance as f64).sqrt() as u64
}

// Price history tracking
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PriceHistory {
    pub movements: Vec<i64>,
    pub last_update_slot: u64,
}

impl PriceHistory {
    pub const MAX_HISTORY: usize = 100;
    
    pub fn add_movement(&mut self, movement: i64, slot: u64) {
        self.movements.push(movement);
        
        // Keep only recent history
        if self.movements.len() > Self::MAX_HISTORY {
            self.movements.remove(0);
        }
        
        self.last_update_slot = slot;
    }
}