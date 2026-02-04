//! Enhanced Verse Classifier with Fuzzy Matching
//!
//! Implements Levenshtein distance for title variations detection
//! as specified in CLAUDE.md requirements.
//!
//! Key features:
//! - Normalized keyword extraction
//! - Fuzzy matching with configurable thresholds
//! - Synonym mapping
//! - Deterministic verse ID generation

use solana_program::{
    keccak,
    msg,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, HashSet};

use crate::error::BettingPlatformError;

/// Stop words to filter out
const STOP_WORDS: &[&str] = &[
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "i",
    "it", "for", "not", "on", "with", "he", "as", "you", "do", "at",
    "this", "but", "his", "by", "from", "will", "or", "which", "is",
    "was", "are", "been", "has", "had", "were", "said", "did", "get",
    "may", "can", "would", "could", "should", "might", "must", "shall",
    "than", "what", "where", "when", "who", "why", "how"
];

/// Verse classification configuration
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VerseConfig {
    pub levenshtein_threshold: u32,
    pub min_keyword_length: usize,
    pub max_keywords: usize,
    pub enable_fuzzy_matching: bool,
}

impl Default for VerseConfig {
    fn default() -> Self {
        Self {
            levenshtein_threshold: 5,  // As specified in CLAUDE.md
            min_keyword_length: 3,
            max_keywords: 5,
            enable_fuzzy_matching: true,
        }
    }
}

/// Enhanced verse classifier with fuzzy matching
pub struct EnhancedVerseClassifier {
    config: VerseConfig,
    synonym_map: HashMap<String, String>,
    cached_verses: HashMap<[u8; 32], VerseInfo>,
}

/// Information about a verse
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VerseInfo {
    pub verse_id: [u8; 32],
    pub normalized_keywords: Vec<String>,
    pub market_count: u32,
    pub created_slot: u64,
    pub last_updated_slot: u64,
}

impl EnhancedVerseClassifier {
    pub fn new(config: VerseConfig) -> Self {
        let synonym_map = Self::build_synonym_map();
        
        Self {
            config,
            synonym_map,
            cached_verses: HashMap::new(),
        }
    }

    /// Build comprehensive synonym map
    fn build_synonym_map() -> HashMap<String, String> {
        let mut map = HashMap::new();
        
        // Crypto synonyms
        map.insert("bitcoin".to_string(), "btc".to_string());
        map.insert("ethereum".to_string(), "eth".to_string());
        map.insert("dogecoin".to_string(), "doge".to_string());
        map.insert("cardano".to_string(), "ada".to_string());
        map.insert("solana".to_string(), "sol".to_string());
        
        // Price comparisons
        map.insert("above".to_string(), ">".to_string());
        map.insert("below".to_string(), "<".to_string());
        map.insert("greater than".to_string(), ">".to_string());
        map.insert("less than".to_string(), "<".to_string());
        map.insert("over".to_string(), ">".to_string());
        map.insert("under".to_string(), "<".to_string());
        map.insert("reach".to_string(), ">".to_string());
        map.insert("hit".to_string(), ">".to_string());
        
        // Numbers
        map.insert("$150k".to_string(), "$150000".to_string());
        map.insert("$100k".to_string(), "$100000".to_string());
        map.insert("$50k".to_string(), "$50000".to_string());
        map.insert("$10k".to_string(), "$10000".to_string());
        map.insert("$1k".to_string(), "$1000".to_string());
        
        // Time periods
        map.insert("end of year".to_string(), "eoy".to_string());
        map.insert("by year end".to_string(), "eoy".to_string());
        map.insert("end of month".to_string(), "eom".to_string());
        map.insert("end of week".to_string(), "eow".to_string());
        
        map
    }

