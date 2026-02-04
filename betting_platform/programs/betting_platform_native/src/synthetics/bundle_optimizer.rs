use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::{HashMap, HashSet};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::synthetics::{SyntheticWrapper, WrapperManager, RoutingEngine};

/// Bundle optimizer for efficient trade execution
pub struct BundleOptimizer {
    pub min_bundle_size: u64,
    pub max_markets_per_bundle: usize,
    pub cu_per_child_market: u64,  // CU per child market in bundle
}

#[derive(Debug, Clone)]
pub struct BundleRequest {
    pub user: Pubkey,
    pub trades: Vec<TradeIntent>,
    pub max_slippage: U64F64,
}

#[derive(Debug, Clone)]
pub struct TradeIntent {
    pub synthetic_id: u128,
    pub is_buy: bool,
    pub amount: u64,
    pub leverage: U64F64,
}

#[derive(Debug, Clone)]
pub struct OptimizedBundle {
    pub bundles: Vec<Bundle>,
    pub total_saved_fee: u64,
    pub execution_plan: ExecutionPlan,
}

#[derive(Debug, Clone)]
pub struct Bundle {
    pub trades: Vec<TradeIntent>,
    pub combined_orders: Vec<CombinedOrder>,
    pub estimated_fee: u64,
    pub saved_fee: u64,
}

#[derive(Debug, Clone)]
pub struct CombinedOrder {
    pub market_id: Pubkey,
    pub total_amount: u64,
    pub average_leverage: U64F64,
    pub is_buy: bool,
}

#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub steps: Vec<ExecutionStep>,
    pub total_gas_estimate: u64,
    pub optimal_ordering: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub bundle_index: usize,
    pub priority: u8,
    pub dependencies: Vec<usize>,
}

impl Default for BundleOptimizer {
    fn default() -> Self {
        Self {
            min_bundle_size: 100, // Minimum 100 tokens to bundle
            max_markets_per_bundle: 10,
            cu_per_child_market: 3_000, // 3k CU per child, 10 children = 30k CU total
        }
    }
}

impl BundleOptimizer {
    pub fn new(min_bundle_size: u64, max_markets_per_bundle: usize) -> Self {
        Self {
            min_bundle_size,
            max_markets_per_bundle,
            cu_per_child_market: 3_000, // 3k CU per child
        }
    }

    /// Optimize bundle of trades
    pub fn optimize_bundle(
        &self,
        request: BundleRequest,
        wrapper_manager: &WrapperManager,
    ) -> Result<OptimizedBundle, ProgramError> {
        // Group trades by overlapping markets
        let market_groups = self.group_by_markets(&request.trades, wrapper_manager)?;

        let mut bundles = Vec::new();
        let mut total_saved = 0u64;

        for group in market_groups {
            if group.len() >= 2 { // Only bundle if multiple trades
                let bundle = self.create_optimized_bundle(group, wrapper_manager)?;
                total_saved += bundle.saved_fee;
                bundles.push(bundle);
            } else {
                // Single trade, no bundling benefit
                bundles.push(Bundle {
                    trades: group.clone(),
                    combined_orders: vec![],
                    estimated_fee: self.calculate_single_fee(&group[0]),
                    saved_fee: 0,
                });
            }
        }

        let execution_plan = self.create_execution_plan(&bundles)?;

        Ok(OptimizedBundle {
            bundles,
            total_saved_fee: total_saved,
            execution_plan,
        })
    }

    /// Group trades by overlapping markets
    fn group_by_markets(
        &self,
        trades: &[TradeIntent],
        wrapper_manager: &WrapperManager,
    ) -> Result<Vec<Vec<TradeIntent>>, ProgramError> {
        let mut groups: Vec<Vec<TradeIntent>> = Vec::new();

        for trade in trades {
            let wrapper = wrapper_manager.wrappers
                .get(&trade.synthetic_id)
                .ok_or(BettingPlatformError::WrapperNotFound)?;

            // Find group with overlapping markets
            let mut added = false;
            for group in &mut groups {
                if self.has_market_overlap(&trade, &group, wrapper_manager)? {
                    group.push(trade.clone());
                    added = true;
                    break;
                }
            }

            if !added {
                groups.push(vec![trade.clone()]);
            }
        }

        Ok(groups)
    }

