//! Type definitions for the API

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use validator::Validate;

/// Market structure
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Market {
    pub id: u128,
    pub title: String,
    pub description: String,
    #[serde(serialize_with = "crate::serialization::serialize_pubkey", deserialize_with = "crate::serialization::deserialize_pubkey")]
    pub creator: Pubkey,
    pub outcomes: Vec<MarketOutcome>,
    pub amm_type: AmmType,
    pub total_liquidity: u64,
    pub total_volume: u64,
    pub resolution_time: i64,
    pub resolved: bool,
    pub winning_outcome: Option<u8>,
    pub created_at: i64,
    pub verse_id: Option<u128>,
    pub current_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MarketOutcome {
    pub id: u8,
    pub name: String,
    pub title: String,
    pub description: String,
    pub total_stake: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum AmmType {
    Lmsr,
    PmAmm,
    L2Amm,
    Hybrid,
    Cpmm,
}

/// Market status
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum MarketStatus {
    Active,
    Paused,
    Resolved,
    Cancelled,
}

/// Market type
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum MarketType {
    Binary,
    Multiple,
    Scalar,
}

/// Position structure
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Position {
    #[serde(serialize_with = "crate::serialization::serialize_pubkey", deserialize_with = "crate::serialization::deserialize_pubkey")]
    pub owner: Pubkey,
    pub market_id: u128,
    pub outcome: u8,
    pub size: u64,
    pub leverage: u32,
    pub entry_price: u64,
    pub liquidation_price: u64,
    pub is_long: bool,
    pub collateral: u64,
    pub created_at: i64,
}

/// Demo account
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct DemoAccount {
    #[serde(serialize_with = "crate::serialization::serialize_pubkey", deserialize_with = "crate::serialization::deserialize_pubkey")]
    pub owner: Pubkey,
    pub balance: u64,
    pub positions_opened: u64,
    pub positions_closed: u64,
    pub total_volume: u64,
    pub total_pnl: i64,
    pub created_at: i64,
}

/// Verse structure
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Verse {
    pub id: u128,
    pub name: String,
    pub parent_id: Option<u128>,
    pub level: u8,
    pub multiplier: u64,
    pub total_liquidity: u64,
    pub active_markets: u32,
}

/// Balance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub sol: u64,
    pub demo_usdc: u64,
    pub mmt: u64,
}

/// Trade request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TradeRequest {
    #[validate(range(min = 1))]
    pub market_id: u128,
    #[validate(range(min = 1))]
    pub amount: u64,
    pub outcome: u8,
    #[validate(range(min = 1, max = 500))]
    pub leverage: u32,
    pub order_type: Option<OrderType>,
    pub limit_price: Option<f64>,
    pub stop_loss: Option<f64>,
}

/// Order type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    MarketUpdate {
        market_id: u128,
        yes_price: f64,
        no_price: f64,
        volume: u64,
    },
    PositionUpdate {
        position_id: String,
        pnl: f64,
        current_price: f64,
    },
    Notification {
        title: String,
        message: String,
        level: String,
    },
}

/// Enhanced market data with verses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketWithVerses {
    pub id: String,
    pub title: String,
    pub category: String,
    pub outcomes: Vec<String>,
    pub outcome_prices: Vec<f64>,
    pub volume_24hr: f64,
    pub liquidity: f64,
    pub verses: Vec<crate::verse_generator::GeneratedVerse>,
}

/// Program state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramState {
    pub admin: Pubkey,
    pub total_markets: u64,
    pub total_volume: u64,
    pub protocol_fee_rate: u16,
    pub min_bet_amount: u64,
    pub max_bet_amount: u64,
    pub emergency_mode: bool,
}

/// Betting instruction enum (matching on-chain)
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub enum BettingInstruction {
    CreateMarket {
        market_id: u128,
        verse_id: u128,
        title: String,
        description: String,
        outcomes: Vec<String>,
        amm_type: u8,
        settle_time: i64,
        initial_liquidity: u64,
    },
    PlaceBet {
        market_id: u128,
        amount: u64,
        outcome: u8,
        leverage: u32,
    },
    CreateDemoAccount,
    ClosePosition {
        position_index: u8,
    },
    ProcessQuantumSettlement {
        position_id: String,
        payout_amount: u64,
    },
    // Add other instructions as needed
}


/// Position status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PositionStatus {
    Open,
    Closed,
    Liquidated,
}

/// Position with enhanced data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub position: Pubkey,
    pub market_id: u128,
    pub amount: u64,
    pub outcome: u8,
    pub leverage: u32,
    pub entry_price: f64,
    pub current_price: f64,
    pub pnl: i128,
    pub status: PositionStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

/// API Request types
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateMarketRequest {
    #[validate(length(min = 1))]
    pub question: String,
    #[validate(length(min = 2, max = 10))]
    pub outcomes: Vec<String>,
    pub end_time: i64,
    pub market_type: Option<MarketType>,
    #[validate(range(max = 10000))] // Max 100% in basis points
    pub fee_rate: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PlaceTradeRequest {
    #[validate(range(min = 1))]
    pub market_id: u128,
    #[validate(range(min = 1))]
    pub amount: u64,
    pub outcome: u8,
    pub leverage: Option<u32>,
    pub order_type: Option<OrderType>,
    #[validate(length(min = 32, max = 44))] // Solana pubkey length
    pub wallet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosePositionRequest {
    pub position_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDemoAccountRequest {
    pub initial_balance: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuantumPositionRequest {
    pub states: Vec<QuantumState>,
    pub entanglement_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumState {
    pub market_id: u128,
    pub outcome: u8,
    pub amount: u64,
    pub leverage: u32,
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeMMTRequest {
    pub amount: u64,
    pub duration: i64,
}