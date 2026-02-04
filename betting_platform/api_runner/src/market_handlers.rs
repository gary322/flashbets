//! Enhanced market handlers with comprehensive data fetching

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{info, warn, error};
use crate::{
    AppState,
    types::{Market, MarketOutcome, AmmType},
    market_data_service::{MarketDataService, MarketFilter, MarketSort},
    cache::CacheKey,
    throughput_optimization::FastJson,
};

/// Enhanced query parameters for markets endpoint
#[derive(Debug, Deserialize)]
pub struct EnhancedMarketsQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub sort: Option<String>,
    pub sort_by: Option<String>,
    pub amm_type: Option<String>,
    pub min_volume: Option<u64>,
    pub min_liquidity: Option<u64>,
    pub creator: Option<String>,
    pub verse_id: Option<u128>,
    pub include_metadata: Option<bool>,
}

/// Enhanced markets response with metadata
#[derive(Serialize, Deserialize)]
pub struct EnhancedMarketsResponse {
    pub markets: Vec<Market>,
    pub pagination: PaginationInfo,
    pub metadata: MarketMetadata,
    pub filters_applied: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct PaginationInfo {
    pub total: usize,
    pub count: usize,
    pub limit: usize,
    pub offset: usize,
    pub has_more: bool,
}

#[derive(Serialize, Deserialize)]
pub struct MarketMetadata {
    pub sources: Vec<String>,
    pub total_volume: u64,
    pub total_liquidity: u64,
    pub active_markets: usize,
    pub resolved_markets: usize,
    pub data_freshness: String,
    pub cache_status: String,
}

/// Get markets with comprehensive data fetching
pub async fn get_markets_enhanced(
    Query(params): Query<EnhancedMarketsQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);
    
    // Build cache key from parameters
    let cache_key = format!(
        "markets:limit={}:offset={}:search={}:status={}:sort={}",
        limit,
        offset,
        params.search.as_deref().unwrap_or(""),
        params.status.as_deref().unwrap_or(""),
        params.sort.as_deref().or(params.sort_by.as_deref()).unwrap_or("")
    );
    
    // Try cache first
    let cache_status = if let Some(cached) = state.cache.get::<EnhancedMarketsResponse>(&cache_key).await {
        info!("Returning cached markets");
        return Ok(Json(cached).into_response());
    } else {
        "miss"
    };
    
    // Fetch from all sources
    let aggregated_data = match MarketDataService::fetch_all_markets(&state, limit * 2, 0).await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to fetch market data: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    let mut markets = aggregated_data.markets;
    let sources: Vec<String> = aggregated_data.sources.iter()
        .map(|s| format!("{:?}", s).to_lowercase())
        .collect();
    
    // Apply filters
    let mut filter = MarketFilter {
        search: params.search.clone(),
        status: params.status.clone(),
        amm_type: params.amm_type.clone(),
        min_volume: params.min_volume,
        min_liquidity: params.min_liquidity,
        creator: params.creator.clone(),
        verse_id: params.verse_id,
    };
    
    let total_before_filter = markets.len();
    filter.apply(&mut markets);
    let total_after_filter = markets.len();
    
    // Apply sorting
    let sort_param = params.sort.as_ref().or(params.sort_by.as_ref());
    if let Some(sort_str) = sort_param {
        if let Some(sort) = MarketSort::from_str(sort_str) {
            sort.apply(&mut markets);
        }
    } else {
        // Default sort by volume
        MarketSort::Volume.apply(&mut markets);
    }
    
    // Calculate metadata before pagination
    let total_volume: u64 = markets.iter().map(|m| m.total_volume).sum();
    let total_liquidity: u64 = markets.iter().map(|m| m.total_liquidity).sum();
    let active_markets = markets.iter().filter(|m| !m.resolved).count();
    let resolved_markets = markets.iter().filter(|m| m.resolved).count();
    
    // Apply pagination
    let total_count = markets.len();
    let paginated_markets: Vec<_> = markets
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    
    // Store count before moving
    let paginated_count = paginated_markets.len();
    
