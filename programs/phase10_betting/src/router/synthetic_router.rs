use anchor_lang::prelude::*;
use crate::types::U64F64;
use crate::router::types::*;
use crate::amm::types::AMMType;
use crate::errors::ErrorCode;
use crate::constants::{LIQUIDITY_WEIGHT_BPS, VOLUME_WEIGHT_BPS, BPS_PRECISION};

impl SyntheticRouter {
    /// Initialize router
    pub fn initialize(
        &mut self,
        verse_id: [u8; 32],
        routing_strategy: RoutingStrategy,
        current_slot: u64,
    ) -> Result<()> {
        self.router_id = Pubkey::new_unique().to_bytes();
        self.verse_id = verse_id;
        self.child_markets = vec![];
        self.routing_weights = vec![];
        self.aggregated_prob = U64F64::zero();
        self.total_liquidity = 0;
        self.routing_strategy = routing_strategy;
        self.performance = RouterPerformance::default();
        self.last_update_slot = current_slot;
        
        Ok(())
    }
    
    /// Add child market to router
    pub fn add_child_market(
        &mut self,
        market_id: String,
        initial_probability: U64F64,
        volume_7d: u64,
        liquidity_depth: u64,
        amm_type: AMMType,
        current_timestamp: i64,
    ) -> Result<()> {
        // Check if we've reached the limit
        if self.child_markets.len() >= 50 {
            return Err(ErrorCode::ChildMarketLimitReached.into());
        }
        
        let child_market = ChildMarket {
            market_id,
            probability: initial_probability,
            volume_7d,
            liquidity_depth,
            last_update: current_timestamp,
            amm_type,
        };
        
        self.child_markets.push(child_market);
        self.total_liquidity = self.total_liquidity
            .checked_add(liquidity_depth)
            .ok_or(ErrorCode::MathOverflow)?;
        
        // Recalculate weights
        self.update_weights()?;
        self.update_aggregated_probability()?;
        
        Ok(())
    }
    
    /// Update routing weights based on latest market data
    pub fn update_weights(&mut self) -> Result<()> {
        let total_weight = self.child_markets.iter()
            .map(|m| {
                // Weight = 70% liquidity + 30% volume
                let liq_weight = U64F64::from_num(m.liquidity_depth) * U64F64::from_num(LIQUIDITY_WEIGHT_BPS) / U64F64::from_num(BPS_PRECISION);
                let vol_weight = U64F64::from_num(m.volume_7d) * U64F64::from_num(VOLUME_WEIGHT_BPS) / U64F64::from_num(BPS_PRECISION);
                liq_weight + vol_weight
            })
            .fold(U64F64::zero(), |a, b| a + b);
        
        if total_weight == U64F64::zero() {
            return Err(ErrorCode::NoLiquidityAvailable.into());
        }
        
        self.routing_weights = self.child_markets.iter()
            .map(|m| {
                let liq_weight = U64F64::from_num(m.liquidity_depth) * U64F64::from_num(LIQUIDITY_WEIGHT_BPS) / U64F64::from_num(BPS_PRECISION);
                let vol_weight = U64F64::from_num(m.volume_7d) * U64F64::from_num(VOLUME_WEIGHT_BPS) / U64F64::from_num(BPS_PRECISION);
                (liq_weight + vol_weight) / total_weight
            })
            .collect();
        
        Ok(())
    }
    
    /// Update aggregated probability from child markets
    pub fn update_aggregated_probability(&mut self) -> Result<()> {
        let total_weight: U64F64 = self.routing_weights.iter().copied().sum();
        
        if total_weight == U64F64::zero() {
            return Ok(());
        }
        
        self.aggregated_prob = self.child_markets.iter()
            .zip(self.routing_weights.iter())
            .map(|(market, &weight)| market.probability * weight)
            .fold(U64F64::zero(), |a, b| a + b) / total_weight;
        
        Ok(())
    }
    
    /// Update performance metrics after route execution
    pub fn update_performance(
        &mut self,
        volume_routed: u64,
        fees_saved: u64,
        execution_improvement_bps: u16,
    ) -> Result<()> {
        self.performance.total_volume_routed = self.performance.total_volume_routed
            .checked_add(volume_routed)
            .ok_or(ErrorCode::MathOverflow)?;
        
        self.performance.total_fees_saved = self.performance.total_fees_saved
            .checked_add(fees_saved)
            .ok_or(ErrorCode::MathOverflow)?;
        
        self.performance.routes_executed += 1;
        
        // Update average execution improvement
        let total_improvement = self.performance.avg_execution_improvement_bps as u64 * 
                               (self.performance.routes_executed - 1);
        self.performance.avg_execution_improvement_bps = 
            ((total_improvement + execution_improvement_bps as u64) / self.performance.routes_executed) as u16;
        
        Ok(())
    }
}