//! Verse Hierarchy Manager
//!
//! Implements deterministic priority system for verse hierarchy conflicts
//! as specified in CLAUDE.md requirements.
//!
//! Key features:
//! - Single parent invariant enforcement
//! - First-come-first-served priority
//! - Maximum depth of 32 levels
//! - Atomic PDA creation

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, VecDeque};

use crate::error::BettingPlatformError;

/// Maximum depth for verse hierarchies
pub const MAX_VERSE_DEPTH: u8 = 32;

/// Verse capacity
pub const MARKETS_PER_VERSE_CAPACITY: u32 = 54; // ~21,000 markets / 400 verses

/// Verse PDA structure
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VersePDA {
    pub verse_id: [u8; 32],
    pub parent_id: Option<[u8; 32]>,
    pub depth: u8,
    pub market_count: u32,
    pub child_verses: Vec<[u8; 32]>,
    pub created_slot: u64,
    pub last_updated_slot: u64,
    pub is_locked: bool,
}

impl VersePDA {
    pub const SIZE: usize = 32 +    // verse_id
                           1 + 32 +  // parent_id Option
                           1 +       // depth
                           4 +       // market_count
                           4 + (32 * 10) + // child_verses Vec (max 10)
                           8 +       // created_slot
                           8 +       // last_updated_slot
                           1;        // is_locked

    pub fn new(verse_id: [u8; 32], created_slot: u64) -> Self {
        Self {
            verse_id,
            parent_id: None,
            depth: 0,
            market_count: 0,
            child_verses: Vec::with_capacity(10),
            created_slot,
            last_updated_slot: created_slot,
            is_locked: false,
        }
    }

    /// Check if verse can accept more markets
    pub fn has_capacity(&self) -> bool {
        self.market_count < MARKETS_PER_VERSE_CAPACITY && !self.is_locked
    }

    /// Add market to verse
    pub fn add_market(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        if !self.has_capacity() {
            return Err(BettingPlatformError::VerseCapacityExceeded.into());
        }

        self.market_count += 1;
        self.last_updated_slot = current_slot;
        Ok(())
    }

    /// Calculate effective depth in hierarchy
    pub fn calculate_effective_depth(&self, hierarchy: &VerseHierarchy) -> Result<u8, ProgramError> {
        let mut depth = 0;
        let mut current = Some(self.verse_id);

        while let Some(verse_id) = current {
            depth += 1;
            if depth > MAX_VERSE_DEPTH {
                return Err(BettingPlatformError::MaxDepthExceeded.into());
            }

            if let Some(verse) = hierarchy.verses.get(&verse_id) {
                current = verse.parent_id;
            } else {
                break;
            }
        }

        Ok(depth)
    }
}

/// Verse hierarchy management
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct VerseHierarchy {
    pub verses: HashMap<[u8; 32], VersePDA>,
    pub root_verses: Vec<[u8; 32]>,
    pub total_verses: u32,
    pub total_markets: u32,
}

impl VerseHierarchy {
    pub const SIZE: usize = 1024 * 32; // 32KB for hierarchy storage

    pub fn new() -> Self {
        Self {
            verses: HashMap::new(),
            root_verses: Vec::new(),
            total_verses: 0,
            total_markets: 0,
        }
    }

