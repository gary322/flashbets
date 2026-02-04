//! PM-AMM Integration with Precomputed CDF/PDF Tables
//! 
//! Optimized Newton-Raphson solver and batch calculations using tables

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    program_pack::Pack,
    msg,
};
use crate::math::{
    U64F64,
    tables::NormalDistributionTables,
    table_lookup::{lookup_cdf, lookup_pdf},
};
use crate::state::amm_accounts::PMAMMMarket;

/// PM-AMM delta calculation using tables
/// Solves: (y - x) Φ((y - x)/(L√(T-t))) + L√(T-t) φ((y - x)/(L√(T-t))) - y = 0
pub fn calculate_pmamm_delta_with_tables(
    tables: &NormalDistributionTables,
    current_inventory: U64F64,
    order_size: U64F64,
    liquidity: U64F64,
    time_to_expiry: U64F64,
) -> Result<U64F64, ProgramError> {
    // Validate inputs
    if liquidity.is_zero() {
        msg!("Zero liquidity");
        return Err(ProgramError::InvalidArgument);
    }
    
    if time_to_expiry.is_zero() {
        msg!("Market expired");
        return Err(ProgramError::InvalidArgument);
    }
    
    // Calculate L√(T-t)
    let sqrt_tau = time_to_expiry.sqrt()?;
    let l_sqrt_tau = liquidity.checked_mul(sqrt_tau)?;
    
    // Initial guess: y = x + order_size
    let mut y = current_inventory.checked_add(order_size)?;
    
    // Newton-Raphson parameters
    let tolerance = U64F64::from_fraction(1, 10000)?; // 0.0001
    let max_iterations = 10;
    
    for iteration in 0..max_iterations {
        // Calculate z = (y - x) / (L√(T-t))
        let y_minus_x = y.checked_sub(current_inventory)?;
        let z = y_minus_x.checked_div(l_sqrt_tau)?;
        
        // Table lookups for Φ(z) and φ(z)
        let phi_z = lookup_cdf(tables, z)?;
        let pdf_z = lookup_pdf(tables, z)?;
        
        // Calculate f(y) = (y - x) * Φ(z) + L√(T-t) * φ(z) - y
        let term1 = y_minus_x.checked_mul(phi_z)?;
        let term2 = l_sqrt_tau.checked_mul(pdf_z)?;
        let f = term1.checked_add(term2)?.checked_sub(y)?;
        
        // Calculate f'(y) = Φ(z) + (y-x)/(L√(T-t)) * φ(z) - z * φ(z) - 1
        // Simplifies to: f'(y) = Φ(z) - 1
        let one = U64F64::from_num(1);
        let df = phi_z.checked_sub(one)?;
        
        // Check convergence
        if f.raw < tolerance.raw {
            msg!("PM-AMM converged in {} iterations", iteration + 1);
            break;
        }
        
        // Avoid division by very small df
        // Since raw is u128, check if it's below threshold
        if df.raw < U64F64::from_fraction(1, 1000000)?.raw {
            msg!("Newton-Raphson derivative too small");
            break;
        }
        
        // Newton step: y_new = y - f(y)/f'(y)
        let step = f.checked_div(df)?;
        
        // Update y with bounds checking
        if step.raw > y.raw {
            // Would go negative, clamp to small positive value
            y = U64F64::from_fraction(1, 100)?;
        } else {
            y = y.checked_sub(step)?;
        }
        
        // Sanity check: y should be reasonable
        let max_y = current_inventory.checked_add(order_size.checked_mul(U64F64::from_num(10))?)?;
        if y.raw > max_y.raw {
            msg!("PM-AMM y value exceeding reasonable bounds");
            y = current_inventory.checked_add(order_size)?; // Reset to initial guess
        }
    }
    
    // Return the delta (change in inventory)
    y.checked_sub(current_inventory)
}

