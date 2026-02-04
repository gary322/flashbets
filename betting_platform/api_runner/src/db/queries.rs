//! Database queries and operations

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use deadpool_postgres::Object;
use crate::db::models::*;

/// User queries
pub struct UserQueries;

impl UserQueries {
    /// Create a new user
    pub async fn create(conn: &Object, wallet_address: &str) -> Result<DbUser> {
        let row = conn.query_one(
            r#"
            INSERT INTO users (wallet_address)
            VALUES ($1)
            ON CONFLICT (wallet_address) DO UPDATE
            SET last_login = NOW(), updated_at = NOW()
            RETURNING *
            "#,
            &[&wallet_address],
        ).await.context("Failed to create user")?;
        
        Ok(DbUser::from(row))
    }
    
    /// Get user by wallet address
    pub async fn get_by_wallet(conn: &Object, wallet_address: &str) -> Result<Option<DbUser>> {
        let row = conn.query_opt(
            "SELECT * FROM users WHERE wallet_address = $1",
            &[&wallet_address],
        ).await.context("Failed to get user")?;
        
        Ok(row.map(DbUser::from))
    }
    
    /// Update user stats
    pub async fn update_stats(
        conn: &Object,
        user_id: i64,
        volume_delta: i64,
        trades_delta: i32,
    ) -> Result<()> {
        conn.execute(
            r#"
            UPDATE users
            SET total_volume = total_volume + $2,
                total_trades = total_trades + $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
            &[&user_id, &volume_delta, &trades_delta],
        ).await.context("Failed to update user stats")?;
        
        Ok(())
    }
}

/// Market queries
pub struct MarketQueries;

impl MarketQueries {
    /// Create or update a market
    pub async fn upsert(
        conn: &Object,
        market_id: &str,
        chain: &str,
        title: &str,
        description: &str,
        creator: &str,
        end_time: DateTime<Utc>,
        metadata: serde_json::Value,
    ) -> Result<DbMarket> {
        let row = conn.query_one(
            r#"
            INSERT INTO markets (market_id, chain, title, description, creator, end_time, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (market_id) DO UPDATE
            SET title = EXCLUDED.title,
                description = EXCLUDED.description,
                metadata = EXCLUDED.metadata
            RETURNING *
            "#,
            &[&market_id, &chain, &title, &description, &creator, &end_time, &metadata],
        ).await.context("Failed to upsert market")?;
        
        Ok(DbMarket::from(row))
    }
    
    /// Get market by market_id
    pub async fn get_by_market_id(conn: &Object, market_id: &str) -> Result<Option<DbMarket>> {
        let row = conn.query_opt(
            "SELECT * FROM markets WHERE market_id = $1",
            &[&market_id],
        ).await.context("Failed to get market")?;
        
        Ok(row.map(DbMarket::from))
    }
    
    /// Get active markets
    pub async fn get_active(conn: &Object, limit: i64) -> Result<Vec<DbMarket>> {
        let rows = conn.query(
            r#"
            SELECT * FROM markets
            WHERE resolved = false AND end_time > NOW()
            ORDER BY total_volume DESC
            LIMIT $1
            "#,
            &[&limit],
        ).await.context("Failed to get active markets")?;
        
        Ok(rows.into_iter().map(DbMarket::from).collect())
    }
    
    /// Update market settlement
    pub async fn settle(
        conn: &Object,
        market_id: i64,
        winning_outcome: i16,
    ) -> Result<()> {
        conn.execute(
            r#"
            UPDATE markets
            SET resolved = true,
                winning_outcome = $2
            WHERE id = $1
            "#,
            &[&market_id, &winning_outcome],
        ).await.context("Failed to settle market")?;
        
        Ok(())
    }
}

/// Position queries
pub struct PositionQueries;

impl PositionQueries {
    /// Create a new position
    pub async fn create(
        conn: &Object,
        position_id: &str,
        user_id: i64,
        market_id: i64,
        outcome: i16,
        amount: i64,
        leverage: i16,
        entry_price: f64,
    ) -> Result<DbPosition> {
        let row = conn.query_one(
            r#"
            INSERT INTO positions (
                position_id, user_id, market_id, outcome, 
                amount, leverage, entry_price, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'open')
            RETURNING *
            "#,
            &[&position_id, &user_id, &market_id, &outcome, 
              &amount, &leverage, &entry_price],
        ).await.context("Failed to create position")?;
        
        Ok(DbPosition::from(row))
    }
    
