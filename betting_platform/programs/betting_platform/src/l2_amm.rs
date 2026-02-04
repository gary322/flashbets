use anchor_lang::prelude::*;
use crate::fixed_math::*;
use crate::errors::ErrorCode;
use crate::pm_amm::normal_pdf;

#[derive(Debug, Clone)]
pub struct L2DistributionAMM {
    pub k: FixedPoint,  // L2 norm constraint
    pub b: FixedPoint,  // Max bound
    pub distribution_type: DistributionType,
    pub parameters: DistributionParams,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum DistributionType {
    Normal { mean: u64, variance: u64 },
    Uniform { min: u64, max: u64 },
    Custom { points: Vec<(u64, u64)> },  // (x, f(x)) pairs
}

#[derive(Debug, Clone)]
pub struct DistributionParams {
    pub discretization_points: usize,
    pub range_min: FixedPoint,
    pub range_max: FixedPoint,
}

impl L2DistributionAMM {
    /// Calculate distribution using L2 norm constraint
    pub fn calculate_distribution(&self) -> Result<Vec<(FixedPoint, FixedPoint)>> {
        let n = self.parameters.discretization_points;
        let mut distribution = Vec::with_capacity(n);

        // Generate x points
        let range = self.parameters.range_max.sub(&self.parameters.range_min)?;
        let dx = range.div(&FixedPoint::from_u64(n as u64 - 1))?;

        match &self.distribution_type {
            DistributionType::Normal { mean, variance } => {
                let mean_fp = FixedPoint::from_raw(*mean);
                let var_fp = FixedPoint::from_raw(*variance);
                let std_dev = var_fp.sqrt()?;

                for i in 0..n {
                    let x = self.parameters.range_min
                        .add(&dx.mul(&FixedPoint::from_u64(i as u64))?)?;

                    // Normal PDF: f(x) = 1/(σ√(2π)) * exp(-0.5 * ((x-μ)/σ)²)
                    let z = (x.sub(&mean_fp)?).div(&std_dev)?;
                    let f_x = normal_pdf(z)?.div(&std_dev)?;

                    // Apply L2 constraint: min(λ * p(x), b)
                    let lambda = self.calculate_lambda(&distribution)?;
                    let constrained_f = lambda.mul(&f_x)?.min(self.b);

                    distribution.push((x, constrained_f));
                }
            },
            DistributionType::Custom { points } => {
                // Interpolate custom points
                for (x_raw, y_raw) in points {
                    let x = FixedPoint::from_raw(*x_raw);
                    let y = FixedPoint::from_raw(*y_raw).min(self.b);
                    distribution.push((x, y));
                }
            },
            _ => return Err(ErrorCode::UnsupportedDistribution.into()),
        }

        // Verify L2 norm constraint
        self.verify_l2_constraint(&distribution)?;

        Ok(distribution)
    }

    /// Calculate lambda to satisfy ||f||_2 = k
    fn calculate_lambda(&self, distribution: &[(FixedPoint, FixedPoint)]) -> Result<FixedPoint> {
        let current_norm = self.calculate_l2_norm(distribution)?;

        if current_norm > FixedPoint::zero() {
            self.k.div(&current_norm)
        } else {
            Ok(FixedPoint::from_u64(1))
        }
    }

    /// Calculate L2 norm using Simpson's rule
    pub fn calculate_l2_norm(&self, distribution: &[(FixedPoint, FixedPoint)]) -> Result<FixedPoint> {
        let n = distribution.len();
        if n < 3 {
            return Err(ErrorCode::InsufficientPoints.into());
        }

        let mut integral = FixedPoint::zero();

        // Simpson's rule for numerical integration
        for i in (0..n-2).step_by(2) {
            let (x0, f0) = &distribution[i];
            let (x1, f1) = &distribution[i + 1];
            let (x2, f2) = &distribution[i + 2];

            let h = x1.sub(&x0)?;

            // Simpson's rule: ∫f²dx ≈ h/3 * (f0² + 4f1² + f2²)
            let f0_squared = f0.mul(&f0)?;
            let f1_squared = f1.mul(&f1)?;
            let f2_squared = f2.mul(&f2)?;

            let weighted_sum = f0_squared
                .add(&f1_squared.mul(&FixedPoint::from_u64(4))?)?
                .add(&f2_squared)?;

            let segment_integral = h.mul(&weighted_sum)?
                .div(&FixedPoint::from_u64(3))?;

            integral = integral.add(&segment_integral)?;
        }

        integral.sqrt()
    }
    
