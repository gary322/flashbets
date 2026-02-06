//! Verse generation from Polymarket markets
//! Groups markets into hierarchical verses with leverage multipliers

use serde::{Serialize, Deserialize};
use crate::verse_catalog;

/// Stop words to filter during keyword extraction
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "will", "be", "is", "are", "was", "were",
    "been", "have", "has", "had", "do", "does", "did", "shall", "should",
    "may", "might", "must", "can", "could", "would", "there", "here",
    "that", "this", "these", "those", "what", "which", "who", "whom",
];

/// Common replacements for normalization
const REPLACEMENTS: &[(&str, &str)] = &[
    // Crypto symbols
    ("bitcoin", "btc"),
    ("ethereum", "eth"),
    ("solana", "sol"),
    ("polygon", "matic"),
    ("dogecoin", "doge"),
    
    // Political terms
    ("presidential", "election"),
    ("president", "election"),
    ("trump", "trump"),
    ("biden", "biden"),
    
    // Time periods
    ("end of year", "eoy"),
    ("end of month", "eom"),
    
    // Price comparisons
    ("above", ">"),
    ("below", "<"),
    ("over", ">"),
    ("under", "<"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedVerse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub level: u8,
    pub multiplier: f64,
    pub category: String,
    pub risk_tier: String,
    pub parent_id: Option<String>,
    pub market_count: usize,
}

#[derive(Debug, Clone)]
pub struct VerseGenerator {
    // No longer need verse_cache since we use the catalog
}

impl VerseGenerator {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Find matching verses for a Polymarket market from the catalog
    pub fn generate_verses_for_market(&mut self, market: &serde_json::Value) -> Vec<GeneratedVerse> {
        let title = market["title"].as_str()
            .or_else(|| market["question"].as_str())
            .or_else(|| market["description"].as_str())
            .unwrap_or("Unknown Market");
            
        let category = market["category"].as_str().unwrap_or("General");
        
        // Extract keywords from title
        let keywords = self.extract_keywords(title);
        
        // Log for debugging
        println!("Market: {}", title);
        println!("Category: {}", category);
        println!("Keywords: {:?}", keywords);
        
        // Find matching verses from the catalog
        let matching_verses = verse_catalog::find_verses_for_market(title, category, &keywords);
        
        println!("Found {} matching verses", matching_verses.len());
        if !matching_verses.is_empty() {
            println!("Verse samples: {:?}", matching_verses.iter().take(3).map(|v| &v.name).collect::<Vec<_>>());
        }
        
        // Convert references to owned values
        matching_verses.into_iter()
            .map(|verse| verse.clone())
            .collect()
    }
    
    fn extract_keywords(&self, title: &str) -> Vec<String> {
        let mut normalized = title.to_lowercase();
        
        // Apply replacements
        for (from, to) in REPLACEMENTS {
            normalized = normalized.replace(from, to);
        }
        
        // Extract words
        let words: Vec<String> = normalized
            .split_whitespace()
            .filter(|w| !STOP_WORDS.contains(w) && w.len() >= 2)
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect())
            .filter(|w: &String| !w.is_empty())
            .collect();
            
        words
    }
}

