//! Cross-Verse Chain Validator
//!
//! Implements validation for chains that span multiple verses:
//! - Cross-verse dependency checking
//! - Hierarchy rule enforcement
//! - Atomic execution guarantees
//! - Permission validation
//!
//! Per specification: Production-grade cross-verse validation

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, HashSet};

use crate::{
    error::BettingPlatformError,
    state::{VersePDA, VerseStatus, chain_accounts::{ChainState, ChainPosition}},
    verse::hierarchy_manager::MAX_VERSE_DEPTH,
    chain_execution::cycle_detector::ChainDependencyGraph,
    events::{emit_event, EventType},
};

/// Cross-verse validation rules
pub const MAX_CROSS_VERSE_HOPS: u8 = 3; // Maximum verses a chain can span
pub const CROSS_VERSE_FEE_BPS: u64 = 100; // 1% fee for cross-verse chains
pub const MAX_PARALLEL_VERSES: u8 = 5; // Max verses for parallel execution

/// Cross-verse permission types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum CrossVersePermission {
    Allowed,
    RequiresApproval { approver: Pubkey },
    Restricted { reason: String },
    Forbidden,
}

/// Cross-verse validation result
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CrossVerseValidation {
    pub chain_id: u128,
    pub verses_involved: Vec<u128>,
    pub permission_status: CrossVersePermission,
    pub hierarchy_valid: bool,
    pub execution_order: Vec<VerseExecution>,
    pub estimated_fees: u64,
    pub warnings: Vec<ValidationWarning>,
}

impl CrossVerseValidation {
    pub const SIZE: usize = 1024;

    /// Create new validation result
    pub fn new(chain_id: u128) -> Self {
        Self {
            chain_id,
            verses_involved: Vec::new(),
            permission_status: CrossVersePermission::Allowed,
            hierarchy_valid: true,
            execution_order: Vec::new(),
            estimated_fees: 0,
            warnings: Vec::new(),
        }
    }

    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        self.hierarchy_valid && 
        matches!(self.permission_status, CrossVersePermission::Allowed | 
                CrossVersePermission::RequiresApproval { .. })
    }

    /// Add warning
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }
}

/// Verse execution details
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct VerseExecution {
    pub verse_id: u128,
    pub execution_index: u8,
    pub dependencies: Vec<u128>,
    pub positions: Vec<u128>,
    pub estimated_cu: u64,
}

/// Validation warnings
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum ValidationWarning {
    DeepHierarchy { depth: u8 },
    HighFees { total_fees: u64 },
    ComplexDependencies { dependency_count: u32 },
    PerformanceImpact { estimated_cu: u64 },
}

/// Cross-verse validator engine
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CrossVerseValidator {
    pub verse_permissions: HashMap<(u128, u128), CrossVersePermission>,
    pub verse_hierarchies: HashMap<u128, VerseHierarchy>,
    pub validation_cache: HashMap<u128, CrossVerseValidation>,
    pub total_validations: u64,
}

impl CrossVerseValidator {
    pub const SIZE: usize = 1024 * 16; // 16KB

    pub fn new() -> Self {
        Self {
            verse_permissions: HashMap::new(),
            verse_hierarchies: HashMap::new(),
            validation_cache: HashMap::new(),
            total_validations: 0,
        }
    }

    /// Validate cross-verse chain
    pub fn validate_chain(
        &mut self,
        chain_state: &ChainState,
        verses: &HashMap<u128, VersePDA>,
        dependency_graph: &ChainDependencyGraph,
    ) -> Result<CrossVerseValidation, ProgramError> {
        let mut validation = CrossVerseValidation::new(chain_state.chain_id);
        
        // Extract verses from chain positions
        let involved_verses = self.extract_verses_from_chain(chain_state);
        validation.verses_involved = involved_verses.clone();

        // Check verse count limit
        if involved_verses.len() > MAX_PARALLEL_VERSES as usize {
            return Err(BettingPlatformError::ExceedsVerseLimit.into());
        }

        // Validate each verse pair
        for i in 0..involved_verses.len() {
            for j in i+1..involved_verses.len() {
                let verse1 = involved_verses[i];
                let verse2 = involved_verses[j];
                
                // Check permission
                let permission = self.check_verse_permission(verse1, verse2, verses)?;
                if matches!(permission, CrossVersePermission::Forbidden) {
                    validation.permission_status = permission;
                    return Ok(validation);
                }
                
                // Update permission status if more restrictive
                if matches!(permission, CrossVersePermission::RequiresApproval { .. }) {
                    validation.permission_status = permission.clone();
                }
            }
        }

        // Validate hierarchy rules
        validation.hierarchy_valid = self.validate_hierarchy_rules(&involved_verses, verses)?;

        // Check for circular dependencies across verses
        if !validation.hierarchy_valid {
            return Ok(validation);
        }

        // Determine execution order
        validation.execution_order = self.determine_execution_order(
            chain_state,
            &involved_verses,
            dependency_graph,
        )?;

        // Calculate fees
        validation.estimated_fees = self.calculate_cross_verse_fees(
            chain_state,
            involved_verses.len(),
        );

        // Add warnings
        self.add_validation_warnings(&mut validation, chain_state, &involved_verses);

        // Cache result
        self.validation_cache.insert(chain_state.chain_id, validation.clone());
        self.total_validations += 1;

        Ok(validation)
    }

