use fixed::types::{U64F64, I64F64};
use anchor_lang::prelude::*;
use std::cmp::min;

pub const MAX_NEWTON_ITERATIONS: u8 = 5;
pub const CONVERGENCE_THRESHOLD: U64F64 = U64F64::from_bits(1844674407); // ~1e-8
pub const FIXED_POINT_SCALE: u64 = 1_000_000_000; // 9 decimals
pub const PHI_TABLE_SIZE: usize = 256;
pub const SQRT_2PI: U64F64 = U64F64::from_bits(2_506_628_274_631_000_000); // √(2π)

#[derive(Clone, Debug)]
pub struct PMAMMState {
    pub liquidity_parameter: U64F64, // L in the formula
    pub initial_time: u64,           // T (total time)
    pub current_time: u64,           // t (elapsed time)
    pub outcome_count: u8,           // N outcomes
    pub prices: Vec<U64F64>,         // Current prices
    pub volumes: Vec<U64F64>,        // Traded volumes
    pub lvr_beta: U64F64,           // β for uniform LVR
    pub phi_lookup_table: [U64F64; PHI_TABLE_SIZE], // Precomputed Φ values
    pub pdf_lookup_table: [U64F64; PHI_TABLE_SIZE], // Precomputed φ values
}

impl PMAMMState {
    pub fn new(
        liquidity_parameter: U64F64,
        duration_slots: u64,
        outcome_count: u8,
        initial_slot: u64,
    ) -> std::result::Result<Self, ProgramError> {
        if outcome_count < 2 || outcome_count > 64 {
            return Err(ProgramError::InvalidArgument);
        }

        let initial_price = U64F64::from_num(1) / U64F64::from_num(outcome_count);
        let prices = vec![initial_price; outcome_count as usize];
        let volumes = vec![U64F64::from_num(0); outcome_count as usize];

        // Calculate β for uniform LVR
        let lvr_beta = Self::calculate_uniform_lvr_beta(liquidity_parameter)?;

        // Initialize lookup tables
        let phi_table = Self::initialize_phi_table()?;
        let pdf_table = Self::initialize_pdf_table()?;

        Ok(Self {
            liquidity_parameter,
            initial_time: initial_slot + duration_slots,
            current_time: initial_slot,
            outcome_count,
            prices,
            volumes,
            lvr_beta,
            phi_lookup_table: phi_table,
            pdf_lookup_table: pdf_table,
        })
    }

    fn calculate_uniform_lvr_beta(L: U64F64) -> std::result::Result<U64F64, ProgramError> {
        // β = L² / (2π) for uniform LVR
        let l_squared = L.saturating_mul(L);
        let two_pi = U64F64::from_num(2) * U64F64::from_num(std::f64::consts::PI);
        Ok(l_squared / two_pi)
    }

    fn initialize_phi_table() -> std::result::Result<[U64F64; PHI_TABLE_SIZE], ProgramError> {
        let mut table = [U64F64::from_num(0); PHI_TABLE_SIZE];

        // Precompute Φ(x) for x in [-4, 4] with 256 points
        for i in 0..PHI_TABLE_SIZE {
            let x = -4.0 + (8.0 * i as f64) / (PHI_TABLE_SIZE - 1) as f64;
            let phi = Self::compute_normal_cdf(x);
            table[i] = U64F64::from_num(phi);
        }

        Ok(table)
    }

    fn initialize_pdf_table() -> std::result::Result<[U64F64; PHI_TABLE_SIZE], ProgramError> {
        let mut table = [U64F64::from_num(0); PHI_TABLE_SIZE];

        // Precompute φ(x) for x in [-4, 4] with 256 points
        for i in 0..PHI_TABLE_SIZE {
            let x = -4.0 + (8.0 * i as f64) / (PHI_TABLE_SIZE - 1) as f64;
            let pdf = Self::compute_normal_pdf(x);
            table[i] = U64F64::from_num(pdf);
        }

        Ok(table)
    }

    fn compute_normal_cdf(x: f64) -> f64 {
        // Approximation using error function
        0.5 * (1.0 + Self::erf(x / std::f64::consts::SQRT_2))
    }

    fn compute_normal_pdf(x: f64) -> f64 {
        (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
    }

    fn erf(x: f64) -> f64 {
        // Abramowitz and Stegun approximation
        let a1 =  0.254829592;
        let a2 = -0.284496736;
        let a3 =  1.421413741;
        let a4 = -1.453152027;
        let a5 =  1.061405429;
        let p  =  0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

        sign * y
    }
}