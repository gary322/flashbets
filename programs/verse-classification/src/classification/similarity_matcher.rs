use solana_program::program_error::ProgramError;
use crate::state::{VerseMetadata, VerseRegistry};
use crate::classification::levenshtein::calculate_levenshtein_distance;

/// Find similar verses based on normalized title and keywords
pub fn find_similar_verse(
    registry: &VerseRegistry,
    normalized_title: &str,
    keywords: &[String],
    _threshold: u8,
) -> Result<Option<[u8; 16]>, ProgramError> {
    // First check exact keyword matches
    let keyword_matches = find_verses_by_keywords(registry, keywords)?;
    
    // Check each match for title similarity
    for _verse_id in keyword_matches {
        // In a real implementation, we would load the verse metadata here
        // For now, we'll simulate checking similarity
        // This would need to be implemented with actual account loading
    }
    
    // Check category patterns if no keyword match
    let category = crate::classification::detect_category(normalized_title, keywords)?;
    let category_verses = registry.find_verses_by_category(&category);
    
    for _verse_id in category_verses {
        // Similar check as above
        // Would need actual metadata loading
    }
    
    Ok(None)
}

/// Find verses that contain any of the given keywords
fn find_verses_by_keywords(
    registry: &VerseRegistry,
    keywords: &[String],
) -> Result<Vec<[u8; 16]>, ProgramError> {
    let mut matching_verses = Vec::new();
    let mut seen = std::collections::HashSet::new();
    
    for keyword in keywords {
        let verses = registry.find_verses_by_keyword(keyword);
        for verse_id in verses {
            if seen.insert(verse_id) {
                matching_verses.push(verse_id);
            }
        }
    }
    
    Ok(matching_verses)
}

/// Calculate similarity score between two verse metadata
pub fn calculate_similarity_score(
    verse1: &VerseMetadata,
    verse2: &VerseMetadata,
) -> Result<f32, ProgramError> {
    // Title similarity (weighted 50%)
    let title_distance = calculate_levenshtein_distance(
        &verse1.normalized_title,
        &verse2.normalized_title,
    )?;
    let title_score = 1.0 - (title_distance as f32 / 
        verse1.normalized_title.len().max(verse2.normalized_title.len()) as f32);
    
    // Keyword overlap (weighted 30%)
    let keyword_score = calculate_keyword_overlap(&verse1.keywords, &verse2.keywords);
    
    // Category match (weighted 20%)
    let category_score = if verse1.category == verse2.category { 1.0 } else { 0.0 };
    
    Ok(title_score * 0.5 + keyword_score * 0.3 + category_score * 0.2)
}

/// Calculate keyword overlap score
fn calculate_keyword_overlap(keywords1: &[String], keywords2: &[String]) -> f32 {
    let set1: std::collections::HashSet<_> = keywords1.iter().collect();
    let set2: std::collections::HashSet<_> = keywords2.iter().collect();
    
    let intersection = set1.intersection(&set2).count();
    let union = set1.union(&set2).count();
    
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Group similar markets into verses
pub struct VerseGrouper {
    pub similarity_threshold: f32,
}

impl VerseGrouper {
    pub fn new(similarity_threshold: f32) -> Self {
        Self { similarity_threshold }
    }
    
    /// Check if a market should belong to an existing verse
    pub fn should_group_together(
        &self,
        market_metadata: &VerseMetadata,
        verse_metadata: &VerseMetadata,
    ) -> Result<bool, ProgramError> {
        let score = calculate_similarity_score(market_metadata, verse_metadata)?;
        Ok(score >= self.similarity_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keyword_overlap() {
        let keywords1 = vec!["bitcoin".to_string(), "price".to_string(), "2025".to_string()];
        let keywords2 = vec!["bitcoin".to_string(), "value".to_string(), "2025".to_string()];
        
        let overlap = calculate_keyword_overlap(&keywords1, &keywords2);
        assert_eq!(overlap, 2.0 / 4.0); // 2 common out of 4 total unique
    }
    
    #[test]
    fn test_similarity_score() {
        let verse1 = VerseMetadata::new(
            [1u8; 16],
            "Bitcoin price above 150000".to_string(),
            "bitcoin price above 150000".to_string(),
            vec!["bitcoin".to_string(), "price".to_string(), "150000".to_string()],
            "crypto".to_string(),
            1,
        );
        
        let verse2 = VerseMetadata::new(
            [2u8; 16],
            "Bitcoin price below 150000".to_string(),
            "bitcoin price below 150000".to_string(),
            vec!["bitcoin".to_string(), "price".to_string(), "150000".to_string()],
            "crypto".to_string(),
            2,
        );
        
        let score = calculate_similarity_score(&verse1, &verse2).unwrap();
        assert!(score > 0.7); // Should be high due to similar keywords and same category
    }
}