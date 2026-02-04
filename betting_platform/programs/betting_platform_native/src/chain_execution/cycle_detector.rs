//! Advanced Cycle Detection for Chain Execution
//!
//! Implements graph-based DFS for circular dependency detection
//! as specified in CLAUDE.md requirements.
//!
//! Key features:
//! - O(V + E) complexity for cycle detection
//! - Handles cross-verse dependencies
//! - Detects both direct and indirect cycles
//! - Maximum depth of 32 for safety

use solana_program::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    error::BettingPlatformError,
    state::chain_accounts::{ChainState, ChainPosition},
};

/// Maximum allowed chain depth
pub const MAX_CHAIN_DEPTH: usize = 32;

/// Chain dependency graph node
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ChainNode {
    pub chain_id: u128,
    pub verse_id: u128,
    pub dependencies: Vec<u128>, // Chain IDs this chain depends on
    pub dependents: Vec<u128>,   // Chain IDs that depend on this chain
    pub positions: Vec<u128>,    // Position IDs in this chain
    pub depth: u8,
}

impl ChainNode {
    pub fn new(chain_id: u128, verse_id: u128) -> Self {
        Self {
            chain_id,
            verse_id,
            dependencies: Vec::new(),
            dependents: Vec::new(),
            positions: Vec::new(),
            depth: 0,
        }
    }

    /// Add dependency to another chain
    pub fn add_dependency(&mut self, dep_chain_id: u128) -> Result<(), ProgramError> {
        if self.dependencies.contains(&dep_chain_id) {
            return Ok(()); // Already exists
        }

        if dep_chain_id == self.chain_id {
            return Err(BettingPlatformError::SelfDependency.into());
        }

        self.dependencies.push(dep_chain_id);
        Ok(())
    }

    /// Add dependent chain
    pub fn add_dependent(&mut self, dependent_chain_id: u128) -> Result<(), ProgramError> {
        if self.dependents.contains(&dependent_chain_id) {
            return Ok(()); // Already exists
        }

        if dependent_chain_id == self.chain_id {
            return Err(BettingPlatformError::SelfDependency.into());
        }

        self.dependents.push(dependent_chain_id);
        Ok(())
    }
}

/// Dependency graph for cycle detection
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ChainDependencyGraph {
    pub nodes: HashMap<u128, ChainNode>,
    pub total_nodes: u32,
    pub total_edges: u32,
}

