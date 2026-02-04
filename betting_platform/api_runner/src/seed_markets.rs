//! Market seeding utilities for test data

use crate::types::{Market, MarketOutcome, AmmType};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use chrono::Utc;
use serde_json;

/// Create test markets with realistic data
pub fn create_test_markets() -> Vec<Market> {
    let creator = Pubkey::new_unique();
    let mut markets = Vec::new();
    
    // Politics Markets
    markets.push(Market {
        id: 1,
        title: "2024 US Presidential Election Winner".to_string(),
        description: "Who will win the 2024 US Presidential Election?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 0, name: "Biden".to_string(), title: "Biden".to_string(), description: "Biden outcome".to_string(), total_stake: 2500000 },
            MarketOutcome { id: 1, name: "Trump".to_string(), title: "Trump".to_string(), description: "Trump outcome".to_string(), total_stake: 2300000 },
            MarketOutcome { id: 2, name: "Other".to_string(), title: "Other".to_string(), description: "Other outcome".to_string(), total_stake: 200000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 5000000,
        total_volume: 8500000,
        resolution_time: 1730851200, // Nov 5, 2024
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 30 * 24 * 3600,
        verse_id: Some(1),
        current_price: 0.5,
    });
    
    markets.push(Market {
        id: 2,
        title: "Democrats Control Senate 2024".to_string(),
        description: "Will Democrats maintain control of the US Senate after 2024 elections?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 3, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1800000 },
            MarketOutcome { id: 4, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2200000 },
        ],
        amm_type: AmmType::Lmsr,
        total_liquidity: 4000000,
        total_volume: 6200000,
        resolution_time: 1730851200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 25 * 24 * 3600,
        verse_id: Some(2),
        current_price: 0.5,
    });
    
    // Sports Markets
    markets.push(Market {
        id: 3,
        title: "Super Bowl 2025 Winner".to_string(),
        description: "Which team will win Super Bowl LIX?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 5, name: "Chiefs".to_string(), title: "Chiefs".to_string(), description: "Chiefs outcome".to_string(), total_stake: 1500000 },
            MarketOutcome { id: 6, name: "49ers".to_string(), title: "49ers".to_string(), description: "49ers outcome".to_string(), total_stake: 1200000 },
            MarketOutcome { id: 7, name: "Bills".to_string(), title: "Bills".to_string(), description: "Bills outcome".to_string(), total_stake: 800000 },
            MarketOutcome { id: 8, name: "Eagles".to_string(), title: "Eagles".to_string(), description: "Eagles outcome".to_string(), total_stake: 700000 },
            MarketOutcome { id: 9, name: "Other".to_string(), title: "Other".to_string(), description: "Other outcome".to_string(), total_stake: 800000 },
        ],
        amm_type: AmmType::Hybrid,
        total_liquidity: 5000000,
        total_volume: 7500000,
        resolution_time: 1738454400, // Feb 2, 2025
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 20 * 24 * 3600,
        verse_id: Some(10),
        current_price: 0.5,
    });
    
    markets.push(Market {
        id: 4,
        title: "NBA Finals 2024 Champion".to_string(),
        description: "Which team will win the 2024 NBA Finals?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 0, name: "Celtics".to_string(), title: "Celtics".to_string(), description: "Celtics outcome".to_string(), total_stake: 900000 },
            MarketOutcome { id: 1, name: "Nuggets".to_string(), title: "Nuggets".to_string(), description: "Nuggets outcome".to_string(), total_stake: 850000 },
            MarketOutcome { id: 2, name: "Lakers".to_string(), title: "Lakers".to_string(), description: "Lakers outcome".to_string(), total_stake: 750000 },
            MarketOutcome { id: 3, name: "Warriors".to_string(), title: "Warriors".to_string(), description: "Warriors outcome".to_string(), total_stake: 700000 },
            MarketOutcome { id: 4, name: "Other".to_string(), title: "Other".to_string(), description: "Other outcome".to_string(), total_stake: 800000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 4000000,
        total_volume: 5500000,
        resolution_time: 1718582400, // Jun 17, 2024
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 15 * 24 * 3600,
        verse_id: Some(11),
        current_price: 0.5,
    });
    
    // Crypto Markets
    markets.push(Market {
        id: 5,
        title: "Bitcoin Above $100k by 2025".to_string(),
        description: "Will Bitcoin price exceed $100,000 before January 1, 2025?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 5, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 3500000 },
            MarketOutcome { id: 6, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1500000 },
        ],
        amm_type: AmmType::Lmsr,
        total_liquidity: 5000000,
        total_volume: 12000000,
        resolution_time: 1735689600, // Jan 1, 2025
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 35 * 24 * 3600,
        verse_id: Some(20),
        current_price: 0.5,
    });
    
    markets.push(Market {
        id: 6,
        title: "Ethereum Flips Bitcoin Market Cap".to_string(),
        description: "Will Ethereum's market cap exceed Bitcoin's by end of 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 7, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 800000 },
            MarketOutcome { id: 8, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 3200000 },
        ],
        amm_type: AmmType::L2Amm,
        total_liquidity: 4000000,
        total_volume: 6800000,
        resolution_time: 1735603200, // Dec 31, 2024
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 28 * 24 * 3600,
        verse_id: Some(21),
        current_price: 0.5,
    });
    
    // Finance Markets
    markets.push(Market {
        id: 7,
        title: "S&P 500 Above 6000 in 2024".to_string(),
        description: "Will the S&P 500 index close above 6000 before end of 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 9, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1200000 },
            MarketOutcome { id: 0, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2800000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 4000000,
        total_volume: 5500000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 22 * 24 * 3600,
        verse_id: Some(30),
        current_price: 0.5,
    });
    
    markets.push(Market {
        id: 8,
        title: "Fed Rate Cut by Q3 2024".to_string(),
        description: "Will the Federal Reserve cut interest rates by end of Q3 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 1, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 2700000 },
            MarketOutcome { id: 2, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1300000 },
        ],
        amm_type: AmmType::Lmsr,
        total_liquidity: 4000000,
        total_volume: 7200000,
        resolution_time: 1727740800, // Sep 30, 2024
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 18 * 24 * 3600,
        verse_id: Some(31),
        current_price: 0.5,
    });
    
    // Technology Markets
    markets.push(Market {
        id: 9,
        title: "Apple Releases AR Glasses in 2024".to_string(),
        description: "Will Apple announce consumer AR glasses before end of 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 3, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 600000 },
            MarketOutcome { id: 4, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2400000 },
        ],
        amm_type: AmmType::Hybrid,
        total_liquidity: 3000000,
        total_volume: 4200000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 12 * 24 * 3600,
        verse_id: Some(40),
        current_price: 0.5,
    });
    
    markets.push(Market {
        id: 10,
        title: "ChatGPT 5 Released in 2024".to_string(),
        description: "Will OpenAI release ChatGPT 5 before end of 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 5, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1800000 },
            MarketOutcome { id: 6, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1200000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 3000000,
        total_volume: 5100000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 10 * 24 * 3600,
        verse_id: Some(41),
        current_price: 0.5,
    });
    
    // Entertainment Markets
    markets.push(Market {
        id: 11,
        title: "Oscars 2024 Best Picture".to_string(),
        description: "Which film will win Best Picture at the 2024 Academy Awards?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 7, name: "Oppenheimer".to_string(), title: "Oppenheimer".to_string(), description: "Oppenheimer outcome".to_string(), total_stake: 1500000 },
            MarketOutcome { id: 8, name: "Killers of the Flower Moon".to_string(), title: "Killers of the Flower Moon".to_string(), description: "Killers of the Flower Moon outcome".to_string(), total_stake: 800000 },
            MarketOutcome { id: 9, name: "Barbie".to_string(), title: "Barbie".to_string(), description: "Barbie outcome".to_string(), total_stake: 600000 },
            MarketOutcome { id: 0, name: "Other".to_string(), title: "Other".to_string(), description: "Other outcome".to_string(), total_stake: 600000 },
        ],
        amm_type: AmmType::L2Amm,
        total_liquidity: 3500000,
        total_volume: 4800000,
        resolution_time: 1709424000, // Mar 3, 2024
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 8 * 24 * 3600,
        verse_id: Some(50),
        current_price: 0.5,
    });
    
    // Climate Markets
    markets.push(Market {
        id: 12,
        title: "2024 Hottest Year on Record".to_string(),
        description: "Will 2024 be recorded as the hottest year globally on record?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 1, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 2200000 },
            MarketOutcome { id: 2, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 800000 },
        ],
        amm_type: AmmType::Lmsr,
        total_liquidity: 3000000,
        total_volume: 3800000,
        resolution_time: 1738368000, // Feb 1, 2025
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 16 * 24 * 3600,
        verse_id: Some(60),
        current_price: 0.5,
    });
    
    // Science Markets
    markets.push(Market {
        id: 13,
        title: "SpaceX Starship Reaches Orbit 2024".to_string(),
        description: "Will SpaceX Starship successfully reach orbit in 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 3, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 2600000 },
            MarketOutcome { id: 4, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1400000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 4000000,
        total_volume: 5600000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 14 * 24 * 3600,
        verse_id: Some(70),
        current_price: 0.5,
    });
    
    // Economics Markets
    markets.push(Market {
        id: 14,
        title: "US Inflation Below 3% by 2024 End".to_string(),
        description: "Will US CPI inflation rate fall below 3% by December 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 5, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 3100000 },
            MarketOutcome { id: 6, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1900000 },
        ],
        amm_type: AmmType::Hybrid,
        total_liquidity: 5000000,
        total_volume: 8200000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 24 * 24 * 3600,
        verse_id: Some(80),
        current_price: 0.5,
    });
    
    markets.push(Market {
        id: 15,
        title: "US Unemployment Above 5% in 2024".to_string(),
        description: "Will US unemployment rate exceed 5% at any point in 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 7, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1100000 },
            MarketOutcome { id: 8, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2900000 },
        ],
        amm_type: AmmType::Lmsr,
        total_liquidity: 4000000,
        total_volume: 5400000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 19 * 24 * 3600,
        verse_id: Some(81),
        current_price: 0.5,
    });
    
    // Health Markets
    markets.push(Market {
        id: 16,
        title: "COVID Vaccine Update Required 2024".to_string(),
        description: "Will a new COVID vaccine be recommended by CDC in 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 9, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 2800000 },
            MarketOutcome { id: 0, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1200000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 4000000,
        total_volume: 5100000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 11 * 24 * 3600,
        verse_id: Some(90),
        current_price: 0.5,
    });
    
    // Energy Markets
    markets.push(Market {
        id: 17,
        title: "Oil Price Above $100 in 2024".to_string(),
        description: "Will WTI crude oil price exceed $100/barrel in 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 1, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 900000 },
            MarketOutcome { id: 2, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2100000 },
        ],
        amm_type: AmmType::L2Amm,
        total_liquidity: 3000000,
        total_volume: 4300000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 13 * 24 * 3600,
        verse_id: Some(100),
        current_price: 0.5,
    });
    
    // Retail Markets
    markets.push(Market {
        id: 18,
        title: "Amazon Stock Split in 2024".to_string(),
        description: "Will Amazon announce a stock split in 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 3, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 700000 },
            MarketOutcome { id: 4, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2300000 },
        ],
        amm_type: AmmType::Lmsr,
        total_liquidity: 3000000,
        total_volume: 3900000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 9 * 24 * 3600,
        verse_id: Some(110),
        current_price: 0.5,
    });
    
    // Legal Markets
    markets.push(Market {
        id: 19,
        title: "TikTok Banned in US by 2024".to_string(),
        description: "Will TikTok be banned or forced to sell in the US by end of 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 5, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1600000 },
            MarketOutcome { id: 6, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2400000 },
        ],
        amm_type: AmmType::Hybrid,
        total_liquidity: 4000000,
        total_volume: 6200000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 17 * 24 * 3600,
        verse_id: Some(120),
        current_price: 0.5,
    });
    
    // Media Markets
    markets.push(Market {
        id: 20,
        title: "Twitter/X Loses 50% Users by 2024".to_string(),
        description: "Will Twitter/X lose more than 50% of active users by end of 2024?".to_string(),
        creator,
        outcomes: vec![
            MarketOutcome { id: 7, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1300000 },
            MarketOutcome { id: 8, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1700000 },
        ],
        amm_type: AmmType::PmAmm,
        total_liquidity: 3000000,
        total_volume: 4100000,
        resolution_time: 1735603200,
        resolved: false,
        winning_outcome: None,
        created_at: Utc::now().timestamp() - 7 * 24 * 3600,
        verse_id: Some(130),
        current_price: 0.5,
    });
    
    markets
}