    // Build filters applied map
    let mut filters_applied = HashMap::new();
    if params.search.is_some() {
        filters_applied.insert("search".to_string(), params.search.unwrap());
    }
    if params.status.is_some() {
        filters_applied.insert("status".to_string(), params.status.unwrap());
    }
    if let Some(sort) = sort_param {
        filters_applied.insert("sort".to_string(), sort.clone());
    }
    if params.amm_type.is_some() {
        filters_applied.insert("amm_type".to_string(), params.amm_type.unwrap());
    }
    if total_before_filter != total_after_filter {
        filters_applied.insert("filtered_out".to_string(), 
            format!("{} markets", total_before_filter - total_after_filter));
    }
    
    let response = EnhancedMarketsResponse {
        markets: paginated_markets,
        pagination: PaginationInfo {
            total: total_count,
            count: paginated_count,
            limit,
            offset,
            has_more: offset + limit < total_count,
        },
        metadata: MarketMetadata {
            sources,
            total_volume,
            total_liquidity,
            active_markets,
            resolved_markets,
            data_freshness: "real-time".to_string(),
            cache_status: cache_status.to_string(),
        },
        filters_applied,
    };
    
    // Cache for 2 minutes
    if let Err(e) = state.cache.set(&cache_key, &response, Some(120)).await {
        warn!("Failed to cache markets response: {}", e);
    }
    
    Ok(FastJson(response).into_response())
}