impl Default for VerseGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_verse_generator_creation() {
        let generator = VerseGenerator::new();
        // Generator should be created successfully
        let _ = generator; // Just to use it
    }
    
    #[test]
    fn test_extract_keywords() {
        let generator = VerseGenerator::new();
        
        // Test basic keyword extraction
        let keywords = generator.extract_keywords("Will Bitcoin price be above $50,000 by end of year?");
        assert!(keywords.contains(&"btc".to_string())); // bitcoin -> btc replacement
        assert!(keywords.contains(&"price".to_string()));
        assert!(keywords.contains(&"50000".to_string()));
        assert!(keywords.contains(&"eoy".to_string())); // end of year -> eoy replacement
        
        // Should not contain stop words
        assert!(!keywords.contains(&"will".to_string()));
        assert!(!keywords.contains(&"be".to_string()));
        assert!(!keywords.contains(&"by".to_string()));
    }
    
    #[test]
    fn test_stop_words_filtering() {
        let generator = VerseGenerator::new();
        
        let keywords = generator.extract_keywords("The president will be in the White House");
        
        // Stop words should be filtered
        for stop_word in STOP_WORDS {
            assert!(!keywords.contains(&stop_word.to_string()));
        }
        
        // Content words should remain
        assert!(keywords.contains(&"election".to_string())); // president -> election
        assert!(keywords.contains(&"white".to_string()));
        assert!(keywords.contains(&"house".to_string()));
    }
    
    #[test]
    fn test_replacements() {
        let generator = VerseGenerator::new();
        
        // Test crypto replacements
        let keywords = generator.extract_keywords("Bitcoin and Ethereum prices");
        assert!(keywords.contains(&"btc".to_string()));
        assert!(keywords.contains(&"eth".to_string()));
        
        // Test political replacements
        let keywords = generator.extract_keywords("Presidential race with Biden and Trump");
        assert!(keywords.contains(&"election".to_string())); // presidential -> election
        assert!(keywords.contains(&"biden".to_string()));
        assert!(keywords.contains(&"trump".to_string()));
        
        // Test time period replacements
        let keywords = generator.extract_keywords("By end of month prediction");
        assert!(keywords.contains(&"eom".to_string()));
    }
    
    #[test]
    fn test_special_character_removal() {
        let generator = VerseGenerator::new();
        
        let keywords = generator.extract_keywords("Will S&P 500 hit 5,000? #markets @prediction");
        
        // Should extract alphanumeric only
        assert!(keywords.contains(&"sp".to_string()));
        assert!(keywords.contains(&"500".to_string()));
        assert!(keywords.contains(&"hit".to_string()));
        assert!(keywords.contains(&"5000".to_string()));
        assert!(keywords.contains(&"markets".to_string()));
        assert!(keywords.contains(&"prediction".to_string()));
    }
    
    #[test]
    fn test_generate_verses_for_market() {
        let mut generator = VerseGenerator::new();
        
        // Test Biden approval market
        let market = json!({
            "title": "Will Joe Biden's FiveThirtyEight approval rating be 43% or higher on December 20?",
            "category": "Politics"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // Should find verses
        assert!(!verses.is_empty());
        
        // Should include politics category
        assert!(verses.iter().any(|v| v.category == "Politics"));
        
        // Should have hierarchical levels
        let levels: Vec<u8> = verses.iter().map(|v| v.level).collect();
        assert!(levels.contains(&1)); // Should have category level
    }
    
    #[test]
    fn test_market_without_category() {
        let mut generator = VerseGenerator::new();
        
        let market = json!({
            "title": "Will the Lakers win the NBA championship?"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // Should still find verses based on title
        assert!(!verses.is_empty());
        
        // Should detect sports category
        assert!(verses.iter().any(|v| v.category == "Sports"));
    }
    
    #[test]
    fn test_market_with_description_fallback() {
        let mut generator = VerseGenerator::new();
        
        // Market with no title but has description
        let market = json!({
            "description": "Bitcoin price prediction for end of year",
            "category": "Crypto"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // Should use description as fallback
        assert!(!verses.is_empty());
        assert!(verses.iter().any(|v| v.category == "Crypto"));
    }
    
    #[test]
    fn test_empty_market() {
        let mut generator = VerseGenerator::new();
        
        let market = json!({});
        
        let verses = generator.generate_verses_for_market(&market);
        
        // Should still return at least general category
        assert!(!verses.is_empty());
    }
    
    #[test]
    fn test_keyword_length_filter() {
        let generator = VerseGenerator::new();
        
        let keywords = generator.extract_keywords("I a an to it ok");
        
        // Should filter out words shorter than 2 characters
        assert!(keywords.contains(&"ok".to_string()));
        assert!(!keywords.contains(&"i".to_string()));
        assert!(!keywords.contains(&"a".to_string()));
    }
    
    #[test]
    fn test_verse_multiplier_inheritance() {
        let mut generator = VerseGenerator::new();
        
        let market = json!({
            "title": "Biden approval rating market",
            "category": "Politics"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // All verses should have valid multipliers
        for verse in &verses {
            assert!(verse.multiplier >= 1.0);
            assert!(verse.multiplier <= 3.0);
        }
    }
    
    #[test]
    fn test_risk_tier_assignment() {
        let mut generator = VerseGenerator::new();
        
        let market = json!({
            "title": "Cryptocurrency volatility prediction",
            "category": "Crypto"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // All verses should have valid risk tiers
        let valid_tiers = ["Low", "Medium", "High"];
        for verse in &verses {
            assert!(valid_tiers.contains(&verse.risk_tier.as_str()));
        }
    }
    
    #[test]
    fn test_parent_child_consistency() {
        let mut generator = VerseGenerator::new();
        
        let market = json!({
            "title": "Presidential election betting market",
            "category": "Politics"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // Check parent-child relationships
        for verse in &verses {
            if let Some(parent_id) = &verse.parent_id {
                // Parent should exist in the result set or catalog
                let parent_exists = verses.iter().any(|v| &v.id == parent_id) ||
                                   verse_catalog::VERSE_CATALOG.contains_key(parent_id);
                assert!(parent_exists);
            }
        }
    }
    
    #[test]
    fn test_max_verses_per_market() {
        let mut generator = VerseGenerator::new();
        
        // Market that could match many verses
        let market = json!({
            "title": "Politics election president biden trump approval rating congress senate",
            "category": "Politics"
        });
        
        let verses = generator.generate_verses_for_market(&market);
        
        // Should be limited to 4 verses maximum
        assert!(verses.len() <= 4);
    }
    
    #[test]
    fn test_case_insensitive_matching() {
        let mut generator = VerseGenerator::new();
        
        let market1 = json!({
            "title": "BITCOIN PRICE PREDICTION",
            "category": "CRYPTO"
        });
        
        let market2 = json!({
            "title": "bitcoin price prediction",
            "category": "crypto"
        });
        
        let verses1 = generator.generate_verses_for_market(&market1);
        let verses2 = generator.generate_verses_for_market(&market2);
        
        // Should produce same results regardless of case
        assert_eq!(verses1.len(), verses2.len());
        for (v1, v2) in verses1.iter().zip(verses2.iter()) {
            assert_eq!(v1.id, v2.id);
        }
    }
}
