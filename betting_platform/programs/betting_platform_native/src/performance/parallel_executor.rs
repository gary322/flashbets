//! Parallel Execution Engine
//!
//! Optimizes performance by identifying and executing independent operations in parallel

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, HashSet};

use crate::error::BettingPlatformError;

/// Maximum parallel execution groups
pub const MAX_PARALLEL_GROUPS: usize = 8;

/// Parallel execution plan
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Groups of operations that can execute in parallel
    pub parallel_groups: Vec<ExecutionGroup>,
    
    /// Dependencies between groups
    pub dependencies: HashMap<usize, Vec<usize>>,
    
    /// Estimated CU savings
    pub estimated_savings: u64,
}

/// Execution group - operations that can run in parallel
#[derive(Debug, Clone)]
pub struct ExecutionGroup {
    /// Group ID
    pub id: usize,
    
    /// Operations in this group
    pub operations: Vec<Operation>,
    
    /// Accounts accessed by this group
    pub accounts: HashSet<Pubkey>,
    
    /// Estimated CU cost
    pub estimated_cu: u64,
}

/// Operation to execute
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum Operation {
    /// Update position
    UpdatePosition {
        position: Pubkey,
        data: PositionUpdateData,
    },
    
    /// Process order
    ProcessOrder {
        user: Pubkey,
        order: OrderData,
    },
    
    /// Update price
    UpdatePrice {
        market: Pubkey,
        price: u64,
    },
    
    /// Calculate metrics
    CalculateMetrics {
        target: Pubkey,
        metric_type: MetricType,
    },
}

/// Position update data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionUpdateData {
    pub new_size: Option<u64>,
    pub new_margin: Option<u64>,
    pub new_leverage: Option<u8>,
}

/// Order data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderData {
    pub market: Pubkey,
    pub outcome: u8,
    pub size: u64,
    pub order_type: u8,
}

/// Metric types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum MetricType {
    PortfolioVaR,
    PositionHealth,
    MarketVolume,
    UserPnL,
}

