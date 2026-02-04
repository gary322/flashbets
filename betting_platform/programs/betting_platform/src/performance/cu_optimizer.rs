use anchor_lang::prelude::*;
use std::collections::HashMap;
use crate::performance::errors::*;

// Precomputed tables for common values
pub struct PrecomputedTables {
    sqrt_lookup: HashMap<u32, u64>,
    tier_caps: HashMap<u32, u64>,
    common_multipliers: HashMap<u64, u64>,
}

impl PrecomputedTables {
    pub fn new() -> Self {
        let mut sqrt_lookup = HashMap::new();
        let mut tier_caps = HashMap::new();
        
        // Precompute square roots for common N values (2-16)
        sqrt_lookup.insert(2, 1_414); // sqrt(2) * 1000
        sqrt_lookup.insert(3, 1_732); // sqrt(3) * 1000
        sqrt_lookup.insert(4, 2_000); // sqrt(4) * 1000
        sqrt_lookup.insert(5, 2_236); // sqrt(5) * 1000
        sqrt_lookup.insert(6, 2_449); // sqrt(6) * 1000
        sqrt_lookup.insert(7, 2_646); // sqrt(7) * 1000
        sqrt_lookup.insert(8, 2_828); // sqrt(8) * 1000
        sqrt_lookup.insert(9, 3_000); // sqrt(9) * 1000
        sqrt_lookup.insert(10, 3_162); // sqrt(10) * 1000
        sqrt_lookup.insert(16, 4_000); // sqrt(16) * 1000
        
        // Precompute tier caps based on N
        tier_caps.insert(2, 100);
        tier_caps.insert(3, 200);
        tier_caps.insert(4, 300);
        tier_caps.insert(5, 400);
        tier_caps.insert(6, 500);
        tier_caps.insert(7, 600);
        tier_caps.insert(8, 700);
        
        // Common multipliers for fixed-point math
        let mut common_multipliers = HashMap::new();
        common_multipliers.insert(100, 100_000_000); // 100 * FIXED_POINT_SCALE
        common_multipliers.insert(10, 10_000_000);   // 10 * FIXED_POINT_SCALE
        common_multipliers.insert(1, 1_000_000);     // 1 * FIXED_POINT_SCALE
        
        Self {
            sqrt_lookup,
            tier_caps,
            common_multipliers,
        }
    }

    pub fn get_sqrt(&self, n: u32) -> Option<u64> {
        self.sqrt_lookup.get(&n).copied()
    }

    pub fn get_tier_cap(&self, n: u32) -> u64 {
        *self.tier_caps.get(&n).unwrap_or(&1000)
    }
}

pub struct BatchProcessor {
    batch_size: usize,
    max_batch_cu: u64,
}

impl BatchProcessor {
    pub fn new(batch_size: usize, max_batch_cu: u64) -> Self {
        Self {
            batch_size,
            max_batch_cu,
        }
    }

    pub fn should_batch(&self, operations: usize) -> bool {
        operations >= self.batch_size
    }

    pub fn calculate_batch_cu(&self, operations: usize, cu_per_op: u64) -> u64 {
        let total_cu = operations as u64 * cu_per_op;
        let batch_overhead = 1000; // Fixed overhead for batching
        
        total_cu.saturating_add(batch_overhead).min(self.max_batch_cu)
    }
}

pub struct CacheManager {
    cache: HashMap<u128, CachedResult>,
    max_entries: usize,
}

#[derive(Clone)]
pub struct CachedResult {
    pub key: u128,
    pub value: AMMResult,
    pub timestamp: i64,
}

#[derive(Clone, Debug)]
pub struct AMMResult {
    pub price: i64,
    pub iterations: u8,
}

impl CacheManager {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_entries,
        }
    }

    pub fn get(&self, key: &u128) -> Option<AMMResult> {
        self.cache.get(key).map(|cached| cached.value.clone())
    }

    pub fn set(&mut self, key: u128, value: AMMResult) {
        if self.cache.len() >= self.max_entries {
            // Simple eviction - remove oldest entry
            if let Some(oldest_key) = self.find_oldest_key() {
                self.cache.remove(&oldest_key);
            }
        }
        
        let cached = CachedResult {
            key,
            value,
            timestamp: Clock::get().unwrap().unix_timestamp,
        };
        
        self.cache.insert(key, cached);
    }

    fn find_oldest_key(&self) -> Option<u128> {
        self.cache
            .values()
            .min_by_key(|cached| cached.timestamp)
            .map(|cached| cached.key)
    }
}

