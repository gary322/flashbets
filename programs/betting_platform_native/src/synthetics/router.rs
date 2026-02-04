use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::HashMap;
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::synthetics::{SyntheticWrapper, WrapperManager};

/// External Polymarket client interface (to be implemented separately)
pub trait PolymarketClient {
    fn place_order(&self, request: OrderRequest) -> Result<OrderResponse, ProgramError>;
}

#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub market_id: String,
    pub side: String, // "buy" or "sell"
    pub amount: u64,
    pub price: Option<f64>,
    pub leverage: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct OrderResponse {
    pub order_id: Pubkey,
    pub fee: u64,
}

pub struct RoutingEngine {
    pub polymarket_client: Box<dyn PolymarketClient>,
    pub wrapper_manager: WrapperManager,
}

#[derive(Debug, Clone)]
pub struct RouteRequest {
    pub synthetic_id: u128,
    pub is_buy: bool,
    pub amount: u64,
    pub leverage: U64F64,
    pub user: Pubkey,
}

#[derive(Debug, Clone)]
pub struct RouteResponse {
    pub orders: Vec<PolymarketOrder>,
    pub total_fee: u64,
    pub saved_fee: u64,
    pub execution_receipt: ExecutionReceipt,
}

#[derive(Debug, Clone)]
pub struct PolymarketOrder {
    pub market_id: Pubkey,
    pub amount: u64,
    pub expected_price: U64F64,
    pub weight: U64F64,
}

/// Execution receipt account structure
pub struct ExecutionReceipt {
    pub synthetic_id: u128,
    pub user: Pubkey,
    pub timestamp: i64,
    pub polymarket_orders: Vec<Pubkey>, // Order IDs from Polymarket
    pub signatures: Vec<[u8; 64]>,       // Polymarket signatures
    pub total_executed: u64,
    pub average_price: U64F64,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum ExecutionStatus {
    Pending = 0,
    PartialFill = 1,
    Complete = 2,
    Failed = 3,
}

impl ExecutionStatus {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(ExecutionStatus::Pending),
            1 => Some(ExecutionStatus::PartialFill),
            2 => Some(ExecutionStatus::Complete),
            3 => Some(ExecutionStatus::Failed),
            _ => None,
        }
    }
}

impl RoutingEngine {
    pub fn new(polymarket_client: Box<dyn PolymarketClient>, wrapper_manager: WrapperManager) -> Self {
        Self {
            polymarket_client,
            wrapper_manager,
        }
    }

    pub fn route_synthetic_trade(
        &self,
        request: RouteRequest,
    ) -> Result<RouteResponse, ProgramError> {
        // Get wrapper
        let wrapper = self.wrapper_manager.wrappers
            .get(&request.synthetic_id)
            .ok_or(BettingPlatformError::WrapperNotFound)?;

        if wrapper.status != crate::synthetics::WrapperStatus::Active {
            return Err(BettingPlatformError::WrapperNotActive.into());
        }

        // Calculate distribution across markets
        let orders = self.calculate_order_distribution(
            &wrapper,
            request.amount,
            request.leverage,
        )?;

        // Execute via Polymarket API
        let mut executed_orders = Vec::new();
        let mut total_fee = 0u64;

        for order in &orders {
            let polymarket_request = OrderRequest {
                market_id: order.market_id.to_string(),
                side: if request.is_buy { "buy".to_string() } else { "sell".to_string() },
                amount: order.amount,
                price: Some(order.expected_price.to_num() as f64),
                leverage: Some(request.leverage.to_num() as f64),
            };

            let response = self.polymarket_client
                .place_order(polymarket_request)?;

            executed_orders.push(response.order_id);
            total_fee += response.fee;
        }

        // Calculate fee savings
        let individual_fee = orders.len() as u64 * self.calculate_base_fee(request.amount);
        let saved_fee = individual_fee.saturating_sub(total_fee);

        // Create execution receipt
        let receipt = ExecutionReceipt {
            synthetic_id: request.synthetic_id,
            user: request.user,
            timestamp: Clock::get()?.unix_timestamp,
            polymarket_orders: executed_orders,
            signatures: vec![], // Will be populated by keeper
            total_executed: request.amount,
            average_price: self.calculate_weighted_price(&orders)?,
            status: ExecutionStatus::Pending,
        };

        Ok(RouteResponse {
            orders,
            total_fee,
            saved_fee,
            execution_receipt: receipt,
        })
    }

    fn calculate_order_distribution(
        &self,
        wrapper: &SyntheticWrapper,
        amount: u64,
        leverage: U64F64,
    ) -> Result<Vec<PolymarketOrder>, ProgramError> {
        let mut orders = Vec::new();

        for (i, market) in wrapper.polymarket_markets.iter().enumerate() {
            let weight = wrapper.weights[i];
            let market_amount = U64F64::from_num(amount).checked_mul(weight)?;

            orders.push(PolymarketOrder {
                market_id: *market,
                amount: market_amount.to_num(),
                expected_price: wrapper.derived_probability,
                weight,
            });
        }

        Ok(orders)
    }

    fn calculate_base_fee(&self, amount: u64) -> u64 {
        // Base fee: 0.15% of amount (15 basis points)
        amount.saturating_mul(15).saturating_div(10_000)
    }