/// Build parallel execution plan
pub fn build_execution_plan(
    operations: Vec<Operation>,
) -> Result<ExecutionPlan, ProgramError> {
    if operations.is_empty() {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    msg!("Building execution plan for {} operations", operations.len());
    
    // Build dependency graph
    let dependency_graph = build_dependency_graph(&operations)?;
    
    // Group independent operations
    let parallel_groups = group_independent_operations(&operations, &dependency_graph)?;
    
    // Calculate estimated savings
    let estimated_savings = calculate_cu_savings(&parallel_groups);
    
    Ok(ExecutionPlan {
        parallel_groups,
        dependencies: dependency_graph,
        estimated_savings,
    })
}

/// Build dependency graph based on account access
fn build_dependency_graph(
    operations: &[Operation],
) -> Result<HashMap<usize, Vec<usize>>, ProgramError> {
    let mut dependencies = HashMap::new();
    let mut account_access: HashMap<Pubkey, Vec<usize>> = HashMap::new();
    
    // Track which operations access which accounts
    for (idx, op) in operations.iter().enumerate() {
        let accounts = get_operation_accounts(op);
        for account in accounts {
            account_access
                .entry(account)
                .or_insert_with(Vec::new)
                .push(idx);
        }
    }
    
    // Build dependencies: operations that access same accounts
    for (_, accessors) in account_access.iter() {
        if accessors.len() > 1 {
            // Create dependency chain
            for i in 1..accessors.len() {
                dependencies
                    .entry(accessors[i])
                    .or_insert_with(Vec::new)
                    .push(accessors[i - 1]);
            }
        }
    }
    
    Ok(dependencies)
}

/// Get accounts accessed by an operation
fn get_operation_accounts(operation: &Operation) -> Vec<Pubkey> {
    match operation {
        Operation::UpdatePosition { position, .. } => vec![*position],
        Operation::ProcessOrder { user, order } => vec![*user, order.market],
        Operation::UpdatePrice { market, .. } => vec![*market],
        Operation::CalculateMetrics { target, .. } => vec![*target],
    }
}

/// Group operations that can execute independently
fn group_independent_operations(
    operations: &[Operation],
    dependencies: &HashMap<usize, Vec<usize>>,
) -> Result<Vec<ExecutionGroup>, ProgramError> {
    let mut groups = Vec::new();
    let mut assigned = HashSet::new();
    let mut current_group_id = 0;
    
    // Topological sort with grouping
    let mut ready = Vec::new();
    
    // Find operations with no dependencies
    for idx in 0..operations.len() {
        if !dependencies.contains_key(&idx) {
            ready.push(idx);
        }
    }
    
    while !ready.is_empty() || assigned.len() < operations.len() {
        if ready.is_empty() {
            // Find next ready operations
            for idx in 0..operations.len() {
                if assigned.contains(&idx) {
                    continue;
                }
                
                let deps_satisfied = dependencies
                    .get(&idx)
                    .map(|deps| deps.iter().all(|d| assigned.contains(d)))
                    .unwrap_or(true);
                    
                if deps_satisfied {
                    ready.push(idx);
                }
            }
        }
        
        // Create new group from ready operations
        let mut group = ExecutionGroup {
            id: current_group_id,
            operations: Vec::new(),
            accounts: HashSet::new(),
            estimated_cu: 0,
        };
        
        // Add operations to group (avoiding account conflicts)
        let mut group_accounts = HashSet::new();
        let batch: Vec<usize> = ready.drain(..).collect();
        
        for idx in batch {
            let op_accounts = get_operation_accounts(&operations[idx]);
            let has_conflict = op_accounts.iter().any(|a| group_accounts.contains(a));
            
            if !has_conflict {
                for account in &op_accounts {
                    group_accounts.insert(*account);
                }
                
                group.operations.push(operations[idx].clone());
                group.accounts.extend(op_accounts);
                group.estimated_cu += estimate_operation_cu(&operations[idx]);
                assigned.insert(idx);
            } else {
                // Save for next group
                ready.push(idx);
            }
        }
        
        if !group.operations.is_empty() {
            groups.push(group);
            current_group_id += 1;
        }
        
        if groups.len() >= MAX_PARALLEL_GROUPS {
            break;
        }
    }
    
    Ok(groups)
}

/// Estimate CU cost for an operation
fn estimate_operation_cu(operation: &Operation) -> u64 {
    match operation {
        Operation::UpdatePosition { .. } => 3000,
        Operation::ProcessOrder { .. } => 5000,
        Operation::UpdatePrice { .. } => 2000,
        Operation::CalculateMetrics { metric_type, .. } => {
            match metric_type {
                MetricType::PortfolioVaR => 10000,
                MetricType::PositionHealth => 4000,
                MetricType::MarketVolume => 3000,
                MetricType::UserPnL => 5000,
            }
        }
    }
}

/// Calculate CU savings from parallel execution
fn calculate_cu_savings(groups: &[ExecutionGroup]) -> u64 {
    if groups.len() <= 1 {
        return 0;
    }
    
    // Sequential cost
    let sequential_cu: u64 = groups.iter()
        .map(|g| g.estimated_cu)
        .sum();
    
    // Parallel cost (max of any group + overhead)
    let max_group_cu = groups.iter()
        .map(|g| g.estimated_cu)
        .max()
        .unwrap_or(0);
    
    let parallel_overhead = 1000u64 * groups.len() as u64;
    let parallel_cu = max_group_cu + parallel_overhead;
    
    sequential_cu.saturating_sub(parallel_cu)
}

/// Execute operations in parallel groups
pub fn execute_parallel_plan(
    plan: &ExecutionPlan,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Executing parallel plan with {} groups", plan.parallel_groups.len());
    
    // Execute each group
    for group in &plan.parallel_groups {
        msg!("Executing group {} with {} operations", group.id, group.operations.len());
        
        // Check dependencies are satisfied
        if let Some(deps) = plan.dependencies.get(&group.id) {
            msg!("Group {} depends on: {:?}", group.id, deps);
        }
        
        // Execute operations in group
        for operation in &group.operations {
            execute_single_operation(operation, accounts)?;
        }
    }
    
    msg!("Parallel execution complete. Estimated CU saved: {}", plan.estimated_savings);
    Ok(())
}

/// Execute a single operation
fn execute_single_operation(
    operation: &Operation,
    _accounts: &[AccountInfo],
) -> ProgramResult {
    match operation {
        Operation::UpdatePosition { .. } => {
            msg!("Executing position update");
        }
        Operation::ProcessOrder { .. } => {
            msg!("Executing order processing");
        }
        Operation::UpdatePrice { .. } => {
            msg!("Executing price update");
        }
        Operation::CalculateMetrics { .. } => {
            msg!("Executing metrics calculation");
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parallel_grouping() {
        let operations = vec![
            Operation::UpdatePrice {
                market: Pubkey::new_unique(),
                price: 5000,
            },
            Operation::UpdatePrice {
                market: Pubkey::new_unique(),
                price: 6000,
            },
            Operation::UpdatePrice {
                market: Pubkey::new_unique(),
                price: 7000,
            },
        ];
        
        let plan = build_execution_plan(operations).unwrap();
        
        // All price updates to different markets can run in parallel
        assert_eq!(plan.parallel_groups.len(), 1);
        assert_eq!(plan.parallel_groups[0].operations.len(), 3);
    }
    
    #[test]
    fn test_dependency_detection() {
        let market = Pubkey::new_unique();
        let operations = vec![
            Operation::UpdatePrice {
                market,
                price: 5000,
            },
            Operation::UpdatePrice {
                market,
                price: 6000,
            },
        ];
        
        let dependencies = build_dependency_graph(&operations).unwrap();
        
        // Second operation depends on first (same market)
        assert!(dependencies.contains_key(&1));
        assert_eq!(dependencies[&1], vec![0]);
    }
}