/// Get a specific market by ID
pub async fn get_market_by_id(
    Path(market_id): Path<u128>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    // Try cache first
    let cache_key = format!("market:{}", market_id);
    if let Some(cached) = state.cache.get::<Market>(&cache_key).await {
        return Ok(Json(cached).into_response());
    }
    
    // Try database
    if !state.database.is_degraded().await {
        if let Ok(conn) = state.database.get_connection().await {
            // Convert market_id to UUID - using a simple approach for now
            let market_uuid = Uuid::from_u128(market_id);
            match crate::db::market_queries::get_market_by_id(&conn, market_uuid).await {
                Ok(Some(db_market)) => {
                    let market = Market {
                        id: db_market.id as u128,
                        title: db_market.question.clone(),
                        description: db_market.description.unwrap_or_default(),
                        outcomes: parse_outcomes(&db_market.outcomes),
                        creator: Pubkey::default(), // TODO: extract from metadata
                        total_liquidity: db_market.total_liquidity as u64,
                        total_volume: db_market.total_volume as u64,
                        resolution_time: db_market.end_time.timestamp(),
                        resolved: db_market.status == "resolved",
                        winning_outcome: db_market.resolution_outcome.map(|o| o as u8),
                        amm_type: parse_amm_type(&db_market.market_type),
                        created_at: db_market.created_at.timestamp(),
                        verse_id: None, // TODO: extract from metadata
                        current_price: 0.5, // Default price
                    };
                    
                    // Cache for 5 minutes
                    let _ = state.cache.set(&cache_key, &market, Some(300)).await;
                    
                    return Ok(Json(market).into_response());
                }
                Ok(None) => {
                    // Not found in database
                }
                Err(e) => {
                    warn!("Failed to fetch market from database: {}", e);
                }
            }
        }
    }
    
    // Try fetching all markets and finding by ID
    match MarketDataService::fetch_all_markets(&state, 1000, 0).await {
        Ok(data) => {
            if let Some(market) = data.markets.into_iter().find(|m| m.id == market_id) {
                // Cache for 5 minutes
                let _ = state.cache.set(&cache_key, &market, Some(300)).await;
                Ok(Json(market).into_response())
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get market statistics
#[derive(Serialize, Deserialize)]
pub struct MarketStatistics {
    pub total_markets: usize,
    pub active_markets: usize,
    pub resolved_markets: usize,
    pub total_volume: u64,
    pub total_liquidity: u64,
    pub average_volume_per_market: u64,
    pub average_liquidity_per_market: u64,
    pub top_markets_by_volume: Vec<MarketSummary>,
    pub trending_markets: Vec<MarketSummary>,
    pub closing_soon: Vec<MarketSummary>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MarketSummary {
    pub id: u128,
    pub title: String,
    pub volume: u64,
    pub liquidity: u64,
    pub closing_time: Option<i64>,
}

pub async fn get_market_statistics(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    // Try cache first
    let cache_key = "market:statistics".to_string();
    if let Some(cached) = state.cache.get::<MarketStatistics>(&cache_key).await {
        return Ok(Json(cached).into_response());
    }
    
    // Fetch all markets
    let data = match MarketDataService::fetch_all_markets(&state, 1000, 0).await {
        Ok(data) => data,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    let markets = data.markets;
    let total_markets = markets.len();
    let active_markets = markets.iter().filter(|m| !m.resolved).count();
    let resolved_markets = markets.iter().filter(|m| m.resolved).count();
    let total_volume: u64 = markets.iter().map(|m| m.total_volume).sum();
    let total_liquidity: u64 = markets.iter().map(|m| m.total_liquidity).sum();
    
    let average_volume_per_market = if total_markets > 0 {
        total_volume / total_markets as u64
    } else {
        0
    };
    
    let average_liquidity_per_market = if total_markets > 0 {
        total_liquidity / total_markets as u64
    } else {
        0
    };
    
    // Top markets by volume
    let mut top_by_volume = markets.clone();
    top_by_volume.sort_by(|a, b| b.total_volume.cmp(&a.total_volume));
    let top_markets_by_volume: Vec<MarketSummary> = top_by_volume
        .into_iter()
        .take(10)
        .map(|m| MarketSummary {
            id: m.id,
            title: m.title,
            volume: m.total_volume,
            liquidity: m.total_liquidity,
            closing_time: Some(m.resolution_time),
        })
        .collect();
    
    // Trending markets (simplified - highest recent volume)
    let trending_markets = top_markets_by_volume.clone();
    
    // Closing soon (next 24 hours)
    let now = chrono::Utc::now().timestamp();
    let tomorrow = now + 86400;
    let mut closing_soon: Vec<_> = markets
        .into_iter()
        .filter(|m| !m.resolved && m.resolution_time > now && m.resolution_time <= tomorrow)
        .map(|m| MarketSummary {
            id: m.id,
            title: m.title,
            volume: m.total_volume,
            liquidity: m.total_liquidity,
            closing_time: Some(m.resolution_time),
        })
        .collect();
    closing_soon.sort_by(|a, b| a.closing_time.cmp(&b.closing_time));
    closing_soon.truncate(10);
    
    let stats = MarketStatistics {
        total_markets,
        active_markets,
        resolved_markets,
        total_volume,
        total_liquidity,
        average_volume_per_market,
        average_liquidity_per_market,
        top_markets_by_volume,
        trending_markets,
        closing_soon,
    };
    
    // Cache for 10 minutes
    let _ = state.cache.set(&cache_key, &stats, Some(600)).await;
    
    Ok(Json(stats).into_response())
}

// Helper functions
fn parse_outcomes(outcomes_json: &serde_json::Value) -> Vec<MarketOutcome> {
    outcomes_json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .enumerate()
                .map(|(idx, name)| MarketOutcome {
                    id: idx as u8,
                    name: name.to_string(),
                    title: name.to_string(),
                    description: format!("Outcome: {}", name),
                    total_stake: 0,
                })
                .collect()
        })
        .unwrap_or_else(|| vec![
            MarketOutcome { 
                id: 0,
                name: "Yes".to_string(), 
                title: "Yes".to_string(),
                description: "Outcome: Yes".to_string(),
                total_stake: 0 
            },
            MarketOutcome { 
                id: 1,
                name: "No".to_string(), 
                title: "No".to_string(),
                description: "Outcome: No".to_string(),
                total_stake: 0 
            },
        ])
}

fn parse_creator(creator_str: &Option<String>) -> solana_sdk::pubkey::Pubkey {
    creator_str
        .as_ref()
        .and_then(|s| solana_sdk::pubkey::Pubkey::from_str(s).ok())
        .unwrap_or_else(solana_sdk::pubkey::Pubkey::new_unique)
}

fn parse_amm_type(market_type: &str) -> AmmType {
    match market_type.to_lowercase().as_str() {
        "cpmm" => AmmType::Cpmm,
        "lmsr" => AmmType::Lmsr,
        "pmamm" => AmmType::PmAmm,
        "l2amm" => AmmType::L2Amm,
        "hybrid" => AmmType::Hybrid,
        _ => AmmType::Cpmm,
    }
}