    fn verify_l2_constraint(&self, distribution: &[(FixedPoint, FixedPoint)]) -> Result<()> {
        let norm = self.calculate_l2_norm(distribution)?;
        let epsilon = FixedPoint::from_float(0.001);
        
        let diff = if norm > self.k {
            norm.sub(&self.k)?
        } else {
            self.k.sub(&norm)?
        };
        
        require!(
            diff < epsilon,
            ErrorCode::InvalidInput
        );
        
        Ok(())
    }
}

#[account]
pub struct L2AMMStatePDA {
    pub market_id: u128,
    pub k_parameter: u64,  // L2 norm constraint
    pub b_bound: u64,      // Max f(x) bound
    pub distribution_type: DistributionType,
    pub discretization_points: u16,
    pub range_min: u64,
    pub range_max: u64,
    pub current_distribution: Vec<(u64, u64)>,  // Compressed distribution
    pub total_volume: u64,
    pub last_update_slot: u64,
}

#[derive(Accounts)]
pub struct L2AMMTrade<'info> {
    #[account(mut)]
    pub l2_state: Account<'info, L2AMMStatePDA>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn execute_l2_trade(
    ctx: Context<L2AMMTrade>,
    outcome: u8,
    amount: u64,
    _is_buy: bool,
) -> Result<()> {
    let market_state = &mut ctx.accounts.l2_state;

    // Convert to fixed point
    let k = FixedPoint::from_raw(market_state.k_parameter);
    let b = FixedPoint::from_raw(market_state.b_bound);
    let range_min = FixedPoint::from_raw(market_state.range_min);
    let range_max = FixedPoint::from_raw(market_state.range_max);

    // Build distribution model
    let amm = L2DistributionAMM {
        k,
        b,
        distribution_type: market_state.distribution_type.clone(),
        parameters: DistributionParams {
            discretization_points: market_state.discretization_points as usize,
            range_min,
            range_max,
        },
    };

    // Calculate current distribution
    let distribution = amm.calculate_distribution()?;

    // Find the price at the outcome index
    let outcome_idx = outcome as usize;
    require!(outcome_idx < distribution.len(), ErrorCode::InvalidOutcome);
    
    let (x_value, current_price) = distribution[outcome_idx];

    // Calculate cost based on the distribution price
    let shares = FixedPoint::from_raw(amount);
    let cost = current_price.mul(&shares)?;

    // Update volume
    market_state.total_volume = market_state.total_volume
        .checked_add(cost.to_u64_truncate())
        .ok_or(ErrorCode::MathOverflow)?;

    market_state.last_update_slot = Clock::get()?.slot;

    // Update distribution state
    market_state.current_distribution = distribution
        .iter()
        .map(|(x, f)| (x.to_raw(), f.to_raw()))
        .collect();

    Ok(())
}

// Initialize L2 AMM market
pub fn initialize_l2_amm_market(
    ctx: Context<InitializeL2AMM>,
    market_id: u128,
    k_parameter: u64,
    b_bound: u64,
    distribution_type: DistributionType,
    discretization_points: u16,
    range_min: u64,
    range_max: u64,
) -> Result<()> {
    let market_state = &mut ctx.accounts.l2_state;
    
    require!(k_parameter > 0, ErrorCode::InvalidInput);
    require!(b_bound > 0, ErrorCode::InvalidInput);
    require!(discretization_points >= 16 && discretization_points <= 256, ErrorCode::InvalidInput);
    require!(range_max > range_min, ErrorCode::InvalidInput);
    
    market_state.market_id = market_id;
    market_state.k_parameter = k_parameter;
    market_state.b_bound = b_bound;
    market_state.distribution_type = distribution_type;
    market_state.discretization_points = discretization_points;
    market_state.range_min = range_min;
    market_state.range_max = range_max;
    market_state.current_distribution = vec![];
    market_state.total_volume = 0;
    market_state.last_update_slot = Clock::get()?.slot;
    
    // Calculate initial distribution
    let k_fp = FixedPoint::from_raw(k_parameter);
    let b_fp = FixedPoint::from_raw(b_bound);
    let range_min_fp = FixedPoint::from_raw(range_min);
    let range_max_fp = FixedPoint::from_raw(range_max);
    
    let amm = L2DistributionAMM {
        k: k_fp,
        b: b_fp,
        distribution_type: market_state.distribution_type.clone(),
        parameters: DistributionParams {
            discretization_points: discretization_points as usize,
            range_min: range_min_fp,
            range_max: range_max_fp,
        },
    };
    
    let distribution = amm.calculate_distribution()?;
    market_state.current_distribution = distribution
        .iter()
        .map(|(x, f)| (x.to_raw(), f.to_raw()))
        .collect();
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(market_id: u128, discretization_points: u16)]
pub struct InitializeL2AMM<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 16 + 8 + 8 + 200 + 2 + 8 + 8 + 
                4 + (16 * discretization_points as usize) + 8 + 8,
        seeds = [b"l2amm", market_id.to_le_bytes().as_ref()],
        bump
    )]
    pub l2_state: Account<'info, L2AMMStatePDA>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}