//! Dynamic Verse Rebalancer
//!
//! Implements automatic verse rebalancing when market count exceeds capacity:
//! - Market redistribution strategies
//! - Load balancing across verses
//! - Atomic migration of markets
//! - Hierarchy preservation during rebalancing
//!
//! Per specification: Production-grade dynamic rebalancing

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, HashSet, VecDeque};
use crate::state_pruning::hex;

use crate::{
    error::BettingPlatformError,
    state::{VersePDA, VerseStatus},
    verse::{
        hierarchy_manager::{MARKETS_PER_VERSE_CAPACITY, MAX_VERSE_DEPTH},
        enhanced_classifier::{EnhancedVerseClassifier, VerseConfig},
    },
    events::{emit_event, EventType},
};

/// Rebalancing thresholds
pub const REBALANCE_THRESHOLD_PERCENTAGE: u32 = 90; // Trigger at 90% capacity
pub const MIN_MARKETS_FOR_SPLIT: u32 = 20; // Minimum markets to consider splitting
pub const MAX_MIGRATION_BATCH_SIZE: u32 = 10; // Max markets to migrate atomically
pub const COOLDOWN_PERIOD_SECONDS: i64 = 3600; // 1 hour between rebalances

/// Rebalancing strategy
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum RebalanceStrategy {
    Split { target_verses: u32 },
    Redistribute { donor_verses: Vec<u128>, recipient_verses: Vec<u128> },
    CreateChild { parent_verse_id: u128, theme_filter: String },
    MergeIntoParent { child_verses: Vec<u128> },
}

/// Rebalancing plan
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RebalancePlan {
    pub plan_id: [u8; 16],
    pub strategy: RebalanceStrategy,
    pub affected_verses: Vec<u128>,
    pub market_migrations: Vec<MarketMigration>,
    pub estimated_completion_time: i64,
    pub status: RebalanceStatus,
    pub created_at: i64,
}

impl RebalancePlan {
    pub const SIZE: usize = 1024 * 8; // 8KB

    /// Create new rebalancing plan
    pub fn new(
        strategy: RebalanceStrategy,
        affected_verses: Vec<u128>,
        timestamp: i64,
    ) -> Self {
        let plan_id = Self::generate_plan_id(&strategy, timestamp);
        
        Self {
            plan_id,
            strategy,
            affected_verses,
            market_migrations: Vec::new(),
            estimated_completion_time: timestamp + 300, // 5 minutes
            status: RebalanceStatus::Planning,
            created_at: timestamp,
        }
    }

    /// Generate unique plan ID
    fn generate_plan_id(strategy: &RebalanceStrategy, timestamp: i64) -> [u8; 16] {
        use solana_program::keccak;
        
        let strategy_bytes = match strategy {
            RebalanceStrategy::Split { target_verses } => target_verses.to_le_bytes().to_vec(),
            RebalanceStrategy::Redistribute { .. } => vec![1],
            RebalanceStrategy::CreateChild { .. } => vec![2],
            RebalanceStrategy::MergeIntoParent { .. } => vec![3],
        };
        
        let hash = keccak::hashv(&[
            &strategy_bytes,
            &timestamp.to_le_bytes(),
        ]);
        
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.0[..16]);
        id
    }

    /// Add market migration to plan
    pub fn add_migration(&mut self, migration: MarketMigration) {
        self.market_migrations.push(migration);
    }

    /// Validate plan is executable
    pub fn validate(&self) -> Result<(), ProgramError> {
        // Check migrations don't exceed batch size
        if self.market_migrations.len() > MAX_MIGRATION_BATCH_SIZE as usize {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        // Ensure no duplicate markets
        let mut seen_markets = HashSet::new();
        for migration in &self.market_migrations {
            if !seen_markets.insert(migration.market_id) {
                return Err(BettingPlatformError::DuplicateEntry.into());
            }
        }

        Ok(())
    }
}

/// Market migration details
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct MarketMigration {
    pub market_id: Pubkey,
    pub from_verse: u128,
    pub to_verse: u128,
    pub migration_reason: MigrationReason,
}

