use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ClassificationEngine {
    pub is_initialized: bool,
    pub authority: Pubkey,
    pub total_verses: u64,
    pub total_markets: u64,
    pub similarity_threshold: u8,        // Levenshtein distance < 5
    pub max_verse_depth: u8,            // 32 as per CLAUDE.md
    pub lowercase_enabled: bool,
    pub punctuation_removal: bool,
    pub number_standardization: bool,
    pub currency_normalization: bool,
    pub bump: u8,
}

impl ClassificationEngine {
    pub const LEN: usize = 1 + 32 + 8 + 8 + 1 + 1 + 1 + 1 + 1 + 1 + 1;
    
    pub fn new(authority: Pubkey, bump: u8) -> Self {
        Self {
            is_initialized: true,
            authority,
            total_verses: 0,
            total_markets: 0,
            similarity_threshold: 5,  // Levenshtein threshold
            max_verse_depth: 32,
            lowercase_enabled: true,
            punctuation_removal: true,
            number_standardization: true,
            currency_normalization: true,
            bump,
        }
    }
}

impl Sealed for ClassificationEngine {}

impl IsInitialized for ClassificationEngine {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for ClassificationEngine {
    const LEN: usize = ClassificationEngine::LEN;
    
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
    
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct NormalizationConfig {
    pub lowercase_enabled: bool,
    pub punctuation_removal: bool,
    pub number_standardization: bool,
    pub date_format: DateFormat,
    pub currency_normalization: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum DateFormat {
    ISO8601,        // YYYY-MM-DD
    USFormat,       // MM/DD/YYYY
    EUFormat,       // DD/MM/YYYY
    UnixTimestamp,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SynonymGroup {
    pub primary: String,
    pub synonyms: Vec<String>,
}