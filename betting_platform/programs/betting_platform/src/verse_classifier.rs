use sha3::{Digest, Keccak256};
use std::collections::HashMap;

// Stop words to filter out when extracting keywords
const STOP_WORDS: &[&str] = &[
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "i",
    "it", "for", "not", "on", "with", "he", "as", "you", "do", "at",
    "this", "but", "his", "by", "from", "will", "or", "which", "is",
    "was", "are", "been", "has", "had", "were", "said", "did", "get",
    "may", "can", "would", "could", "should", "might", "must", "shall",
    "than", "what", "where", "when", "who", "why", "how"
];

pub struct VerseClassifier {
    keyword_map: HashMap<String, String>,
}

impl VerseClassifier {
    pub fn new() -> Self {
        let mut keyword_map = HashMap::new();

        // Common crypto replacements
        keyword_map.insert("bitcoin".to_string(), "btc".to_string());
        keyword_map.insert("ethereum".to_string(), "eth".to_string());
        keyword_map.insert("dogecoin".to_string(), "doge".to_string());
        keyword_map.insert("cardano".to_string(), "ada".to_string());
        keyword_map.insert("solana".to_string(), "sol".to_string());
        keyword_map.insert("polygon".to_string(), "matic".to_string());
        
        // Presidential/political replacements
        keyword_map.insert("president".to_string(), "election".to_string());
        keyword_map.insert("presidential".to_string(), "election".to_string());
        keyword_map.insert("presidency".to_string(), "election".to_string());
        
        // Price level replacements
        keyword_map.insert("above".to_string(), ">".to_string());
        keyword_map.insert("below".to_string(), "<".to_string());
        keyword_map.insert("greater than".to_string(), ">".to_string());
        keyword_map.insert("less than".to_string(), "<".to_string());
        keyword_map.insert("over".to_string(), ">".to_string());
        keyword_map.insert("under".to_string(), "<".to_string());
        
        // Time replacements
        keyword_map.insert("by end of".to_string(), "eoy".to_string());
        keyword_map.insert("end of year".to_string(), "eoy".to_string());
        keyword_map.insert("end of month".to_string(), "eom".to_string());
        keyword_map.insert("end of week".to_string(), "eow".to_string());
        keyword_map.insert("end of day".to_string(), "eod".to_string());
        
        // Common market terms
        keyword_map.insert("all time high".to_string(), "ath".to_string());
        keyword_map.insert("all-time high".to_string(), "ath".to_string());
        keyword_map.insert("market cap".to_string(), "mcap".to_string());
        keyword_map.insert("market capitalization".to_string(), "mcap".to_string());
        keyword_map.insert("trading volume".to_string(), "volume".to_string());
        keyword_map.insert("24h volume".to_string(), "volume".to_string());

        Self { keyword_map }
    }

    // For compatibility with existing code that expects u128
    pub fn classify_market(&self, title: &str) -> u128 {
        let verse_id = self.classify_market_to_verse(title);
        // Convert first 16 bytes of [u8; 32] to u128
        u128::from_le_bytes(verse_id[0..16].try_into().unwrap())
    }

    // CLAUDE.md: Grouping Algorithm (21k markets → <500 verses)
    pub fn classify_market_to_verse(&self, market_title: &str) -> [u8; 32] {
        // Step 1: Normalize title
        let normalized = self.normalize_title(market_title);

        // Step 2: Extract keywords
        let keywords = self.extract_keywords(&normalized);

        // Step 3: Generate verse ID via hash
        let verse_data = keywords.join("|");
        let mut hasher = Keccak256::new();
        hasher.update(verse_data.as_bytes());
        let result = hasher.finalize();

        // Step 4: Return full 32-byte hash as verse_id
        result.into()
    }

    fn normalize_title(&self, title: &str) -> String {
        let mut normalized = title.to_lowercase();

        // Apply keyword replacements
        for (from, to) in &self.keyword_map {
            normalized = normalized.replace(from, to);
        }

        // Remove punctuation
        normalized = normalized
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '$' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        normalized
    }

    fn extract_keywords(&self, normalized: &str) -> Vec<String> {
        let mut keywords: Vec<String> = normalized
            .split_whitespace()
            .filter(|word| !STOP_WORDS.contains(word))
            .filter(|word| word.len() > 2) // Skip very short words
            .map(|word| word.to_string())
            .collect();
        
        // Sort keywords for deterministic hashing
        keywords.sort();
        
        // Take max 5 keywords for grouping
        keywords.truncate(5);
        
        keywords
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verse_classification() {
        let classifier = VerseClassifier::new();
        
        // Similar markets should classify to same verse
        let btc_market1 = "Will Bitcoin price be above $50,000 by end of year?";
        let btc_market2 = "Bitcoin above $50k EOY?";
        let btc_market3 = "BTC > $50,000 by end of 2024?";
        
        let verse1 = classifier.classify_market(btc_market1);
        let verse2 = classifier.classify_market(btc_market2);
        let verse3 = classifier.classify_market(btc_market3);
        
        // These should all map to the same verse
        assert_eq!(verse1, verse2);
        assert_eq!(verse2, verse3);
        
        // Different topic should map to different verse
        let eth_market = "Will Ethereum be above $3,000?";
        let eth_verse = classifier.classify_market(eth_market);
        assert_ne!(verse1, eth_verse);
    }
}