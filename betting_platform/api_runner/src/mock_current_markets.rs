use crate::types::{Market, MarketOutcome, AmmType};
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;

pub fn get_mock_current_markets() -> Vec<Market> {
    let current_timestamp = Utc::now().timestamp();
    let end_2024_timestamp = chrono::DateTime::parse_from_rfc3339("2024-12-31T23:59:59Z").unwrap().timestamp();
    
    vec![
        Market {
            id: 1000,
            title: "Will Bitcoin reach $100,000 by end of 2024?".to_string(),
            description: "This market resolves to 'Yes' if Bitcoin reaches or exceeds $100,000 USD on any major exchange before December 31, 2024 11:59 PM UTC.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 0, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1500000 },
                MarketOutcome { id: 1, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1000000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 2500000,
            total_volume: 15000000,
            resolution_time: end_2024_timestamp,
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z").unwrap().timestamp(),
            verse_id: Some(2), // Crypto verse
            current_price: 0.5,
        },
        Market {
            id: 1001,
            title: "Will Donald Trump win the 2024 US Presidential Election?".to_string(),
            description: "This market resolves to 'Yes' if Donald Trump wins the 2024 US Presidential Election.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 2, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 3000000 },
                MarketOutcome { id: 3, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 2000000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 5000000,
            total_volume: 50000000,
            resolution_time: chrono::DateTime::parse_from_rfc3339("2024-11-06T00:00:00Z").unwrap().timestamp(),
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2023-11-01T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(1), // Politics verse
            current_price: 0.5,
        },
        Market {
            id: 1002,
            title: "Will AI achieve AGI (Artificial General Intelligence) by 2027?".to_string(),
            description: "Resolves 'Yes' if a major AI research organization or company officially announces achieving AGI by end of 2027.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 4, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 800000 },
                MarketOutcome { id: 0, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1000000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 1800000,
            total_volume: 8000000,
            resolution_time: chrono::DateTime::parse_from_rfc3339("2027-12-31T23:59:59Z").unwrap().timestamp(),
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-02-01T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(9), // Technology verse
            current_price: 0.5,
        },
        Market {
            id: 1003,
            title: "Will Ethereum price exceed $10,000 in 2024?".to_string(),
            description: "This market resolves to 'Yes' if ETH trades above $10,000 USD on any major exchange in 2024.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 1, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 400000 },
                MarketOutcome { id: 2, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 800000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 1200000,
            total_volume: 6000000,
            resolution_time: end_2024_timestamp,
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-01-10T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(2), // Crypto verse
            current_price: 0.5,
        },
        Market {
            id: 1004,
            title: "Will SpaceX successfully land humans on Mars before 2030?".to_string(),
            description: "Resolves 'Yes' if SpaceX successfully lands at least one human on Mars before January 1, 2030.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 3, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1800000 },
                MarketOutcome { id: 4, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1200000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 3000000,
            total_volume: 12000000,
            resolution_time: chrono::DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z").unwrap().timestamp(),
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-03-01T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(6), // Space verse
            current_price: 0.5,
        },
        Market {
            id: 1005,
            title: "Will Taylor Swift release a new album in 2024?".to_string(),
            description: "This market resolves to 'Yes' if Taylor Swift officially releases a new studio album in 2024.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 0, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 600000 },
                MarketOutcome { id: 1, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 200000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 800000,
            total_volume: 3000000,
            resolution_time: end_2024_timestamp,
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-01-05T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(4), // Entertainment verse
            current_price: 0.5,
        },
        Market {
            id: 1006,
            title: "Will the S&P 500 close above 5,500 in 2024?".to_string(),
            description: "Resolves 'Yes' if the S&P 500 index closes above 5,500 on any trading day in 2024.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 2, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 1400000 },
                MarketOutcome { id: 3, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 800000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 2200000,
            total_volume: 10000000,
            resolution_time: end_2024_timestamp,
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-01-20T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(5), // Business verse
            current_price: 0.5,
        },
        Market {
            id: 1007,
            title: "Will there be a major earthquake (7.0+) in California in 2024?".to_string(),
            description: "Resolves 'Yes' if an earthquake of magnitude 7.0 or higher occurs in California in 2024.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 4, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 200000 },
                MarketOutcome { id: 0, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 400000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 600000,
            total_volume: 2000000,
            resolution_time: end_2024_timestamp,
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(11), // Environmental verse
            current_price: 0.5,
        },
        Market {
            id: 1008,
            title: "Will Apple release a foldable iPhone by 2025?".to_string(),
            description: "This market resolves to 'Yes' if Apple officially announces or releases a foldable iPhone model by end of 2025.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 1, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 500000 },
                MarketOutcome { id: 2, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1000000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 1500000,
            total_volume: 5000000,
            resolution_time: chrono::DateTime::parse_from_rfc3339("2025-12-31T23:59:59Z").unwrap().timestamp(),
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-02-15T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(9), // Technology verse
            current_price: 0.5,
        },
        Market {
            id: 1009,
            title: "Will the Kansas City Chiefs win Super Bowl LVIII?".to_string(),
            description: "Resolves 'Yes' if the Kansas City Chiefs win Super Bowl LVIII in February 2024.".to_string(),
            outcomes: vec![
                MarketOutcome { id: 3, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 2200000 },
                MarketOutcome { id: 4, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 1800000 }
            ],
            creator: Pubkey::default(),
            total_liquidity: 4000000,
            total_volume: 20000000,
            resolution_time: chrono::DateTime::parse_from_rfc3339("2024-02-12T00:00:00Z").unwrap().timestamp(),
            resolved: false,
            winning_outcome: None,
            amm_type: AmmType::Hybrid,
            created_at: chrono::DateTime::parse_from_rfc3339("2024-01-15T00:00:00Z").unwrap().timestamp(),
            verse_id: Some(3), // Sports verse
            current_price: 0.5,
        },
    ]
}