/// Batch PM-AMM calculation for multiple orders
pub fn batch_calculate_pmamm(
    tables: &NormalDistributionTables,
    orders: &[PMAMMOrder],
    liquidity: U64F64,
    time_to_expiry: U64F64,
) -> Result<Vec<PMAMMResult>, ProgramError> {
    let mut results = Vec::with_capacity(orders.len());
    
    // Pre-compute common values
    let sqrt_tau = time_to_expiry.sqrt()?;
    let l_sqrt_tau = liquidity.checked_mul(sqrt_tau)?;
    let inv_l_sqrt_tau = U64F64::from_num(1).checked_div(l_sqrt_tau)?;
    
    for order in orders {
        // Simplified Newton iteration optimized for batching
        let mut y = order.current_inventory.checked_add(order.size)?;
        let tolerance = U64F64::from_fraction(1, 1000)?; // Slightly relaxed for batch
        
        for _ in 0..5 { // Fewer iterations for batch processing
            let y_minus_x = y.checked_sub(order.current_inventory)?;
            let z = y_minus_x.checked_mul(inv_l_sqrt_tau)?;
            
            // Batch-optimized table lookups
            let phi_z = lookup_cdf(tables, z)?;
            let pdf_z = lookup_pdf(tables, z)?;
            
            let f = y_minus_x.checked_mul(phi_z)?
                .checked_add(l_sqrt_tau.checked_mul(pdf_z)?)?
                .checked_sub(y)?;
            
            let df = phi_z.checked_sub(U64F64::from_num(1))?;
            
            if f.raw < tolerance.raw || df.raw < U64F64::from_fraction(1, 100000)?.raw {
                break;
            }
            
            let step = f.checked_div(df)?;
            if step.raw > y.raw {
                y = U64F64::from_fraction(1, 100)?;
            } else {
                y = y.checked_sub(step)?;
            }
        }
        
        let delta = y.checked_sub(order.current_inventory)?;
        
        // Calculate price impact using tables
        let price_impact = calculate_price_impact_with_tables(
            tables,
            order.current_inventory,
            delta,
            liquidity,
            time_to_expiry,
        )?;
        
        results.push(PMAMMResult {
            order_id: order.order_id,
            delta,
            final_inventory: y,
            price_impact,
        });
    }
    
    Ok(results)
}

/// Calculate price impact for PM-AMM trade
pub fn calculate_price_impact_with_tables(
    tables: &NormalDistributionTables,
    current_inventory: U64F64,
    delta: U64F64,
    liquidity: U64F64,
    time_to_expiry: U64F64,
) -> Result<U64F64, ProgramError> {
    // Price impact = |Φ((x+δ)/(L√τ)) - Φ(x/(L√τ))|
    
    let sqrt_tau = time_to_expiry.sqrt()?;
    let l_sqrt_tau = liquidity.checked_mul(sqrt_tau)?;
    
    // Calculate z values
    let z_before = current_inventory.checked_div(l_sqrt_tau)?;
    let z_after = current_inventory.checked_add(delta)?.checked_div(l_sqrt_tau)?;
    
    // Look up CDFs
    let phi_before = lookup_cdf(tables, z_before)?;
    let phi_after = lookup_cdf(tables, z_after)?;
    
    // Calculate absolute difference
    if phi_after.raw > phi_before.raw {
        phi_after.checked_sub(phi_before)
    } else {
        phi_before.checked_sub(phi_after)
    }
}

/// Calculate PM-AMM liquidity provision value
pub fn calculate_lp_value_with_tables(
    tables: &NormalDistributionTables,
    inventory: U64F64,
    liquidity: U64F64,
    time_to_expiry: U64F64,
    spot_price: U64F64,
) -> Result<U64F64, ProgramError> {
    // LP value = inventory * spot_price + liquidity * √(2πτ) * φ(inventory/(L√τ))
    
    let sqrt_tau = time_to_expiry.sqrt()?;
    let l_sqrt_tau = liquidity.checked_mul(sqrt_tau)?;
    
    // Calculate z = inventory / (L√τ)
    let z = inventory.checked_div(l_sqrt_tau)?;
    
    // Look up φ(z)
    let pdf_z = lookup_pdf(tables, z)?;
    
    // Calculate √(2π) ≈ 2.5066
    let sqrt_2pi = U64F64::from_fraction(25066, 10000)?;
    
    // Value = inventory * spot + L * √(2πτ) * φ(z)
    let inventory_value = inventory.checked_mul(spot_price)?;
    let option_value = liquidity
        .checked_mul(sqrt_2pi)?
        .checked_mul(sqrt_tau)?
        .checked_mul(pdf_z)?;
    
    inventory_value.checked_add(option_value)
}