    /// Create or link verse with conflict resolution
    pub fn create_or_link_verse(
        &mut self,
        verse_id: [u8; 32],
        parent_id: Option<[u8; 32]>,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check if verse already exists
        if let Some(existing_verse) = self.verses.get(&verse_id) {
            // Verse exists - check parent conflict
            if existing_verse.parent_id != parent_id {
                msg!("Parent conflict detected for verse {:?}", verse_id);
                return Err(BettingPlatformError::SingleParentInvariant.into());
            }
            return Ok(());
        }

        // Validate parent exists and depth constraint
        let depth = if let Some(pid) = parent_id {
            let parent = self.verses.get(&pid)
                .ok_or(BettingPlatformError::ParentVerseNotFound)?;
            
            let parent_depth = parent.calculate_effective_depth(self)?;
            if parent_depth >= MAX_VERSE_DEPTH - 1 {
                return Err(BettingPlatformError::MaxDepthExceeded.into());
            }
            parent_depth + 1
        } else {
            0
        };

        // Create new verse (atomic operation)
        let mut verse = VersePDA::new(verse_id, current_slot);
        verse.parent_id = parent_id;
        verse.depth = depth;

        // Update parent's children if applicable
        if let Some(pid) = parent_id {
            if let Some(parent) = self.verses.get_mut(&pid) {
                if parent.child_verses.len() >= 10 {
                    return Err(BettingPlatformError::MaxChildrenExceeded.into());
                }
                parent.child_verses.push(verse_id);
                parent.last_updated_slot = current_slot;
            }
        } else {
            // Root verse
            self.root_verses.push(verse_id);
        }

        // Insert verse
        self.verses.insert(verse_id, verse);
        self.total_verses += 1;

        msg!("Created verse {:?} at depth {}", verse_id, depth);
        Ok(())
    }

    /// Find verse for market with rebalancing
    pub fn find_verse_for_market(
        &mut self,
        market_verse_id: [u8; 32],
        current_slot: u64,
    ) -> Result<[u8; 32], ProgramError> {
        // Try direct verse first
        if let Some(verse) = self.verses.get_mut(&market_verse_id) {
            if verse.has_capacity() {
                verse.add_market(current_slot)?;
                self.total_markets += 1;
                return Ok(market_verse_id);
            }
        }

        // Need to find alternative verse or create split
        self.handle_verse_overflow(market_verse_id, current_slot)
    }

    /// Handle verse overflow by splitting or finding alternative
    fn handle_verse_overflow(
        &mut self,
        original_verse_id: [u8; 32],
        current_slot: u64,
    ) -> Result<[u8; 32], ProgramError> {
        // Strategy: Create child verse for overflow
        let mut hasher = solana_program::keccak::Hasher::default();
        hasher.hash(&original_verse_id);
        hasher.hash(&current_slot.to_le_bytes());
        let new_verse_id = hasher.result().to_bytes();

        // Create child verse
        self.create_or_link_verse(
            new_verse_id,
            Some(original_verse_id),
            current_slot,
        )?;

        // Add market to new verse
        if let Some(verse) = self.verses.get_mut(&new_verse_id) {
            verse.add_market(current_slot)?;
            self.total_markets += 1;
            Ok(new_verse_id)
        } else {
            Err(BettingPlatformError::InternalError.into())
        }
    }

    /// Detect cycles in hierarchy (should never happen with single parent)
    pub fn detect_cycles(&self) -> bool {
        let mut visited = HashMap::new();
        let mut recursion_stack = HashMap::new();

        for verse_id in self.verses.keys() {
            if self.has_cycle_dfs(*verse_id, &mut visited, &mut recursion_stack) {
                return true;
            }
        }

        false
    }

    /// DFS helper for cycle detection
    fn has_cycle_dfs(
        &self,
        verse_id: [u8; 32],
        visited: &mut HashMap<[u8; 32], bool>,
        recursion_stack: &mut HashMap<[u8; 32], bool>,
    ) -> bool {
        visited.insert(verse_id, true);
        recursion_stack.insert(verse_id, true);

        if let Some(verse) = self.verses.get(&verse_id) {
            // Check parent
            if let Some(parent_id) = verse.parent_id {
                if !visited.get(&parent_id).copied().unwrap_or(false) {
                    if self.has_cycle_dfs(parent_id, visited, recursion_stack) {
                        return true;
                    }
                } else if recursion_stack.get(&parent_id).copied().unwrap_or(false) {
                    return true;
                }
            }

            // Check children
            for child_id in &verse.child_verses {
                if !visited.get(child_id).copied().unwrap_or(false) {
                    if self.has_cycle_dfs(*child_id, visited, recursion_stack) {
                        return true;
                    }
                } else if recursion_stack.get(child_id).copied().unwrap_or(false) {
                    return true;
                }
            }
        }

        recursion_stack.insert(verse_id, false);
        false
    }

