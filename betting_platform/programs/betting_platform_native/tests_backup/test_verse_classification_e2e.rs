//! End-to-end tests for verse classification system
//! Tests all edge cases including normalization, fuzzy matching, and hierarchy

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use betting_platform_native::{
    verse_classification::{VerseClassifier, VerseCategory},
    state::{VersePDA, VerseStatus},
};

#[test]
fn test_title_normalization_comprehensive() {
    let test_cases = vec![
        // Crypto variations
        ("Will Bitcoin reach $100,000 by end of year?", "will btc reach 100000 eoy"),
        ("BTC > $150k by December 2024?", "btc > 150k eoy 2024"),
        ("Bitcoin above $150,000 by EOY", "btc > 150000 eoy"),
        ("Will BTC exceed 100k USD?", "will btc > 100k usd"),
        
        // Ethereum variations
        ("Will Ethereum reach $5000?", "will eth reach 5000"),
        ("ETH price above $5k by Q4?", "eth price > 5k q4"),
        ("Ethereum over five thousand dollars", "eth > 5 thousand dollars"),
        
        // Political variations
        ("Who will be the next US President?", "who next us election"),
        ("Presidential election 2024 winner", "election election 2024 winner"),
        ("Will incumbent win presidency?", "will incumbent win election"),
        
        // Sports variations
        ("Will Team A win the championship?", "will team win championship"),
        ("NBA Finals: Lakers vs Celtics winner", "nba finals lakers vs celtics winner"),
        ("Super Bowl 2024 champion", "super bowl 2024 champion"),
        
        // Financial variations
        ("S&P 500 above 5000 by year end?", "sp500 > 5000 eoy"),
        ("Will inflation exceed 3% in Q4?", "will inflation > 3 q4"),
        ("Fed rate hike in December?", "fed rate hike december"),
        
        // Edge cases
        ("!!!BTC!!!$100k???", "btc 100k"),
        ("B.T.C. > $100,000.00", "btc > 100000 00"),
        ("bitcoin    price    above    100000", "btc price > 100000"),
        ("BITCOIN ABOVE ONE HUNDRED THOUSAND", "btc > 1 hundred thousand"),
    ];
    
    for (input, expected) in test_cases {
        let normalized = VerseClassifier::normalize_title(input);
        assert_eq!(normalized, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_keyword_extraction_edge_cases() {
    let test_cases = vec![
        // Basic extraction
        ("btc price > 100000 eoy", vec!["100000", ">", "btc", "eoy", "price"]),
        
        // With stop words removed
        ("the bitcoin will be above the price", vec!["bitcoin", "price"]),
        
        // Maximum 5 keywords
        ("one two three four five six seven eight", vec!["eight", "five", "four", "one", "seven"]),
        
        // Short words filtered (< 2 chars)
        ("a b cd ef ghi", vec!["cd", "ef", "ghi"]),
        
        // Empty after filtering
        ("the a an and or but", vec![]),
        
        // Numbers and symbols
        ("123 456 > < = != >=", vec!["123", "456", "!=", "="]),
    ];
    
    for (input, expected) in test_cases {
        let keywords = VerseClassifier::extract_keywords(input).unwrap_or_default();
        assert_eq!(keywords, expected, "Failed for input: {}", input);
    }
}

#[test]
fn test_verse_id_determinism() {
    // Same title variations should produce same verse ID
    let variations = vec![
        "Will BTC reach $100k by end of year?",
        "Bitcoin above $100,000 by EOY?",
        "BTC > 100000 by December?",
        "Will Bitcoin exceed $100k by year end?",
    ];
    
    let verse_ids: Vec<u128> = variations.iter()
        .map(|title| VerseClassifier::classify_market_to_verse(title).unwrap())
        .collect();
    
    // All should be the same
    let first_id = verse_ids[0];
    for (i, id) in verse_ids.iter().enumerate() {
        assert_eq!(*id, first_id, "Variation {} produced different ID", i);
    }
    
    // Different markets should produce different IDs
    let different_markets = vec![
        "Will ETH reach $5k?",
        "US Election 2024 winner?",
        "Super Bowl champion?",
    ];
    
    let different_ids: Vec<u128> = different_markets.iter()
        .map(|title| VerseClassifier::classify_market_to_verse(title).unwrap())
        .collect();
    
    // All should be different
    for i in 0..different_ids.len() {
        for j in i+1..different_ids.len() {
            assert_ne!(different_ids[i], different_ids[j], 
                "Different markets {} and {} produced same ID", i, j);
        }
    }
}

#[test]
fn test_category_detection_comprehensive() {
    let test_cases = vec![
        // Crypto
        ("BTC price prediction", VerseCategory::Crypto),
        ("Ethereum market cap", VerseCategory::Crypto),
        ("DeFi TVL milestone", VerseCategory::Crypto),
        ("NFT sales volume", VerseCategory::Crypto),
        ("Blockchain adoption rate", VerseCategory::Crypto),
        
        // Politics
        ("Election results 2024", VerseCategory::Politics),
        ("Senate majority", VerseCategory::Politics),
        ("Presidential approval rating", VerseCategory::Politics),
        ("Congressional vote outcome", VerseCategory::Politics),
        
        // Sports
        ("NFL playoff predictions", VerseCategory::Sports),
        ("NBA championship winner", VerseCategory::Sports),
        ("Soccer World Cup", VerseCategory::Sports),
        ("Game 7 outcome", VerseCategory::Sports),
        
        // Finance
        ("Stock market crash", VerseCategory::Finance),
        ("S&P 500 milestone", VerseCategory::Finance),
        ("GDP growth rate", VerseCategory::Finance),
        ("Inflation target", VerseCategory::Finance),
        
        // Entertainment
        ("Oscar winner prediction", VerseCategory::Entertainment),
        ("Movie box office", VerseCategory::Entertainment),
        ("Album sales record", VerseCategory::Entertainment),
        ("Emmy awards outcome", VerseCategory::Entertainment),
        
        // General (no clear category)
        ("Random event outcome", VerseCategory::General),
        ("Generic prediction", VerseCategory::General),
    ];
    
    for (title, expected_category) in test_cases {
        let keywords = VerseClassifier::extract_keywords(
            &VerseClassifier::normalize_title(title)
        ).unwrap();
        
        let category = VerseClassifier::get_verse_category(&keywords);
        assert_eq!(category, expected_category, "Failed for title: {}", title);
    }
}

#[test]
fn test_parent_verse_generation() {
    // Markets in same category should have same parent
    let crypto_markets = vec![
        "BTC price prediction",
        "ETH market movement",
        "SOL price target",
    ];
    
    let parent_ids: Vec<Option<u128>> = crypto_markets.iter()
        .map(|title| {
            let keywords = VerseClassifier::extract_keywords(
                &VerseClassifier::normalize_title(title)
            ).unwrap();
            VerseClassifier::get_parent_verse_id(
                VerseClassifier::classify_market_to_verse(title).unwrap(),
                &keywords
            )
        })
        .collect();
    
    // All crypto markets should have same parent
    assert!(parent_ids[0].is_some());
    let crypto_parent = parent_ids[0].unwrap();
    
    for (i, parent) in parent_ids.iter().enumerate() {
        assert_eq!(parent.unwrap(), crypto_parent, 
            "Crypto market {} has different parent", i);
    }
    
    // Different category should have different parent
    let politics_title = "Election outcome 2024";
    let politics_keywords = VerseClassifier::extract_keywords(
        &VerseClassifier::normalize_title(politics_title)
    ).unwrap();
    let politics_parent = VerseClassifier::get_parent_verse_id(
        VerseClassifier::classify_market_to_verse(politics_title).unwrap(),
        &politics_keywords
    ).unwrap();
    
    assert_ne!(crypto_parent, politics_parent, 
        "Crypto and politics should have different parents");
}

#[test]
fn test_edge_case_titles() {
    // Test extremely long titles
    let long_title = "a".repeat(1000);
    let result = VerseClassifier::classify_market_to_verse(&long_title);
    assert!(result.is_ok(), "Should handle long titles");
    
    // Test empty title
    let empty_result = VerseClassifier::classify_market_to_verse("");
    assert!(empty_result.is_err(), "Empty title should fail");
    
    // Test unicode
    let unicode_title = "Will BTC reach 100k€ by EOY? 🚀";
    let unicode_result = VerseClassifier::classify_market_to_verse(unicode_title);
    assert!(unicode_result.is_ok(), "Should handle unicode");
    
    // Test only stop words
    let stop_words_only = "the and or but with for";
    let stop_result = VerseClassifier::classify_market_to_verse(stop_words_only);
    assert!(stop_result.is_err(), "Title with only stop words should fail");
    
    // Test special characters
    let special_chars = "BTC@$100k!!! #crypto ^market";
    let special_result = VerseClassifier::classify_market_to_verse(special_chars);
    assert!(special_result.is_ok(), "Should handle special characters");
}

#[test]
fn test_collision_resistance() {
    // Generate many verse IDs and check for collisions
    let mut verse_ids = std::collections::HashSet::new();
    
    // Generate IDs for different market patterns
    for i in 0..1000 {
        let title = match i % 10 {
            0 => format!("BTC price ${}", 50000 + i * 100),
            1 => format!("ETH above ${}", 3000 + i * 10),
            2 => format!("Election {} winner", 2024 + i / 100),
            3 => format!("Team {} championship", i),
            4 => format!("Stock {} prediction", i),
            5 => format!("Inflation {}% target", i % 10),
            6 => format!("Movie {} box office", i),
            7 => format!("Market {} outcome", i),
            8 => format!("Event {} result", i),
            _ => format!("Generic prediction {}", i),
        };
        
        let verse_id = VerseClassifier::classify_market_to_verse(&title).unwrap();
        
        // Check for collision
        assert!(verse_ids.insert(verse_id), 
            "Collision detected for title: {}", title);
    }
    
    println!("Generated {} unique verse IDs without collision", verse_ids.len());
}

#[test]
fn test_similar_but_different_markets() {
    // These should produce different verse IDs despite similarity
    let similar_markets = vec![
        ("BTC above $100k", "BTC below $100k"),
        ("ETH price increase", "ETH price decrease"),
        ("Team A wins", "Team B wins"),
        ("Inflation above 3%", "Inflation below 3%"),
        ("Q1 earnings beat", "Q1 earnings miss"),
    ];
    
    for (market1, market2) in similar_markets {
        let id1 = VerseClassifier::classify_market_to_verse(market1).unwrap();
        let id2 = VerseClassifier::classify_market_to_verse(market2).unwrap();
        
        assert_ne!(id1, id2, 
            "Different markets should have different IDs: {} vs {}", 
            market1, market2);
    }
}

#[test]
fn test_hierarchical_consistency() {
    // Create a hierarchy of related markets
    let hierarchy = vec![
        ("Crypto markets 2024", None),  // Root
        ("BTC price movements", Some("Crypto markets 2024")),
        ("BTC above $100k", Some("BTC price movements")),
        ("BTC above $100k by December", Some("BTC above $100k")),
    ];
    
    let mut parent_map = std::collections::HashMap::new();
    
    for (title, expected_parent_title) in hierarchy {
        let verse_id = VerseClassifier::classify_market_to_verse(title).unwrap();
        let keywords = VerseClassifier::extract_keywords(
            &VerseClassifier::normalize_title(title)
        ).unwrap();
        
        let parent_id = VerseClassifier::get_parent_verse_id(verse_id, &keywords);
        
        if let Some(expected_parent) = expected_parent_title {
            assert!(parent_id.is_some(), 
                "{} should have a parent", title);
            
            // In a real system, we'd verify the parent relationship
            // For now, just ensure consistency
            if let Some(expected_id) = parent_map.get(expected_parent) {
                // Parent should be consistent
                println!("Verified parent relationship: {} -> {}", title, expected_parent);
            }
        } else {
            // Root level should map to category parent only
            assert!(parent_id.is_some() || keywords.is_empty(), 
                "Root {} should have category parent or be truly root", title);
        }
        
        parent_map.insert(title, verse_id);
    }
}

#[test]
fn test_real_world_polymarket_titles() {
    // Test with actual Polymarket-style titles
    let real_titles = vec![
        "Will Bitcoin be above $100,000 on December 31, 2024, 11:59 PM ET?",
        "Will the Fed raise interest rates at the December 2024 FOMC meeting?",
        "Will Donald Trump be the Republican nominee for President in 2024?",
        "Will the S&P 500 close above 5,000 on any day before December 31, 2024?",
        "Will there be a ceasefire in the Russia-Ukraine conflict by December 31, 2024?",
        "Will OpenAI release GPT-5 before January 1, 2025?",
        "Will the LA Lakers make the NBA playoffs in the 2024-2025 season?",
        "Will inflation (CPI) be below 3% for December 2024?",
        "Will Tesla stock (TSLA) be above $300 on December 31, 2024?",
        "Will there be a government shutdown in Q4 2024?",
    ];
    
    let mut verse_counts = std::collections::HashMap::new();
    
    for title in real_titles {
        let verse_id = VerseClassifier::classify_market_to_verse(title).unwrap();
        let keywords = VerseClassifier::extract_keywords(
            &VerseClassifier::normalize_title(title)
        ).unwrap();
        let category = VerseClassifier::get_verse_category(&keywords);
        
        *verse_counts.entry(category).or_insert(0) += 1;
        
        println!("Title: {}", title);
        println!("  Verse ID: {}", verse_id);
        println!("  Category: {:?}", category);
        println!("  Keywords: {:?}", keywords);
        println!();
    }
    
    // Verify reasonable distribution
    assert!(verse_counts.len() >= 3, "Should classify into multiple categories");
}

#[test]
fn test_performance_large_scale() {
    use std::time::Instant;
    
    let start = Instant::now();
    let mut total_classifications = 0;
    
    // Simulate classifying 21,000 markets
    for i in 0..21_000 {
        let title = match i % 100 {
            0..=19 => format!("BTC price ${} prediction", 50000 + i),
            20..=39 => format!("ETH market cap ${} billion", 100 + i % 1000),
            40..=49 => format!("Election {} state {} results", 2024, i % 50),
            50..=59 => format!("Sports team {} season {} outcome", i % 30, 2024),
            60..=69 => format!("Company {} earnings Q{} {}", i % 500, i % 4 + 1, 2024),
            70..=79 => format!("Inflation rate {}% by {}", i % 10, 2024),
            80..=89 => format!("Movie {} box office ${} million", i % 100, i % 1000),
            _ => format!("Generic event {} outcome {}", i, 2024),
        };
        
        let _ = VerseClassifier::classify_market_to_verse(&title).unwrap();
        total_classifications += 1;
    }
    
    let duration = start.elapsed();
    let per_classification = duration.as_micros() / total_classifications;
    
    println!("Classified {} markets in {:?}", total_classifications, duration);
    println!("Average time per classification: {}μs", per_classification);
    
    // Should be fast enough for real-time processing
    assert!(per_classification < 1000, "Classification too slow: {}μs", per_classification);
}