impl ChainDependencyGraph {
    pub const SIZE: usize = 1024 * 16; // 16KB for graph storage

    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            total_nodes: 0,
            total_edges: 0,
        }
    }

    /// Add chain to graph
    pub fn add_chain(&mut self, chain_id: u128, verse_id: u128) -> Result<(), ProgramError> {
        if self.nodes.contains_key(&chain_id) {
            return Ok(()); // Already exists
        }

        let node = ChainNode::new(chain_id, verse_id);
        self.nodes.insert(chain_id, node);
        self.total_nodes += 1;

        Ok(())
    }

    /// Add dependency between chains
    pub fn add_dependency(
        &mut self,
        from_chain_id: u128,
        to_chain_id: u128,
    ) -> Result<(), ProgramError> {
        // Ensure both nodes exist
        if !self.nodes.contains_key(&from_chain_id) {
            return Err(BettingPlatformError::ChainNotFound.into());
        }
        if !self.nodes.contains_key(&to_chain_id) {
            return Err(BettingPlatformError::ChainNotFound.into());
        }

        // Add forward dependency
        if let Some(from_node) = self.nodes.get_mut(&from_chain_id) {
            from_node.add_dependency(to_chain_id)?;
        }

        // Add reverse dependency
        if let Some(to_node) = self.nodes.get_mut(&to_chain_id) {
            to_node.add_dependent(from_chain_id)?;
        }

        self.total_edges += 1;

        // Check for cycles after adding edge
        if self.has_cycle_from(from_chain_id)? {
            // Revert the edge
            if let Some(from_node) = self.nodes.get_mut(&from_chain_id) {
                from_node.dependencies.retain(|&x| x != to_chain_id);
            }
            if let Some(to_node) = self.nodes.get_mut(&to_chain_id) {
                to_node.dependents.retain(|&x| x != from_chain_id);
            }
            self.total_edges -= 1;

            return Err(BettingPlatformError::CircularDependency.into());
        }

        Ok(())
    }

    /// Detect cycles using DFS (three-color algorithm)
    pub fn has_cycle(&self) -> Result<bool, ProgramError> {
        let mut colors: HashMap<u128, NodeColor> = HashMap::new();

        // Initialize all nodes as white
        for &chain_id in self.nodes.keys() {
            colors.insert(chain_id, NodeColor::White);
        }

        // Check each component
        for &chain_id in self.nodes.keys() {
            if colors.get(&chain_id) == Some(&NodeColor::White) {
                if self.dfs_visit(chain_id, &mut colors)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Alias for has_cycle for backward compatibility
    pub fn detect_cycles(&self) -> Result<bool, ProgramError> {
        self.has_cycle()
    }

    /// Check for cycle starting from specific node
    pub fn has_cycle_from(&self, start_chain_id: u128) -> Result<bool, ProgramError> {
        let mut colors: HashMap<u128, NodeColor> = HashMap::new();

        // Initialize reachable nodes as white
        let reachable = self.get_reachable_nodes(start_chain_id);
        for chain_id in reachable {
            colors.insert(chain_id, NodeColor::White);
        }

        // Run DFS from start node
        self.dfs_visit(start_chain_id, &mut colors)
    }

    /// DFS visit for cycle detection
    fn dfs_visit(
        &self,
        chain_id: u128,
        colors: &mut HashMap<u128, NodeColor>,
    ) -> Result<bool, ProgramError> {
        // Mark as gray (visiting)
        colors.insert(chain_id, NodeColor::Gray);

        if let Some(node) = self.nodes.get(&chain_id) {
            for &dep_chain_id in &node.dependencies {
                match colors.get(&dep_chain_id).copied() {
                    Some(NodeColor::Gray) => {
                        // Found back edge - cycle detected
                        msg!("Cycle detected: {} -> {}", chain_id, dep_chain_id);
                        return Ok(true);
                    }
                    Some(NodeColor::White) => {
                        // Recursively visit
                        if self.dfs_visit(dep_chain_id, colors)? {
                            return Ok(true);
                        }
                    }
                    _ => {} // Black node, already processed
                }
            }
        }

        // Mark as black (completed)
        colors.insert(chain_id, NodeColor::Black);
        Ok(false)
    }

    /// Get all nodes reachable from start
    fn get_reachable_nodes(&self, start: u128) -> HashSet<u128> {
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();
        
        queue.push_back(start);
        reachable.insert(start);

        while let Some(current) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current) {
                for &dep in &node.dependencies {
                    if reachable.insert(dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }

        reachable
    }

    /// Calculate chain depth (longest path from root)
    pub fn calculate_chain_depth(&self, chain_id: u128) -> Result<u8, ProgramError> {
        let mut visited = HashSet::new();
        self.calculate_depth_recursive(chain_id, &mut visited, 0)
    }

    fn calculate_depth_recursive(
        &self,
        chain_id: u128,
        visited: &mut HashSet<u128>,
        current_depth: u8,
    ) -> Result<u8, ProgramError> {
        if current_depth >= MAX_CHAIN_DEPTH as u8 {
            return Err(BettingPlatformError::MaxDepthExceeded.into());
        }

        if !visited.insert(chain_id) {
            // Already visited - potential cycle
            return Ok(current_depth);
        }

        let mut max_depth = current_depth;

        if let Some(node) = self.nodes.get(&chain_id) {
            for &dep_chain_id in &node.dependencies {
                let dep_depth = self.calculate_depth_recursive(
                    dep_chain_id,
                    visited,
                    current_depth + 1,
                )?;
                max_depth = max_depth.max(dep_depth);
            }
        }

        visited.remove(&chain_id);
        Ok(max_depth)
    }

    /// Validate chain can be added without violations
    pub fn validate_chain_addition(
        &self,
        new_chain_id: u128,
        dependencies: &[u128],
    ) -> Result<(), ProgramError> {
        // Check max depth constraint
        let mut max_dep_depth = 0u8;
        for &dep_id in dependencies {
            let dep_depth = self.calculate_chain_depth(dep_id)?;
            max_dep_depth = max_dep_depth.max(dep_depth);
        }

        if max_dep_depth >= MAX_CHAIN_DEPTH as u8 - 1 {
            return Err(BettingPlatformError::MaxDepthExceeded.into());
        }

        // Simulate adding chain to check for cycles
        let mut temp_graph = self.clone();
        temp_graph.add_chain(new_chain_id, 0)?; // verse_id doesn't matter for validation

        for &dep_id in dependencies {
            temp_graph.add_dependency(new_chain_id, dep_id)?;
        }

        Ok(())
    }

    /// Get topological ordering (if no cycles)
    pub fn topological_sort(&self) -> Result<Vec<u128>, ProgramError> {
        if self.has_cycle()? {
            return Err(BettingPlatformError::CircularDependency.into());
        }

        let mut in_degree: HashMap<u128, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Calculate in-degrees
        for (&chain_id, node) in &self.nodes {
            in_degree.insert(chain_id, node.dependents.len());
            if node.dependents.is_empty() {
                queue.push_back(chain_id);
            }
        }

        // Process nodes with no incoming edges
        while let Some(chain_id) = queue.pop_front() {
            result.push(chain_id);

            if let Some(node) = self.nodes.get(&chain_id) {
                for &dep_id in &node.dependencies {
                    if let Some(degree) = in_degree.get_mut(&dep_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dep_id);
                        }
                    }
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err(BettingPlatformError::CircularDependency.into());
        }

        Ok(result)
    }

    /// Get execution order for chains (reverse topological sort)
    pub fn get_execution_order(&self) -> Result<Vec<u128>, ProgramError> {
        let mut order = self.topological_sort()?;
        order.reverse();
        Ok(order)
    }

    /// Find all cycles in the graph
    pub fn find_all_cycles(&self) -> Vec<Vec<u128>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();

        for &chain_id in self.nodes.keys() {
            if !visited.contains(&chain_id) {
                self.find_cycles_dfs(chain_id, &mut visited, &mut path, &mut cycles);
            }
        }

        cycles
    }

    fn find_cycles_dfs(
        &self,
        chain_id: u128,
        visited: &mut HashSet<u128>,
        path: &mut Vec<u128>,
        cycles: &mut Vec<Vec<u128>>,
    ) {
        if path.contains(&chain_id) {
            // Found cycle
            let start_idx = path.iter().position(|&id| id == chain_id).unwrap();
            let cycle = path[start_idx..].to_vec();
            cycles.push(cycle);
            return;
        }

        if visited.contains(&chain_id) {
            return;
        }

        path.push(chain_id);

        if let Some(node) = self.nodes.get(&chain_id) {
            for &dep_id in &node.dependencies {
                self.find_cycles_dfs(dep_id, visited, path, cycles);
            }
        }

        path.pop();
        visited.insert(chain_id);
    }
}

/// Node color for DFS
#[derive(Clone, Copy, PartialEq)]
enum NodeColor {
    White,  // Not visited
    Gray,   // Visiting
    Black,  // Completed
}

/// Cross-verse validation for chains
pub struct CrossVerseValidator;

impl CrossVerseValidator {
    /// Validate chain doesn't create cross-verse cycles
    pub fn validate_cross_verse_chain(
        graph: &ChainDependencyGraph,
        chain_id: u128,
        verse_id: u128,
    ) -> Result<(), ProgramError> {
        // Get all chains in dependency path
        let mut visited = HashSet::new();
        let mut verse_chain = HashMap::new();

        Self::collect_verse_chains(graph, chain_id, &mut visited, &mut verse_chain)?;

        // Check for verse conflicts
        for (&dep_chain_id, &dep_verse_id) in &verse_chain {
            if dep_verse_id != verse_id {
                // Cross-verse dependency - check if allowed
                if !Self::is_cross_verse_allowed(verse_id, dep_verse_id) {
                    msg!(
                        "Cross-verse dependency not allowed: {} -> {}",
                        verse_id, dep_verse_id
                    );
                    return Err(BettingPlatformError::CrossVerseNotAllowed.into());
                }
            }
        }

        Ok(())
    }

    fn collect_verse_chains(
        graph: &ChainDependencyGraph,
        chain_id: u128,
        visited: &mut HashSet<u128>,
        verse_chain: &mut HashMap<u128, u128>,
    ) -> Result<(), ProgramError> {
        if !visited.insert(chain_id) {
            return Ok(());
        }

        if let Some(node) = graph.nodes.get(&chain_id) {
            verse_chain.insert(chain_id, node.verse_id);

            for &dep_id in &node.dependencies {
                Self::collect_verse_chains(graph, dep_id, visited, verse_chain)?;
            }
        }

        Ok(())
    }

    fn is_cross_verse_allowed(verse1: u128, verse2: u128) -> bool {
        // In production, this would check verse hierarchy rules
        // For now, allow all cross-verse dependencies
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_cycle_detection() {
        let mut graph = ChainDependencyGraph::new();

        // Create chains
        graph.add_chain(1, 100).unwrap();
        graph.add_chain(2, 100).unwrap();
        graph.add_chain(3, 100).unwrap();

        // Add dependencies: 1 -> 2 -> 3
        graph.add_dependency(1, 2).unwrap();
        graph.add_dependency(2, 3).unwrap();

        // No cycle yet
        assert!(!graph.has_cycle().unwrap());

        // Try to add 3 -> 1 (creates cycle)
        let result = graph.add_dependency(3, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_self_dependency() {
        let mut graph = ChainDependencyGraph::new();
        graph.add_chain(1, 100).unwrap();

        // Try self-dependency
        let result = graph.add_dependency(1, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_topological_sort() {
        let mut graph = ChainDependencyGraph::new();

        // Create DAG: 1 -> 2 -> 4
        //                 \-> 3 /
        graph.add_chain(1, 100).unwrap();
        graph.add_chain(2, 100).unwrap();
        graph.add_chain(3, 100).unwrap();
        graph.add_chain(4, 100).unwrap();

        graph.add_dependency(1, 2).unwrap();
        graph.add_dependency(1, 3).unwrap();
        graph.add_dependency(2, 4).unwrap();
        graph.add_dependency(3, 4).unwrap();

        let order = graph.topological_sort().unwrap();
        
        // 1 should come before 2 and 3
        let pos_1 = order.iter().position(|&x| x == 1).unwrap();
        let pos_2 = order.iter().position(|&x| x == 2).unwrap();
        let pos_3 = order.iter().position(|&x| x == 3).unwrap();
        let pos_4 = order.iter().position(|&x| x == 4).unwrap();

        assert!(pos_1 < pos_2);
        assert!(pos_1 < pos_3);
        assert!(pos_2 < pos_4);
        assert!(pos_3 < pos_4);
    }

    #[test]
    fn test_depth_calculation() {
        let mut graph = ChainDependencyGraph::new();

        // Create chain: 1 -> 2 -> 3 -> 4
        for i in 1..=4 {
            graph.add_chain(i, 100).unwrap();
        }

        for i in 1..4 {
            graph.add_dependency(i, i + 1).unwrap();
        }

        // Depth from 1 should be 3
        assert_eq!(graph.calculate_chain_depth(1).unwrap(), 3);
        // Depth from 4 should be 0 (no dependencies)
        assert_eq!(graph.calculate_chain_depth(4).unwrap(), 0);
    }

    #[test]
    fn test_find_all_cycles() {
        let mut graph = ChainDependencyGraph::new();

        // Create two cycles: 1 -> 2 -> 1 and 3 -> 4 -> 5 -> 3
        for i in 1..=5 {
            graph.add_chain(i, 100).unwrap();
        }

        // First cycle
        graph.nodes.get_mut(&1).unwrap().add_dependency(2).unwrap();
        graph.nodes.get_mut(&2).unwrap().add_dependency(1).unwrap();

        // Second cycle
        graph.nodes.get_mut(&3).unwrap().add_dependency(4).unwrap();
        graph.nodes.get_mut(&4).unwrap().add_dependency(5).unwrap();
        graph.nodes.get_mut(&5).unwrap().add_dependency(3).unwrap();

        let cycles = graph.find_all_cycles();
        assert_eq!(cycles.len(), 2);
    }
}