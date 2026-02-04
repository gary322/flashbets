use solana_program::program_error::ProgramError;
use sha3::{Digest, Keccak256};

/// Calculate deterministic verse ID using Keccak256
pub fn calculate_verse_id(
    normalized_title: &str,
    sorted_keywords: &[String],
) -> Result<[u8; 16], ProgramError> {
    let mut hasher = Keccak256::new();
    
    // Hash normalized title
    hasher.update(normalized_title.as_bytes());
    
    // Hash sorted keywords
    for keyword in sorted_keywords {
        hasher.update(keyword.as_bytes());
    }
    
    let result = hasher.finalize();
    
    // Take first 16 bytes for u128 verse ID
    let mut verse_id = [0u8; 16];
    verse_id.copy_from_slice(&result[..16]);
    
    Ok(verse_id)
}

/// Convert verse ID bytes to u128
pub fn verse_id_to_u128(verse_id: &[u8; 16]) -> u128 {
    u128::from_le_bytes(*verse_id)
}

/// Convert u128 to verse ID bytes
pub fn u128_to_verse_id(value: u128) -> [u8; 16] {
    value.to_le_bytes()
}

/// Generate a deterministic seed for verse PDAs
pub fn get_verse_pda_seeds(verse_id: &[u8; 16]) -> Vec<Vec<u8>> {
    vec![
        b"verse".to_vec(),
        verse_id.to_vec(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_verse_id_calculation() {
        let title = "bitcoin price above 150000";
        let keywords = vec![
            "150000".to_string(),
            "bitcoin".to_string(),
            "price".to_string(),
        ];
        
        let verse_id = calculate_verse_id(title, &keywords).unwrap();
        assert_eq!(verse_id.len(), 16);
        
        // Should be deterministic
        let verse_id2 = calculate_verse_id(title, &keywords).unwrap();
        assert_eq!(verse_id, verse_id2);
    }
    
    #[test]
    fn test_different_inputs_different_ids() {
        let keywords1 = vec!["bitcoin".to_string(), "150000".to_string()];
        let keywords2 = vec!["ethereum".to_string(), "150000".to_string()];
        
        let id1 = calculate_verse_id("test1", &keywords1).unwrap();
        let id2 = calculate_verse_id("test2", &keywords2).unwrap();
        
        assert_ne!(id1, id2);
    }
    
    #[test]
    fn test_verse_id_conversions() {
        let value: u128 = 12345678901234567890;
        let verse_id = u128_to_verse_id(value);
        let converted_back = verse_id_to_u128(&verse_id);
        
        assert_eq!(value, converted_back);
    }
}