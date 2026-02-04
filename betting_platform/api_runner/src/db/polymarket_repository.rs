//! Polymarket Database Repository
//! Handles all database operations for Polymarket integration

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use deadpool_postgres::{Client, Pool};
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;
use uuid::Uuid;
use rust_decimal::Decimal;
use ethereum_types::{Address, U256};

/// Polymarket order status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Failed,
}

impl From<String> for OrderStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => OrderStatus::Pending,
            "open" => OrderStatus::Open,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "cancelled" => OrderStatus::Cancelled,
            "expired" => OrderStatus::Expired,
            "failed" => OrderStatus::Failed,
            _ => OrderStatus::Pending,
        }
    }
}

impl From<crate::integration::polymarket_clob::OrderStatus> for OrderStatus {
    fn from(status: crate::integration::polymarket_clob::OrderStatus) -> Self {
        match status {
            crate::integration::polymarket_clob::OrderStatus::Pending => OrderStatus::Pending,
            crate::integration::polymarket_clob::OrderStatus::Open => OrderStatus::Open,
            crate::integration::polymarket_clob::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            crate::integration::polymarket_clob::OrderStatus::Filled => OrderStatus::Filled,
            crate::integration::polymarket_clob::OrderStatus::Cancelled => OrderStatus::Cancelled,
            crate::integration::polymarket_clob::OrderStatus::Expired => OrderStatus::Expired,
            crate::integration::polymarket_clob::OrderStatus::Failed => OrderStatus::Failed,
        }
    }
}

/// Polymarket market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketMarket {
    pub id: i64,
    pub internal_market_id: i64,
    pub condition_id: String,
    pub token_id: String,
    pub outcome_prices: Vec<Decimal>,
    pub liquidity: Decimal,
    pub volume_24h: Decimal,
    pub last_price: Option<Decimal>,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    pub resolved: bool,
    pub winning_outcome: Option<i32>,
    pub sync_enabled: bool,
    pub last_sync: Option<DateTime<Utc>>,
}

/// Polymarket order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketOrder {
    pub id: i64,
    pub order_id: String,
    pub order_hash: Option<String>,
    pub wallet_address: String,
    pub market_id: i64,
    pub condition_id: String,
    pub token_id: String,
    pub side: String,
    pub size: Decimal,
    pub price: Decimal,
    pub filled_amount: Decimal,
    pub remaining_amount: Option<Decimal>,
    pub average_fill_price: Option<Decimal>,
    pub status: OrderStatus,
    pub signature: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// CTF position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfPosition {
    pub id: i64,
    pub wallet_address: String,
    pub condition_id: String,
    pub position_id: String,
    pub outcome_index: i32,
    pub balance: Decimal,
    pub locked_balance: Decimal,
    pub average_price: Option<Decimal>,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
}

/// Polymarket repository
pub struct PolymarketRepository {
    pool: Pool,
}

impl PolymarketRepository {
    /// Create new repository
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
    
    /// Get database client from pool
    async fn get_client(&self) -> Result<Client> {
        self.pool.get().await
            .context("Failed to get database client from pool")
    }
    
    // Market Operations
    
    /// Create or update market mapping
    pub async fn upsert_market(
        &self,
        internal_market_id: i64,
        condition_id: &str,
        token_id: &str,
    ) -> Result<i64> {
        let client = self.get_client().await?;
        
        let row = client.query_one(
            r#"
            INSERT INTO polymarket_markets (
                internal_market_id, condition_id, token_id
            ) VALUES ($1, $2, $3)
            ON CONFLICT (condition_id) 
            DO UPDATE SET 
                internal_market_id = EXCLUDED.internal_market_id,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id
            "#,
            &[&internal_market_id, &condition_id, &token_id]
        ).await?;
        
        Ok(row.get(0))
    }
    
    /// Get market by condition ID
    pub async fn get_market_by_condition(&self, condition_id: &str) -> Result<Option<PolymarketMarket>> {
        let client = self.get_client().await?;
        
        let row = client.query_opt(
            r#"
            SELECT * FROM polymarket_markets 
            WHERE condition_id = $1
            "#,
            &[&condition_id]
        ).await?;
        
        Ok(row.map(|r| self.parse_market_row(&r)))
    }
    