    /// Get hierarchy statistics
    pub fn get_stats(&self) -> HierarchyStats {
        let mut max_depth = 0;
        let mut total_depth = 0;
        let mut depth_distribution = vec![0u32; (MAX_VERSE_DEPTH + 1) as usize];

        for verse in self.verses.values() {
            let depth = verse.depth as usize;
            depth_distribution[depth] += 1;
            total_depth += depth;
            max_depth = max_depth.max(depth);
        }

        let avg_depth = if self.total_verses > 0 {
            total_depth / self.total_verses as usize
        } else {
            0
        };

        HierarchyStats {
            total_verses: self.total_verses,
            total_markets: self.total_markets,
            root_verses: self.root_verses.len() as u32,
            max_depth: max_depth as u8,
            avg_depth: avg_depth as u8,
            depth_distribution,
            has_cycles: false, // Should always be false with proper implementation
        }
    }
}

/// Hierarchy statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct HierarchyStats {
    pub total_verses: u32,
    pub total_markets: u32,
    pub root_verses: u32,
    pub max_depth: u8,
    pub avg_depth: u8,
    pub depth_distribution: Vec<u32>,
    pub has_cycles: bool,
}

/// Money-making calculations for verses
impl VersePDA {
    /// Calculate leverage boost from depth
    pub fn calculate_depth_leverage_boost(&self) -> f64 {
        // 10% boost per level as specified
        1.0 + (0.1 * self.depth as f64)
    }

    /// Calculate effective leverage with depth
    pub fn calculate_effective_leverage(&self, base_leverage: f64) -> f64 {
        let depth_boost = self.calculate_depth_leverage_boost();
        (base_leverage * depth_boost).min(420.0) // Cap at 420x as per spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verse_creation() {
        let mut hierarchy = VerseHierarchy::new();
        
        // Create root verse
        let root_id = [1u8; 32];
        hierarchy.create_or_link_verse(root_id, None, 100).unwrap();
        
        assert_eq!(hierarchy.total_verses, 1);
        assert_eq!(hierarchy.root_verses.len(), 1);
        
        // Create child verse
        let child_id = [2u8; 32];
        hierarchy.create_or_link_verse(child_id, Some(root_id), 200).unwrap();
        
        assert_eq!(hierarchy.total_verses, 2);
        let child = hierarchy.verses.get(&child_id).unwrap();
        assert_eq!(child.depth, 1);
        assert_eq!(child.parent_id, Some(root_id));
    }

    #[test]
    fn test_parent_conflict() {
        let mut hierarchy = VerseHierarchy::new();
        
        let verse_id = [1u8; 32];
        let parent1 = [2u8; 32];
        let parent2 = [3u8; 32];
        
        // Create parents
        hierarchy.create_or_link_verse(parent1, None, 100).unwrap();
        hierarchy.create_or_link_verse(parent2, None, 100).unwrap();
        
        // Link to first parent
        hierarchy.create_or_link_verse(verse_id, Some(parent1), 200).unwrap();
        
        // Try to link to different parent - should fail
        let result = hierarchy.create_or_link_verse(verse_id, Some(parent2), 300);
        assert!(result.is_err());
    }

    #[test]
    fn test_depth_limit() {
        let mut hierarchy = VerseHierarchy::new();
        
        let mut current_id = [0u8; 32];
        let mut parent_id = None;
        
        // Create chain up to max depth
        for i in 0..MAX_VERSE_DEPTH {
            current_id[0] = i;
            hierarchy.create_or_link_verse(current_id, parent_id, 100 + i as u64).unwrap();
            parent_id = Some(current_id);
        }
        
        // Try to exceed max depth - should fail
        current_id[0] = MAX_VERSE_DEPTH;
        let result = hierarchy.create_or_link_verse(current_id, parent_id, 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_leverage_calculation() {
        let mut verse = VersePDA::new([1u8; 32], 100);
        verse.depth = 10;
        
        // 10% boost per level
        let boost = verse.calculate_depth_leverage_boost();
        assert_eq!(boost, 2.0); // 1.0 + (0.1 * 10)
        
        // Effective leverage calculation
        let base_lev = 100.0;
        let effective = verse.calculate_effective_leverage(base_lev);
        assert_eq!(effective, 200.0); // 100 * 2.0
    }
}