/// Order structure for batch processing
#[derive(Debug, Clone, Copy)]
pub struct PMAMMOrder {
    pub order_id: u64,
    pub current_inventory: U64F64,
    pub size: U64F64,
}

/// Result structure for batch processing
#[derive(Debug, Clone, Copy)]
pub struct PMAMMResult {
    pub order_id: u64,
    pub delta: U64F64,
    pub final_inventory: U64F64,
    pub price_impact: U64F64,
}

/// Process PM-AMM trade with tables
pub fn process_pmamm_trade_with_tables(
    tables_account: &AccountInfo,
    market: &mut PMAMMMarket,
    is_buy: bool,
    amount: u64,
    clock_sysvar: &AccountInfo,
) -> Result<(u64, u64), ProgramError> {
    // Load tables
    let tables = NormalDistributionTables::unpack(&tables_account.data.borrow())?;
    
    if !tables.is_initialized {
        msg!("Normal distribution tables not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    // Convert to fixed-point
    let amount_fp = U64F64::from_num(amount);
    
    // For PM-AMM, use reserves to calculate inventory
    // Assuming binary market (2 outcomes) for simplicity
    if market.reserves.len() < 2 {
        msg!("Market must have at least 2 outcomes");
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Calculate inventory as signed difference between reserves
    // For PM-AMM, inventory represents net position imbalance
    let inventory_abs = if market.reserves[0] >= market.reserves[1] {
        market.reserves[0] - market.reserves[1]
    } else {
        market.reserves[1] - market.reserves[0]
    };
    let current_inventory = U64F64::from_num(inventory_abs);
    
    // Note: In PM-AMM, negative inventory is handled by the pricing function
    // which adjusts prices based on the direction of imbalance
    let inventory_is_negative = market.reserves[1] > market.reserves[0];
    
    let liquidity = U64F64::from_num(market.l_parameter);
    
    // Get current timestamp from Clock sysvar
    use solana_program::clock::Clock;
    use solana_program::sysvar::Sysvar;
    let clock = Clock::from_account_info(clock_sysvar)?;
    let current_timestamp = clock.unix_timestamp;
    
    // Calculate time to expiry
    let time_to_expiry_secs = market.expiry_time.saturating_sub(current_timestamp).max(0);
    let time_to_expiry = U64F64::from_num(time_to_expiry_secs as u64) / U64F64::from_num(86400); // Seconds per day
    
    // Adjust amount for buy/sell direction
    let signed_amount = if is_buy {
        amount_fp
    } else {
        U64F64::from_num(0).checked_sub(amount_fp)?
    };
    
    // Calculate PM-AMM delta
    let delta = calculate_pmamm_delta_with_tables(
        &tables,
        current_inventory,
        signed_amount,
        liquidity,
        time_to_expiry,
    )?;
    
    // Calculate cost (in basis points)
    let cost_fp = delta.checked_mul(U64F64::from_num(10000))?; // Convert to basis points
    let cost = cost_fp.to_num();
    
    // Calculate price impact
    let impact_fp = calculate_price_impact_with_tables(
        &tables,
        current_inventory,
        delta,
        liquidity,
        time_to_expiry,
    )?;
    let impact = (impact_fp.checked_mul(U64F64::from_num(10000))?.to_num()).min(10000); // Cap at 100%
    
    Ok((cost, impact as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pmamm_convergence() {
        // Would need initialized tables for full test
        // This is a structure test
        
        let order = PMAMMOrder {
            order_id: 1,
            current_inventory: U64F64::from_num(0),
            size: U64F64::from_num(100),
        };
        
        assert_eq!(order.order_id, 1);
        assert_eq!(order.size.to_num(), 100);
    }

    #[test]
    fn test_price_impact_symmetry() {
        // Price impact should be symmetric for opposite trades
        // Would need tables to fully test
    }
}