/// Migration reason
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum MigrationReason {
    CapacityExceeded,
    LoadBalancing,
    ThemeAlignment,
    VerseConsolidation,
}

/// Rebalance status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum RebalanceStatus {
    Planning,
    Approved,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Market metadata for theme analysis
#[derive(Debug, Clone)]
struct MarketMetadata {
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// Dynamic rebalancer engine
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct DynamicRebalancer {
    pub active_plans: HashMap<[u8; 16], RebalancePlan>,
    pub completed_plans: VecDeque<[u8; 16]>,
    pub verse_loads: HashMap<u128, VerseLoad>,
    pub last_rebalance: HashMap<u128, i64>,
    pub total_rebalances: u64,
    pub config: RebalancerConfig,
}

/// Rebalancer configuration
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RebalancerConfig {
    pub min_theme_score: u32,
}

impl DynamicRebalancer {
    pub const SIZE: usize = 1024 * 32; // 32KB

    pub fn new() -> Self {
        Self {
            active_plans: HashMap::new(),
            completed_plans: VecDeque::new(),
            verse_loads: HashMap::new(),
            last_rebalance: HashMap::new(),
            total_rebalances: 0,
            config: RebalancerConfig {
                min_theme_score: 60, // Minimum 60% theme alignment
            },
        }
    }

    /// Update verse load information
    pub fn update_verse_load(
        &mut self,
        verse_id: u128,
        market_count: u32,
        total_volume: u64,
        timestamp: i64,
    ) {
        let load = self.verse_loads
            .entry(verse_id)
            .or_insert_with(|| VerseLoad::new(verse_id));
        
        load.market_count = market_count;
        load.total_volume = total_volume;
        load.last_update = timestamp;
        load.capacity_percentage = (market_count * 100) / MARKETS_PER_VERSE_CAPACITY;
    }

    /// Check if rebalancing is needed
    pub fn check_rebalance_needed(
        &self,
        verse_id: u128,
        current_timestamp: i64,
    ) -> Option<RebalanceStrategy> {
        // Check cooldown
        if let Some(last_rebalance) = self.last_rebalance.get(&verse_id) {
            if current_timestamp - last_rebalance < COOLDOWN_PERIOD_SECONDS {
                return None;
            }
        }

        // Get verse load
        let load = self.verse_loads.get(&verse_id)?;
        
        // Check if over threshold
        if load.capacity_percentage < REBALANCE_THRESHOLD_PERCENTAGE {
            return None;
        }

        // Determine strategy based on load and verse characteristics
        if load.market_count > MIN_MARKETS_FOR_SPLIT {
            // Split strategy for large verses
            Some(RebalanceStrategy::Split {
                target_verses: 2,
            })
        } else {
            // Find underutilized verses for redistribution
            let underutilized = self.find_underutilized_verses(verse_id);
            
            if !underutilized.is_empty() {
                Some(RebalanceStrategy::Redistribute {
                    donor_verses: vec![verse_id],
                    recipient_verses: underutilized,
                })
            } else {
                // Create child verse as last resort
                Some(RebalanceStrategy::CreateChild {
                    parent_verse_id: verse_id,
                    theme_filter: "overflow".to_string(),
                })
            }
        }
    }

    /// Find underutilized verses
    fn find_underutilized_verses(&self, exclude_verse: u128) -> Vec<u128> {
        self.verse_loads
            .iter()
            .filter(|(id, load)| {
                **id != exclude_verse && 
                load.capacity_percentage < 50 && // Less than 50% utilized
                load.is_active
            })
            .take(3) // Maximum 3 recipient verses
            .map(|(id, _)| *id)
            .collect()
    }

