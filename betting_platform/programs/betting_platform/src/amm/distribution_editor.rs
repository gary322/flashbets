use anchor_lang::prelude::*;
use crate::amm::l2_distribution::{
    Distribution, DistributionPoint, CurveType, FIXED_POINT_SCALE
};

#[derive(Clone, Debug)]
pub struct DistributionEditor {
    pub curve_type: CurveType,
    pub control_points: Vec<ControlPoint>,
    pub constraints: DistributionConstraints,
}

#[derive(Clone, Debug, AnchorSerialize, AnchorDeserialize)]
pub struct ControlPoint {
    pub x: u64,
    pub value: u64,
    pub locked: bool,
}

#[derive(Clone, Debug)]
pub struct DistributionConstraints {
    pub l2_norm: u64,
    pub max_bound: u64,
    pub must_integrate_to_one: bool,
}

#[error_code]
pub enum EditorError {
    #[msg("Invalid point index")]
    InvalidPointIndex,
    
    #[msg("Cannot modify locked point")]
    PointLocked,
    
    #[msg("Invalid distribution parameters")]
    InvalidParameters,
    
    #[msg("Constraint violation")]
    ConstraintViolation,
}

impl DistributionEditor {
    pub fn new(constraints: DistributionConstraints) -> Self {
        Self {
            curve_type: CurveType::Custom { points: vec![] },
            control_points: vec![],
            constraints,
        }
    }

    /// Create a normal distribution
    pub fn create_normal_distribution(
        &self,
        mean: u64,
        variance: u64,
        num_points: usize,
    ) -> Result<Distribution> {
        let mut points = Vec::with_capacity(num_points);

        // Calculate range (mean ± 4σ)
        let std_dev = self.fixed_sqrt(variance)?;
        let four_sigma = 4 * std_dev;
        
        let min_x = mean.saturating_sub(four_sigma);
        let max_x = mean.saturating_add(four_sigma);

        let step = (max_x - min_x) / (num_points as u64 - 1);

        for i in 0..num_points {
            let x = min_x + step * i as u64;

            // Calculate normal PDF
            let diff = if x > mean { x - mean } else { mean - x };
            let diff_squared = (diff as u128 * diff as u128) / FIXED_POINT_SCALE as u128;
            let exponent = (diff_squared * FIXED_POINT_SCALE as u128) / (2 * variance as u128);

            // Approximation for exp(-x) using Taylor series
            let exp_neg = self.approximate_exp_negative(exponent as u64)?;

            // Normalization constant: 1/sqrt(2*pi*variance)
            let two_pi = 2 * 3141593 * variance / 1000000; // Approximate 2*pi
            let sqrt_denominator = self.fixed_sqrt(two_pi)?;
            let coefficient = FIXED_POINT_SCALE * FIXED_POINT_SCALE / sqrt_denominator;

            let f_x = (coefficient as u128 * exp_neg as u128 / FIXED_POINT_SCALE as u128) as u64;

            points.push(DistributionPoint {
                x,
                f_x,
                weight: FIXED_POINT_SCALE, // Equal weight
            });
        }

        // Calculate L2 norm
        let l2_norm = self.calculate_l2_norm(&points)?;

        Ok(Distribution {
            curve_type: CurveType::Normal { mean, variance },
            points,
            l2_norm,
        })
    }

    /// Create a uniform distribution
    pub fn create_uniform_distribution(
        &self,
        min: u64,
        max: u64,
        num_points: usize,
    ) -> Result<Distribution> {
        if min >= max {
            return Err(EditorError::InvalidParameters.into());
        }

        let mut points = Vec::with_capacity(num_points);
        let height = FIXED_POINT_SCALE / (max - min); // Normalized height
        let step = (max - min) / (num_points as u64 - 1);

        for i in 0..num_points {
            let x = min + step * i as u64;
            
            points.push(DistributionPoint {
                x,
                f_x: height,
                weight: FIXED_POINT_SCALE,
            });
        }

        let l2_norm = self.calculate_l2_norm(&points)?;

        Ok(Distribution {
            curve_type: CurveType::Uniform { min, max },
            points,
            l2_norm,
        })
    }

    /// Drag a curve point to a new value
    pub fn drag_curve_point(
        &mut self,
        index: usize,
        new_value: u64,
    ) -> Result<Distribution> {
        if index >= self.control_points.len() {
            return Err(EditorError::InvalidPointIndex.into());
        }

        if self.control_points[index].locked {
            return Err(EditorError::PointLocked.into());
        }

        // Update control point
        self.control_points[index].value = new_value;

        // Rebuild distribution maintaining constraints
        let mut dist = self.rebuild_from_control_points()?;

        // Ensure L2 norm constraint
        self.enforce_l2_constraint(&mut dist)?;

        // Ensure max bound constraint
        self.enforce_max_bound_constraint(&mut dist)?;

        Ok(dist)
    }

