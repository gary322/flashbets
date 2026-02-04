use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VerseRegistry {
    pub is_initialized: bool,
    pub total_verses: u64,
    pub keyword_mappings: Vec<KeywordMapping>,
    pub category_mappings: Vec<CategoryMapping>,
    pub bump: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct KeywordMapping {
    pub keyword: String,
    pub verse_ids: Vec<[u8; 16]>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CategoryMapping {
    pub category: String,
    pub verse_ids: Vec<[u8; 16]>,
}

impl VerseRegistry {
    pub const BASE_LEN: usize = 1 + 8 + 4 + 4 + 1; // Without dynamic data
    
    pub fn new(bump: u8) -> Self {
        Self {
            is_initialized: true,
            total_verses: 0,
            keyword_mappings: Vec::new(),
            category_mappings: Vec::new(),
            bump,
        }
    }
    
    pub fn add_verse_to_keyword(&mut self, keyword: &str, verse_id: [u8; 16]) -> Result<(), ProgramError> {
        // Find or create keyword mapping
        if let Some(mapping) = self.keyword_mappings.iter_mut().find(|m| m.keyword == keyword) {
            if !mapping.verse_ids.contains(&verse_id) {
                mapping.verse_ids.push(verse_id);
            }
        } else {
            self.keyword_mappings.push(KeywordMapping {
                keyword: keyword.to_string(),
                verse_ids: vec![verse_id],
            });
        }
        Ok(())
    }
    
    pub fn add_verse_to_category(&mut self, category: &str, verse_id: [u8; 16]) -> Result<(), ProgramError> {
        // Find or create category mapping
        if let Some(mapping) = self.category_mappings.iter_mut().find(|m| m.category == category) {
            if !mapping.verse_ids.contains(&verse_id) {
                mapping.verse_ids.push(verse_id);
            }
        } else {
            self.category_mappings.push(CategoryMapping {
                category: category.to_string(),
                verse_ids: vec![verse_id],
            });
        }
        Ok(())
    }
    
    pub fn find_verses_by_keyword(&self, keyword: &str) -> Vec<[u8; 16]> {
        self.keyword_mappings
            .iter()
            .find(|m| m.keyword == keyword)
            .map(|m| m.verse_ids.clone())
            .unwrap_or_default()
    }
    
    pub fn find_verses_by_category(&self, category: &str) -> Vec<[u8; 16]> {
        self.category_mappings
            .iter()
            .find(|m| m.category == category)
            .map(|m| m.verse_ids.clone())
            .unwrap_or_default()
    }
    
    pub fn calculate_size(&self) -> usize {
        Self::BASE_LEN
            + self.keyword_mappings.iter()
                .map(|m| 4 + m.keyword.len() + 4 + m.verse_ids.len() * 16)
                .sum::<usize>()
            + self.category_mappings.iter()
                .map(|m| 4 + m.category.len() + 4 + m.verse_ids.len() * 16)
                .sum::<usize>()
    }
}

// Helper structure for search results
#[derive(Debug, Clone)]
pub struct SearchCriteria {
    pub keywords: Vec<String>,
    pub category: Option<String>,
    pub min_volume: Option<u64>,
    pub max_depth: Option<u8>,
}