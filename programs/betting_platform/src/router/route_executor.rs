use anchor_lang::prelude::*;
use fixed::types::U64F64;
use crate::router::types::*;
use crate::errors::ErrorCode;
use crate::constants::{POLYMARKET_FEE_BPS, PROTOCOL_FEE_BPS};

pub struct RouteExecutor;

impl RouteExecutor {
    /// Calculate optimal route for a synthetic trade
    pub fn calculate_route(
        router: &SyntheticRouter,
        trade_size: u64,
        is_buy: bool,
    ) -> Result<RouteResult> {
        let mut remaining_size = trade_size;
        let mut route_legs: Vec<RouteLeg> = Vec::new();
        let mut total_cost = 0u64;
        let mut total_fees = 0u64;
        
        match router.routing_strategy {
            RoutingStrategy::ProportionalLiquidity => {
                // Route proportionally based on liquidity weights
                for (i, child) in router.child_markets.iter().enumerate() {
                    let weight = router.routing_weights[i];
                    let leg_size = (U64F64::from_num(trade_size) * weight).to_num::<u64>();
                    
                    if leg_size > 0 {
                        let (cost, fee) = Self::calculate_leg_cost(
                            child,
                            leg_size,
                            is_buy,
                        )?;
                        
                        route_legs.push(RouteLeg {
                            market_id: child.market_id.clone(),
                            size: leg_size,
                            expected_price: child.probability,
                            expected_slippage_bps: Self::estimate_slippage(child, leg_size),
                            fee,
                        });
                        
                        total_cost += cost;
                        total_fees += fee;
                        remaining_size = remaining_size.saturating_sub(leg_size);
                    }
                }
            },
            
            RoutingStrategy::BestPriceFirst => {
                // Sort markets by price (best first)
                let mut sorted_markets: Vec<(usize, &ChildMarket)> =
                    router.child_markets.iter().enumerate().collect();
                
                if is_buy {
                    sorted_markets.sort_by_key(|(_, m)| m.probability.to_bits());
                } else {
                    sorted_markets.sort_by_key(|(_, m)| u64::MAX - m.probability.to_bits());
                }
                
                // Route through best prices until filled
                for (_idx, child) in sorted_markets {
                    let available_liquidity = Self::get_available_liquidity(child, is_buy);
                    let leg_size = remaining_size.min(available_liquidity);
                    
                    if leg_size > 0 {
                        let (cost, fee) = Self::calculate_leg_cost(child, leg_size, is_buy)?;
                        
                        route_legs.push(RouteLeg {
                            market_id: child.market_id.clone(),
                            size: leg_size,
                            expected_price: child.probability,
                            expected_slippage_bps: Self::estimate_slippage(child, leg_size),
                            fee,
                        });
                        
                        total_cost += cost;
                        total_fees += fee;
                        remaining_size -= leg_size;
                        
                        if remaining_size == 0 {
                            break;
                        }
                    }
                }
            },
            
            RoutingStrategy::MinimizeSlippage => {
                // Use iterative algorithm to minimize total slippage
                let optimal_allocation = Self::optimize_for_slippage(
                    &router.child_markets,
                    trade_size,
                    is_buy,
                )?;
                
                for (i, &allocation) in optimal_allocation.iter().enumerate() {
                    if allocation > 0 {
                        let child = &router.child_markets[i];
                        let (cost, fee) = Self::calculate_leg_cost(child, allocation, is_buy)?;
                        
                        route_legs.push(RouteLeg {
                            market_id: child.market_id.clone(),
                            size: allocation,
                            expected_price: child.probability,
                            expected_slippage_bps: Self::estimate_slippage(child, allocation),
                            fee,
                        });
                        
                        total_cost += cost;
                        total_fees += fee;
                    }
                }
                remaining_size = 0; // Optimization should allocate all
            },
            
            _ => {
                return Err(ErrorCode::UnsupportedRoutingStrategy.into());
            }
        }
        
        // Calculate aggregate metrics
        let avg_price = if trade_size > 0 {
            U64F64::from_num(total_cost) / U64F64::from_num(trade_size)
        } else {
            U64F64::from_num(0)
        };
        
        let total_slippage_bps = Self::calculate_total_slippage(&route_legs, router.aggregated_prob);
        
        Ok(RouteResult {
            route_legs,
            total_cost,
            total_fees,
            avg_execution_price: avg_price,
            total_slippage_bps,
            unfilled_amount: remaining_size,
        })
    }
    