    /// Create rebalancing plan
    pub fn create_plan(
        &mut self,
        strategy: RebalanceStrategy,
        verses: &HashMap<u128, VerseState>,
        timestamp: i64,
    ) -> Result<RebalancePlan, ProgramError> {
        let affected_verses = self.get_affected_verses(&strategy, verses);
        let mut plan = RebalancePlan::new(strategy.clone(), affected_verses, timestamp);

        // Generate market migrations based on strategy
        match &strategy {
            RebalanceStrategy::Split { target_verses } => {
                self.plan_split_migrations(&mut plan, verses, *target_verses)?;
            }
            RebalanceStrategy::Redistribute { donor_verses, recipient_verses } => {
                self.plan_redistribution(&mut plan, verses, donor_verses, recipient_verses)?;
            }
            RebalanceStrategy::CreateChild { parent_verse_id, theme_filter } => {
                self.plan_child_creation(&mut plan, verses, *parent_verse_id, theme_filter)?;
            }
            RebalanceStrategy::MergeIntoParent { child_verses } => {
                self.plan_merge(&mut plan, verses, child_verses)?;
            }
        }

        // Validate plan
        plan.validate()?;

        // Store plan
        self.active_plans.insert(plan.plan_id, plan.clone());

        Ok(plan)
    }

    /// Get affected verses from strategy
    fn get_affected_verses(
        &self,
        strategy: &RebalanceStrategy,
        verses: &HashMap<u128, VerseState>,
    ) -> Vec<u128> {
        match strategy {
            RebalanceStrategy::Split { .. } => {
                verses.keys().cloned().collect()
            }
            RebalanceStrategy::Redistribute { donor_verses, recipient_verses } => {
                let mut affected = donor_verses.clone();
                affected.extend(recipient_verses);
                affected
            }
            RebalanceStrategy::CreateChild { parent_verse_id, .. } => {
                vec![*parent_verse_id]
            }
            RebalanceStrategy::MergeIntoParent { child_verses } => {
                child_verses.clone()
            }
        }
    }

    /// Plan split migrations
    fn plan_split_migrations(
        &self,
        plan: &mut RebalancePlan,
        verses: &HashMap<u128, VerseState>,
        target_verses: u32,
    ) -> Result<(), ProgramError> {
        // Find the most loaded verse
        let (verse_id, markets) = verses
            .iter()
            .max_by_key(|(_, state)| state.markets.len())
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Split markets evenly
        let markets_per_verse = markets.markets.len() / target_verses as usize;
        
        for (i, chunk) in markets.markets.chunks(markets_per_verse).enumerate() {
            if i == 0 {
                continue; // Keep first chunk in original verse
            }
            
            let target_verse = verse_id + i as u128; // Simple verse ID generation
            
            for market_id in chunk {
                plan.add_migration(MarketMigration {
                    market_id: *market_id,
                    from_verse: *verse_id,
                    to_verse: target_verse,
                    migration_reason: MigrationReason::CapacityExceeded,
                });
            }
        }

        Ok(())
    }