/// Store for seeded markets
pub struct SeededMarketStore {
    markets: HashMap<u128, Market>,
}

impl SeededMarketStore {
    pub fn new() -> Self {
        let markets = create_test_markets();
        tracing::info!("Creating SeededMarketStore with {} markets", markets.len());
        let market_map = markets.into_iter()
            .map(|m| (m.id, m))
            .collect();
            
        Self {
            markets: market_map,
        }
    }
    
    /// Get market as JSON value (for compatibility)
    pub async fn get_market(&self, id: u64) -> Option<serde_json::Value> {
        self.get_by_id(id as u128)
            .map(|market| serde_json::json!({
                "id": market.id,
                "title": market.title,
                "description": market.description,
                "outcomes": market.outcomes.iter().map(|o| serde_json::json!({
                    "name": o.name,
                    "total_stake": o.total_stake
                })).collect::<Vec<_>>(),
                "total_liquidity": market.total_liquidity,
                "total_volume": market.total_volume,
                "resolution_time": market.resolution_time,
                "resolved": market.resolved,
                "winning_outcome": market.winning_outcome,
                "created_at": market.created_at,
                "verse_id": market.verse_id,
            }))
    }
    
    /// Get all markets as JSON values (for compatibility)
    pub async fn get_all_markets(&self) -> Vec<serde_json::Value> {
        self.get_all().into_iter()
            .map(|market| serde_json::json!({
                "id": market.id,
                "title": market.title,
                "description": market.description,
                "outcomes": market.outcomes.iter().map(|o| serde_json::json!({
                    "name": o.name,
                    "total_stake": o.total_stake
                })).collect::<Vec<_>>(),
                "total_liquidity": market.total_liquidity,
                "total_volume": market.total_volume,
                "resolution_time": market.resolution_time,
                "resolved": market.resolved,
                "winning_outcome": market.winning_outcome,
                "created_at": market.created_at,
                "verse_id": market.verse_id,
            }))
            .collect()
    }
    