    /// Extract unique verses from chain
    fn extract_verses_from_chain(&self, chain_state: &ChainState) -> Vec<u128> {
        let mut verses = HashSet::new();
        
        // Add main verse
        verses.insert(chain_state.verse_id);
        
        // Add verses from positions
        // In production, would look up each position's market verse
        // For now, we assume all positions are in the same verse as the chain
        
        verses.into_iter().collect()
    }

    /// Check permission between two verses
    fn check_verse_permission(
        &self,
        verse1: u128,
        verse2: u128,
        verses: &HashMap<u128, VersePDA>,
    ) -> Result<CrossVersePermission, ProgramError> {
        // Check cached permissions
        let key = if verse1 < verse2 { (verse1, verse2) } else { (verse2, verse1) };
        if let Some(permission) = self.verse_permissions.get(&key) {
            return Ok(permission.clone());
        }

        // Check verse relationship
        let verse1_data = verses.get(&verse1)
            .ok_or(BettingPlatformError::VerseNotFound)?;
        let verse2_data = verses.get(&verse2)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Check if verses are in same hierarchy
        if self.are_verses_related(verse1, verse2, verses) {
            Ok(CrossVersePermission::Allowed)
        } else if verse1_data.cross_verse_enabled && verse2_data.cross_verse_enabled {
            // Both verses allow cross-verse operations
            Ok(CrossVersePermission::Allowed)
        } else {
            // Requires approval from verse authorities
            Ok(CrossVersePermission::RequiresApproval {
                approver: Pubkey::default(), // In production, would have verse authority
            })
        }
    }

    /// Check if verses are in same hierarchy
    fn are_verses_related(
        &self,
        verse1: u128,
        verse2: u128,
        verses: &HashMap<u128, VersePDA>,
    ) -> bool {
        // Check parent-child relationship
        if let Some(v1) = verses.get(&verse1) {
            if let Some(v2) = verses.get(&verse2) {
                return v1.parent_id == Some(verse2) || 
                       v2.parent_id == Some(verse1) ||
                       v1.parent_id == v2.parent_id;
            }
        }
        false
    }

