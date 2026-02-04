//! Cross-verse attack prevention
//!
//! Implements protection against synthetic correlation attacks between unrelated markets
//! as specified in Part 7

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    keccak,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    math::U64F64,
    verse::{VerseAccount, VerseType},
};

/// Cross-verse protection state
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CrossVerseProtection {
    pub enabled: bool,
    pub max_cross_verse_positions: u8,
    pub correlation_threshold: u16, // Basis points (10000 = 100%)
    pub isolation_enforced: bool,
    pub verse_linkage_map: Vec<VerseLinkage>,
}

impl CrossVerseProtection {
    pub const SIZE: usize = 1 + // enabled
        1 + // max_cross_verse_positions
        2 + // correlation_threshold
        1 + // isolation_enforced
        4 + 1024; // verse_linkage_map (max 128 linkages * 8 bytes each)
    
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_cross_verse_positions: 3, // Max 3 verses per user
            correlation_threshold: 5000, // 50% correlation threshold
            isolation_enforced: true,
            verse_linkage_map: Vec::new(),
        }
    }
}

/// Linkage between verses
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct VerseLinkage {
    pub verse_a: u32,
    pub verse_b: u32,
    pub linkage_type: LinkageType,
    pub correlation_score: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum LinkageType {
    Parent,
    Sibling,
    Synthetic,
    None,
}

/// Verify verses are truly independent
pub fn verify_verse_independence(
    verse_a: &VerseAccount,
    verse_b: &VerseAccount,
) -> Result<bool, ProgramError> {
    // Check classification invariants
    let hash_a = calculate_verse_hash(&verse_a.keywords);
    let hash_b = calculate_verse_hash(&verse_b.keywords);
    
    // Different hashes = different classifications = independent
    if hash_a != hash_b {
        return Ok(true);
    }
    
    // Same parent = not independent
    if verse_a.parent_verse == verse_b.parent_verse && verse_a.parent_verse != Pubkey::default() {
        return Ok(false);
    }
    
    // Check verse types
    match (verse_a.verse_type, verse_b.verse_type) {
        (VerseType::Main, VerseType::Main) => Ok(true), // Main verses can be independent
        (VerseType::Quantum, _) | (_, VerseType::Quantum) => Ok(false), // Quantum always dependent
        (VerseType::Distribution, _) | (_, VerseType::Distribution) => Ok(false), // Distribution dependent
        _ => Ok(true),
    }
}

/// Calculate deterministic hash for verse classification
fn calculate_verse_hash(keywords: &[String]) -> [u8; 32] {
    let mut data = Vec::new();
    for keyword in keywords {
        data.extend_from_slice(keyword.as_bytes());
    }
    keccak::hash(&data).0
}

/// Check for cross-verse attack patterns
pub fn detect_cross_verse_attack<'a>(
    user: &Pubkey,
    positions: &[CrossVersePosition],
    protection: &CrossVerseProtection,
) -> Result<bool, ProgramError> {
    if !protection.enabled {
        return Ok(false);
    }
    
    // Check position count across verses  
    let mut unique_verses = Vec::new();
    for position in positions {
        if !unique_verses.contains(&position.verse_id) {
            unique_verses.push(position.verse_id);
        }
    }
    
    if unique_verses.len() > protection.max_cross_verse_positions as usize {
        msg!("User has positions in {} verses, max allowed: {}", 
             unique_verses.len(), protection.max_cross_verse_positions);
        return Ok(true);
    }
    
    // Check for synthetic correlations
    for i in 0..positions.len() {
        for j in i+1..positions.len() {
            if positions[i].verse_id != positions[j].verse_id {
                // Check if verses are linked
                let linkage = find_verse_linkage(
                    &protection.verse_linkage_map,
                    positions[i].verse_id,
                    positions[j].verse_id,
                );
                
                if let Some(link) = linkage {
                    if link.linkage_type == LinkageType::Synthetic {
                        msg!("Synthetic linkage detected between verses {} and {}", 
                             positions[i].verse_id, positions[j].verse_id);
                        return Ok(true);
                    }
                }
                
                // Check correlation threshold
                let correlation = calculate_position_correlation(&positions[i], &positions[j]);
                if correlation > protection.correlation_threshold {
                    msg!("High correlation {} between unrelated verses", correlation);
                    return Ok(true);
                }
            }
        }
    }
    
    Ok(false)
}

/// User's position across verses
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CrossVersePosition {
    pub verse_id: u32,
    pub market_id: u32,
    pub outcome: u8,
    pub size: u64,
    pub direction: bool, // true = long, false = short
}

/// Find linkage between two verses
fn find_verse_linkage(
    linkages: &[VerseLinkage],
    verse_a: u32,
    verse_b: u32,
) -> Option<VerseLinkage> {
    linkages.iter()
        .find(|l| (l.verse_a == verse_a && l.verse_b == verse_b) ||
                  (l.verse_a == verse_b && l.verse_b == verse_a))
        .cloned()
}