#[derive(Clone)]
pub struct LeverageParams {
    pub depth: u64,
    pub coverage: u64, // Fixed-point representation
    pub n: u32,
}

pub struct CUOptimizer {
    pub precomputed_tables: PrecomputedTables,
    pub batch_processor: BatchProcessor,
    pub cache_manager: CacheManager,
}

impl CUOptimizer {
    pub fn new() -> Self {
        Self {
            precomputed_tables: PrecomputedTables::new(),
            batch_processor: BatchProcessor::new(10, 100_000),
            cache_manager: CacheManager::new(1000),
        }
    }

    pub fn optimize_leverage_calculation(
        &self,
        params: &LeverageParams,
    ) -> Result<u64> {
        // Use precomputed square roots for common N values
        let sqrt_n = self.precomputed_tables.get_sqrt(params.n)
            .unwrap_or_else(|| self.compute_sqrt_fixed_point(params.n));
        
        // Optimize formula: min(100 × (1 + 0.1 × depth), coverage × 100/√N, tier_cap)
        // Using fixed-point arithmetic to avoid floating point operations
        
        // Calculate depth_factor = 100 × (1 + 0.1 × depth)
        // = 100 + 10 × depth
        let depth_factor = 100u64
            .saturating_add(10u64.saturating_mul(params.depth));
        
        // Calculate coverage_factor = coverage × 100 / sqrt_n
        // coverage is already in fixed-point, sqrt_n is in thousandths
        let coverage_factor = params.coverage
            .saturating_mul(100_000) // Scale to match sqrt precision
            .checked_div(sqrt_n)
            .ok_or(OptimizationError::DivisionByZero)?;
        
        let tier_cap = self.precomputed_tables.get_tier_cap(params.n);
        
        Ok(depth_factor.min(coverage_factor).min(tier_cap))
    }

    pub fn compute_sqrt_fixed_point(&self, n: u32) -> u64 {
        // Fast integer square root approximation
        // Newton-Raphson method with fixed iterations
        let mut x = n as u64 * 1000; // Initial guess
        
        for _ in 0..4 {
            x = (x + (n as u64 * 1_000_000) / x) / 2;
        }
        
        x
    }

    pub fn optimize_amm_calculation(
        &mut self,
        amm_type: AMMType,
        params: &AMMParams,
    ) -> Result<AMMResult> {
        match amm_type {
            AMMType::LMSR => self.optimize_lmsr(params),
            AMMType::PMAMM => self.optimize_pm_amm(params),
            AMMType::L2 => self.optimize_l2_distribution(params),
        }
    }

    fn optimize_lmsr(&self, params: &AMMParams) -> Result<AMMResult> {
        // LMSR: Price = exp(q/b) / sum(exp(qi/b))
        // Use approximations for exponential calculations
        
        let b = params.liquidity_parameter;
        let q = params.outcome_quantity;
        
        // For small values, use Taylor expansion: exp(x) ≈ 1 + x + x²/2
        let exp_approx = if q.abs() < (b / 10) as i64 {
            FIXED_POINT_SCALE + q * FIXED_POINT_SCALE / b as i64 +
            (q * q * FIXED_POINT_SCALE / 2) / (b as i64 * b as i64)
        } else {
            // Fall back to full calculation
            self.calculate_exp_fixed_point(q, b)?
        };
        
        Ok(AMMResult {
            price: exp_approx,
            iterations: 1,
        })
    }

