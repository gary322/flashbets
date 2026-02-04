//! Database models and entities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

/// User account stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbUser {
    pub id: i64,
    pub wallet_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub total_volume: i64,
    pub total_trades: i32,
    pub is_active: bool,
}

impl From<Row> for DbUser {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            wallet_address: row.get("wallet_address"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            last_login: row.get("last_login"),
            total_volume: row.get("total_volume"),
            total_trades: row.get("total_trades"),
            is_active: row.get("is_active"),
        }
    }
}

/// Market stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbMarket {
    pub id: i64,
    pub market_id: String,
    pub chain: String,
    pub title: String,
    pub description: String,
    pub creator: String,
    pub created_at: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub resolved: bool,
    pub winning_outcome: Option<i16>,
    pub total_volume: i64,
    pub total_liquidity: i64,
    pub metadata: serde_json::Value,
}

impl From<Row> for DbMarket {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            market_id: row.get("market_id"),
            chain: row.get("chain"),
            title: row.get("title"),
            description: row.get("description"),
            creator: row.get("creator"),
            created_at: row.get("created_at"),
            end_time: row.get("end_time"),
            resolved: row.get("resolved"),
            winning_outcome: row.get("winning_outcome"),
            total_volume: row.get("total_volume"),
            total_liquidity: row.get("total_liquidity"),
            metadata: row.get("metadata"),
        }
    }
}

/// Position stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPosition {
    pub id: i64,
    pub position_id: String,
    pub user_id: i64,
    pub market_id: i64,
    pub outcome: i16,
    pub amount: i64,
    pub leverage: i16,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub pnl: Option<i64>,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

impl From<Row> for DbPosition {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            position_id: row.get("position_id"),
            user_id: row.get("user_id"),
            market_id: row.get("market_id"),
            outcome: row.get("outcome"),
            amount: row.get("amount"),
            leverage: row.get("leverage"),
            entry_price: row.get("entry_price"),
            exit_price: row.get("exit_price"),
            pnl: row.get("pnl"),
            status: row.get("status"),
            opened_at: row.get("opened_at"),
            closed_at: row.get("closed_at"),
            metadata: row.get("metadata"),
        }
    }
}

/// Trade/Transaction stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTrade {
    pub id: i64,
    pub trade_id: String,
    pub user_id: i64,
    pub market_id: i64,
    pub position_id: Option<i64>,
    pub trade_type: String,
    pub outcome: i16,
    pub amount: i64,
    pub price: f64,
    pub fee: i64,
    pub signature: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

impl From<Row> for DbTrade {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            trade_id: row.get("trade_id"),
            user_id: row.get("user_id"),
            market_id: row.get("market_id"),
            position_id: row.get("position_id"),
            trade_type: row.get("trade_type"),
            outcome: row.get("outcome"),
            amount: row.get("amount"),
            price: row.get("price"),
            fee: row.get("fee"),
            signature: row.get("signature"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            confirmed_at: row.get("confirmed_at"),
            metadata: row.get("metadata"),
        }
    }
}

/// Settlement record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSettlement {
    pub id: i64,
    pub market_id: i64,
    pub settled_at: DateTime<Utc>,
    pub winning_outcome: i16,
    pub settlement_price: f64,
    pub total_payout: i64,
    pub oracle_source: String,
    pub oracle_data: serde_json::Value,
    pub signature: String,
}

impl From<Row> for DbSettlement {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            market_id: row.get("market_id"),
            settled_at: row.get("settled_at"),
            winning_outcome: row.get("winning_outcome"),
            settlement_price: row.get("settlement_price"),
            total_payout: row.get("total_payout"),
            oracle_source: row.get("oracle_source"),
            oracle_data: row.get("oracle_data"),
            signature: row.get("signature"),
        }
    }
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbAuditLog {
    pub id: i64,
    pub user_id: Option<i64>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: String,
    pub changes: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl From<Row> for DbAuditLog {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            user_id: row.get("user_id"),
            action: row.get("action"),
            entity_type: row.get("entity_type"),
            entity_id: row.get("entity_id"),
            ip_address: row.get("ip_address"),
            user_agent: row.get("user_agent"),
            request_id: row.get("request_id"),
            changes: row.get("changes"),
            created_at: row.get("created_at"),
        }
    }
}

/// API key for programmatic access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbApiKey {
    pub id: i64,
    pub user_id: i64,
    pub key_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub rate_limit: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
}

impl From<Row> for DbApiKey {
    fn from(row: Row) -> Self {
        Self {
            id: row.get("id"),
            user_id: row.get("user_id"),
            key_hash: row.get("key_hash"),
            name: row.get("name"),
            permissions: row.get("permissions"),
            rate_limit: row.get("rate_limit"),
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
            created_at: row.get("created_at"),
            is_active: row.get("is_active"),
        }
    }
}