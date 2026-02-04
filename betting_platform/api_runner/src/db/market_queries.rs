//! Database queries for markets with graceful degradation

use anyhow::{Result, Context};
use deadpool_postgres::GenericClient;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDb {
    pub id: i64,
    pub market_id: uuid::Uuid,
    pub question: String,
    pub description: Option<String>,
    pub outcomes: serde_json::Value,
    pub market_type: String,
    pub status: String,
    pub end_time: DateTime<Utc>,
    pub resolution_time: Option<DateTime<Utc>>,
    pub resolution_outcome: Option<i32>,
    pub total_volume: i64,
    pub total_liquidity: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// Get all markets from database
pub async fn get_all_markets<C: GenericClient>(
    client: &C,
    limit: i64,
    offset: i64,
) -> Result<Vec<MarketDb>> {
    let query = r#"
        SELECT 
            id,
            market_id,
            question,
            description,
            outcomes,
            market_type::text,
            status::text,
            end_time,
            resolution_time,
            resolution_outcome,
            total_volume::bigint,
            total_liquidity::bigint,
            created_at,
            updated_at,
            metadata
        FROM markets
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
    "#;
    
    let rows = client.query(query, &[&limit, &offset])
        .await
        .context("Failed to query markets")?;
    
    rows.into_iter()
        .map(|row| {
            Ok(MarketDb {
                id: row.get(0),
                market_id: row.get(1),
                question: row.get(2),
                description: row.get(3),
                outcomes: row.get(4),
                market_type: row.get(5),
                status: row.get(6),
                end_time: row.get(7),
                resolution_time: row.get(8),
                resolution_outcome: row.get(9),
                total_volume: row.get(10),
                total_liquidity: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
                metadata: row.get(14),
            })
        })
        .collect()
}

/// Get active markets
pub async fn get_active_markets<C: GenericClient>(
    client: &C,
    limit: i64,
    offset: i64,
) -> Result<Vec<MarketDb>> {
    let query = r#"
        SELECT 
            id,
            market_id,
            question,
            description,
            outcomes,
            market_type::text,
            status::text,
            end_time,
            resolution_time,
            resolution_outcome,
            total_volume::bigint,
            total_liquidity::bigint,
            created_at,
            updated_at,
            metadata
        FROM markets
        WHERE status = 'active' AND end_time > NOW()
        ORDER BY total_volume DESC
        LIMIT $1 OFFSET $2
    "#;
    
    let rows = client.query(query, &[&limit, &offset])
        .await
        .context("Failed to query active markets")?;
    
    rows.into_iter()
        .map(|row| {
            Ok(MarketDb {
                id: row.get(0),
                market_id: row.get(1),
                question: row.get(2),
                description: row.get(3),
                outcomes: row.get(4),
                market_type: row.get(5),
                status: row.get(6),
                end_time: row.get(7),
                resolution_time: row.get(8),
                resolution_outcome: row.get(9),
                total_volume: row.get(10),
                total_liquidity: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
                metadata: row.get(14),
            })
        })
        .collect()
}

/// Get market by ID
pub async fn get_market_by_id<C: GenericClient>(
    client: &C,
    market_id: uuid::Uuid,
) -> Result<Option<MarketDb>> {
    let query = r#"
        SELECT 
            id,
            market_id,
            question,
            description,
            outcomes,
            market_type::text,
            status::text,
            end_time,
            resolution_time,
            resolution_outcome,
            total_volume::bigint,
            total_liquidity::bigint,
            created_at,
            updated_at,
            metadata
        FROM markets
        WHERE market_id = $1
    "#;
    
    let row = client.query_opt(query, &[&market_id])
        .await
        .context("Failed to query market by ID")?;
    
    Ok(row.map(|row| MarketDb {
        id: row.get(0),
        market_id: row.get(1),
        question: row.get(2),
        description: row.get(3),
        outcomes: row.get(4),
        market_type: row.get(5),
        status: row.get(6),
        end_time: row.get(7),
        resolution_time: row.get(8),
        resolution_outcome: row.get(9),
        total_volume: row.get(10),
        total_liquidity: row.get(11),
        created_at: row.get(12),
        updated_at: row.get(13),
        metadata: row.get(14),
    }))
}

/// Insert a new market
pub async fn insert_market<C: GenericClient>(
    client: &C,
    question: &str,
    description: Option<&str>,
    outcomes: &serde_json::Value,
    market_type: &str,
    end_time: DateTime<Utc>,
) -> Result<MarketDb> {
    let query = r#"
        INSERT INTO markets (
            question,
            description,
            outcomes,
            market_type,
            end_time
        ) VALUES ($1, $2, $3, $4::market_type, $5)
        RETURNING 
            id,
            market_id,
            question,
            description,
            outcomes,
            market_type::text,
            status::text,
            end_time,
            resolution_time,
            resolution_outcome,
            total_volume::bigint,
            total_liquidity::bigint,
            created_at,
            updated_at,
            metadata
    "#;
    
    let row = client.query_one(
        query, 
        &[&question, &description, &outcomes, &market_type, &end_time]
    )
    .await
    .context("Failed to insert market")?;
    
    Ok(MarketDb {
        id: row.get(0),
        market_id: row.get(1),
        question: row.get(2),
        description: row.get(3),
        outcomes: row.get(4),
        market_type: row.get(5),
        status: row.get(6),
        end_time: row.get(7),
        resolution_time: row.get(8),
        resolution_outcome: row.get(9),
        total_volume: row.get(10),
        total_liquidity: row.get(11),
        created_at: row.get(12),
        updated_at: row.get(13),
        metadata: row.get(14),
    })
}

/// Count total markets
pub async fn count_markets<C: GenericClient>(client: &C) -> Result<i64> {
    let row = client.query_one("SELECT COUNT(*) FROM markets", &[])
        .await
        .context("Failed to count markets")?;
    
    Ok(row.get(0))
}

/// Count active markets
pub async fn count_active_markets<C: GenericClient>(client: &C) -> Result<i64> {
    let row = client.query_one(
        "SELECT COUNT(*) FROM markets WHERE status = 'active' AND end_time > NOW()", 
        &[]
    )
    .await
    .context("Failed to count active markets")?;
    
    Ok(row.get(0))
}