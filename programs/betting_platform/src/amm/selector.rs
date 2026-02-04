use anchor_lang::prelude::*;
use fixed::types::U64F64;
use crate::amm::types::*;
use crate::errors::ErrorCode;
use crate::constants::SLOTS_PER_DAY;

impl HybridAMMSelector {
    /// Select optimal AMM based on market characteristics
    pub fn select_amm(
        market_type: &MarketType,
        time_to_expiry: u64,
        override_flags: &AMMOverrideFlags,
        _current_metrics: &AMMPerformanceMetrics,
    ) -> AMMType {
        // Check overrides first
        if override_flags.force_time_decay && time_to_expiry < SLOTS_PER_DAY {
            return AMMType::PMAMM;
        }
        
        if override_flags.force_distribution {
            return AMMType::L2Distribution;
        }
        
        // Deterministic selection based on market type
        match market_type {
            MarketType::Binary => {
                // Use PM-AMM if close to expiry for time decay benefits
                if time_to_expiry < SLOTS_PER_DAY {
                    AMMType::PMAMM
                } else {
                    AMMType::LMSR
                }
            },
            
            MarketType::MultiOutcome { count } => {
                if *count <= 64 {
                    AMMType::PMAMM // PM-AMM optimized for multi-outcome
                } else {
                    AMMType::L2Distribution // Too many outcomes for PM-AMM
                }
            },
            
            MarketType::Continuous { .. } => {
                AMMType::L2Distribution // L2 designed for continuous
            },
            
            MarketType::Verse { depth } => {
                // Deeper verses benefit from PM-AMM's uniform LVR
                if *depth > 4 {
                    AMMType::PMAMM
                } else {
                    AMMType::LMSR
                }
            },
            
            MarketType::Quantum { .. } => {
                // Quantum always uses PM-AMM for uniform treatment
                AMMType::PMAMM
            },
        }
    }
    
    /// Calculate switching cost if AMM type were to change
    pub fn calculate_switching_cost(
        &self,
        new_amm: AMMType,
        current_liquidity: u64,
    ) -> Result<u64> {
        if self.amm_type == new_amm {
            return Ok(0);
        }
        
        // Estimate rebalancing cost (0.1% of liquidity)
        let base_cost = current_liquidity / 1000;
        
        // Add complexity factor
        let complexity_multiplier = match (self.amm_type, new_amm) {
            (AMMType::LMSR, AMMType::PMAMM) => U64F64::from_num(1.2),
            (AMMType::PMAMM, AMMType::L2Distribution) => U64F64::from_num(1.5),
            (AMMType::L2Distribution, _) => U64F64::from_num(2.0),
            _ => U64F64::from_num(1),
        };
        
        let total_cost = U64F64::from_num(base_cost) * complexity_multiplier;
        Ok(total_cost.to_num())
    }
    
    /// Update performance metrics after trade
    pub fn update_metrics(
        &mut self,
        trade_volume: u64,
        slippage_bps: u16,
        lvr_amount: U64F64,
    ) -> Result<()> {
        self.performance_metrics.total_volume = self.performance_metrics.total_volume
            .checked_add(trade_volume)
            .ok_or(ErrorCode::MathOverflow)?;
        
        self.performance_metrics.trade_count += 1;
        
        // Update average slippage
        let total_slippage = self.performance_metrics.avg_slippage_bps as u64 *
                            (self.performance_metrics.trade_count - 1);
        self.performance_metrics.avg_slippage_bps =
            ((total_slippage + slippage_bps as u64) / self.performance_metrics.trade_count) as u16;
        
        self.performance_metrics.total_lvr = self.performance_metrics.total_lvr + lvr_amount;
        
        // Recalculate efficiency score
        self.performance_metrics.efficiency_score = self.calculate_efficiency_score();
        
        Ok(())
    }
    
    /// Calculate efficiency score (0-100)
    fn calculate_efficiency_score(&self) -> u8 {
        // Base score starts at 100
        let mut score = 100u8;
        
        // Deduct for high slippage (>10bps)
        if self.performance_metrics.avg_slippage_bps > 10 {
            let penalty = ((self.performance_metrics.avg_slippage_bps - 10) / 2).min(30);
            score = score.saturating_sub(penalty as u8);
        }
        
        // Deduct for high LVR relative to volume
        if self.performance_metrics.total_volume > 0 {
            let lvr_ratio = self.performance_metrics.total_lvr /
                           U64F64::from_num(self.performance_metrics.total_volume);
            if lvr_ratio > U64F64::from_num(0.01) { // >1% LVR
                let penalty = ((lvr_ratio.to_num::<f64>() * 1000.0) as u8).min(20);
                score = score.saturating_sub(penalty);
            }
        }
        
        score
    }
    
    /// Initialize AMM selector
    pub fn initialize(
        &mut self,
        market_id: [u8; 32],
        market_type: MarketType,
        time_to_expiry: u64,
        current_slot: u64,
    ) -> Result<()> {
        self.market_id = market_id;
        self.market_type = market_type;
        self.time_to_expiry = time_to_expiry;
        self.current_slot = current_slot;
        self.override_flags = AMMOverrideFlags::default();
        self.performance_metrics = AMMPerformanceMetrics::default();
        self.transition_state = None;
        
        // Select initial AMM type
        self.amm_type = Self::select_amm(
            &market_type,
            time_to_expiry,
            &self.override_flags,
            &self.performance_metrics,
        );
        
        Ok(())
    }
}