    /// Get user positions
    pub async fn get_by_user(
        conn: &Object,
        user_id: i64,
        status: Option<&str>,
    ) -> Result<Vec<DbPosition>> {
        let query = if let Some(status) = status {
            conn.query(
                r#"
                SELECT * FROM positions
                WHERE user_id = $1 AND status = $2
                ORDER BY opened_at DESC
                "#,
                &[&user_id, &status],
            ).await
        } else {
            conn.query(
                r#"
                SELECT * FROM positions
                WHERE user_id = $1
                ORDER BY opened_at DESC
                "#,
                &[&user_id],
            ).await
        }.context("Failed to get user positions")?;
        
        Ok(query.into_iter().map(DbPosition::from).collect())
    }
    
    /// Close a position
    pub async fn close(
        conn: &Object,
        position_id: &str,
        exit_price: f64,
        pnl: i64,
    ) -> Result<()> {
        conn.execute(
            r#"
            UPDATE positions
            SET status = 'closed',
                exit_price = $2,
                pnl = $3,
                closed_at = NOW()
            WHERE position_id = $1
            "#,
            &[&position_id, &exit_price, &pnl],
        ).await.context("Failed to close position")?;
        
        Ok(())
    }
}

/// Trade queries
pub struct TradeQueries;

impl TradeQueries {
    /// Record a trade
    pub async fn create(
        conn: &Object,
        trade_id: &str,
        user_id: i64,
        market_id: i64,
        position_id: Option<i64>,
        trade_type: &str,
        outcome: i16,
        amount: i64,
        price: f64,
        fee: i64,
        signature: &str,
    ) -> Result<DbTrade> {
        let row = conn.query_one(
            r#"
            INSERT INTO trades (
                trade_id, user_id, market_id, position_id,
                trade_type, outcome, amount, price, fee,
                signature, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'pending')
            RETURNING *
            "#,
            &[&trade_id, &user_id, &market_id, &position_id,
              &trade_type, &outcome, &amount, &price, &fee, &signature],
        ).await.context("Failed to create trade")?;
        
        Ok(DbTrade::from(row))
    }
    
    /// Confirm a trade
    pub async fn confirm(conn: &Object, trade_id: &str) -> Result<()> {
        conn.execute(
            r#"
            UPDATE trades
            SET status = 'confirmed',
                confirmed_at = NOW()
            WHERE trade_id = $1
            "#,
            &[&trade_id],
        ).await.context("Failed to confirm trade")?;
        
        Ok(())
    }
    
    /// Get user trade history
    pub async fn get_user_history(
        conn: &Object,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DbTrade>> {
        let rows = conn.query(
            r#"
            SELECT * FROM trades
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            &[&user_id, &limit, &offset],
        ).await.context("Failed to get trade history")?;
        
        Ok(rows.into_iter().map(DbTrade::from).collect())
    }
}

/// Audit log queries
pub struct AuditQueries;

impl AuditQueries {
    /// Log an action
    pub async fn log(
        conn: &Object,
        user_id: Option<i64>,
        action: &str,
        entity_type: &str,
        entity_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        request_id: &str,
        changes: serde_json::Value,
    ) -> Result<()> {
        conn.execute(
            r#"
            INSERT INTO audit_logs (
                user_id, action, entity_type, entity_id,
                ip_address, user_agent, request_id, changes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            &[&user_id, &action, &entity_type, &entity_id,
              &ip_address, &user_agent, &request_id, &changes],
        ).await.context("Failed to log audit entry")?;
        
        Ok(())
    }
}