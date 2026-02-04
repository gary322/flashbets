use super::core::*;
use fixed::types::{U64F64, I64F64};
use anchor_lang::prelude::*;

#[derive(Debug, Clone)]
pub enum SolverError {
    MarketExpired,
    ConvergenceFailed,
    InvalidInput,
    MathOverflow,
    DivisionByZero,
}

impl From<SolverError> for ProgramError {
    fn from(e: SolverError) -> Self {
        match e {
            SolverError::MarketExpired => ProgramError::Custom(100),
            SolverError::ConvergenceFailed => ProgramError::Custom(101),
            SolverError::InvalidInput => ProgramError::Custom(102),
            SolverError::MathOverflow => ProgramError::Custom(103),
            SolverError::DivisionByZero => ProgramError::Custom(104),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PMPriceResult {
    pub new_price: U64F64,
    pub old_price: U64F64,
    pub price_impact: U64F64,
    pub lvr_cost: U64F64,
    pub iterations: u8,
    pub slippage: U64F64,
}

pub struct NewtonRaphsonSolver {
    pub max_iterations: u8,
    pub convergence_threshold: U64F64,
    pub derivative_epsilon: U64F64,
}

impl NewtonRaphsonSolver {
    pub fn new() -> Self {
        Self {
            max_iterations: MAX_NEWTON_ITERATIONS,
            convergence_threshold: CONVERGENCE_THRESHOLD,
            derivative_epsilon: U64F64::from_num(0.0000001),
        }
    }

    pub fn solve_pm_amm_price(
        &self,
        state: &PMAMMState,
        outcome_index: u8,
        order_size: I64F64, // Can be negative for sells
    ) -> std::result::Result<PMPriceResult, SolverError> {
        let current_price = state.prices[outcome_index as usize];
        let time_remaining = state.initial_time.saturating_sub(state.current_time);

        if time_remaining == 0 {
            return Err(SolverError::MarketExpired);
        }

        // Calculate time-decay factor L√(T-t)
        let sqrt_time = self.fixed_sqrt(U64F64::from_num(time_remaining))?;
        let l_sqrt_t = state.liquidity_parameter.saturating_mul(sqrt_time);

        // Initial guess for Newton-Raphson
        let mut y = current_price;
        let x = current_price;

        let mut iterations = 0;
        let mut converged = false;

        while iterations < self.max_iterations && !converged {
            // Calculate f(y) and f'(y)
            let (f_y, df_dy) = self.calculate_derivatives(
                x,
                y,
                l_sqrt_t,
                &state.phi_lookup_table,
                &state.pdf_lookup_table,
            )?;

            // Check convergence
            if f_y < self.convergence_threshold {
                converged = true;
                break;
            }

            // Newton-Raphson update: y_{n+1} = y_n - f(y_n)/f'(y_n)
            let delta = f_y / df_dy;
            y = y.saturating_sub(delta);

            // Ensure price stays in valid range [0.001, 0.999]
            y = y.max(U64F64::from_num(0.001)).min(U64F64::from_num(0.999));

            iterations += 1;
        }

        if !converged {
            return Err(SolverError::ConvergenceFailed);
        }

        // Calculate LVR for this trade
        let lvr = self.calculate_uniform_lvr(
            state,
            outcome_index,
            y,
            time_remaining,
        )?;

        Ok(PMPriceResult {
            new_price: y,
            old_price: x,
            price_impact: if y >= x { y - x } else { x - y } / x,
            lvr_cost: lvr,
            iterations,
            slippage: self.calculate_slippage(x, y, order_size)?,
        })
    }

    fn calculate_derivatives(
        &self,
        x: U64F64,
        y: U64F64,
        l_sqrt_t: U64F64,
        phi_table: &[U64F64; PHI_TABLE_SIZE],
        pdf_table: &[U64F64; PHI_TABLE_SIZE],
    ) -> std::result::Result<(U64F64, U64F64), SolverError> {
        // z = (y - x) / (L√(T-t))
        let y_minus_x = y.saturating_sub(x);
        let z = if l_sqrt_t > U64F64::from_num(0) {
            y_minus_x / l_sqrt_t
        } else {
            return Err(SolverError::DivisionByZero);
        };

        // Look up Φ(z) and φ(z) from precomputed tables
        let phi_z = self.lookup_phi(z, phi_table)?;
        let pdf_z = self.lookup_pdf(z, pdf_table)?;

        // f(y) = (y - x) * Φ(z) + L√(T-t) * φ(z) - y
        let f_y = y_minus_x.saturating_mul(phi_z)
            .saturating_add(l_sqrt_t.saturating_mul(pdf_z))
            .saturating_sub(y);

        // f'(y) = Φ(z) + (y-x)/(L√(T-t)) * φ(z) + L√(T-t) * φ'(z) * 1/(L√(T-t)) - 1
        // Simplifying: f'(y) = Φ(z) + z * φ(z) + φ'(z) - 1
        // Note: φ'(z) = -z * φ(z) for standard normal
        let df_dy = phi_z
            .saturating_add(z.saturating_mul(pdf_z))
            .saturating_sub(z.saturating_mul(pdf_z)) // φ'(z) term
            .saturating_sub(U64F64::from_num(1));

        Ok((f_y, df_dy))
    }

    fn lookup_phi(&self, z: U64F64, table: &[U64F64; PHI_TABLE_SIZE]) -> std::result::Result<U64F64, SolverError> {
        // Map z from [-4, 4] to table index [0, 255]
        let z_f64: f64 = z.to_num();
        let z_clamped = z_f64.max(-4.0).min(4.0);
        let normalized = (z_clamped + 4.0) / 8.0;
        let index = (normalized * (PHI_TABLE_SIZE - 1) as f64) as usize;

        // Linear interpolation for accuracy
        if index < PHI_TABLE_SIZE - 1 {
            let frac = U64F64::from_num(normalized * (PHI_TABLE_SIZE - 1) as f64 - index as f64);
            let val1 = table[index];
            let val2 = table[index + 1];
            Ok(val1 + frac * (val2 - val1))
        } else {
            Ok(table[PHI_TABLE_SIZE - 1])
        }
    }

    fn lookup_pdf(&self, z: U64F64, table: &[U64F64; PHI_TABLE_SIZE]) -> std::result::Result<U64F64, SolverError> {
        // Similar to lookup_phi
        self.lookup_phi(z, table)
    }

    fn calculate_uniform_lvr(
        &self,
        state: &PMAMMState,
        outcome_index: u8,
        new_price: U64F64,
        time_remaining: u64,
    ) -> std::result::Result<U64F64, SolverError> {
        // LVR_t = β * V_t / (T-t) for uniform LVR
        let v_t = self.calculate_portfolio_value(state, outcome_index)?;
        let lvr = state.lvr_beta
            .saturating_mul(v_t)
            .saturating_div(U64F64::from_num(time_remaining));
        Ok(lvr)
    }

    fn calculate_portfolio_value(
        &self,
        state: &PMAMMState,
        outcome_index: u8,
    ) -> std::result::Result<U64F64, SolverError> {
        // Portfolio value = price * volume for this outcome
        let price = state.prices[outcome_index as usize];
        let volume = state.volumes[outcome_index as usize];
        Ok(price.saturating_mul(volume))
    }

    fn calculate_slippage(
        &self,
        old_price: U64F64,
        new_price: U64F64,
        order_size: I64F64,
    ) -> std::result::Result<U64F64, SolverError> {
        // Slippage = |new_price - old_price| / old_price * |order_size|
        let price_diff = if new_price >= old_price {
            new_price - old_price
        } else {
            old_price - new_price
        };
        let order_size_abs = U64F64::from_num(order_size.abs());
        Ok(price_diff / old_price * order_size_abs)
    }

    fn fixed_sqrt(&self, x: U64F64) -> std::result::Result<U64F64, SolverError> {
        // Newton-Raphson for square root
        if x == U64F64::from_num(0) {
            return Ok(U64F64::from_num(0));
        }

        let mut guess = x / U64F64::from_num(2);
        for _ in 0..10 {
            let new_guess = (guess + x / guess) / U64F64::from_num(2);
            let diff = if new_guess >= guess {
                new_guess - guess
            } else {
                guess - new_guess
            };
            if diff < self.derivative_epsilon {
                return Ok(new_guess);
            }
            guess = new_guess;
        }
        Ok(guess)
    }
}