use anchor_lang::prelude::*;
use crate::fixed_math::*;
use crate::errors::ErrorCode;

#[derive(Debug, Clone)]
pub struct PMAMMMarket {
    pub l: FixedPoint,  // Liquidity parameter
    pub t: FixedPoint,  // Time to expiry
    pub current_price: FixedPoint,
    pub inventory: FixedPoint,
    pub tau: FixedPoint,  // Time decay parameter (default 0.1)
}

impl PMAMMMarket {
    /// Solve implicit equation using Newton-Raphson with time decay
    /// (y - x) * Φ((y - x)/(L√(T-t))) + L√(T-t) * φ((y - x)/(L√(T-t))) - y = 0
    pub fn solve_trade(
        &self,
        order_size: FixedPoint,
        current_time: FixedPoint,
    ) -> Result<FixedPoint> {
        let time_remaining = self.t.sub(&current_time)?;
        let tau_adjusted = time_remaining.mul(&self.tau)?;  // Apply time decay factor
        let tau_sqrt = tau_adjusted.sqrt()?;
        let l_tau = self.l.mul(&tau_sqrt)?;

        // Initial guess
        let mut y = self.current_price.add(&order_size.mul(&FixedPoint::from_float(0.5))?)?;

        // Newton-Raphson iterations
        const MAX_ITERATIONS: u32 = 10;
        const EPSILON: f64 = 1e-8;

        for _ in 0..MAX_ITERATIONS {
            let z = (y.sub(&self.current_price)?).div(&l_tau)?;

            // Calculate Φ(z) and φ(z)
            let phi_z = normal_cdf(z)?;
            let pdf_z = normal_pdf(z)?;

            // f(y) = (y - x) * Φ(z) + L√(T-t) * φ(z) - y
            let f_y = (y.sub(&self.current_price)?)
                .mul(&phi_z)?
                .add(&l_tau.mul(&pdf_z)?)?
                .sub(&y)?;

            // f'(y) = Φ(z) + (y-x)/(L√(T-t)) * φ(z) + L√(T-t) * φ'(z) * 1/(L√(T-t)) - 1
            let df_dy = phi_z
                .add(&z.mul(&pdf_z)?)?
                .sub(&FixedPoint::from_u64(1))?;

            // Newton step
            let delta = f_y.div(&df_dy)?;

            // Check convergence
            if delta.abs()?.to_float() < EPSILON {
                return Ok(y);
            }

            y = y.sub(&delta)?;
        }

        Err(ErrorCode::ConvergenceFailed.into())
    }

    /// Calculate uniform LVR
    pub fn calculate_lvr(&self, current_time: FixedPoint) -> Result<FixedPoint> {
        let remaining_time = self.t.sub(&current_time)?;
        let beta = FixedPoint::from_float(0.05); // 5% LVR target

        // LVR = β * V_t / (T - t)
        let v_t = self.inventory.mul(&self.current_price)?;
        beta.mul(&v_t)?.div(&remaining_time)
    }
}

/// Precomputed normal CDF values for efficiency
pub fn normal_cdf(z: FixedPoint) -> Result<FixedPoint> {
    // Use precomputed lookup table
    let z_float = z.to_float();

    // Simplified approximation for demonstration
    // In production, use precomputed tables
    if z_float.abs() > 6.0 {
        return Ok(if z_float > 0.0 {
            FixedPoint::from_u64(1)
        } else {
            FixedPoint::zero()
        });
    }

    // Use error function approximation
    let erf_z = erf_approximation(z_float / 2.0_f64.sqrt())?;
    Ok(FixedPoint::from_float(0.5 * (1.0 + erf_z)))
}

/// Normal PDF calculation
pub fn normal_pdf(z: FixedPoint) -> Result<FixedPoint> {
    let z_squared = z.mul(&z)?;
    let neg_half_z_squared = z_squared.div(&FixedPoint::from_u64(2))?.neg()?;
    let exp_term = neg_half_z_squared.exp()?;

    let sqrt_2pi = FixedPoint::from_float(2.506628274631);
    exp_term.div(&sqrt_2pi)
}

#[account]
pub struct PMAMMStatePDA {
    pub market_id: u128,
    pub l_parameter: u64,  // Liquidity parameter
    pub expiry_time: i64,
    pub current_price: u64,
    pub inventory: u64,
    pub total_volume: u64,
    pub last_update_slot: u64,
}