    pub fn get_all(&self) -> Vec<Market> {
        self.markets.values().cloned().collect()
    }
    
    pub fn get_by_id(&self, id: u128) -> Option<&Market> {
        self.markets.get(&id)
    }
    
    pub fn search(&self, query: &str) -> Vec<Market> {
        let query_lower = query.to_lowercase();
        self.markets.values()
            .filter(|m| {
                m.title.to_lowercase().contains(&query_lower) ||
                m.description.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }
    
    pub fn get_by_category(&self, category: &str) -> Vec<Market> {
        // Map verse IDs to categories (simplified)
        let category_ranges = match category.to_lowercase().as_str() {
            "politics" => 1..10,
            "sports" => 10..20,
            "crypto" => 20..30,
            "finance" => 30..40,
            "technology" => 40..50,
            "entertainment" => 50..60,
            "climate" => 60..70,
            "science" => 70..80,
            "economics" => 80..90,
            "health" => 90..100,
            "energy" => 100..110,
            "retail" => 110..120,
            "legal" => 120..130,
            "media" => 130..140,
            _ => 0..0,
        };
        
        self.markets.values()
            .filter(|m| {
                m.verse_id.map_or(false, |v| category_ranges.contains(&(v as u32)))
            })
            .cloned()
            .collect()
    }
}