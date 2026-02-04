use anchor_lang::prelude::*;
use crate::types::U64F64;
use crate::amm::types::AMMType;

#[account]
pub struct SyntheticRouter {
    /// Router identifier
    pub router_id: [u8; 32],
    
    /// Verse being routed
    pub verse_id: [u8; 32],
    
    /// Child markets in this verse
    pub child_markets: Vec<ChildMarket>,
    
    /// Routing weights based on liquidity/volume
    pub routing_weights: Vec<U64F64>,
    
    /// Aggregated probability
    pub aggregated_prob: U64F64,
    
    /// Total liquidity available
    pub total_liquidity: u64,
    
    /// Routing strategy
    pub routing_strategy: RoutingStrategy,
    
    /// Performance tracking
    pub performance: RouterPerformance,
    
    /// Last update slot
    pub last_update_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ChildMarket {
    /// Polymarket market ID
    pub market_id: String,
    
    /// Current probability from Polymarket
    pub probability: U64F64,
    
    /// 7-day volume for weighting
    pub volume_7d: u64,
    
    /// Liquidity depth
    pub liquidity_depth: u64,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// AMM type for this market
    pub amm_type: AMMType,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum RoutingStrategy {
    /// Route proportionally by liquidity
    ProportionalLiquidity,
    
    /// Route to best price first
    BestPriceFirst,
    
    /// Minimize slippage across all
    MinimizeSlippage,
    
    /// Maximize fee savings
    MaximizeFeeSavings,
    
    /// Custom weighted routing
    CustomWeighted,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, Default)]
pub struct RouterPerformance {
    /// Total volume routed
    pub total_volume_routed: u64,
    
    /// Fees saved vs individual trades
    pub total_fees_saved: u64,
    
    /// Average execution improvement in bps
    pub avg_execution_improvement_bps: u16,
    
    /// Number of routes executed
    pub routes_executed: u64,
    
    /// Failed route attempts
    pub failed_routes: u64,
}

#[derive(Debug, Clone)]
pub struct RouteLeg {
    pub market_id: String,
    pub size: u64,
    pub expected_price: U64F64,
    pub expected_slippage_bps: u16,
    pub fee: u64,
}

#[derive(Debug)]
pub struct RouteResult {
    pub route_legs: Vec<RouteLeg>,
    pub total_cost: u64,
    pub total_fees: u64,
    pub avg_execution_price: U64F64,
    pub total_slippage_bps: u16,
    pub unfilled_amount: u64,
}

impl SyntheticRouter {
    pub const FIXED_LEN: usize = 8 + // discriminator
        32 + // router_id
        32 + // verse_id
        8 + // aggregated_prob
        8 + // total_liquidity
        1 + // routing_strategy
        40 + // performance
        8; // last_update_slot
    
    // Dynamic size calculation based on child markets
    pub fn len(num_markets: usize) -> usize {
        Self::FIXED_LEN + 
        4 + (num_markets * 200) + // child_markets (estimated 200 bytes each)
        4 + (num_markets * 8) // routing_weights
    }
}