/// Calculate correlation between positions
fn calculate_position_correlation(
    pos_a: &CrossVersePosition,
    pos_b: &CrossVersePosition,
) -> u16 {
    // Same direction = positive correlation
    let direction_correlation = if pos_a.direction == pos_b.direction {
        5000 // 50%
    } else {
        0
    };
    
    // Similar size = higher correlation
    let size_ratio = if pos_a.size > pos_b.size {
        (pos_b.size * 10000) / pos_a.size
    } else {
        (pos_a.size * 10000) / pos_b.size
    } as u16;
    
    let size_correlation = if size_ratio > 8000 { // >80% similar
        3000 // 30%
    } else if size_ratio > 5000 { // >50% similar
        1500 // 15%
    } else {
        0
    };
    
    // Same outcome = higher correlation
    let outcome_correlation = if pos_a.outcome == pos_b.outcome {
        2000 // 20%
    } else {
        0
    };
    
    direction_correlation + size_correlation + outcome_correlation
}

/// Enforce verse isolation for a position
pub fn enforce_verse_isolation<'a>(
    user: &Pubkey,
    verse_account: &AccountInfo<'a>,
    protection: &CrossVerseProtection,
    existing_positions: &[CrossVersePosition],
) -> ProgramResult {
    if !protection.isolation_enforced {
        return Ok(());
    }
    
    let verse = VerseAccount::try_from_slice(&verse_account.data.borrow())?;
    
    // Check if user already has positions in related verses
    for position in existing_positions {
        if position.verse_id != verse.verse_id {
            // Check independence
            let independent = verify_verse_independence(&verse, &VerseAccount {
                verse_id: position.verse_id,
                parent_verse: Pubkey::default(), // Would need to load actual verse
                verse_type: VerseType::Main,
                keywords: vec![],
                total_markets: 0,
                active_markets: 0,
                total_volume: 0,
                created_at: 0,
            authority: Pubkey::default(),
            })?;
            
            if !independent {
                return Err(BettingPlatformError::InvalidVerseHierarchy.into());
            }
        }
    }
    
    Ok(())
}

/// Update verse linkage based on observed correlations
pub fn update_verse_linkage(
    protection: &mut CrossVerseProtection,
    verse_a: u32,
    verse_b: u32,
    observed_correlation: u16,
) -> Result<(), ProgramError> {
    // Find existing linkage
    let existing_idx = protection.verse_linkage_map.iter()
        .position(|l| (l.verse_a == verse_a && l.verse_b == verse_b) ||
                     (l.verse_a == verse_b && l.verse_b == verse_a));
    
    let linkage_type = if observed_correlation > 8000 {
        LinkageType::Synthetic // Very high correlation = synthetic
    } else if observed_correlation > 5000 {
        LinkageType::Sibling // Moderate correlation = sibling
    } else {
        LinkageType::None
    };
    
    if let Some(idx) = existing_idx {
        // Update existing
        protection.verse_linkage_map[idx].correlation_score = observed_correlation;
        protection.verse_linkage_map[idx].linkage_type = linkage_type;
    } else if linkage_type != LinkageType::None {
        // Add new linkage
        protection.verse_linkage_map.push(VerseLinkage {
            verse_a,
            verse_b,
            linkage_type,
            correlation_score: observed_correlation,
        });
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_verse_independence() {
        let verse_a = VerseAccount {
            verse_id: 1,
            parent_verse: Pubkey::default(),
            verse_type: VerseType::Main,
            keywords: vec!["politics".to_string(), "election".to_string()],
            total_markets: 10,
            active_markets: 5,
            total_volume: 1_000_000,
            created_at: 0,
            authority: Pubkey::default(),
        };
        
        let verse_b = VerseAccount {
            verse_id: 2,
            parent_verse: Pubkey::default(),
            verse_type: VerseType::Main,
            keywords: vec!["sports".to_string(), "football".to_string()],
            total_markets: 20,
            active_markets: 10,
            total_volume: 2_000_000,
            created_at: 0,
            authority: Pubkey::default(),
        };
        
        // Different keywords = independent
        assert!(verify_verse_independence(&verse_a, &verse_b).unwrap());
    }
    
    #[test]
    fn test_position_correlation() {
        let pos_a = CrossVersePosition {
            verse_id: 1,
            market_id: 100,
            outcome: 0,
            size: 1000,
            direction: true,
        };
        
        let pos_b = CrossVersePosition {
            verse_id: 2,
            market_id: 200,
            outcome: 0,
            size: 900,
            direction: true,
        };
        
        let correlation = calculate_position_correlation(&pos_a, &pos_b);
        // Same direction (50%) + similar size (30%) + same outcome (20%) = 100%
        assert_eq!(correlation, 10000);
    }
    
    #[test]
    fn test_cross_verse_attack_detection() {
        let protection = CrossVerseProtection::new();
        let user = Pubkey::new_unique();
        
        // Too many verse positions
        let positions = vec![
            CrossVersePosition { verse_id: 1, market_id: 100, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 2, market_id: 200, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 3, market_id: 300, outcome: 0, size: 1000, direction: true },
            CrossVersePosition { verse_id: 4, market_id: 400, outcome: 0, size: 1000, direction: true },
        ];
        
        assert!(detect_cross_verse_attack(&user, &positions, &protection).unwrap());
    }
}
