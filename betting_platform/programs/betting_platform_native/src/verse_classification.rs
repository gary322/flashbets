//! Verse classification system
//!
//! Groups 21,000 markets into ~400 verses using deterministic classification

use solana_program::{
    keccak::hash,
    msg,
    program_error::ProgramError,
};

use crate::error::BettingPlatformError;

/// Stop words to filter during keyword extraction
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "will", "be", "is", "are", "was", "were",
    "been", "have", "has", "had", "do", "does", "did", "shall", "should",
    "may", "might", "must", "can", "could", "would", "there", "here",
    "that", "this", "these", "those", "what", "which", "who", "whom",
    "whose", "when", "where", "why", "how",
];

/// Common replacements for normalization
const REPLACEMENTS: &[(&str, &str)] = &[
    // Crypto symbols
    ("bitcoin", "btc"),
    ("ethereum", "eth"),
    ("binance coin", "bnb"),
    ("cardano", "ada"),
    ("solana", "sol"),
    ("polygon", "matic"),
    ("dogecoin", "doge"),
    
    // Political terms
    ("president", "election"),
    ("presidential", "election"),
    ("governor", "election"),
    ("senate", "election"),
    ("congress", "election"),
    
    // Time periods
    ("by end of", "eoy"),
    ("end of year", "eoy"),
    ("end of month", "eom"),
    ("end of week", "eow"),
    ("by december", "eoy"),
    ("by january", "q1"),
    
    // Price comparisons
    ("above", ">"),
    ("below", "<"),
    ("greater than", ">"),
    ("less than", "<"),
    ("over", ">"),
    ("under", "<"),
    ("exceed", ">"),
    ("fall below", "<"),
    
    // Common patterns
    ("will reach", "price"),
    ("will hit", "price"),
    ("will trade at", "price"),
    ("be worth", "price"),
    ("valued at", "price"),
];

/// Verse classification system
pub struct VerseClassifier;

