//! Comprehensive market data service for fetching and aggregating market data

use crate::{
    types::{Market, MarketOutcome, AmmType},
    AppState,
};
use anyhow::Result;
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Market data source types
#[derive(Debug, Clone, PartialEq)]
pub enum MarketDataSource {
    Database,
    Polymarket,
    Solana,
    Seeded,
    Mock,
}

/// Aggregated market data with source tracking
#[derive(Debug, Clone)]
pub struct AggregatedMarketData {
    pub markets: Vec<Market>,
    pub sources: Vec<MarketDataSource>,
    pub total_from_db: usize,
    pub total_from_polymarket: usize,
    pub total_from_solana: usize,
    pub total_from_seeded: usize,
}

/// Market data service for comprehensive market fetching
pub struct MarketDataService;

impl MarketDataService {
    /// Fetch markets from all available sources with proper fallback
    pub async fn fetch_all_markets(
        state: &AppState,
        limit: usize,
        offset: usize,
    ) -> Result<AggregatedMarketData> {
        let mut all_markets = Vec::new();
        let mut sources = Vec::new();
        let mut total_from_db = 0;
        let mut total_from_polymarket = 0;
        let mut total_from_solana = 0;
        let mut total_from_seeded = 0;
        
        // 1. Try database first (most reliable source)
        if !state.database.is_degraded().await {
            match Self::fetch_from_database(state, limit, offset).await {
                Ok(db_markets) => {
                    total_from_db = db_markets.len();
                    if !db_markets.is_empty() {
                        info!("Fetched {} markets from database", db_markets.len());
                        all_markets.extend(db_markets);
                        sources.push(MarketDataSource::Database);
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch from database: {}", e);
                }
            }
        }
        
        // 2. Try Polymarket if we need more markets
        if all_markets.len() < limit {
            match Self::fetch_from_polymarket(state, limit - all_markets.len()).await {
                Ok(polymarket_markets) => {
                    total_from_polymarket = polymarket_markets.len();
                    if !polymarket_markets.is_empty() {
                        info!("Fetched {} markets from Polymarket", polymarket_markets.len());
                        all_markets.extend(polymarket_markets);
                        sources.push(MarketDataSource::Polymarket);
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch from Polymarket: {}", e);
                }
            }
        }
        
        // 3. Try Solana blockchain if we still need more
        if all_markets.len() < limit {
            match Self::fetch_from_solana(state, limit - all_markets.len()).await {
                Ok(solana_markets) => {
                    total_from_solana = solana_markets.len();
                    if !solana_markets.is_empty() {
                        info!("Fetched {} markets from Solana", solana_markets.len());
                        all_markets.extend(solana_markets);
                        sources.push(MarketDataSource::Solana);
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch from Solana: {}", e);
                }
            }
        }
        
        // 4. Use seeded markets as fallback
        if all_markets.is_empty() {
            let seeded_markets = state.seeded_markets.get_all();
            total_from_seeded = seeded_markets.len();
            info!("Using {} seeded markets as fallback", seeded_markets.len());
            all_markets.extend(seeded_markets);
            sources.push(MarketDataSource::Seeded);
        }
        
        // Deduplicate markets by ID
        let mut seen_ids = HashMap::new();
        all_markets.retain(|market| {
            seen_ids.insert(market.id, ()).is_none()
        });
        
        Ok(AggregatedMarketData {
            markets: all_markets,
            sources,
            total_from_db,
            total_from_polymarket,
            total_from_solana,
            total_from_seeded,
        })
    }
    
    /// Fetch markets from database
    async fn fetch_from_database(
        state: &AppState,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Market>> {
        let conn = state.database.get_connection().await?;
        let db_markets = crate::db::market_queries::get_all_markets(
            &conn,
            limit as i64,
            offset as i64,
        ).await?;
        
        // Convert database markets to API format
        Ok(db_markets.into_iter().map(|m| Market {
            id: m.id as u128,
            title: m.question.clone(),
            description: m.description.unwrap_or_default(),
            outcomes: Self::parse_outcomes(&m.outcomes),
            creator: Pubkey::new_unique(), // Default creator since MarketDb doesn't have this field
            total_liquidity: m.total_liquidity as u64,
            total_volume: m.total_volume as u64,
            resolution_time: m.end_time.timestamp(),
            resolved: m.status == "resolved",
            winning_outcome: m.resolution_outcome.map(|o| o as u8),
            amm_type: Self::parse_amm_type(&m.market_type),
            created_at: m.created_at.timestamp(),
            verse_id: None, // Default to None since MarketDb doesn't have this field
            current_price: 0.5, // Default price
        }).collect())
    }
    
    /// Fetch markets from Polymarket
    async fn fetch_from_polymarket(
        state: &AppState,
        limit: usize,
    ) -> Result<Vec<Market>> {
        let polymarket_markets = state.polymarket_public_client
            .get_current_markets(limit)
            .await?;
        
        // Filter out historical markets
        let current_year = Utc::now().format("%Y").to_string().parse::<i32>().unwrap_or(2024);
        let relevant_markets: Vec<_> = polymarket_markets
            .into_iter()
            .filter(|m| {
                // Keep markets that mention current or future years
                (current_year..=current_year + 3).any(|year| {
                    m.question.contains(&year.to_string())
                }) || !m.question.chars().any(|c| c.is_numeric() && c as u32 >= '2' as u32 && c as u32 <= '9' as u32)
            })
            .collect();
        
        // Convert to internal format
        Ok(relevant_markets.into_iter().enumerate().map(|(index, pm)| {
            let outcomes = Self::parse_polymarket_outcomes(&pm.outcomes, &pm.outcome_prices);
            
            Market {
                id: (index as u128 + 100000), // High offset to avoid conflicts
                title: pm.question.clone(),
                description: pm.description.clone(),
                creator: Pubkey::default(),
                outcomes,
                amm_type: AmmType::Hybrid,
                total_volume: (pm.volume_num * 1_000_000.0) as u64,
                total_liquidity: (pm.liquidity_num * 1_000_000.0) as u64,
                resolution_time: pm.end_date_iso
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                    .map(|d| d.timestamp())
                    .unwrap_or_else(|| Utc::now().timestamp() + 86400 * 7),
                resolved: pm.closed,
                winning_outcome: None,
                current_price: 0.5, // Calculate from outcome prices
                created_at: Utc::now().timestamp() - 86400, // Assume created 1 day ago
                verse_id: Some(50 + index as u128),
            }
        }).collect())
    }
    
    /// Fetch markets from Solana blockchain
    async fn fetch_from_solana(
        state: &AppState,
        limit: usize,
    ) -> Result<Vec<Market>> {
        // Try to fetch on-chain markets
        match state.platform_client.get_all_markets().await {
            Ok(mut solana_markets) => {
                solana_markets.truncate(limit);
                Ok(solana_markets)
            },
            Err(e) => {
                error!("Failed to fetch from Solana: {}", e);
                Ok(Vec::new())
            }
        }
    }
    
    /// Parse outcomes from JSON value
    fn parse_outcomes(outcomes_json: &serde_json::Value) -> Vec<MarketOutcome> {
        outcomes_json
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .enumerate()
                    .map(|(i, name)| MarketOutcome {
                        id: i as u8,
                        name: name.to_string(),
                        title: name.to_string(),
                        description: format!("{} outcome", name),
                        total_stake: 0,
                    })
                    .collect()
            })
            .unwrap_or_else(|| vec![
                MarketOutcome { id: 0, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 0 },
                MarketOutcome { id: 1, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 0 },
            ])
    }
    
    /// Parse Polymarket outcomes
    fn parse_polymarket_outcomes(
        outcomes_str: &str,
        prices_str: &str,
    ) -> Vec<MarketOutcome> {
        let outcomes: Vec<String> = serde_json::from_str(outcomes_str).unwrap_or_default();
        let prices: Vec<String> = serde_json::from_str(prices_str).unwrap_or_default();
        
        outcomes
            .into_iter()
            .zip(prices.into_iter())
            .enumerate()
            .map(|(i, (name, price))| {
                let price_f64 = price.parse::<f64>().unwrap_or(0.5);
                MarketOutcome {
                    id: i as u8,
                    name: name.clone(),
                    title: name.clone(),
                    description: format!("{} outcome", name),
                    total_stake: (price_f64 * 1_000_000.0) as u64,
                }
            })
            .collect()
    }
    
    /// Parse creator from string
    fn parse_creator(creator_str: &Option<String>) -> Pubkey {
        creator_str
            .as_ref()
            .and_then(|s| Pubkey::from_str(s).ok())
            .unwrap_or_else(Pubkey::new_unique)
    }
    
    /// Parse AMM type from string
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
}

/// Enhanced market filtering and sorting
pub struct MarketFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub amm_type: Option<String>,
    pub min_volume: Option<u64>,
    pub min_liquidity: Option<u64>,
    pub creator: Option<String>,
    pub verse_id: Option<u128>,
}

impl MarketFilter {
    pub fn apply(&self, markets: &mut Vec<Market>) {
        // Search filter
        if let Some(search) = &self.search {
            let search_lower = search.to_lowercase();
            markets.retain(|m| {
                m.title.to_lowercase().contains(&search_lower) ||
                m.description.to_lowercase().contains(&search_lower) ||
                m.outcomes.iter().any(|o| o.name.to_lowercase().contains(&search_lower))
            });
        }
        
        // Status filter
        if let Some(status) = &self.status {
            let now = Utc::now().timestamp();
            match status.as_str() {
                "active" => markets.retain(|m| !m.resolved && m.resolution_time > now),
                "resolved" => markets.retain(|m| m.resolved),
                "pending" => markets.retain(|m| !m.resolved && m.resolution_time <= now),
                _ => {}
            }
        }
        
        // AMM type filter
        if let Some(amm_type) = &self.amm_type {
            let amm_type_lower = amm_type.to_lowercase();
            markets.retain(|m| {
                format!("{:?}", m.amm_type).to_lowercase() == amm_type_lower
            });
        }
        
        // Volume filter
        if let Some(min_volume) = self.min_volume {
            markets.retain(|m| m.total_volume >= min_volume);
        }
        
        // Liquidity filter
        if let Some(min_liquidity) = self.min_liquidity {
            markets.retain(|m| m.total_liquidity >= min_liquidity);
        }
        
        // Creator filter
        if let Some(creator) = &self.creator {
            if let Ok(creator_pubkey) = Pubkey::from_str(creator) {
                markets.retain(|m| m.creator == creator_pubkey);
            }
        }
        
        // Verse filter
        if let Some(verse_id) = self.verse_id {
            markets.retain(|m| m.verse_id == Some(verse_id));
        }
    }
}

/// Market sorting options
pub enum MarketSort {
    Volume,
    Liquidity,
    Created,
    EndTime,
    Activity,
}

impl MarketSort {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "volume" => Some(MarketSort::Volume),
            "liquidity" => Some(MarketSort::Liquidity),
            "created" | "newest" => Some(MarketSort::Created),
            "ending" | "end_time" => Some(MarketSort::EndTime),
            "activity" | "active" => Some(MarketSort::Activity),
            _ => None,
        }
    }
    
    pub fn apply(&self, markets: &mut Vec<Market>) {
        match self {
            MarketSort::Volume => markets.sort_by(|a, b| b.total_volume.cmp(&a.total_volume)),
            MarketSort::Liquidity => markets.sort_by(|a, b| b.total_liquidity.cmp(&a.total_liquidity)),
            MarketSort::Created => markets.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            MarketSort::EndTime => markets.sort_by(|a, b| a.resolution_time.cmp(&b.resolution_time)),
            MarketSort::Activity => {
                // Sort by recent volume + liquidity changes (simplified)
                markets.sort_by(|a, b| {
                    let a_activity = a.total_volume + a.total_liquidity;
                    let b_activity = b.total_volume + b.total_liquidity;
                    b_activity.cmp(&a_activity)
                });
            }
        }
    }
}