    /// Classify market title to verse with fuzzy matching
    pub fn classify_with_fuzzy_matching(
        &mut self,
        title: &str,
        current_slot: u64,
    ) -> Result<([u8; 32], bool), ProgramError> {
        // Step 1: Normalize and extract keywords
        let normalized = self.normalize_title(title);
        let keywords = self.extract_keywords(&normalized);
        
        // Step 2: Generate deterministic verse ID
        let verse_id = self.generate_verse_id(&keywords);
        
        // Step 3: Check for similar existing verses if fuzzy matching enabled
        if self.config.enable_fuzzy_matching {
            if let Some(existing_verse_id) = self.find_similar_verse(&keywords)? {
                msg!("Found similar verse via fuzzy matching");
                return Ok((existing_verse_id, true));
            }
        }
        
        // Step 4: Create new verse entry
        let verse_info = VerseInfo {
            verse_id,
            normalized_keywords: keywords,
            market_count: 1,
            created_slot: current_slot,
            last_updated_slot: current_slot,
        };
        
        self.cached_verses.insert(verse_id, verse_info);
        
        Ok((verse_id, false))
    }

    /// Normalize title with synonym replacement
    fn normalize_title(&self, title: &str) -> String {
        let mut normalized = title.to_lowercase();
        
        // Apply all synonyms
        for (from, to) in &self.synonym_map {
            normalized = normalized.replace(from, to);
        }
        
        // Clean punctuation except $ and numbers
        normalized = normalized
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '$' || c == '>' || c == '<' {
                    c
                } else {
                    ' '
                }
            })
            .collect::<String>();
        
        // Collapse whitespace
        normalized.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Extract keywords from normalized title
    fn extract_keywords(&self, normalized: &str) -> Vec<String> {
        let mut keywords: Vec<String> = normalized
            .split_whitespace()
            .filter(|word| !STOP_WORDS.contains(word))
            .filter(|word| word.len() >= self.config.min_keyword_length)
            .map(|word| word.to_string())
            .collect();
        
        // Sort for deterministic ordering
        keywords.sort();
        keywords.dedup();
        
        // Limit keywords
        keywords.truncate(self.config.max_keywords);
        
        keywords
    }

    /// Generate verse ID from keywords
    fn generate_verse_id(&self, keywords: &[String]) -> [u8; 32] {
        let verse_data = keywords.join("|");
        keccak::hash(verse_data.as_bytes()).to_bytes()
    }

    /// Find similar verse using Levenshtein distance
    fn find_similar_verse(&self, keywords: &[String]) -> Result<Option<[u8; 32]>, ProgramError> {
        let keyword_str = keywords.join(" ");
        
        for (verse_id, verse_info) in &self.cached_verses {
            let existing_str = verse_info.normalized_keywords.join(" ");
            let distance = self.levenshtein_distance(&keyword_str, &existing_str);
            
            if distance <= self.config.levenshtein_threshold {
                msg!("Found similar verse with distance {}", distance);
                return Ok(Some(*verse_id));
            }
        }
        
        Ok(None)
    }

    /// Calculate Levenshtein distance between two strings
    pub fn levenshtein_distance(&self, s1: &str, s2: &str) -> u32 {
        let len1 = s1.chars().count();
        let len2 = s2.chars().count();
        
        if len1 == 0 {
            return len2 as u32;
        }
        if len2 == 0 {
            return len1 as u32;
        }
        
        let mut matrix = vec![vec![0u32; len2 + 1]; len1 + 1];
        
        // Initialize first row and column
        for i in 0..=len1 {
            matrix[i][0] = i as u32;
        }
        for j in 0..=len2 {
            matrix[0][j] = j as u32;
        }
        
        // Fill matrix
        let chars1: Vec<char> = s1.chars().collect();
        let chars2: Vec<char> = s2.chars().collect();
        
        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
                
                matrix[i][j] = std::cmp::min(
                    matrix[i - 1][j] + 1,      // Deletion
                    std::cmp::min(
                        matrix[i][j - 1] + 1,   // Insertion
                        matrix[i - 1][j - 1] + cost  // Substitution
                    )
                );
            }
        }
        
        matrix[len1][len2]
    }

    /// Check if two titles are similar enough to be in same verse
    pub fn are_titles_similar(&self, title1: &str, title2: &str) -> bool {
        let normalized1 = self.normalize_title(title1);
        let normalized2 = self.normalize_title(title2);
        
        let distance = self.levenshtein_distance(&normalized1, &normalized2);
        distance <= self.config.levenshtein_threshold
    }

    /// Get verse statistics
    pub fn get_verse_stats(&self) -> VerseStats {
        let total_verses = self.cached_verses.len() as u32;
        let total_markets: u32 = self.cached_verses.values()
            .map(|v| v.market_count)
            .sum();
        
        let avg_markets_per_verse = if total_verses > 0 {
            total_markets / total_verses
        } else {
            0
        };
        
        VerseStats {
            total_verses,
            total_markets,
            avg_markets_per_verse,
            fuzzy_match_enabled: self.config.enable_fuzzy_matching,
        }
    }
}