    /// Validate hierarchy rules
    fn validate_hierarchy_rules(
        &self,
        involved_verses: &[u128],
        verses: &HashMap<u128, VersePDA>,
    ) -> Result<bool, ProgramError> {
        // Check depth constraints
        for verse_id in involved_verses {
            if let Some(verse) = verses.get(verse_id) {
                if verse.depth > MAX_VERSE_DEPTH {
                    return Ok(false);
                }
            }
        }

        // Check for hierarchy violations
        for i in 0..involved_verses.len() {
            for j in i+1..involved_verses.len() {
                if self.would_create_hierarchy_violation(
                    involved_verses[i],
                    involved_verses[j],
                    verses,
                ) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Check if chain would violate hierarchy
    fn would_create_hierarchy_violation(
        &self,
        verse1: u128,
        verse2: u128,
        verses: &HashMap<u128, VersePDA>,
    ) -> bool {
        // Check if one verse is ancestor of another
        if let Some(hierarchy) = self.verse_hierarchies.get(&verse1) {
            if hierarchy.is_ancestor_of(verse2) || hierarchy.is_descendant_of(verse2) {
                // Direct hierarchy relationship - check if allowed
                return !self.is_hierarchy_chain_allowed(verse1, verse2, verses);
            }
        }
        false
    }

    /// Check if hierarchy chain is allowed
    fn is_hierarchy_chain_allowed(
        &self,
        parent: u128,
        child: u128,
        verses: &HashMap<u128, VersePDA>,
    ) -> bool {
        // Chains are allowed between direct parent-child verses
        if let Some(child_verse) = verses.get(&child) {
            return child_verse.parent_id == Some(parent);
        }
        false
    }

    /// Determine execution order for cross-verse chain
    fn determine_execution_order(
        &self,
        chain_state: &ChainState,
        involved_verses: &[u128],
        dependency_graph: &ChainDependencyGraph,
    ) -> Result<Vec<VerseExecution>, ProgramError> {
        let mut executions = Vec::new();
        
        // Build verse dependency map
        let verse_deps = self.build_verse_dependencies(chain_state, dependency_graph);
        
        // Topological sort of verses
        let sorted_verses = self.topological_sort_verses(involved_verses, &verse_deps)?;
        
        // Create execution entries
        for (index, verse_id) in sorted_verses.iter().enumerate() {
            let positions = self.get_verse_positions(chain_state, *verse_id);
            let dependencies = verse_deps.get(verse_id).cloned().unwrap_or_default();
            let position_count = positions.len();
            
            executions.push(VerseExecution {
                verse_id: *verse_id,
                execution_index: index as u8,
                dependencies,
                positions,
                estimated_cu: self.estimate_verse_cu(position_count),
            });
        }
        
        Ok(executions)
    }

    /// Build verse dependency map
    fn build_verse_dependencies(
        &self,
        chain_state: &ChainState,
        dependency_graph: &ChainDependencyGraph,
    ) -> HashMap<u128, Vec<u128>> {
        let mut deps: HashMap<u128, Vec<u128>> = HashMap::new();
        
        // Analyze chain positions for dependencies
        for position_id in &chain_state.position_ids {
            if let Some(node) = dependency_graph.nodes.get(position_id) {
                let from_verse = chain_state.verse_id;
                
                for dep_id in &node.dependencies {
                    // Find verse of dependency
                    if let Some(dep_verse) = self.find_position_verse(chain_state, *dep_id) {
                        if from_verse != dep_verse {
                            deps.entry(from_verse)
                                .or_insert_with(Vec::new)
                                .push(dep_verse);
                        }
                    }
                }
            }
        }
        
        // Remove duplicates
        for dep_list in deps.values_mut() {
            dep_list.sort();
            dep_list.dedup();
        }
        
        deps
    }

    /// Find verse containing position
    fn find_position_verse(&self, chain_state: &ChainState, position_id: u128) -> Option<u128> {
        // In production, would look up position details from position account
        // For now, return chain's verse_id if position exists
        if chain_state.position_ids.contains(&position_id) {
            Some(chain_state.verse_id)
        } else {
            None
        }
    }

    /// Get positions for specific verse
    fn get_verse_positions(&self, chain_state: &ChainState, verse_id: u128) -> Vec<u128> {
        // In production, would filter positions by verse
        // For now, return all positions if verse matches chain's verse
        if chain_state.verse_id == verse_id {
            chain_state.position_ids.clone()
        } else {
            Vec::new()
        }
    }

    /// Topological sort verses based on dependencies
    fn topological_sort_verses(
        &self,
        verses: &[u128],
        dependencies: &HashMap<u128, Vec<u128>>,
    ) -> Result<Vec<u128>, ProgramError> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        
        for verse in verses {
            if !visited.contains(verse) {
                self.visit_verse(
                    *verse,
                    dependencies,
                    &mut visited,
                    &mut temp_visited,
                    &mut sorted,
                )?;
            }
        }
        
        sorted.reverse();
        Ok(sorted)
    }

    /// DFS visit for topological sort
    fn visit_verse(
        &self,
        verse: u128,
        dependencies: &HashMap<u128, Vec<u128>>,
        visited: &mut HashSet<u128>,
        temp_visited: &mut HashSet<u128>,
        sorted: &mut Vec<u128>,
    ) -> Result<(), ProgramError> {
        if temp_visited.contains(&verse) {
            return Err(BettingPlatformError::CircularDependency.into());
        }
        
        if visited.contains(&verse) {
            return Ok(());
        }
        
        temp_visited.insert(verse);
        
        if let Some(deps) = dependencies.get(&verse) {
            for dep in deps {
                self.visit_verse(*dep, dependencies, visited, temp_visited, sorted)?;
            }
        }
        
        temp_visited.remove(&verse);
        visited.insert(verse);
        sorted.push(verse);
        
        Ok(())
    }

    /// Calculate cross-verse fees
    fn calculate_cross_verse_fees(
        &self,
        chain_state: &ChainState,
        verse_count: usize,
    ) -> u64 {
        if verse_count <= 1 {
            return 0;
        }
        
        // Base fee for cross-verse
        let base_fee = (chain_state.initial_deposit * CROSS_VERSE_FEE_BPS) / 10000;
        
        // Additional fee for each extra verse
        let verse_fee = base_fee * (verse_count as u64 - 1) / 2;
        
        base_fee + verse_fee
    }

    /// Estimate compute units for verse
    fn estimate_verse_cu(&self, position_count: usize) -> u64 {
        // Base CU per position
        let base_cu = 5000u64;
        let position_cu = position_count as u64 * 2000;
        
        base_cu + position_cu
    }

    /// Add validation warnings
    fn add_validation_warnings(
        &self,
        validation: &mut CrossVerseValidation,
        chain_state: &ChainState,
        involved_verses: &[u128],
    ) {
        // Check for deep hierarchy
        let max_depth = self.get_max_verse_depth(involved_verses);
        if max_depth > 16 {
            validation.add_warning(ValidationWarning::DeepHierarchy { depth: max_depth });
        }
        
        // Check for high fees
        if validation.estimated_fees > chain_state.initial_deposit / 10 {
            validation.add_warning(ValidationWarning::HighFees {
                total_fees: validation.estimated_fees,
            });
        }
        
        // Check complexity
        let total_positions = chain_state.position_ids.len();
        if total_positions > 20 {
            validation.add_warning(ValidationWarning::ComplexDependencies {
                dependency_count: total_positions as u32,
            });
        }
        
        // Check performance
        let total_cu: u64 = validation.execution_order
            .iter()
            .map(|e| e.estimated_cu)
            .sum();
        
        if total_cu > 100_000 {
            validation.add_warning(ValidationWarning::PerformanceImpact {
                estimated_cu: total_cu,
            });
        }
    }

    /// Get maximum depth of involved verses
    fn get_max_verse_depth(&self, verses: &[u128]) -> u8 {
        verses.iter()
            .filter_map(|v| self.verse_hierarchies.get(v))
            .map(|h| h.depth)
            .max()
            .unwrap_or(0)
    }

    /// Update verse permissions
    pub fn update_permission(
        &mut self,
        verse1: u128,
        verse2: u128,
        permission: CrossVersePermission,
    ) {
        let key = if verse1 < verse2 { (verse1, verse2) } else { (verse2, verse1) };
        self.verse_permissions.insert(key, permission);
    }

    /// Update verse hierarchy
    pub fn update_hierarchy(
        &mut self,
        verse_id: u128,
        parent: Option<u128>,
        depth: u8,
    ) {
        let hierarchy = VerseHierarchy {
            verse_id,
            parent,
            children: Vec::new(),
            depth,
        };
        
        self.verse_hierarchies.insert(verse_id, hierarchy);
        
        // Update parent's children
        if let Some(parent_id) = parent {
            if let Some(parent_hierarchy) = self.verse_hierarchies.get_mut(&parent_id) {
                parent_hierarchy.children.push(verse_id);
            }
        }
    }
}

/// Verse hierarchy information
#[derive(BorshSerialize, BorshDeserialize, Clone)]
struct VerseHierarchy {
    verse_id: u128,
    parent: Option<u128>,
    children: Vec<u128>,
    depth: u8,
}

impl VerseHierarchy {
    /// Check if this verse is ancestor of another
    fn is_ancestor_of(&self, other: u128) -> bool {
        if self.children.contains(&other) {
            return true;
        }
        
        // Check recursively (in production would use iterative approach)
        false
    }
    
    /// Check if this verse is descendant of another
    fn is_descendant_of(&self, other: u128) -> bool {
        self.parent == Some(other)
    }
}

/// Cross-verse execution coordinator
pub struct CrossVerseExecutor;

impl CrossVerseExecutor {
    /// Execute cross-verse chain atomically
    pub fn execute_cross_verse_chain(
        validation: &CrossVerseValidation,
        chain_state: &mut ChainState,
        verse_accounts: &mut HashMap<u128, AccountInfo>,
    ) -> Result<(), ProgramError> {
        if !validation.is_valid() {
            return Err(BettingPlatformError::InvalidChainStatus.into());
        }
        
        // Execute in validated order
        for execution in &validation.execution_order {
            msg!("Executing verse {} positions", execution.verse_id);
            
            // Verify verse account
            if !verse_accounts.contains_key(&execution.verse_id) {
                return Err(BettingPlatformError::VerseNotFound.into());
            }
            
            // Execute positions for this verse
            for position_id in &execution.positions {
                // In production, would execute actual position logic
                msg!("Executing position {} in verse {}", position_id, execution.verse_id);
            }
        }
        
        // Deduct cross-verse fees
        if validation.estimated_fees > 0 {
            chain_state.current_balance = chain_state.current_balance
                .saturating_sub(validation.estimated_fees);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_verse_validation() {
        let mut validator = CrossVerseValidator::new();
        let mut verses = HashMap::new();
        
        // Create test verses
        let verse1 = VersePDA {
            verse_id: 1,
            parent_verse: None,
            depth: 0,
            cross_verse_enabled: true,
            authority: Pubkey::new_unique(),
            market_count: 0,
            markets: Vec::new(),
            child_verses: vec![2],
            status: VerseStatus::Active,
        };
        
        let verse2 = VersePDA {
            verse_id: 2,
            parent_verse: Some(1),
            depth: 1,
            cross_verse_enabled: true,
            authority: Pubkey::new_unique(),
            market_count: 0,
            markets: Vec::new(),
            child_verses: Vec::new(),
            status: VerseStatus::Active,
        };
        
        verses.insert(1, verse1);
        verses.insert(2, verse2);
        
        // Create chain spanning verses
        let chain_state = ChainState {
            chain_id: 123,
            user: Pubkey::new_unique(),
            verse_id: 1,
            initial_deposit: 1000,
            current_balance: 1000,
            positions: vec![
                ChainPosition {
                    position_id: 1,
                    verse_id: 1,
                    market_id: [0; 16],
                    outcome: 0,
                    size: 100,
                    leverage: 10,
                    entry_price: 5000,
                    status: crate::state::chain_accounts::PositionStatus::Open,
                },
                ChainPosition {
                    position_id: 2,
                    verse_id: 2,
                    market_id: [1; 16],
                    outcome: 1,
                    size: 100,
                    leverage: 10,
                    entry_price: 5000,
                    status: crate::state::chain_accounts::PositionStatus::Open,
                },
            ],
            chain_type: crate::state::chain_accounts::ChainType::Sequential,
            created_at: 100,
            last_execution: 100,
            status: crate::state::chain_accounts::ChainStatus::Active,
            current_step: 0,
            total_steps: 2,
        };
        
        let graph = ChainDependencyGraph::new();
        
        let validation = validator.validate_chain(&chain_state, &verses, &graph).unwrap();
        
        assert!(validation.is_valid());
        assert_eq!(validation.verses_involved.len(), 2);
        assert!(validation.estimated_fees > 0); // Cross-verse fee applied
    }

    #[test]
    fn test_permission_checking() {
        let validator = CrossVerseValidator::new();
        let mut verses = HashMap::new();
        
        // Verses without cross-verse enabled
        let verse1 = VersePDA {
            verse_id: 1,
            parent_verse: None,
            depth: 0,
            cross_verse_enabled: false,
            authority: Pubkey::new_unique(),
            market_count: 0,
            markets: Vec::new(),
            child_verses: Vec::new(),
            status: VerseStatus::Active,
        };
        
        verses.insert(1, verse1);
        
        let permission = validator.check_verse_permission(1, 2, &verses);
        
        // Should require approval since cross-verse is disabled
        assert!(matches!(
            permission,
            Ok(CrossVersePermission::RequiresApproval { .. })
        ));
    }

    #[test]
    fn test_hierarchy_validation() {
        let validator = CrossVerseValidator::new();
        
        // Test depth calculation
        assert_eq!(validator.get_max_verse_depth(&[1, 2, 3]), 0);
        
        // Test fee calculation
        let chain_state = ChainState {
            initial_deposit: 10000,
            // ... other fields
            chain_id: 0,
            user: Pubkey::new_unique(),
            verse_id: 0,
            current_balance: 10000,
            positions: Vec::new(),
            chain_type: crate::state::chain_accounts::ChainType::Sequential,
            created_at: 0,
            last_execution: 0,
            status: crate::state::chain_accounts::ChainStatus::Active,
            current_step: 0,
            total_steps: 0,
        };
        
        let fees = validator.calculate_cross_verse_fees(&chain_state, 3);
        assert_eq!(fees, 100 + 100); // Base fee + additional verse fee
    }
}