use crate::state::{NormalizationConfig, SynonymGroup};
use crate::normalization::{standardize_numbers, normalize_dates, normalize_currency};
use solana_program::program_error::ProgramError;

pub struct TextNormalizer;

impl TextNormalizer {
    /// Main normalization function that applies all rules
    pub fn normalize_title(
        title: &str,
        config: &NormalizationConfig,
        synonyms: &[SynonymGroup],
    ) -> Result<String, ProgramError> {
        let mut normalized = title.to_string();
        
        // Step 1: Lowercase if enabled
        if config.lowercase_enabled {
            normalized = normalized.to_lowercase();
        }
        
        // Step 2: Remove punctuation
        if config.punctuation_removal {
            normalized = Self::remove_punctuation(&normalized);
        }
        
        // Step 3: Standardize numbers
        if config.number_standardization {
            normalized = standardize_numbers(&normalized)?;
        }
        
        // Step 4: Apply synonym replacements
        normalized = Self::apply_synonyms(&normalized, synonyms);
        
        // Step 5: Normalize dates
        normalized = normalize_dates(&normalized, config.date_format)?;
        
        // Step 6: Currency normalization
        if config.currency_normalization {
            normalized = normalize_currency(&normalized)?;
        }
        
        // Step 7: Collapse multiple spaces
        normalized = Self::collapse_spaces(&normalized);
        
        Ok(normalized)
    }
    
    /// Extract keywords from normalized text
    pub fn extract_keywords(
        normalized_text: &str,
        stopwords: &[&str],
    ) -> Result<Vec<String>, ProgramError> {
        // Split by whitespace
        let words: Vec<String> = normalized_text
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        // Filter stopwords and short words
        let keywords: Vec<String> = words
            .into_iter()
            .filter(|word| {
                !stopwords.contains(&word.as_str()) &&
                word.len() >= 2 &&
                !word.chars().all(|c| c.is_numeric())
            })
            .collect();
        
        // Sort for consistent hashing
        let mut sorted_keywords = keywords;
        sorted_keywords.sort();
        sorted_keywords.dedup();
        
        Ok(sorted_keywords)
    }
    
    fn remove_punctuation(text: &str) -> String {
        text.chars()
            .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
            .collect()
    }
    
    fn apply_synonyms(text: &str, synonyms: &[SynonymGroup]) -> String {
        let mut result = text.to_string();
        
        for synonym_group in synonyms {
            for synonym in &synonym_group.synonyms {
                // Simple replace for now (regex would need external crate)
                result = result.replace(synonym, &synonym_group.primary);
            }
        }
        
        result
    }
    
    fn collapse_spaces(text: &str) -> String {
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DateFormat;
    
    #[test]
    fn test_punctuation_removal() {
        let text = "Hello, world! How are you?";
        let result = TextNormalizer::remove_punctuation(text);
        assert_eq!(result, "Hello  world  How are you ");
    }
    
    #[test]
    fn test_collapse_spaces() {
        let text = "Hello   world    test";
        let result = TextNormalizer::collapse_spaces(text);
        assert_eq!(result, "Hello world test");
    }
    
    #[test]
    fn test_keyword_extraction() {
        let text = "bitcoin price above 150000 december 2025";
        let keywords = TextNormalizer::extract_keywords(text, &crate::normalization::STOPWORDS).unwrap();
        // Note: STOPWORDS includes "above", and numbers are filtered out
        assert_eq!(keywords, vec!["bitcoin", "december", "price"]);
    }
}