    fn calculate_weighted_price(&self, orders: &[PolymarketOrder]) -> Result<U64F64, ProgramError> {
        let mut weighted_sum = U64F64::from_num(0);
        let mut total_weight = U64F64::from_num(0);

        for order in orders {
            let weighted_price = order.expected_price.checked_mul(order.weight)?;
            weighted_sum = weighted_sum.checked_add(weighted_price)?;
            total_weight = total_weight.checked_add(order.weight)?;
        }

        if total_weight.is_zero() {
            return Ok(U64F64::from_num(0));
        }

        weighted_sum.checked_div(total_weight)
    }
}

/// Routing configuration for different strategies
#[derive(Debug, Clone, Copy)]
pub enum RoutingStrategy {
    ProportionalVolume,   // Route based on volume weights
    BestPrice,           // Route to best price first
    MinimalSlippage,     // Optimize for lowest slippage
    BalancedLiquidity,   // Balance across liquidity sources
}

pub struct RoutingConfig {
    pub max_slippage_bps: u16, // Max allowed slippage
    pub routing_strategy: RoutingStrategy,
    pub batch_size: u32, // For multi-market orders
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: 200, // 2%
            routing_strategy: RoutingStrategy::ProportionalVolume,
            batch_size: 10,
        }
    }
}

/// Advanced routing optimizer
pub struct RoutingOptimizer {
    pub config: RoutingConfig,
}

impl RoutingOptimizer {
    pub fn new(config: RoutingConfig) -> Self {
        Self { config }
    }

    pub fn optimize_routing(
        &self,
        wrapper: &SyntheticWrapper,
        amount: u64,
    ) -> Result<Vec<PolymarketOrder>, ProgramError> {
        match self.config.routing_strategy {
            RoutingStrategy::ProportionalVolume => self.route_proportional_volume(wrapper, amount),
            RoutingStrategy::BestPrice => self.route_best_price(wrapper, amount),
            RoutingStrategy::MinimalSlippage => self.route_minimal_slippage(wrapper, amount),
            RoutingStrategy::BalancedLiquidity => self.route_balanced_liquidity(wrapper, amount),
        }
    }

    fn route_proportional_volume(
        &self,
        wrapper: &SyntheticWrapper,
        amount: u64,
    ) -> Result<Vec<PolymarketOrder>, ProgramError> {
        let mut orders = Vec::new();
        
        for (i, market) in wrapper.polymarket_markets.iter().enumerate() {
            let weight = wrapper.weights[i];
            let market_amount = U64F64::from_num(amount).checked_mul(weight)?;

            orders.push(PolymarketOrder {
                market_id: *market,
                amount: market_amount.to_num(),
                expected_price: wrapper.derived_probability,
                weight,
            });
        }

        Ok(orders)
    }

    fn route_best_price(
        &self,
        wrapper: &SyntheticWrapper,
        amount: u64,
    ) -> Result<Vec<PolymarketOrder>, ProgramError> {
        // TODO: Implement best price routing based on real-time price data
        // For now, fallback to proportional
        self.route_proportional_volume(wrapper, amount)
    }

    fn route_minimal_slippage(
        &self,
        wrapper: &SyntheticWrapper,
        amount: u64,
    ) -> Result<Vec<PolymarketOrder>, ProgramError> {
        // TODO: Implement slippage optimization
        // For now, fallback to proportional
        self.route_proportional_volume(wrapper, amount)
    }

    fn route_balanced_liquidity(
        &self,
        wrapper: &SyntheticWrapper,
        amount: u64,
    ) -> Result<Vec<PolymarketOrder>, ProgramError> {
        // TODO: Implement liquidity-based routing
        // For now, fallback to proportional
        self.route_proportional_volume(wrapper, amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetics::{SyntheticType, WrapperStatus};
    use solana_program::clock::Clock;

    struct MockPolymarketClient;

    impl PolymarketClient for MockPolymarketClient {
        fn place_order(&self, _request: OrderRequest) -> Result<OrderResponse, ProgramError> {
            Ok(OrderResponse {
                order_id: Pubkey::new_unique(),
                fee: 100, // Mock fee
            })
        }
    }

    #[test]
    fn test_order_distribution() {
        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 1,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(0.6), U64F64::from_num(0.4)],
            derived_probability: U64F64::from_num(0.75),
            total_volume_7d: 1000000,
            last_update_slot: 100,
            status: WrapperStatus::Active,
            bump: 1,
        };

        let engine = RoutingEngine::new(
            Box::new(MockPolymarketClient),
            WrapperManager::new(),
        );

        let orders = engine.calculate_order_distribution(
            &wrapper,
            1000,
            U64F64::from_num(10),
        ).unwrap();

        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].amount, 600);
        assert_eq!(orders[1].amount, 400);
    }

    #[test]
    fn test_routing_optimizer() {
        let config = RoutingConfig::default();
        let optimizer = RoutingOptimizer::new(config);

        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 1,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(0.5), U64F64::from_num(0.5)],
            derived_probability: U64F64::from_num(0.5),
            total_volume_7d: 1000000,
            last_update_slot: 100,
            status: WrapperStatus::Active,
            bump: 1,
        };

        let orders = optimizer.optimize_routing(&wrapper, 1000).unwrap();

        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].amount, 500);
        assert_eq!(orders[1].amount, 500);
    }
}