    fn optimize_pm_amm(&mut self, params: &AMMParams) -> Result<AMMResult> {
        // Use cached Newton-Raphson iterations
        let cache_key = self.compute_cache_key(params);
        
        if let Some(cached_result) = self.cache_manager.get(&cache_key) {
            return Ok(cached_result);
        }
        
        // Optimize Newton-Raphson with fixed-point math
        let mut y = params.initial_guess;
        let mut iterations = 0u8;
        
        while iterations < NEWTON_RAPHSON_MAX_ITERATIONS {
            let (f_y, df_dy) = self.compute_pm_amm_derivatives_optimized(y, params)?;
            
            if f_y.abs() < CONVERGENCE_THRESHOLD {
                break;
            }
            
            // Newton-Raphson step: y = y - f(y)/f'(y)
            let delta = f_y.saturating_mul(FIXED_POINT_SCALE)
                .checked_div(df_dy)
                .ok_or(OptimizationError::DivisionByZero)?;
            
            y = y.saturating_sub(delta);
            iterations += 1;
        }
        
        let result = AMMResult { price: y, iterations };
        self.cache_manager.set(cache_key, result.clone());
        
        Ok(result)
    }

    fn optimize_l2_distribution(&self, params: &AMMParams) -> Result<AMMResult> {
        // L2 distribution optimization
        // Use precomputed normalization factors
        
        let mean = params.distribution_mean;
        let variance = params.distribution_variance;
        
        // Fast approximation for normal distribution
        let z_score = (params.outcome_quantity - mean) * FIXED_POINT_SCALE / variance;
        
        // Use piecewise linear approximation for CDF
        let price = if z_score.abs() > 3 * FIXED_POINT_SCALE {
            if z_score > 0 { FIXED_POINT_SCALE } else { 0 }
        } else {
            // Linear interpolation in [-3, 3] range
            (z_score + 3 * FIXED_POINT_SCALE) / 6
        };
        
        Ok(AMMResult {
            price,
            iterations: 1,
        })
    }

    fn compute_cache_key(&self, params: &AMMParams) -> u128 {
        // Simple hash function for cache key
        let mut key = params.liquidity_parameter as u128;
        key = key.wrapping_mul(31).wrapping_add(params.outcome_quantity as u128);
        key = key.wrapping_mul(31).wrapping_add(params.initial_guess as u128);
        key
    }

    fn compute_pm_amm_derivatives_optimized(
        &self,
        y: i64,
        params: &AMMParams,
    ) -> Result<(i64, i64)> {
        // Optimized derivative calculation
        // f(y) and f'(y) for PM-AMM
        
        let l = params.liquidity_parameter as i64;
        let q = params.outcome_quantity;
        
        // Simplified calculation using fixed-point math
        let f_y = y.saturating_mul(y).saturating_sub(l * l).saturating_add(q * y);
        let df_dy = 2i64.saturating_mul(y).saturating_add(q);
        
        Ok((f_y / FIXED_POINT_SCALE, df_dy))
    }

    fn calculate_exp_fixed_point(&self, x: i64, scale: u64) -> Result<i64> {
        // Fixed-point exponential approximation
        // Using scaled arithmetic to avoid overflow
        
        let scaled_x = x * FIXED_POINT_SCALE / scale as i64;
        
        // Taylor series: exp(x) = 1 + x + x²/2! + x³/3! + ...
        let mut result = FIXED_POINT_SCALE;
        let mut term = scaled_x;
        
        for i in 1..5 {
            result = result.saturating_add(term);
            term = term.saturating_mul(scaled_x) / ((i + 1) * FIXED_POINT_SCALE);
        }
        
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AMMType {
    LMSR,
    PMAMM,
    L2,
}

#[derive(Clone)]
pub struct AMMParams {
    pub liquidity_parameter: u64,
    pub outcome_quantity: i64,
    pub initial_guess: i64,
    pub distribution_mean: i64,
    pub distribution_variance: i64,
}

// Helper function for profiling optimization
pub fn profile_optimization<F, R>(
    optimizer: &mut CUOptimizer,
    operation: &str,
    f: F,
) -> Result<(R, u64)>
where
    F: FnOnce(&mut CUOptimizer) -> Result<R>,
{
    let start = Clock::get()?.slot;
    let result = f(optimizer)?;
    let end = Clock::get()?.slot;
    
    let slots_used = end.saturating_sub(start);
    msg!("Optimization {} completed in {} slots", operation, slots_used);
    
    Ok((result, slots_used))
}