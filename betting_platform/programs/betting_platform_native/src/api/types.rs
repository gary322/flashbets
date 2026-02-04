//! API Types and Data Structures
//!
//! Common types used across the REST API and WebSocket interfaces

#[cfg(all(not(target_arch = "bpf"), not(target_os = "solana")))]
pub mod api_types {
    use serde::{Deserialize, Serialize};
    use solana_program::pubkey::Pubkey;
    use crate::math::U64F64;

/// API Response wrapper
#[derive(Serialize, Deserialize, Debug)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
    pub timestamp: i64,
    pub request_id: String,
}

/// API Error structure
#[cfg(all(not(target_arch = "bpf"), not(target_os = "solana")))]
#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
    pub details: Option<String>,
}

/// Market data response
#[cfg(all(not(target_arch = "bpf"), not(target_os = "solana")))]
#[derive(Serialize, Deserialize, Debug)]
pub struct MarketData {
    pub market_id: String,
    pub verse_id: String,
    pub outcomes: Vec<OutcomeData>,
    pub total_volume: u64,
    pub total_liquidity: u64,
    pub last_update: i64,
    pub status: MarketStatus,
}

/// Outcome data
#[cfg(all(not(target_arch = "bpf"), not(target_os = "solana")))]
#[derive(Serialize, Deserialize, Debug)]
pub struct OutcomeData {
    pub outcome_id: u8,
    pub name: String,
    pub price: f64,
    pub volume: u64,
    pub liquidity: u64,
}

/// Market status
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MarketStatus {
    Active,
    Halted,
    Resolved,
    Suspended,
}

/// Order request
#[derive(Serialize, Deserialize, Debug)]
pub struct OrderRequest {
    pub market_id: String,
    pub outcome: u8,
    pub side: OrderSide,
    pub size: u64,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub leverage: Option<u8>,
    pub reduce_only: bool,
    pub post_only: bool,
    pub client_order_id: Option<String>,
}

/// Order side
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    Iceberg,
    Twap,
    Peg,
}

/// Order response
#[derive(Serialize, Deserialize, Debug)]
pub struct OrderResponse {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub status: OrderStatus,
    pub filled_size: u64,
    pub remaining_size: u64,
    pub average_price: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Order status
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Position data
#[derive(Serialize, Deserialize, Debug)]
pub struct PositionData {
    pub position_id: String,
    pub market_id: String,
    pub outcome: u8,
    pub size: u64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub liquidation_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub margin: u64,
    pub leverage: u8,
    pub is_long: bool,
    pub created_at: i64,
}

/// Portfolio summary
#[derive(Serialize, Deserialize, Debug)]
pub struct PortfolioSummary {
    pub total_value: u64,
    pub total_collateral: u64,
    pub total_margin_used: u64,
    pub available_margin: u64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub position_count: u32,
    pub open_orders: u32,
    pub margin_ratio: f64,
    pub liquidation_risk: RiskLevel,
}

/// Risk level
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Greeks data
#[derive(Serialize, Deserialize, Debug)]
pub struct GreeksData {
    pub portfolio_delta: f64,
    pub portfolio_gamma: f64,
    pub portfolio_vega: f64,
    pub portfolio_theta: f64,
    pub portfolio_rho: f64,
    pub last_update: i64,
}

/// Cross margin data
#[derive(Serialize, Deserialize, Debug)]
pub struct CrossMarginData {
    pub mode: String,
    pub gross_margin: u64,
    pub net_margin: u64,
    pub efficiency_improvement: f64,
    pub total_collateral: u64,
    pub available_collateral: u64,
    pub position_count: u32,
}

/// Stress test result
#[derive(Serialize, Deserialize, Debug)]
pub struct StressTestResult {
    pub scenario: String,
    pub initial_value: u64,
    pub stressed_value: i64,
    pub total_pnl: i64,
    pub positions_at_risk: u32,
    pub margin_shortfall: i64,
    pub stressed_var: u64,
    pub risk_score: u16,
    pub health_status: String,
}

/// WebSocket subscription request
#[derive(Serialize, Deserialize, Debug)]
pub struct SubscriptionRequest {
    pub channels: Vec<Channel>,
    pub markets: Option<Vec<String>>,
    pub auth_token: Option<String>,
}

/// WebSocket channel types
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Trades,
    OrderBook,
    Prices,
    Portfolio,
    Orders,
    Positions,
}

/// WebSocket message
#[derive(Serialize, Deserialize, Debug)]
pub struct WebSocketMessage {
    pub channel: Channel,
    pub event: String,
    pub data: serde_json::Value,
    pub timestamp: i64,
    pub sequence: u64,
}

/// Rate limit info
#[derive(Serialize, Deserialize, Debug)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: i64,
    pub retry_after: Option<u32>,
}

/// Pagination parameters
#[derive(Serialize, Deserialize, Debug)]
pub struct PaginationParams {
    pub page: u32,
    pub per_page: u32,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}

/// Sort order
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 100,
            sort_by: None,
            sort_order: None,
        }
    }
}

/// Convert internal types to API types
impl From<crate::state::ProposalState> for MarketStatus {
    fn from(state: crate::state::ProposalState) -> Self {
        match state {
            crate::state::ProposalState::Active => MarketStatus::Active,
            crate::state::ProposalState::Paused => MarketStatus::Suspended,
            crate::state::ProposalState::Resolved => MarketStatus::Resolved,
        }
    }
}

/// Convert U64F64 to f64 for API responses
pub fn fixed_to_float(value: U64F64) -> f64 {
    // Convert fixed point to float
    let integer_part = value.to_num() as f64;
    let fraction_part = value.frac() as f64 / (1u64 << 32) as f64;
    integer_part + fraction_part
}

/// Convert f64 to U64F64 for internal use
pub fn float_to_fixed(value: f64) -> U64F64 {
    U64F64::from_num((value * (1u64 << 32) as f64) as u64) / U64F64::from_num(1u64 << 32)
}

} // end of api_types module