    /// Update market data from Polymarket
    pub async fn update_market_data(
        &self,
        condition_id: &str,
        liquidity: Decimal,
        volume_24h: Decimal,
        last_price: Option<Decimal>,
        bid: Option<Decimal>,
        ask: Option<Decimal>,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            UPDATE polymarket_markets SET
                liquidity = $2,
                volume_24h = $3,
                last_price = $4,
                bid = $5,
                ask = $6,
                last_sync = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE condition_id = $1
            "#,
            &[&condition_id, &liquidity, &volume_24h, &last_price, &bid, &ask]
        ).await?;
        
        Ok(())
    }
    
    /// Get markets needing sync
    pub async fn get_markets_for_sync(&self, limit: i64) -> Result<Vec<PolymarketMarket>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT * FROM polymarket_markets 
            WHERE sync_enabled = true 
            AND (last_sync IS NULL OR last_sync < NOW() - INTERVAL '1 minute')
            ORDER BY last_sync ASC NULLS FIRST
            LIMIT $1
            "#,
            &[&limit]
        ).await?;
        
        Ok(rows.iter().map(|r| self.parse_market_row(r)).collect())
    }
    
    // Order Operations
    
    /// Create new order
    pub async fn create_order(
        &self,
        order_id: &str,
        wallet_address: &str,
        market_id: i64,
        condition_id: &str,
        token_id: &str,
        side: &str,
        size: Decimal,
        price: Decimal,
        signature: &str,
    ) -> Result<i64> {
        let client = self.get_client().await?;
        
        let row = client.query_one(
            r#"
            INSERT INTO polymarket_orders (
                order_id, wallet_address, market_id, condition_id, 
                token_id, side, size, price, remaining_amount, 
                status, signature
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', $10)
            RETURNING id
            "#,
            &[&order_id, &wallet_address, &market_id, &condition_id, 
              &token_id, &side, &size, &price, &size, &signature]
        ).await?;
        
        Ok(row.get(0))
    }
    
    /// Update order status
    pub async fn update_order_status(
        &self,
        order_id: &str,
        status: OrderStatus,
        filled_amount: Option<Decimal>,
        average_fill_price: Option<Decimal>,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        let status_str = match status {
            OrderStatus::Pending => "pending",
            OrderStatus::Open => "open",
            OrderStatus::PartiallyFilled => "partially_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Expired => "expired",
            OrderStatus::Failed => "failed",
        };
        
        client.execute(
            r#"
            UPDATE polymarket_orders SET
                status = $2::polymarket_order_status,
                filled_amount = COALESCE($3, filled_amount),
                average_fill_price = COALESCE($4, average_fill_price),
                remaining_amount = size - COALESCE($3, filled_amount),
                updated_at = CURRENT_TIMESTAMP,
                filled_at = CASE 
                    WHEN $2 = 'filled' THEN CURRENT_TIMESTAMP 
                    ELSE filled_at 
                END,
                cancelled_at = CASE 
                    WHEN $2 = 'cancelled' THEN CURRENT_TIMESTAMP 
                    ELSE cancelled_at 
                END
            WHERE order_id = $1
            "#,
            &[&order_id, &status_str, &filled_amount, &average_fill_price]
        ).await?;
        
        Ok(())
    }
    
    /// Get order by ID
    pub async fn get_order(&self, order_id: &str) -> Result<Option<PolymarketOrder>> {
        let client = self.get_client().await?;
        
        let row = client.query_opt(
            r#"
            SELECT * FROM polymarket_orders 
            WHERE order_id = $1
            "#,
            &[&order_id]
        ).await?;
        
        Ok(row.map(|r| self.parse_order_row(&r)))
    }
    
    /// Get user's open orders
    pub async fn get_user_open_orders(&self, wallet_address: &str) -> Result<Vec<PolymarketOrder>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT * FROM polymarket_orders 
            WHERE wallet_address = $1 
            AND status IN ('pending', 'open', 'partially_filled')
            ORDER BY created_at DESC
            "#,
            &[&wallet_address]
        ).await?;
        
        Ok(rows.iter().map(|r| self.parse_order_row(r)).collect())
    }
    
    // CTF Position Operations
    
    /// Upsert CTF position
    pub async fn upsert_ctf_position(
        &self,
        wallet_address: &str,
        condition_id: &str,
        position_id: &str,
        outcome_index: i32,
        balance: Decimal,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            INSERT INTO polymarket_ctf_positions (
                wallet_address, condition_id, position_id, 
                outcome_index, balance
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (wallet_address, position_id) 
            DO UPDATE SET 
                balance = EXCLUDED.balance,
                updated_at = CURRENT_TIMESTAMP
            "#,
            &[&wallet_address, &condition_id, &position_id, &outcome_index, &balance]
        ).await?;
        
        Ok(())
    }
    
    /// Get user CTF positions
    pub async fn get_user_ctf_positions(&self, wallet_address: &str) -> Result<Vec<CtfPosition>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT * FROM polymarket_ctf_positions 
            WHERE wallet_address = $1 AND balance > 0
            ORDER BY balance DESC
            "#,
            &[&wallet_address]
        ).await?;
        
        Ok(rows.iter().map(|r| self.parse_ctf_position_row(r)).collect())
    }
    
    /// Update position P&L
    pub async fn update_position_pnl(
        &self,
        position_id: &str,
        realized_pnl: Decimal,
        unrealized_pnl: Decimal,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            UPDATE polymarket_ctf_positions SET
                realized_pnl = $2,
                unrealized_pnl = $3,
                updated_at = CURRENT_TIMESTAMP
            WHERE position_id = $1
            "#,
            &[&position_id, &realized_pnl, &unrealized_pnl]
        ).await?;
        
        Ok(())
    }
    
    // Trade Operations
    
    /// Record trade
    pub async fn record_trade(
        &self,
        trade_id: &str,
        order_id: &str,
        wallet_address: &str,
        market_id: i64,
        side: &str,
        price: Decimal,
        size: Decimal,
        fee: Decimal,
        executed_at: DateTime<Utc>,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            INSERT INTO polymarket_trades (
                trade_id, order_id, wallet_address, market_id,
                side, price, size, fee, executed_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (trade_id) DO NOTHING
            "#,
            &[&trade_id, &order_id, &wallet_address, &market_id,
              &side, &price, &size, &fee, &executed_at]
        ).await?;
        
        Ok(())
    }
    
    /// Get user trades
    pub async fn get_user_trades(
        &self,
        wallet_address: &str,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT 
                t.*,
                m.condition_id,
                m.token_id
            FROM polymarket_trades t
            JOIN polymarket_markets m ON t.market_id = m.id
            WHERE t.wallet_address = $1
            ORDER BY t.executed_at DESC
            LIMIT $2
            "#,
            &[&wallet_address, &limit]
        ).await?;
        
        Ok(rows.iter().map(|r| {
            serde_json::json!({
                "trade_id": r.get::<_, String>("trade_id"),
                "order_id": r.get::<_, String>("order_id"),
                "condition_id": r.get::<_, String>("condition_id"),
                "side": r.get::<_, String>("side"),
                "price": r.get::<_, Decimal>("price").to_string(),
                "size": r.get::<_, Decimal>("size").to_string(),
                "fee": r.get::<_, Decimal>("fee").to_string(),
                "executed_at": r.get::<_, DateTime<Utc>>("executed_at"),
            })
        }).collect())
    }
    
    // Balance Operations
    
    /// Update user balance
    pub async fn update_balance(
        &self,
        wallet_address: &str,
        token_type: &str,
        token_address: &str,
        balance: Decimal,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            INSERT INTO polymarket_balances (
                wallet_address, token_type, token_address, balance
            ) VALUES ($1, $2::polymarket_token_type, $3, $4)
            ON CONFLICT (wallet_address, token_address, chain) 
            DO UPDATE SET 
                balance = EXCLUDED.balance,
                last_updated = CURRENT_TIMESTAMP
            "#,
            &[&wallet_address, &token_type, &token_address, &balance]
        ).await?;
        
        Ok(())
    }
    
    /// Get user balances
    pub async fn get_user_balances(&self, wallet_address: &str) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT * FROM polymarket_balances 
            WHERE wallet_address = $1
            ORDER BY balance DESC
            "#,
            &[&wallet_address]
        ).await?;
        
        Ok(rows.iter().map(|r| {
            serde_json::json!({
                "token_type": r.get::<_, String>("token_type"),
                "token_address": r.get::<_, String>("token_address"),
                "balance": r.get::<_, Decimal>("balance").to_string(),
                "locked_balance": r.get::<_, Decimal>("locked_balance").to_string(),
                "last_updated": r.get::<_, DateTime<Utc>>("last_updated"),
            })
        }).collect())
    }
    
    // Order Book Operations
    
    /// Save order book snapshot
    pub async fn save_orderbook_snapshot(
        &self,
        market_id: i64,
        condition_id: &str,
        token_id: &str,
        bids: serde_json::Value,
        asks: serde_json::Value,
        mid_price: Option<Decimal>,
        spread: Option<Decimal>,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            INSERT INTO polymarket_orderbook_snapshots (
                market_id, condition_id, token_id, bids, asks,
                mid_price, spread
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            &[&market_id, &condition_id, &token_id, &bids, &asks, &mid_price, &spread]
        ).await?;
        
        Ok(())
    }
    
    /// Record price history
    pub async fn record_price(
        &self,
        market_id: i64,
        condition_id: &str,
        token_id: &str,
        outcome_index: Option<i32>,
        price: Decimal,
        volume: Option<Decimal>,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            INSERT INTO polymarket_price_history (
                market_id, condition_id, token_id, outcome_index,
                price, volume, timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            &[&market_id, &condition_id, &token_id, &outcome_index, 
              &price, &volume, &timestamp]
        ).await?;
        
        Ok(())
    }
    
    /// Get price history
    pub async fn get_price_history(
        &self,
        condition_id: &str,
        hours: i32,
    ) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT 
                outcome_index,
                price,
                volume,
                timestamp
            FROM polymarket_price_history
            WHERE condition_id = $1
            AND timestamp > NOW() - INTERVAL '%s hours'
            ORDER BY timestamp ASC
            "#,
            &[&condition_id, &hours]
        ).await?;
        
        Ok(rows.iter().map(|r| {
            serde_json::json!({
                "outcome_index": r.get::<_, Option<i32>>("outcome_index"),
                "price": r.get::<_, Decimal>("price").to_string(),
                "volume": r.get::<_, Option<Decimal>>("volume").map(|v| v.to_string()),
                "timestamp": r.get::<_, DateTime<Utc>>("timestamp"),
            })
        }).collect())
    }
    
    // WebSocket Event Operations
    
    /// Log WebSocket event
    pub async fn log_ws_event(
        &self,
        event_type: &str,
        channel: Option<&str>,
        data: serde_json::Value,
    ) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            INSERT INTO polymarket_ws_events (
                event_type, channel, data
            ) VALUES ($1, $2, $3)
            "#,
            &[&event_type, &channel, &data]
        ).await?;
        
        Ok(())
    }
    
    /// Get unprocessed WebSocket events
    pub async fn get_unprocessed_ws_events(&self, limit: i64) -> Result<Vec<serde_json::Value>> {
        let client = self.get_client().await?;
        
        let rows = client.query(
            r#"
            SELECT * FROM polymarket_ws_events
            WHERE processed = false
            ORDER BY received_at ASC
            LIMIT $1
            "#,
            &[&limit]
        ).await?;
        
        Ok(rows.iter().map(|r| {
            serde_json::json!({
                "id": r.get::<_, i64>("id"),
                "event_type": r.get::<_, String>("event_type"),
                "channel": r.get::<_, Option<String>>("channel"),
                "data": r.get::<_, serde_json::Value>("data"),
                "received_at": r.get::<_, DateTime<Utc>>("received_at"),
            })
        }).collect())
    }
    
    /// Mark WebSocket event as processed
    pub async fn mark_ws_event_processed(&self, event_id: i64) -> Result<()> {
        let client = self.get_client().await?;
        
        client.execute(
            r#"
            UPDATE polymarket_ws_events
            SET processed = true, processed_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            &[&event_id]
        ).await?;
        
        Ok(())
    }
    
    // Helper methods
    
    fn parse_market_row(&self, row: &Row) -> PolymarketMarket {
        PolymarketMarket {
            id: row.get("id"),
            internal_market_id: row.get("internal_market_id"),
            condition_id: row.get("condition_id"),
            token_id: row.get("token_id"),
            outcome_prices: row.get::<_, Option<serde_json::Value>>("outcome_prices")
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            liquidity: row.get("liquidity"),
            volume_24h: row.get("volume_24h"),
            last_price: row.get("last_price"),
            bid: row.get("bid"),
            ask: row.get("ask"),
            resolved: row.get("resolved"),
            winning_outcome: row.get("winning_outcome"),
            sync_enabled: row.get("sync_enabled"),
            last_sync: row.get("last_sync"),
        }
    }
    
    fn parse_order_row(&self, row: &Row) -> PolymarketOrder {
        PolymarketOrder {
            id: row.get("id"),
            order_id: row.get("order_id"),
            order_hash: row.get("order_hash"),
            wallet_address: row.get("wallet_address"),
            market_id: row.get("market_id"),
            condition_id: row.get("condition_id"),
            token_id: row.get("token_id"),
            side: row.get("side"),
            size: row.get("size"),
            price: row.get("price"),
            filled_amount: row.get("filled_amount"),
            remaining_amount: row.get("remaining_amount"),
            average_fill_price: row.get("average_fill_price"),
            status: row.get::<_, String>("status").into(),
            signature: row.get("signature"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }
    
    fn parse_ctf_position_row(&self, row: &Row) -> CtfPosition {
        CtfPosition {
            id: row.get("id"),
            wallet_address: row.get("wallet_address"),
            condition_id: row.get("condition_id"),
            position_id: row.get("position_id"),
            outcome_index: row.get("outcome_index"),
            balance: row.get("balance"),
            locked_balance: row.get("locked_balance"),
            average_price: row.get("average_price"),
            realized_pnl: row.get("realized_pnl"),
            unrealized_pnl: row.get("unrealized_pnl"),
        }
    }
}

// Statistics and Analytics

impl PolymarketRepository {
    /// Get market statistics
    pub async fn get_market_stats(&self, market_id: i64) -> Result<serde_json::Value> {
        let client = self.get_client().await?;
        
        let row = client.query_one(
            r#"
            SELECT 
                COUNT(DISTINCT o.wallet_address) as unique_traders,
                COUNT(o.id) as total_orders,
                SUM(CASE WHEN o.status = 'filled' THEN 1 ELSE 0 END) as filled_orders,
                AVG(o.filled_amount) as avg_order_size,
                MAX(o.created_at) as last_order_time
            FROM polymarket_orders o
            WHERE o.market_id = $1
            "#,
            &[&market_id]
        ).await?;
        
        Ok(serde_json::json!({
            "unique_traders": row.get::<_, i64>("unique_traders"),
            "total_orders": row.get::<_, i64>("total_orders"),
            "filled_orders": row.get::<_, i64>("filled_orders"),
            "avg_order_size": row.get::<_, Option<Decimal>>("avg_order_size").map(|d| d.to_string()),
            "last_order_time": row.get::<_, Option<DateTime<Utc>>>("last_order_time"),
        }))
    }
    
    /// Get user statistics
    pub async fn get_user_stats(&self, wallet_address: &str) -> Result<serde_json::Value> {
        let client = self.get_client().await?;
        
        let row = client.query_one(
            r#"
            SELECT 
                COUNT(DISTINCT market_id) as markets_traded,
                COUNT(id) as total_orders,
                SUM(CASE WHEN status = 'filled' THEN 1 ELSE 0 END) as filled_orders,
                SUM(filled_amount) as total_volume,
                COUNT(DISTINCT DATE(created_at)) as active_days
            FROM polymarket_orders
            WHERE wallet_address = $1
            "#,
            &[&wallet_address]
        ).await?;
        
        Ok(serde_json::json!({
            "markets_traded": row.get::<_, i64>("markets_traded"),
            "total_orders": row.get::<_, i64>("total_orders"),
            "filled_orders": row.get::<_, i64>("filled_orders"),
            "total_volume": row.get::<_, Option<Decimal>>("total_volume").map(|d| d.to_string()),
            "active_days": row.get::<_, i64>("active_days"),
        }))
    }
}