    /// Rebuild distribution from control points
    fn rebuild_from_control_points(&self) -> Result<Distribution> {
        let mut points = Vec::new();

        // Interpolate between control points
        for i in 0..self.control_points.len() - 1 {
            let start = &self.control_points[i];
            let end = &self.control_points[i + 1];

            // Linear interpolation
            let steps = 10; // Points between control points
            for j in 0..steps {
                let t = j as u64 * FIXED_POINT_SCALE / steps as u64;
                let x = start.x + (end.x - start.x) * t / FIXED_POINT_SCALE;
                let f_x = start.value + (end.value - start.value) * t / FIXED_POINT_SCALE;

                points.push(DistributionPoint {
                    x,
                    f_x,
                    weight: FIXED_POINT_SCALE,
                });
            }
        }

        // Add final point
        if let Some(last) = self.control_points.last() {
            points.push(DistributionPoint {
                x: last.x,
                f_x: last.value,
                weight: FIXED_POINT_SCALE,
            });
        }

        let l2_norm = self.calculate_l2_norm(&points)?;

        Ok(Distribution {
            curve_type: CurveType::Custom { 
                points: self.control_points.iter()
                    .map(|cp| (cp.x, cp.value))
                    .collect()
            },
            points,
            l2_norm,
        })
    }

    /// Enforce L2 norm constraint
    fn enforce_l2_constraint(&self, dist: &mut Distribution) -> Result<()> {
        let current_norm = dist.l2_norm;
        let target_norm = self.constraints.l2_norm;

        if current_norm == 0 {
            return Err(EditorError::InvalidParameters.into());
        }

        let scale_factor = target_norm * FIXED_POINT_SCALE / current_norm;

        for point in &mut dist.points {
            point.f_x = (point.f_x as u128 * scale_factor as u128 / FIXED_POINT_SCALE as u128) as u64;
        }

        dist.l2_norm = target_norm;
        Ok(())
    }

    /// Enforce max bound constraint
    fn enforce_max_bound_constraint(&self, dist: &mut Distribution) -> Result<()> {
        let mut modified = false;

        for point in &mut dist.points {
            if point.f_x > self.constraints.max_bound {
                point.f_x = self.constraints.max_bound;
                modified = true;
            }
        }

        if modified {
            // Recalculate L2 norm
            dist.l2_norm = self.calculate_l2_norm(&dist.points)?;
        }

        Ok(())
    }

    /// Calculate L2 norm of points
    fn calculate_l2_norm(&self, points: &[DistributionPoint]) -> Result<u64> {
        let mut sum_squared = 0u128;

        for point in points {
            let f_squared = (point.f_x as u128 * point.f_x as u128) / FIXED_POINT_SCALE as u128;
            sum_squared = sum_squared.saturating_add(f_squared);
        }

        Ok(self.fixed_sqrt(sum_squared as u64)?)
    }

    /// Fixed-point square root
    fn fixed_sqrt(&self, x: u64) -> Result<u64> {
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

    /// Approximate e^(-x) for small x using Taylor series
    fn approximate_exp_negative(&self, x: u64) -> Result<u64> {
        // For e^(-x), use Taylor series: 1 - x + x²/2 - x³/6 + ...
        // Work in higher precision to avoid overflow
        
        if x > 10 * FIXED_POINT_SCALE {
            return Ok(0); // Very small value
        }

        let mut result = FIXED_POINT_SCALE as u128; // 1.0
        let mut term = FIXED_POINT_SCALE as u128;
        let x_128 = x as u128;

        // First few terms of Taylor series
        for i in 1..6 {
            term = (term * x_128) / (i as u128 * FIXED_POINT_SCALE as u128);
            if i % 2 == 1 {
                result = result.saturating_sub(term);
            } else {
                result = result.saturating_add(term);
            }
        }

        Ok((result.min(FIXED_POINT_SCALE as u128)) as u64)
    }

    fn abs_diff(&self, a: u64, b: u64) -> u64 {
        if a > b { a - b } else { b - a }
    }

    /// Add a control point
    pub fn add_control_point(&mut self, x: u64, value: u64, locked: bool) {
        self.control_points.push(ControlPoint { x, value, locked });
        self.control_points.sort_by_key(|p| p.x);
    }

    /// Remove a control point
    pub fn remove_control_point(&mut self, index: usize) -> Result<()> {
        if index >= self.control_points.len() {
            return Err(EditorError::InvalidPointIndex.into());
        }

        if self.control_points[index].locked {
            return Err(EditorError::PointLocked.into());
        }

        self.control_points.remove(index);
        Ok(())
    }
}