/// Verse statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VerseStats {
    pub total_verses: u32,
    pub total_markets: u32,
    pub avg_markets_per_verse: u32,
    pub fuzzy_match_enabled: bool,
}

/// Example usage demonstrating the requirement
pub fn demonstrate_fuzzy_matching() {
    let mut classifier = EnhancedVerseClassifier::new(VerseConfig::default());
    
    // These should match as same verse
    let title1 = "BTC > $150k";
    let title2 = "Bitcoin above $150,000";
    
    let normalized1 = classifier.normalize_title(title1);
    let normalized2 = classifier.normalize_title(title2);
    
    msg!("Title 1 normalized: {}", normalized1);  // "btc > $150000"
    msg!("Title 2 normalized: {}", normalized2);  // "btc > $150000"
    
    let distance = classifier.levenshtein_distance(&normalized1, &normalized2);
    msg!("Levenshtein distance: {}", distance);  // Should be 0
    
    assert!(classifier.are_titles_similar(title1, title2));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        let classifier = EnhancedVerseClassifier::new(VerseConfig::default());
        
        // Exact match
        assert_eq!(classifier.levenshtein_distance("hello", "hello"), 0);
        
        // One character difference
        assert_eq!(classifier.levenshtein_distance("hello", "hallo"), 1);
        
        // Multiple differences
        assert_eq!(classifier.levenshtein_distance("kitten", "sitting"), 3);
        
        // Empty strings
        assert_eq!(classifier.levenshtein_distance("", "abc"), 3);
        assert_eq!(classifier.levenshtein_distance("abc", ""), 3);
    }

    #[test]
    fn test_normalization() {
        let classifier = EnhancedVerseClassifier::new(VerseConfig::default());
        
        // Test crypto normalization
        let normalized = classifier.normalize_title("Will Bitcoin reach $150k?");
        assert!(normalized.contains("btc"));
        assert!(normalized.contains("$150000"));
        
        // Test comparison normalization
        let normalized = classifier.normalize_title("ETH above $3,000 by EOY");
        assert!(normalized.contains("eth"));
        assert!(normalized.contains(">"));
        assert!(normalized.contains("eoy"));
    }

    #[test]
    fn test_similar_titles() {
        let classifier = EnhancedVerseClassifier::new(VerseConfig::default());
        
        // These should be similar (same verse)
        assert!(classifier.are_titles_similar(
            "BTC > $150k",
            "Bitcoin above $150,000"
        ));
        
        // These should be different
        assert!(!classifier.are_titles_similar(
            "BTC > $150k",
            "ETH > $10k"
        ));
    }

    #[test]
    fn test_fuzzy_matching_integration() {
        let mut classifier = EnhancedVerseClassifier::new(VerseConfig::default());
        
        // Add first market
        let (verse1, is_match1) = classifier.classify_with_fuzzy_matching(
            "Will Bitcoin reach $150k by end of year?",
            100
        ).unwrap();
        assert!(!is_match1); // First one, no match
        
        // Add similar market
        let (verse2, is_match2) = classifier.classify_with_fuzzy_matching(
            "BTC > $150,000 EOY?",
            200
        ).unwrap();
        assert!(is_match2); // Should match existing verse
        assert_eq!(verse1, verse2); // Same verse ID
    }
}