    /// Calculate cost for a single leg
    fn calculate_leg_cost(
        market: &ChildMarket,
        size: u64,
        is_buy: bool,
    ) -> Result<(u64, u64)> {
        let base_price = market.probability;
        let slippage = Self::estimate_slippage(market, size);
        
        // Adjust price for slippage
        let execution_price = if is_buy {
            base_price * (U64F64::from_num(1) + U64F64::from_num(slippage) / U64F64::from_num(10_000))
        } else {
            base_price * (U64F64::from_num(1) - U64F64::from_num(slippage) / U64F64::from_num(10_000))
        };
        
        let cost = (U64F64::from_num(size) * execution_price).to_num::<u64>();
        
        // Polymarket fee (1.5%) + our fee (0.03-0.28%)
        let polymarket_fee = (size as u128 * POLYMARKET_FEE_BPS as u128 / 10_000) as u64;
        let our_fee = (size as u128 * PROTOCOL_FEE_BPS as u128 / 10_000) as u64;
        let total_fee = polymarket_fee + our_fee;
        
        Ok((cost, total_fee))
    }
    
    /// Estimate slippage for a given size
    fn estimate_slippage(market: &ChildMarket, size: u64) -> u16 {
        // Simple model: slippage = size / (2 * liquidity_depth) * 10000 bps
        if market.liquidity_depth == 0 {
            return 1000; // 10% max slippage
        }
        
        let slippage = (size as u128 * 10_000) / (2 * market.liquidity_depth as u128);
        (slippage as u16).min(1000)
    }
    
    /// Get available liquidity at current price level
    fn get_available_liquidity(market: &ChildMarket, _is_buy: bool) -> u64 {
        // Simplified: assume 10% of total liquidity available at touch
        market.liquidity_depth / 10
    }
    
    /// Optimize allocation to minimize slippage
    fn optimize_for_slippage(
        markets: &[ChildMarket],
        total_size: u64,
        _is_buy: bool,
    ) -> Result<Vec<u64>> {
        let n = markets.len();
        let mut allocation = vec![0u64; n];
        let mut remaining = total_size;
        
        // Greedy algorithm: allocate to market with lowest marginal slippage
        while remaining > 0 {
            let mut best_idx = 0;
            let mut best_marginal_slippage = u16::MAX;
            
            for i in 0..n {
                let current_alloc = allocation[i];
                let marginal_size = remaining.min(markets[i].liquidity_depth / 100);
                
                if marginal_size > 0 {
                    let marginal_slippage = Self::estimate_slippage(
                        &markets[i],
                        current_alloc + marginal_size,
                    ).saturating_sub(Self::estimate_slippage(&markets[i], current_alloc));
                    
                    if marginal_slippage < best_marginal_slippage {
                        best_marginal_slippage = marginal_slippage;
                        best_idx = i;
                    }
                }
            }
            
            if best_marginal_slippage == u16::MAX {
                break; // No more liquidity available
            }
            
            let alloc_size = remaining.min(markets[best_idx].liquidity_depth / 100);
            allocation[best_idx] += alloc_size;
            remaining -= alloc_size;
        }
        
        Ok(allocation)
    }
    
    /// Calculate total slippage across all legs
    fn calculate_total_slippage(
        legs: &[RouteLeg],
        base_price: U64F64,
    ) -> u16 {
        let mut total_cost = 0u64;
        let mut total_size = 0u64;
        
        for leg in legs {
            total_cost += (U64F64::from_num(leg.size) * leg.expected_price).to_num::<u64>();
            total_size += leg.size;
        }
        
        if total_size == 0 {
            return 0;
        }
        
        let avg_price = U64F64::from_num(total_cost) / U64F64::from_num(total_size);
        let price_diff = if avg_price > base_price {
            avg_price - base_price
        } else {
            base_price - avg_price
        };
        
        ((price_diff / base_price) * U64F64::from_num(10_000)).to_num::<u16>()
    }
}