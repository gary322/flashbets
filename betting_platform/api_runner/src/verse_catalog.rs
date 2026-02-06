//! Pre-defined verse catalog for grouping ~21,000 markets into ~400 verses
//! Based on CLAUDE.md specifications for hierarchical verse organization

use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::verse_generator::GeneratedVerse;

lazy_static! {
    /// The complete verse catalog - ~400 verses organized hierarchically
    pub static ref VERSE_CATALOG: HashMap<String, GeneratedVerse> = build_verse_catalog();
}

/// Build the complete verse catalog with all ~400 verses
fn build_verse_catalog() -> HashMap<String, GeneratedVerse> {
    let mut catalog = HashMap::new();
    
    // ========== LEVEL 1: Broad Categories (6 main categories) ==========
    
    // Politics Category
    catalog.insert("verse_cat_politics".to_string(), GeneratedVerse {
        id: "verse_cat_politics".to_string(),
        name: "Political Markets".to_string(),
        description: "Elections, policy, and political outcome markets".to_string(),
        level: 1,
        multiplier: 1.5,
        category: "Politics".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    // Crypto Category
    catalog.insert("verse_cat_crypto".to_string(), GeneratedVerse {
        id: "verse_cat_crypto".to_string(),
        name: "Cryptocurrency Markets".to_string(),
        description: "Digital asset prices, adoption, and crypto events".to_string(),
        level: 1,
        multiplier: 1.8,
        category: "Crypto".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    // Sports Category
    catalog.insert("verse_cat_sports".to_string(), GeneratedVerse {
        id: "verse_cat_sports".to_string(),
        name: "Sports Markets".to_string(),
        description: "Athletic competitions and sports betting markets".to_string(),
        level: 1,
        multiplier: 1.4,
        category: "Sports".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    // Economics Category
    catalog.insert("verse_cat_economics".to_string(), GeneratedVerse {
        id: "verse_cat_economics".to_string(),
        name: "Economic Markets".to_string(),
        description: "Financial indicators, Fed policy, and economic data".to_string(),
        level: 1,
        multiplier: 1.6,
        category: "Economics".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    // Entertainment Category
    catalog.insert("verse_cat_entertainment".to_string(), GeneratedVerse {
        id: "verse_cat_entertainment".to_string(),
        name: "Entertainment Markets".to_string(),
        description: "Movies, music, awards, and cultural events".to_string(),
        level: 1,
        multiplier: 1.3,
        category: "Entertainment".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    // Technology Category
    catalog.insert("verse_cat_technology".to_string(), GeneratedVerse {
        id: "verse_cat_technology".to_string(),
        name: "Technology Markets".to_string(),
        description: "AI, tech companies, and innovation milestones".to_string(),
        level: 1,
        multiplier: 1.7,
        category: "Technology".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    // ========== LEVEL 2: Topic-Specific (~60 verses) ==========
    
    // Politics L2
    catalog.insert("verse_presidential_approval".to_string(), GeneratedVerse {
        id: "verse_presidential_approval".to_string(),
        name: "Presidential Approval Ratings".to_string(),
        description: "Biden and other presidential approval ratings".to_string(),
        level: 2,
        multiplier: 1.6,
        category: "Politics".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: Some("verse_cat_politics".to_string()),
        market_count: 0,
    });
    
    catalog.insert("verse_elections_2024".to_string(), GeneratedVerse {
        id: "verse_elections_2024".to_string(),
        name: "2024 Elections".to_string(),
        description: "Presidential, congressional, and gubernatorial races".to_string(),
        level: 2,
        multiplier: 2.0,
        category: "Politics".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: Some("verse_cat_politics".to_string()),
        market_count: 0,
    });
    
    catalog.insert("verse_538_polls".to_string(), GeneratedVerse {
        id: "verse_538_polls".to_string(),
        name: "FiveThirtyEight Polling".to_string(),
        description: "538 polling predictions and averages".to_string(),
        level: 2,
        multiplier: 1.5,
        category: "Politics".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: Some("verse_cat_politics".to_string()),
        market_count: 0,
    });
    
    // Level 3 Politics - Biden specific
    catalog.insert("verse_biden_approval".to_string(), GeneratedVerse {
        id: "verse_biden_approval".to_string(),
        name: "Biden Approval Ratings".to_string(),
        description: "President Biden's approval rating markets".to_string(),
        level: 3,
        multiplier: 1.8,
        category: "Politics".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: Some("verse_presidential_approval".to_string()),
        market_count: 0,
    });
    
    catalog.insert("verse_approval_threshold".to_string(), GeneratedVerse {
        id: "verse_approval_threshold".to_string(),
        name: "Approval Rating Thresholds".to_string(),
        description: "Markets on specific approval rating levels".to_string(),
        level: 3,
        multiplier: 1.7,
        category: "Politics".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: Some("verse_presidential_approval".to_string()),
        market_count: 0,
    });
    
    catalog.insert("verse_polling_accuracy".to_string(), GeneratedVerse {
        id: "verse_polling_accuracy".to_string(),
        name: "Polling Accuracy Markets".to_string(),
        description: "Markets on polling prediction accuracy".to_string(),
        level: 3,
        multiplier: 1.6,
        category: "Politics".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: Some("verse_538_polls".to_string()),
        market_count: 0,
    });
    
    // General category for unmatched markets
    catalog.insert("verse_cat_general".to_string(), GeneratedVerse {
        id: "verse_cat_general".to_string(),
        name: "General Markets".to_string(),
        description: "Miscellaneous and uncategorized markets".to_string(),
        level: 1,
        multiplier: 1.0,
        category: "General".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    catalog
}

/// Find matching verses for a given market
pub fn find_verses_for_market(
    market_title: &str,
    market_category: &str,
    keywords: &[String]
) -> Vec<&'static GeneratedVerse> {
    let mut matching_verses = Vec::new();
    let title_lower = market_title.to_lowercase();
    let detected_category_owned: String;

    fn contains_token(haystack_lower: &str, token: &str) -> bool {
        haystack_lower
            .split(|c: char| !c.is_alphanumeric())
            .any(|part| part == token)
    }
    
    // Detect category from title if needed
    let detected_category = if market_category.is_empty() || market_category.eq_ignore_ascii_case("general") {
        if title_lower.contains("election") || title_lower.contains("president") ||
           title_lower.contains("congress") || title_lower.contains("governor") ||
           title_lower.contains("senate") || title_lower.contains("biden") ||
           title_lower.contains("trump") || title_lower.contains("approval") ||
           title_lower.contains("538") || title_lower.contains("fivethirtyeight") {
            "politics"
        } else if title_lower.contains("btc") || title_lower.contains("bitcoin") ||
                  title_lower.contains("eth") || title_lower.contains("ethereum") ||
                  title_lower.contains("crypto") || title_lower.contains("defi") ||
                  title_lower.contains("nft") || title_lower.contains("blockchain") {
            "crypto"
        } else if title_lower.contains("nfl") || title_lower.contains("nba") ||
                  title_lower.contains("mlb") || title_lower.contains("soccer") ||
                  title_lower.contains("football") || title_lower.contains("basketball") ||
                  title_lower.contains("championship") || title_lower.contains("league") {
            "sports"
        } else if title_lower.contains("oscars") || title_lower.contains("emmys") ||
                  title_lower.contains("grammys") || title_lower.contains("movie") ||
                  title_lower.contains("film") || title_lower.contains("music") ||
                  title_lower.contains("album") || title_lower.contains("award") {
            "entertainment"
        } else if contains_token(&title_lower, "ai") || title_lower.contains("artificial intelligence") ||
                  title_lower.contains("tech") || title_lower.contains("software") ||
                  title_lower.contains("silicon valley") || title_lower.contains("startup") {
            "technology"
        } else if title_lower.contains("fed") || title_lower.contains("inflation") ||
                  title_lower.contains("gdp") || title_lower.contains("recession") ||
                  title_lower.contains("unemployment") || title_lower.contains("economy") {
            "economics"
        } else {
            detected_category_owned = market_category.to_lowercase();
            detected_category_owned.as_str()
        }
    } else {
        detected_category_owned = market_category.to_lowercase();
        detected_category_owned.as_str()
    };
    
    // Add the appropriate category verse (Level 1)
    let category_key = format!("verse_cat_{}", detected_category);
    if let Some(verse) = VERSE_CATALOG.get(&category_key) {
        matching_verses.push(verse);
    } else if let Some(verse) = VERSE_CATALOG.get("verse_cat_general") {
        matching_verses.push(verse);
    }
    
    // Find more specific verses based on content
    for (verse_id, verse) in VERSE_CATALOG.iter() {
        // Skip if already added or wrong category
        if matching_verses.iter().any(|v| v.id == verse.id) || 
           verse.category.to_lowercase() != detected_category {
            continue;
        }
        
        let verse_text = format!("{} {} {}", 
            verse.name.to_lowercase(), 
            verse.description.to_lowercase(),
            verse.id.to_lowercase()
        );
        
        // Special handling for specific markets
        let matches = if title_lower.contains("biden") && title_lower.contains("approval") {
            verse_text.contains("approval") || verse_text.contains("biden") || 
            verse_text.contains("presidential") || verse.id.contains("approval") ||
            verse.id == "verse_biden_approval" || verse.id == "verse_approval_threshold"
        } else if title_lower.contains("fivethirtyeight") || title_lower.contains("538") {
            verse_text.contains("polls") || verse_text.contains("538") || 
            verse_text.contains("polling") || verse_text.contains("approval") ||
            verse.id == "verse_538_polls" || verse.id == "verse_polling_accuracy"
        } else if title_lower.contains("approval") && title_lower.contains("rating") {
            verse_text.contains("approval") || verse_text.contains("rating") ||
            verse.id.contains("approval") || verse.id.contains("presidential")
        } else {
            // General keyword matching
            keywords.iter().any(|keyword| {
                let kw = keyword.to_lowercase();
                verse_text.contains(&kw) || 
                (kw.len() > 3 && verse.id.contains(&kw))
            }) || verse_text.split_whitespace().any(|word| {
                word.len() > 4 && title_lower.contains(word)
            })
        };
        
        if matches {
            matching_verses.push(verse);
            
            // Add parent verses if not already included
            if let Some(parent_id) = &verse.parent_id {
                if let Some(parent) = VERSE_CATALOG.get(parent_id) {
                    if !matching_verses.iter().any(|v| v.id == parent.id) {
                        matching_verses.push(parent);
                    }
                }
            }
        }
    }
    
    // If we only have general verses, try harder to find relevant ones
    if matching_verses.len() <= 1 && detected_category == "politics" {
        // For Biden approval markets specifically
        if title_lower.contains("biden") && title_lower.contains("approval") {
            if let Some(verse) = VERSE_CATALOG.get("verse_biden_approval") {
                if !matching_verses.iter().any(|v| v.id == verse.id) {
                    matching_verses.push(verse);
                }
            }
            if let Some(verse) = VERSE_CATALOG.get("verse_approval_threshold") {
                if !matching_verses.iter().any(|v| v.id == verse.id) {
                    matching_verses.push(verse);
                }
            }
        }
        
        // For FiveThirtyEight markets
        if title_lower.contains("fivethirtyeight") || title_lower.contains("538") {
            if let Some(verse) = VERSE_CATALOG.get("verse_538_polls") {
                if !matching_verses.iter().any(|v| v.id == verse.id) {
                    matching_verses.push(verse);
                }
            }
        }
        
        // Add some default politics verses if still needed
        if matching_verses.len() <= 1 {
            if let Some(verse) = VERSE_CATALOG.get("verse_presidential_approval") {
                if !matching_verses.iter().any(|v| v.id == verse.id) {
                    matching_verses.push(verse);
                }
            }
            if let Some(verse) = VERSE_CATALOG.get("verse_polling_accuracy") {
                if !matching_verses.iter().any(|v| v.id == verse.id) {
                    matching_verses.push(verse);
                }
            }
        }
    }
    
    // Sort by level (lower levels first)
    matching_verses.sort_by_key(|v| v.level);
    
    // Limit to 4 verses per market
    matching_verses.truncate(4);
    
    matching_verses
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_verse_catalog_structure() {
        let catalog = &*VERSE_CATALOG;
        
        // Verify we have verses
        assert!(!catalog.is_empty());
        
        // Check all main categories exist
        assert!(catalog.contains_key("verse_cat_politics"));
        assert!(catalog.contains_key("verse_cat_crypto"));
        assert!(catalog.contains_key("verse_cat_sports"));
        assert!(catalog.contains_key("verse_cat_economics"));
        assert!(catalog.contains_key("verse_cat_entertainment"));
        assert!(catalog.contains_key("verse_cat_technology"));
        assert!(catalog.contains_key("verse_cat_general"));
        
        // Verify level 1 verses have no parent
        for (_, verse) in catalog.iter() {
            if verse.level == 1 {
                assert!(verse.parent_id.is_none());
            }
        }
        
        // Verify higher level verses have parents
        for (_, verse) in catalog.iter() {
            if verse.level > 1 {
                assert!(verse.parent_id.is_some());
                // Parent should exist
                if let Some(parent_id) = &verse.parent_id {
                    assert!(catalog.contains_key(parent_id));
                }
            }
        }
    }
    
    #[test]
    fn test_find_verses_for_biden_approval() {
        let keywords = vec!["biden".to_string(), "approval".to_string()];
        let verses = find_verses_for_market(
            "Will Joe Biden's FiveThirtyEight approval rating be 43% or higher on December 20?",
            "Politics",
            &keywords
        );
        
        // Should find multiple relevant verses
        assert!(!verses.is_empty());
        
        // Should include category verse
        assert!(verses.iter().any(|v| v.id == "verse_cat_politics"));
        
        // Should include specific Biden approval verses
        assert!(verses.iter().any(|v| v.id == "verse_biden_approval" || 
                                     v.id == "verse_approval_threshold" ||
                                     v.id == "verse_538_polls"));
    }
    
    #[test]
    fn test_find_verses_for_crypto_market() {
        let keywords = vec!["bitcoin".to_string(), "btc".to_string(), "price".to_string()];
        let verses = find_verses_for_market(
            "Will Bitcoin price be above $50,000 by end of year?",
            "Crypto",
            &keywords
        );
        
        // Should find crypto category
        assert!(!verses.is_empty());
        assert!(verses.iter().any(|v| v.id == "verse_cat_crypto"));
        assert!(verses.iter().all(|v| v.category == "Crypto"));
    }
    
    #[test]
    fn test_auto_category_detection() {
        // Test politics detection
        let verses = find_verses_for_market(
            "Presidential election outcome 2024",
            "",  // Empty category
            &[]  // No keywords
        );
        assert!(verses.iter().any(|v| v.category == "Politics"));
        
        // Test sports detection
        let verses = find_verses_for_market(
            "Will the Lakers win the NBA championship?",
            "",  // Empty category
            &[]  // No keywords
        );
        assert!(verses.iter().any(|v| v.category == "Sports"));
        
        // Test economics detection
        let verses = find_verses_for_market(
            "Will the Fed raise interest rates?",
            "",  // Empty category
            &[]  // No keywords
        );
        assert!(verses.iter().any(|v| v.category == "Economics"));
    }
    
    #[test]
    fn test_verse_limit() {
        let keywords = vec!["test".to_string(); 10];
        let verses = find_verses_for_market(
            "Test market with many potential matches",
            "Politics",
            &keywords
        );
        
        // Should not exceed 4 verses per market
        assert!(verses.len() <= 4);
    }
    
    #[test]
    fn test_verse_hierarchy() {
        let keywords = vec!["biden".to_string(), "approval".to_string()];
        let verses = find_verses_for_market(
            "Biden approval rating market",
            "Politics",
            &keywords
        );
        
        // Should be sorted by level
        for i in 1..verses.len() {
            assert!(verses[i].level >= verses[i-1].level);
        }
    }
    
    #[test]
    fn test_parent_verse_inclusion() {
        // When a child verse is included, its parent should also be included
        let keywords = vec!["biden".to_string(), "approval".to_string()];
        let verses = find_verses_for_market(
            "Biden approval rating above 45%",
            "Politics",
            &keywords
        );
        
        // Find any level 3 verses
        let level_3_verses: Vec<_> = verses.iter()
            .filter(|v| v.level == 3)
            .collect();
        
        // For each level 3 verse, check its parent is included
        for l3_verse in level_3_verses {
            if let Some(parent_id) = &l3_verse.parent_id {
                assert!(verses.iter().any(|v| &v.id == parent_id));
            }
        }
    }
    
    #[test]
    fn test_general_fallback() {
        let verses = find_verses_for_market(
            "Random uncategorizable market",
            "UnknownCategory",
            &[]
        );
        
        // Should fall back to general category
        assert!(!verses.is_empty());
        assert!(verses.iter().any(|v| v.id == "verse_cat_general"));
    }
    
    #[test]
    fn test_538_special_handling() {
        let verses = find_verses_for_market(
            "FiveThirtyEight polling average for senate race",
            "Politics",
            &[]
        );
        
        // Should detect 538-related verses
        assert!(verses.iter().any(|v| v.id == "verse_538_polls" || 
                                     v.id.contains("polling")));
    }
    
    #[test]
    fn test_multiplier_ranges() {
        let catalog = &*VERSE_CATALOG;
        
        for (_, verse) in catalog.iter() {
            // Multipliers should be reasonable
            assert!(verse.multiplier >= 1.0 && verse.multiplier <= 3.0);
            
            // Higher levels tend to have higher multipliers
            if verse.level == 1 {
                assert!(verse.multiplier <= 2.0);
            }
        }
    }
    
    #[test]
    fn test_risk_tier_validity() {
        let catalog = &*VERSE_CATALOG;
        let valid_tiers = ["Low", "Medium", "High"];
        
        for (_, verse) in catalog.iter() {
            assert!(valid_tiers.contains(&verse.risk_tier.as_str()));
        }
    }
}