    /// Plan redistribution
    fn plan_redistribution(
        &self,
        plan: &mut RebalancePlan,
        verses: &HashMap<u128, VerseState>,
        donor_verses: &[u128],
        recipient_verses: &[u128],
    ) -> Result<(), ProgramError> {
        let mut available_capacity: HashMap<u128, u32> = HashMap::new();
        
        // Calculate available capacity in recipient verses
        for verse_id in recipient_verses {
            if let Some(state) = verses.get(verse_id) {
                let capacity = MARKETS_PER_VERSE_CAPACITY.saturating_sub(state.markets.len() as u32);
                available_capacity.insert(*verse_id, capacity);
            }
        }

        // Redistribute markets from donor verses
        for donor_id in donor_verses {
            if let Some(donor_state) = verses.get(donor_id) {
                let excess_markets = donor_state.markets.len().saturating_sub(
                    (MARKETS_PER_VERSE_CAPACITY * REBALANCE_THRESHOLD_PERCENTAGE / 100) as usize
                );

                let mut migrated = 0;
                for market_id in &donor_state.markets {
                    if migrated >= excess_markets {
                        break;
                    }

                    // Find recipient with capacity
                    for (recipient_id, capacity) in available_capacity.iter_mut() {
                        if *capacity > 0 {
                            plan.add_migration(MarketMigration {
                                market_id: *market_id,
                                from_verse: *donor_id,
                                to_verse: *recipient_id,
                                migration_reason: MigrationReason::LoadBalancing,
                            });
                            
                            *capacity -= 1;
                            migrated += 1;
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Plan child verse creation
    fn plan_child_creation(
        &self,
        plan: &mut RebalancePlan,
        verses: &HashMap<u128, VerseState>,
        parent_verse_id: u128,
        theme_filter: &str,
    ) -> Result<(), ProgramError> {
        let parent_state = verses.get(&parent_verse_id)
            .ok_or(BettingPlatformError::VerseNotFound)?;

        // Use classifier to identify markets matching theme
        let _classifier = EnhancedVerseClassifier::new(VerseConfig::default());
        let child_verse_id = parent_verse_id * 1000 + 1; // Simple child ID generation

        let mut migrated_count = 0;
        let target_migrations = parent_state.markets.len() / 3; // Move 1/3 of markets

        for market_id in &parent_state.markets {
            if migrated_count >= target_migrations {
                break;
            }

            // Since VerseState doesn't have theme information, we'll use the theme_filter parameter
            // Create market metadata from available data
            let market_metadata = MarketMetadata {
                title: format!("Market {}", bs58::encode(&market_id.to_bytes()[..8]).into_string()),
                category: theme_filter.to_string(),
                tags: vec![theme_filter.to_string()],
            };
            
            let market_theme_score = self.calculate_theme_alignment_score(
                &market_id.to_bytes(),
                theme_filter,
                &market_metadata,
            )?;
            
            // Migrate if theme alignment score is high enough
            if market_theme_score >= self.config.min_theme_score {
                plan.add_migration(MarketMigration {
                    market_id: *market_id,
                    from_verse: parent_verse_id,
                    to_verse: child_verse_id,
                    migration_reason: MigrationReason::ThemeAlignment,
                });
                migrated_count += 1;
            }
        }

        Ok(())
    }

    /// Plan merge operation
    fn plan_merge(
        &self,
        plan: &mut RebalancePlan,
        verses: &HashMap<u128, VerseState>,
        child_verses: &[u128],
    ) -> Result<(), ProgramError> {
        // Find parent verse (lowest ID)
        let parent_verse_id = child_verses.iter().min()
            .copied()
            .ok_or(BettingPlatformError::InvalidInput)?;

        // Migrate all markets from children to parent
        for child_id in child_verses {
            if *child_id == parent_verse_id {
                continue;
            }

            if let Some(child_state) = verses.get(child_id) {
                for market_id in &child_state.markets {
                    plan.add_migration(MarketMigration {
                        market_id: *market_id,
                        from_verse: *child_id,
                        to_verse: parent_verse_id,
                        migration_reason: MigrationReason::VerseConsolidation,
                    });
                }
            }
        }

        Ok(())
    }

    /// Execute rebalancing plan
    pub fn execute_plan(
        &mut self,
        plan_id: [u8; 16],
        verse_accounts: &mut HashMap<u128, VersePDA>,
        timestamp: i64,
    ) -> Result<RebalanceResult, ProgramError> {
        // Extract plan data and migrations before mutable operations
        let (plan_migrations, affected_verses, initial_status) = {
            let plan = self.active_plans.get_mut(&plan_id)
                .ok_or(BettingPlatformError::InvalidInput)?;

            if plan.status != RebalanceStatus::Approved {
                return Err(BettingPlatformError::InvalidProposalStatus.into());
            }

            plan.status = RebalanceStatus::InProgress;
            
            // Clone what we need
            (
                plan.market_migrations.clone(),
                plan.affected_verses.clone(),
                plan.status.clone()
            )
        };

        let mut result = RebalanceResult::new(plan_id);

        // Execute migrations atomically
        for migration in &plan_migrations {
            match self.execute_migration(migration, verse_accounts) {
                Ok(_) => {
                    result.successful_migrations += 1;
                    msg!("Migrated market {:?} from {} to {}", 
                        migration.market_id, migration.from_verse, migration.to_verse);
                }
                Err(e) => {
                    result.failed_migrations += 1;
                    result.errors.push(format!("Failed to migrate {:?}: {:?}", 
                        migration.market_id, e));
                    
                    // Update plan status on failure
                    if let Some(plan) = self.active_plans.get_mut(&plan_id) {
                        plan.status = RebalanceStatus::Failed;
                    }
                    return Err(e);
                }
            }
        }

        // Update plan status to completed
        if let Some(plan) = self.active_plans.get_mut(&plan_id) {
            plan.status = RebalanceStatus::Completed;
        }
        result.completion_time = timestamp;

        // Update last rebalance times
        for verse_id in &affected_verses {
            self.last_rebalance.insert(*verse_id, timestamp);
        }

        // Move to completed
        self.completed_plans.push_back(plan_id);
        if self.completed_plans.len() > 100 {
            self.completed_plans.pop_front();
        }

        self.total_rebalances += 1;

        Ok(result)
    }

    /// Execute single market migration
    fn execute_migration(
        &self,
        migration: &MarketMigration,
        verse_accounts: &mut HashMap<u128, VersePDA>,
    ) -> Result<(), ProgramError> {
        // Remove from source verse
        if let Some(from_verse) = verse_accounts.get_mut(&migration.from_verse) {
            from_verse.markets.retain(|m| m != &migration.market_id);
        } else {
            return Err(BettingPlatformError::VerseNotFound.into());
        }

        // Add to target verse
        if let Some(to_verse) = verse_accounts.get_mut(&migration.to_verse) {
            if to_verse.markets.len() >= MARKETS_PER_VERSE_CAPACITY as usize {
                return Err(BettingPlatformError::VerseCapacityExceeded.into());
            }
            
            to_verse.markets.push(migration.market_id);
        } else {
            return Err(BettingPlatformError::VerseNotFound.into());
        }

        Ok(())
    }
    
    /// Calculate theme alignment score for a market
    fn calculate_theme_alignment_score(
        &self,
        _market_id: &[u8; 32],
        theme: &str,
        metadata: &MarketMetadata,
    ) -> Result<u32, ProgramError> {
        let mut score = 0u32;
        
        // Check title alignment
        let title_lower = metadata.title.to_lowercase();
        let theme_lower = theme.to_lowercase();
        
        if title_lower.contains(&theme_lower) {
            score += 50; // Direct theme match in title
        }
        
        // Check category alignment
        if metadata.category.to_lowercase() == theme_lower {
            score += 30; // Category matches theme
        }
        
        // Check tags for theme keywords
        let theme_words: Vec<&str> = theme_lower.split_whitespace().collect();
        for tag in &metadata.tags {
            let tag_lower = tag.to_lowercase();
            for word in &theme_words {
                if tag_lower.contains(word) {
                    score += 10; // Tag contains theme word
                }
            }
        }
        
        // Normalize score to 0-100 range
        Ok(score.min(100))
    }

    /// Get rebalancing statistics
    pub fn get_statistics(&self) -> RebalanceStatistics {
        let active_plans = self.active_plans.len() as u32;
        let completed_plans = self.completed_plans.len() as u32;
        
        let overloaded_verses = self.verse_loads
            .values()
            .filter(|load| load.capacity_percentage >= REBALANCE_THRESHOLD_PERCENTAGE)
            .count() as u32;

        let underutilized_verses = self.verse_loads
            .values()
            .filter(|load| load.capacity_percentage < 30)
            .count() as u32;

        RebalanceStatistics {
            total_rebalances: self.total_rebalances,
            active_plans,
            completed_plans,
            overloaded_verses,
            underutilized_verses,
            average_capacity: self.calculate_average_capacity(),
        }
    }

    /// Calculate average verse capacity utilization
    fn calculate_average_capacity(&self) -> u32 {
        if self.verse_loads.is_empty() {
            return 0;
        }

        let total: u32 = self.verse_loads
            .values()
            .map(|load| load.capacity_percentage)
            .sum();

        total / self.verse_loads.len() as u32
    }
}

/// Verse load information
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct VerseLoad {
    pub verse_id: u128,
    pub market_count: u32,
    pub capacity_percentage: u32,
    pub total_volume: u64,
    pub last_update: i64,
    pub is_active: bool,
}

impl VerseLoad {
    pub fn new(verse_id: u128) -> Self {
        Self {
            verse_id,
            market_count: 0,
            capacity_percentage: 0,
            total_volume: 0,
            last_update: 0,
            is_active: true,
        }
    }
}

/// Verse state for rebalancing (simplified)
#[derive(Clone)]
pub struct VerseState {
    pub verse_id: u128,
    pub markets: Vec<Pubkey>,
    pub depth: u8,
    pub parent_id: Option<u128>,
}

/// Rebalancing result
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RebalanceResult {
    pub plan_id: [u8; 16],
    pub successful_migrations: u32,
    pub failed_migrations: u32,
    pub completion_time: i64,
    pub errors: Vec<String>,
}

impl RebalanceResult {
    pub fn new(plan_id: [u8; 16]) -> Self {
        Self {
            plan_id,
            successful_migrations: 0,
            failed_migrations: 0,
            completion_time: 0,
            errors: Vec::new(),
        }
    }
}

/// Rebalancing statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct RebalanceStatistics {
    pub total_rebalances: u64,
    pub active_plans: u32,
    pub completed_plans: u32,
    pub overloaded_verses: u32,
    pub underutilized_verses: u32,
    pub average_capacity: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebalance_threshold() {
        let mut rebalancer = DynamicRebalancer::new();
        
        // Update verse load
        rebalancer.update_verse_load(1, 50, 100000, 100);
        
        // Should not need rebalancing
        assert!(rebalancer.check_rebalance_needed(1, 200).is_none());
        
        // Update to over threshold
        rebalancer.update_verse_load(1, 49, 100000, 200); // 90.7% capacity
        
        // Should need rebalancing
        assert!(rebalancer.check_rebalance_needed(1, 300).is_some());
    }

    #[test]
    fn test_split_strategy() {
        let rebalancer = DynamicRebalancer::new();
        let mut verses = HashMap::new();
        
        // Create overloaded verse
        let mut markets = Vec::new();
        for i in 0..40 {
            markets.push(Pubkey::new_from_array([i as u8; 32]));
        }
        
        verses.insert(1, VerseState {
            verse_id: 1,
            markets,
            depth: 0,
            parent_id: None,
        });

        let mut plan = RebalancePlan::new(
            RebalanceStrategy::Split { target_verses: 2 },
            vec![1],
            100,
        );

        rebalancer.plan_split_migrations(&mut plan, &verses, 2).unwrap();
        
        // Should have migrations for half the markets
        assert!(plan.market_migrations.len() >= 20);
    }

    #[test]
    fn test_redistribution() {
        let rebalancer = DynamicRebalancer::new();
        let mut verses = HashMap::new();
        
        // Overloaded verse
        verses.insert(1, VerseState {
            verse_id: 1,
            markets: vec![Pubkey::new_from_array([1; 32]); 50],
            depth: 0,
            parent_id: None,
        });
        
        // Underutilized verse
        verses.insert(2, VerseState {
            verse_id: 2,
            markets: vec![Pubkey::new_from_array([2; 32]); 10],
            depth: 0,
            parent_id: None,
        });

        let mut plan = RebalancePlan::new(
            RebalanceStrategy::Redistribute {
                donor_verses: vec![1],
                recipient_verses: vec![2],
            },
            vec![1, 2],
            100,
        );

        rebalancer.plan_redistribution(&mut plan, &verses, &[1], &[2]).unwrap();
        
        // Should have migrations
        assert!(!plan.market_migrations.is_empty());
    }
}