impl VerseClassifier {
    /// Classify a market title into a verse ID
    /// Returns a deterministic u128 verse_id based on normalized keywords
    pub fn classify_market_to_verse(market_title: &str) -> Result<u128, ProgramError> {
        // Step 1: Normalize the title
        let normalized = Self::normalize_title(market_title);
        
        // Step 2: Extract keywords
        let keywords = Self::extract_keywords(&normalized)?;
        
        if keywords.is_empty() {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        // Step 3: Generate verse ID via deterministic hash
        let verse_data = keywords.join("|");
        let hash_bytes = hash(verse_data.as_bytes()).to_bytes();
        
        // Step 4: Convert first 16 bytes to u128
        let verse_id = u128::from_le_bytes(
            hash_bytes[0..16]
                .try_into()
                .map_err(|_| BettingPlatformError::InvalidConversion)?
        );
        
        msg!("Classified '{}' -> '{}' -> verse_id: {}", 
            market_title, verse_data, verse_id);
        
        Ok(verse_id)
    }
    
    /// Normalize market title for consistent classification
    fn normalize_title(title: &str) -> String {
        let mut normalized = title.to_lowercase().trim().to_string();
        
        // Apply all replacements
        for (pattern, replacement) in REPLACEMENTS {
            normalized = normalized.replace(pattern, replacement);
        }
        
        // Remove special characters except spaces and numbers
        normalized = normalized
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>();
        
        // Collapse multiple spaces
        while normalized.contains("  ") {
            normalized = normalized.replace("  ", " ");
        }
        
        normalized.trim().to_string()
    }
    
    /// Extract up to 5 keywords from normalized title
    fn extract_keywords(normalized: &str) -> Result<Vec<String>, ProgramError> {
        let mut keywords: Vec<String> = normalized
            .split_whitespace()
            .filter(|word| {
                // Keep if not a stop word and has meaningful length
                !STOP_WORDS.contains(word) && word.len() >= 2
            })
            .take(5) // Maximum 5 keywords
            .map(|s| s.to_string())
            .collect();
        
        // Sort keywords for deterministic ordering
        keywords.sort();
        
        Ok(keywords)
    }
    
    /// Get verse category from keywords (for hierarchical organization)
    pub fn get_verse_category(keywords: &[String]) -> VerseCategory {
        // Check for crypto-related keywords
        if keywords.iter().any(|k| {
            ["btc", "eth", "sol", "crypto", "defi", "nft", "blockchain"].contains(&k.as_str())
        }) {
            return VerseCategory::Crypto;
        }
        
        // Check for political keywords
        if keywords.iter().any(|k| {
            ["election", "vote", "political", "senate", "congress", "president"].contains(&k.as_str())
        }) {
            return VerseCategory::Politics;
        }
        
        // Check for sports keywords
        if keywords.iter().any(|k| {
            ["nfl", "nba", "soccer", "football", "basketball", "sports", "game", "match", "win"].contains(&k.as_str())
        }) {
            return VerseCategory::Sports;
        }
        
        // Check for financial keywords
        if keywords.iter().any(|k| {
            ["stock", "market", "nasdaq", "sp500", "dow", "finance", "economy", "gdp", "inflation"].contains(&k.as_str())
        }) {
            return VerseCategory::Finance;
        }
        
        // Check for entertainment keywords
        if keywords.iter().any(|k| {
            ["movie", "film", "oscar", "emmy", "entertainment", "celebrity", "music", "album"].contains(&k.as_str())
        }) {
            return VerseCategory::Entertainment;
        }
        
        // Default category
        VerseCategory::General
    }
    
    /// Generate parent verse ID for hierarchical organization
    pub fn get_parent_verse_id(verse_id: u128, keywords: &[String]) -> Option<u128> {
        let category = Self::get_verse_category(keywords);
        
        match category {
            VerseCategory::General => None, // Root level
            _ => {
                // Generate parent ID from category
                let parent_data = format!("parent_{:?}", category);
                let hash_bytes = hash(parent_data.as_bytes()).to_bytes();
                Some(u128::from_le_bytes(hash_bytes[0..16].try_into().unwrap()))
            }
        }
    }
}

/// Verse categories for hierarchical organization
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerseCategory {
    Crypto,
    Politics,
    Sports,
    Finance,
    Entertainment,
    General,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_title() {
        // Test crypto normalization
        assert_eq!(
            VerseClassifier::normalize_title("Will Bitcoin reach $100,000 by end of year?"),
            "will btc reach 100000 eoy"
        );
        
        // Test political normalization
        assert_eq!(
            VerseClassifier::normalize_title("Who will be the next US President?"),
            "who next us election"
        );
        
        // Test price normalization
        assert_eq!(
            VerseClassifier::normalize_title("Will ETH price go above $5000?"),
            "will eth price go > 5000"
        );
    }
    
    #[test]
    fn test_extract_keywords() {
        let normalized = "btc price > 100000 eoy";
        let keywords = VerseClassifier::extract_keywords(normalized).unwrap();
        
        assert_eq!(keywords, vec!["100000", ">", "btc", "eoy", "price"]);
    }
    
    #[test]
    fn test_deterministic_classification() {
        // Same title should always produce same verse_id
        let title = "Will Bitcoin reach $100,000 by end of year?";
        let verse_id1 = VerseClassifier::classify_market_to_verse(title).unwrap();
        let verse_id2 = VerseClassifier::classify_market_to_verse(title).unwrap();
        
        assert_eq!(verse_id1, verse_id2);
    }
    
    #[test]
    fn test_similar_markets_same_verse() {
        // Similar markets should map to same verse
        let verse_id1 = VerseClassifier::classify_market_to_verse(
            "Will BTC exceed $100k by December?"
        ).unwrap();
        
        let verse_id2 = VerseClassifier::classify_market_to_verse(
            "Bitcoin above $100,000 by end of year?"
        ).unwrap();
        
        assert_eq!(verse_id1, verse_id2);
    }
    
    #[test]
    fn test_category_detection() {
        let crypto_keywords = vec!["btc".to_string(), "price".to_string()];
        assert_eq!(
            VerseClassifier::get_verse_category(&crypto_keywords),
            VerseCategory::Crypto
        );
        
        let politics_keywords = vec!["election".to_string(), "2024".to_string()];
        assert_eq!(
            VerseClassifier::get_verse_category(&politics_keywords),
            VerseCategory::Politics
        );
    }
}