#[derive(Accounts)]
pub struct PMAMMTrade<'info> {
    #[account(mut)]
    pub pmamm_state: Account<'info, PMAMMStatePDA>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn execute_pmamm_trade(
    ctx: Context<PMAMMTrade>,
    _outcome: u8,
    amount: u64,
    is_buy: bool,
) -> Result<()> {
    let market_state = &mut ctx.accounts.pmamm_state;
    let current_time = Clock::get()?.unix_timestamp;

    // Convert to fixed point
    let l = FixedPoint::from_raw(market_state.l_parameter);
    let t = FixedPoint::from_i64(market_state.expiry_time);
    let current_time_fp = FixedPoint::from_i64(current_time);
    let current_price = FixedPoint::from_raw(market_state.current_price);
    let inventory = FixedPoint::from_raw(market_state.inventory);
    let order_size = FixedPoint::from_raw(amount);

    // Build market model with time decay tau=0.1
    let market = PMAMMMarket {
        l,
        t,
        current_price,
        inventory,
        tau: FixedPoint::from_float(0.1),  // Time decay parameter from spec
    };

    // Solve for execution price
    let execution_price = market.solve_trade(
        if is_buy { order_size } else { order_size.neg()? },
        current_time_fp
    )?;

    // Calculate cost
    let cost = execution_price.sub(&current_price)?.mul(&order_size)?;

    // Update state
    market_state.current_price = execution_price.to_raw();
    
    if is_buy {
        market_state.inventory = inventory.add(&order_size)?.to_raw();
    } else {
        market_state.inventory = inventory.sub(&order_size)?.to_raw();
    }

    // Update volume
    market_state.total_volume = market_state.total_volume
        .checked_add(cost.abs()?.to_u64_truncate())
        .ok_or(ErrorCode::MathOverflow)?;

    market_state.last_update_slot = Clock::get()?.slot;

    Ok(())
}

// Initialize PM-AMM market
pub fn initialize_pmamm_market(
    ctx: Context<InitializePMAMM>,
    market_id: u128,
    l_parameter: u64,
    expiry_time: i64,
    initial_price: u64,
) -> Result<()> {
    let market_state = &mut ctx.accounts.pmamm_state;
    let current_time = Clock::get()?.unix_timestamp;
    
    require!(expiry_time > current_time, ErrorCode::InvalidInput);
    require!(l_parameter > 0, ErrorCode::InvalidInput);
    require!(initial_price > 0, ErrorCode::InvalidInput);
    
    market_state.market_id = market_id;
    market_state.l_parameter = l_parameter;
    market_state.expiry_time = expiry_time;
    market_state.current_price = initial_price;
    market_state.inventory = 0;
    market_state.total_volume = 0;
    market_state.last_update_slot = Clock::get()?.slot;
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(market_id: u128)]
pub struct InitializePMAMM<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 16 + 8 + 8 + 8 + 8 + 8 + 8,
        seeds = [b"pmamm", market_id.to_le_bytes().as_ref()],
        bump
    )]
    pub pmamm_state: Account<'info, PMAMMStatePDA>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

// Precomputed normal CDF lookup table for production use
pub const NORMAL_CDF_TABLE_SIZE: usize = 256;
pub const NORMAL_CDF_TABLE_RANGE: f64 = 6.0;

pub struct NormalCDFTable {
    pub values: [u64; NORMAL_CDF_TABLE_SIZE],
}

impl NormalCDFTable {
    pub fn new() -> Self {
        let mut values = [0u64; NORMAL_CDF_TABLE_SIZE];
        let step = 2.0 * NORMAL_CDF_TABLE_RANGE / (NORMAL_CDF_TABLE_SIZE as f64 - 1.0);
        
        for i in 0..NORMAL_CDF_TABLE_SIZE {
            let z = -NORMAL_CDF_TABLE_RANGE + (i as f64) * step;
            let cdf_value = 0.5 * (1.0 + erf_approximation(z / 2.0_f64.sqrt()).unwrap_or(0.0));
            values[i] = (cdf_value * PRECISION as f64) as u64;
        }
        
        Self { values }
    }
    
    pub fn lookup(&self, z: f64) -> u64 {
        if z <= -NORMAL_CDF_TABLE_RANGE {
            return 0;
        }
        if z >= NORMAL_CDF_TABLE_RANGE {
            return PRECISION as u64;
        }
        
        let normalized = (z + NORMAL_CDF_TABLE_RANGE) / (2.0 * NORMAL_CDF_TABLE_RANGE);
        let index = (normalized * (NORMAL_CDF_TABLE_SIZE as f64 - 1.0)) as usize;
        
        if index < NORMAL_CDF_TABLE_SIZE - 1 {
            // Linear interpolation
            let fraction = normalized * (NORMAL_CDF_TABLE_SIZE as f64 - 1.0) - index as f64;
            let lower = self.values[index];
            let upper = self.values[index + 1];
            lower + ((upper - lower) as f64 * fraction) as u64
        } else {
            self.values[NORMAL_CDF_TABLE_SIZE - 1]
        }
    }
}