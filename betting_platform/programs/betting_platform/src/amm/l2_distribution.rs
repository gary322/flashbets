use anchor_lang::prelude::*;

pub const SIMPSON_POINTS: usize = 10;
pub const L2_NORM_K: u64 = 100_000; // Default k value
pub const MAX_F_BOUND: u64 = 1_000; // Default b value
pub const FIXED_POINT_SCALE: u64 = 1_000_000; // 6 decimal places

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct L2DistributionAMM {
    pub norm_constraint: u64, // Fixed point representation
    pub max_bound: u64,
    pub discretization_points: Vec<DistributionPoint>,
    #[cfg(feature = "no-anchor")]
    pub integration_cache: HashMap<u64, IntegrationResult>,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct DistributionPoint {
    pub x: u64, // Fixed point
    pub f_x: u64, // Fixed point
    pub weight: u64, // Fixed point
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct Distribution {
    pub curve_type: CurveType,
    pub points: Vec<DistributionPoint>,
    pub l2_norm: u64, // Fixed point
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub enum CurveType {
    Normal { mean: u64, variance: u64 },
    Uniform { min: u64, max: u64 },
    Custom { points: Vec<(u64, u64)> },
    Bimodal {
        mean1: u64,
        mean2: u64,
        variance1: u64,
        variance2: u64,
        weight1: u64,
    },
}

#[derive(Clone, Debug)]
pub struct IntegrationResult {
    pub value: u64,
    pub computed_slot: u64,
}

#[derive(Clone, Debug)]
pub struct DistributionPrice {
    pub price: u64,
    pub probability: u64,
    pub slippage: u64,
    pub max_loss: u64,
}

#[error_code]
pub enum L2Error {
    #[msg("L2 norm constraint violated")]
    NormConstraintViolated,
    
    #[msg("Max bound constraint violated")]
    MaxBoundViolated,
    
    #[msg("Invalid distribution parameters")]
    InvalidDistribution,
    
    #[msg("Integration failed")]
    IntegrationFailed,
    
    #[msg("Insufficient liquidity")]
    InsufficientLiquidity,
    
    #[msg("Fixed point overflow")]
    FixedPointOverflow,
}

impl L2DistributionAMM {
    pub fn new(norm_constraint: u64, max_bound: u64) -> Self {
        Self {
            norm_constraint,
            max_bound,
            discretization_points: Vec::new(),
            #[cfg(feature = "no-anchor")]
            integration_cache: HashMap::new(),
        }
    }

    /// Price a distribution bet
    pub fn price_distribution_bet(
        &mut self,
        distribution: &Distribution,
        bet_amount: u64,
        outcome_range: (u64, u64),
    ) -> Result<DistributionPrice> {
        // Validate L2 norm constraint
        let norm = self.calculate_l2_norm(distribution)?;
        if norm > self.norm_constraint {
            return Err(L2Error::NormConstraintViolated.into());
        }

        // Calculate probability mass in range using Simpson's rule
        let prob_mass = self.integrate_simpson(
            distribution,
            outcome_range.0,
            outcome_range.1,
        )?;

        // Apply max bound constraint
        let bounded_distribution = self.apply_max_bound(distribution)?;

        // Calculate AMM price
        let price = self.calculate_amm_price(
            &bounded_distribution,
            bet_amount,
            prob_mass,
        )?;

        let max_loss = self.calculate_max_loss(&bounded_distribution);
        
        Ok(DistributionPrice {
            price,
            probability: prob_mass,
            slippage: self.calculate_slippage(bet_amount, prob_mass),
            max_loss,
        })
    }

    /// Calculate L2 norm of distribution
    fn calculate_l2_norm(&self, dist: &Distribution) -> Result<u64> {
        let mut sum_squared = 0u128;

        for point in &dist.points {
            let f_squared = (point.f_x as u128) * (point.f_x as u128) / FIXED_POINT_SCALE as u128;
            sum_squared = sum_squared.saturating_add(f_squared);
        }

        // sqrt approximation for fixed point
        Ok(self.fixed_sqrt(sum_squared as u64)?)
    }

    /// Simpson's rule integration
    pub fn integrate_simpson(
        &mut self,
        dist: &Distribution,
        a: u64,
        b: u64,
    ) -> Result<u64> {
        #[cfg(feature = "no-anchor")]
        {
            // Check cache
            let cache_key = self.compute_cache_key(dist, a, b);
            if let Some(cached) = self.integration_cache.get(&cache_key) {
                return Ok(cached.value);
            }
        }

        let h = (b - a) / (SIMPSON_POINTS as u64 - 1);
        let mut integral = 0u128;

        for i in 0..SIMPSON_POINTS {
            let x = a + h * i as u64;
            let y = self.evaluate_distribution(dist, x)?;

            let weight = if i == 0 || i == SIMPSON_POINTS - 1 {
                FIXED_POINT_SCALE
            } else if i % 2 == 0 {
                2 * FIXED_POINT_SCALE
            } else {
                4 * FIXED_POINT_SCALE
            };

            integral = integral.saturating_add((weight as u128 * y as u128) / FIXED_POINT_SCALE as u128);
        }

        let result = ((integral * h as u128) / (3 * FIXED_POINT_SCALE) as u128) as u64;

        #[cfg(feature = "no-anchor")]
        {
            // Cache result
            self.integration_cache.insert(cache_key, IntegrationResult {
                value: result,
                computed_slot: Clock::get()?.slot,
            });
        }

        Ok(result)
    }

    /// Apply max bound constraint
    pub fn apply_max_bound(&self, dist: &Distribution) -> Result<Distribution> {
        let mut bounded = dist.clone();

        for point in &mut bounded.points {
            if point.f_x > self.max_bound {
                point.f_x = self.max_bound;
            }
        }

        // Adjust to maintain L2 norm
        self.adjust_for_norm_constraint(&mut bounded)?;

        Ok(bounded)
    }

    /// Adjust distribution to satisfy norm constraint
    fn adjust_for_norm_constraint(
        &self,
        dist: &mut Distribution,
    ) -> Result<()> {
        // Lagrange multiplier optimization
        let mut lambda = FIXED_POINT_SCALE;
        let tolerance = 100; // 0.0001 in fixed point

        for _ in 0..10 { // Max iterations
            let current_norm = self.calculate_l2_norm(dist)?;

            if self.abs_diff(current_norm, self.norm_constraint) < tolerance {
                break;
            }

            // Adjust lambda
            lambda = (lambda * self.norm_constraint) / current_norm;

            // Scale distribution
            for point in &mut dist.points {
                point.f_x = (point.f_x as u128 * lambda as u128 / FIXED_POINT_SCALE as u128) as u64;

                // Ensure max bound
                if point.f_x > self.max_bound {
                    point.f_x = self.max_bound;
                }
            }
        }

        Ok(())
    }

    /// Evaluate distribution at a point
    fn evaluate_distribution(&self, dist: &Distribution, x: u64) -> Result<u64> {
        // Find surrounding points and interpolate
        let mut left_idx = 0;
        let mut right_idx = dist.points.len() - 1;

        for (i, point) in dist.points.iter().enumerate() {
            if point.x <= x {
                left_idx = i;
            }
            if point.x >= x && i < right_idx {
                right_idx = i;
                break;
            }
        }

        if left_idx == right_idx {
            return Ok(dist.points[left_idx].f_x);
        }

        // Linear interpolation
        let left = &dist.points[left_idx];
        let right = &dist.points[right_idx];

        let dx = right.x - left.x;
        let dy = if right.f_x > left.f_x {
            right.f_x - left.f_x
        } else {
            left.f_x - right.f_x
        };

        let ratio = ((x - left.x) * FIXED_POINT_SCALE) / dx;
        let interpolated = if right.f_x > left.f_x {
            left.f_x + (dy * ratio / FIXED_POINT_SCALE)
        } else {
            left.f_x - (dy * ratio / FIXED_POINT_SCALE)
        };

        Ok(interpolated)
    }

    /// Calculate AMM price based on distribution
    fn calculate_amm_price(
        &self,
        dist: &Distribution,
        bet_amount: u64,
        prob_mass: u64,
    ) -> Result<u64> {
        // Simplified AMM pricing
        // Price = prob_mass * (1 + slippage_factor * bet_amount / liquidity)
        let base_price = prob_mass;
        let slippage_factor = 100; // 0.0001 in fixed point
        let liquidity = 10_000 * FIXED_POINT_SCALE; // 10k units

        let slippage = (slippage_factor * bet_amount) / liquidity;
        let price = base_price + (base_price * slippage / FIXED_POINT_SCALE);

        Ok(price.min(FIXED_POINT_SCALE)) // Cap at 1.0
    }

    /// Calculate slippage
    fn calculate_slippage(&self, bet_amount: u64, prob_mass: u64) -> u64 {
        // Slippage increases with bet size and decreases with liquidity
        let base_slippage = 100; // 0.01% in basis points
        let size_factor = (bet_amount * base_slippage) / (100 * FIXED_POINT_SCALE);
        
        size_factor * (FIXED_POINT_SCALE - prob_mass) / FIXED_POINT_SCALE
    }

    /// Calculate maximum loss
    fn calculate_max_loss(&mut self, dist: &Distribution) -> u64 {
        // Max loss is the integral of the entire distribution
        self.integrate_simpson(dist, 0, FIXED_POINT_SCALE).unwrap_or(FIXED_POINT_SCALE)
    }

    /// Fixed point square root approximation
    pub fn fixed_sqrt(&self, x: u64) -> Result<u64> {
        if x == 0 {
            return Ok(0);
        }

        // Newton-Raphson method
        let mut guess = x / 2;
        let mut prev_guess = x;

        while self.abs_diff(guess, prev_guess) > 1 {
            prev_guess = guess;
            guess = (guess + x / guess) / 2;
        }

        Ok(guess * 1000) // Adjust for fixed point
    }

    /// Absolute difference helper
    fn abs_diff(&self, a: u64, b: u64) -> u64 {
        if a > b { a - b } else { b - a }
    }

    #[cfg(feature = "no-anchor")]
    fn compute_cache_key(&self, dist: &Distribution, a: u64, b: u64) -> u64 {
        // Simple hash for caching
        let mut key = a ^ b;
        key ^= dist.points.len() as u64;
        if let Some(first) = dist.points.first() {
            key ^= first.x ^ first.f_x;
        }
        key
    }
}