    /// Check if trade has overlapping markets with group
    fn has_market_overlap(
        &self,
        trade: &TradeIntent,
        group: &[TradeIntent],
        wrapper_manager: &WrapperManager,
    ) -> Result<bool, ProgramError> {
        let trade_wrapper = wrapper_manager.wrappers
            .get(&trade.synthetic_id)
            .ok_or(BettingPlatformError::WrapperNotFound)?;

        let trade_markets: HashSet<_> = trade_wrapper.polymarket_markets.iter().collect();

        for group_trade in group {
            let group_wrapper = wrapper_manager.wrappers
                .get(&group_trade.synthetic_id)
                .ok_or(BettingPlatformError::WrapperNotFound)?;

            let group_markets: HashSet<_> = group_wrapper.polymarket_markets.iter().collect();

            // Check for intersection
            if trade_markets.intersection(&group_markets).count() > 0 {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Create optimized bundle from grouped trades
    fn create_optimized_bundle(
        &self,
        trades: Vec<TradeIntent>,
        wrapper_manager: &WrapperManager,
    ) -> Result<Bundle, ProgramError> {
        let mut market_amounts: HashMap<Pubkey, (u64, u64, U64F64)> = HashMap::new(); // (buy_amount, sell_amount, total_leverage)
        
        // Aggregate amounts by market
        for trade in &trades {
            let wrapper = wrapper_manager.wrappers
                .get(&trade.synthetic_id)
                .ok_or(BettingPlatformError::WrapperNotFound)?;

            for (i, market) in wrapper.polymarket_markets.iter().enumerate() {
                let weight = wrapper.weights[i];
                let market_amount = U64F64::from_num(trade.amount).checked_mul(weight)?.to_num();

                let entry = market_amounts.entry(*market).or_insert((0, 0, U64F64::from_num(0)));
                
                if trade.is_buy {
                    entry.0 += market_amount;
                } else {
                    entry.1 += market_amount;
                }
                
                // Update leverage (weighted average)
                let old_total = entry.0 + entry.1 - market_amount;
                if old_total > 0 {
                    let old_weight = U64F64::from_num(old_total);
                    let new_weight = U64F64::from_num(market_amount);
                    entry.2 = (entry.2.checked_mul(old_weight)? + trade.leverage.checked_mul(new_weight)?)
                        .checked_div(old_weight + new_weight)?;
                } else {
                    entry.2 = trade.leverage;
                }
            }
        }

        // Create combined orders
        let mut combined_orders = Vec::new();
        for (market_id, (buy_amount, sell_amount, avg_leverage)) in market_amounts {
            if buy_amount > sell_amount {
                combined_orders.push(CombinedOrder {
                    market_id,
                    total_amount: buy_amount - sell_amount,
                    average_leverage: avg_leverage,
                    is_buy: true,
                });
            } else if sell_amount > buy_amount {
                combined_orders.push(CombinedOrder {
                    market_id,
                    total_amount: sell_amount - buy_amount,
                    average_leverage: avg_leverage,
                    is_buy: false,
                });
            }
            // If equal, they cancel out - no order needed
        }

        // Calculate fees
        let individual_fees: u64 = trades.iter()
            .map(|t| self.calculate_single_fee(t))
            .sum();

        let bundled_fee = self.calculate_bundled_fee(&combined_orders);
        let saved_fee = individual_fees.saturating_sub(bundled_fee);

        Ok(Bundle {
            trades,
            combined_orders,
            estimated_fee: bundled_fee,
            saved_fee,
        })
    }

    /// Calculate fee for single trade
    fn calculate_single_fee(&self, trade: &TradeIntent) -> u64 {
        // Base fee: 0.15% of amount (15 basis points)
        trade.amount.saturating_mul(15).saturating_div(10_000)
    }

    /// Calculate fee for bundled trades
    fn calculate_bundled_fee(&self, orders: &[CombinedOrder]) -> u64 {
        // Bundled fee: 0.06% of total amount (6 basis points) - 60% savings
        let total_amount: u64 = orders.iter()
            .map(|o| o.total_amount)
            .sum();
        
        total_amount.saturating_mul(6).saturating_div(10_000)
    }

    /// Create execution plan for bundles
    fn create_execution_plan(&self, bundles: &[Bundle]) -> Result<ExecutionPlan, ProgramError> {
        let mut steps = Vec::new();
        let mut dependencies = self.analyze_dependencies(bundles)?;

        // Topological sort for optimal ordering
        let optimal_ordering = self.topological_sort(bundles.len(), &dependencies)?;

        for (priority, bundle_index) in optimal_ordering.iter().enumerate() {
            steps.push(ExecutionStep {
                bundle_index: *bundle_index,
                priority: priority as u8,
                dependencies: dependencies.get(bundle_index).cloned().unwrap_or_default(),
            });
        }

        // Calculate total CU based on number of child markets per bundle
        let total_gas_estimate = bundles.iter()
            .map(|b| b.trades.len() as u64 * self.cu_per_child_market)
            .sum::<u64>()
            .min(1_400_000); // Cap at Solana block limit

        Ok(ExecutionPlan {
            steps,
            total_gas_estimate,
            optimal_ordering,
        })
    }

    /// Analyze dependencies between bundles
    fn analyze_dependencies(&self, bundles: &[Bundle]) -> Result<HashMap<usize, Vec<usize>>, ProgramError> {
        let mut dependencies: HashMap<usize, Vec<usize>> = HashMap::new();

        // For now, assume no dependencies (can be enhanced based on market conditions)
        for i in 0..bundles.len() {
            dependencies.insert(i, Vec::new());
        }

        Ok(dependencies)
    }

    /// Topological sort for execution ordering
    fn topological_sort(
        &self,
        num_bundles: usize,
        dependencies: &HashMap<usize, Vec<usize>>,
    ) -> Result<Vec<usize>, ProgramError> {
        let mut in_degree = vec![0; num_bundles];
        let mut adj_list: HashMap<usize, Vec<usize>> = HashMap::new();

        // Build adjacency list and calculate in-degrees
        for (node, deps) in dependencies {
            for &dep in deps {
                adj_list.entry(dep).or_insert_with(Vec::new).push(*node);
                in_degree[*node] += 1;
            }
        }

        // Queue for nodes with no dependencies
        let mut queue = Vec::new();
        for (i, &degree) in in_degree.iter().enumerate() {
            if degree == 0 {
                queue.push(i);
            }
        }

        let mut result = Vec::new();

        while let Some(node) = queue.pop() {
            result.push(node);

            if let Some(neighbors) = adj_list.get(&node) {
                for &neighbor in neighbors {
                    in_degree[neighbor] -= 1;
                    if in_degree[neighbor] == 0 {
                        queue.push(neighbor);
                    }
                }
            }
        }

        if result.len() != num_bundles {
            return Err(ProgramError::InvalidAccountData); // Cycle detected
        }

        Ok(result)
    }
}

/// Fee calculator for different bundle types
pub struct FeeCalculator {
    pub base_fee_bps: u16,
    pub bundle_discount_bps: u16,
    pub volume_tiers: Vec<VolumeTier>,
}

#[derive(Debug, Clone)]
pub struct VolumeTier {
    pub min_volume: u64,
    pub discount_bps: u16,
}

impl Default for FeeCalculator {
    fn default() -> Self {
        use crate::constants::{BASE_FEE_BPS, POLYMARKET_FEE_BPS};
        let total_fee_bps = BASE_FEE_BPS + POLYMARKET_FEE_BPS; // 178bp
        let bundle_discount_bps = (total_fee_bps as u32 * 60 / 100) as u16; // 60% discount = 107bp
        
        Self {
            base_fee_bps: total_fee_bps, // 178bp (28bp + 150bp)
            bundle_discount_bps, // 107bp discount (60% off)
            volume_tiers: vec![
                VolumeTier { min_volume: 10_000, discount_bps: 1 },
                VolumeTier { min_volume: 100_000, discount_bps: 2 },
                VolumeTier { min_volume: 1_000_000, discount_bps: 3 },
            ],
        }
    }
}

impl FeeCalculator {
    /// Calculate fees with all discounts applied
    pub fn calculate_total_fee(
        &self,
        amount: u64,
        is_bundled: bool,
        num_markets: u32,
    ) -> u64 {
        let mut fee_bps = self.base_fee_bps;

        // Apply bundle discount
        if is_bundled && num_markets > 1 {
            fee_bps = fee_bps.saturating_sub(self.bundle_discount_bps);
        }

        // Apply volume discount
        for tier in &self.volume_tiers {
            if amount >= tier.min_volume {
                fee_bps = fee_bps.saturating_sub(tier.discount_bps);
            }
        }

        // Calculate final fee
        amount.saturating_mul(fee_bps as u64).saturating_div(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_grouping() {
        let optimizer = BundleOptimizer::default();
        let mut wrapper_manager = WrapperManager::new();

        // Create test wrappers
        let market1 = Pubkey::new_unique();
        let market2 = Pubkey::new_unique();
        let market3 = Pubkey::new_unique();

        let clock = Clock::default();

        // Wrapper 1: markets 1,2
        wrapper_manager.create_verse_wrapper(
            1,
            vec![market1, market2],
            None,
            &clock,
        ).unwrap();

        // Wrapper 2: markets 2,3
        wrapper_manager.create_verse_wrapper(
            2,
            vec![market2, market3],
            None,
            &clock,
        ).unwrap();

        // Wrapper 3: only market 3
        wrapper_manager.create_verse_wrapper(
            3,
            vec![market3],
            None,
            &clock,
        ).unwrap();

        let trades = vec![
            TradeIntent {
                synthetic_id: 1,
                is_buy: true,
                amount: 1000,
                leverage: U64F64::from_num(10),
            },
            TradeIntent {
                synthetic_id: 2,
                is_buy: true,
                amount: 2000,
                leverage: U64F64::from_num(20),
            },
            TradeIntent {
                synthetic_id: 3,
                is_buy: false,
                amount: 500,
                leverage: U64F64::from_num(5),
            },
        ];

        let groups = optimizer.group_by_markets(&trades, &wrapper_manager).unwrap();

        // Should have 2 groups: (1,2) share market2, and (3) separate
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_fee_calculation() {
        let calculator = FeeCalculator::default();

        // Single trade: 0.15%
        let single_fee = calculator.calculate_total_fee(100_000, false, 1);
        assert_eq!(single_fee, 150); // 100,000 * 0.0015

        // Bundled trade: 0.06% (60% discount)
        let bundled_fee = calculator.calculate_total_fee(100_000, true, 3);
        assert_eq!(bundled_fee, 40); // 100,000 * 0.0004 (0.15% - 0.09% - 0.02% volume discount)
    }

    #[test]
    fn test_bundle_optimization() {
        let optimizer = BundleOptimizer::default();
        let mut wrapper_manager = WrapperManager::new();

        let market = Pubkey::new_unique();
        let clock = Clock::default();

        wrapper_manager.create_verse_wrapper(
            1,
            vec![market],
            None,
            &clock,
        ).unwrap();

        let trades = vec![
            TradeIntent {
                synthetic_id: 1,
                is_buy: true,
                amount: 1000,
                leverage: U64F64::from_num(10),
            },
            TradeIntent {
                synthetic_id: 1,
                is_buy: false,
                amount: 500,
                leverage: U64F64::from_num(10),
            },
        ];

        let request = BundleRequest {
            user: Pubkey::new_unique(),
            trades,
            max_slippage: U64F64::from_num(2) / U64F64::from_num(100), // 0.02
        };

        let optimized = optimizer.optimize_bundle(request, &wrapper_manager).unwrap();

        // Should net out to single buy order of 500
        assert_eq!(optimized.bundles.len(), 1);
        assert_eq!(optimized.bundles[0].combined_orders.len(), 1);
        assert_eq!(optimized.bundles[0].combined_orders[0].total_amount, 500);
        assert!(optimized.bundles[0].combined_orders[0].is_buy);
    }
}