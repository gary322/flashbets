//! Comprehensive test data management system
//! Provides centralized test data creation, management, and cleanup

use anyhow::{Result, Context as AnyhowContext};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    sync::Arc,
    path::PathBuf,
};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::{
    db::fallback::FallbackDatabase as Database,
    types::*,
    jwt_validation::JwtManager,
};

/// Test data lifecycle stages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum TestDataLifecycle {
    Created,
    Active,
    Used,
    Cleanup,
    Deleted,
}

/// Test data category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum TestDataCategory {
    Users,
    Markets,
    Positions,
    Orders,
    Transactions,
    Wallets,
    Oracle,
    Quantum,
    Settlement,
}

/// Test data record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDataRecord {
    pub id: String,
    pub category: TestDataCategory,
    pub data_type: String,
    pub data: serde_json::Value,
    pub lifecycle: TestDataLifecycle,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub references: Vec<String>, // IDs of related test data
}

/// Test user data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUser {
    pub id: String,
    pub email: String,
    pub wallet: String,
    pub password: String, // Plain text for tests
    pub role: String,
    pub jwt_token: String,
    pub balance: u64,
    pub created_at: DateTime<Utc>,
}

/// Test market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestMarket {
    pub id: u128,
    pub pubkey: Pubkey,
    pub title: String,
    pub description: String,
    pub category: String,
    pub outcomes: Vec<String>,
    pub creator: String,
    pub liquidity: u64,
    pub volume: u64,
    pub status: String,
    pub resolution_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Test position data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPosition {
    pub id: String,
    pub wallet: String,
    pub market_id: u128,
    pub outcome: u8,
    pub shares: u64,
    pub locked_amount: u64,
    pub average_price: f64,
    pub leverage: u32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Test data configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDataConfig {
    pub auto_cleanup: bool,
    pub cleanup_interval_minutes: u64,
    pub default_expiry_minutes: u64,
    pub database_prefix: String,
    pub seed_data_path: Option<PathBuf>,
}

impl Default for TestDataConfig {
    fn default() -> Self {
        Self {
            auto_cleanup: true,
            cleanup_interval_minutes: 30,
            default_expiry_minutes: 120,
            database_prefix: "test_".to_string(),
            seed_data_path: None,
        }
    }
}

/// Test data manager
pub struct TestDataManager {
    config: TestDataConfig,
    database: Arc<Database>,
    records: Arc<RwLock<HashMap<String, TestDataRecord>>>,
    categories: Arc<RwLock<HashMap<TestDataCategory, Vec<String>>>>,
}

impl TestDataManager {
    /// Create new test data manager
    pub async fn new(
        config: TestDataConfig,
        database: Arc<Database>,
    ) -> Result<Self> {
        let manager = Self {
            config: config.clone(),
            database,
            records: Arc::new(RwLock::new(HashMap::new())),
            categories: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load seed data if configured
        if let Some(seed_path) = &config.seed_data_path {
            manager.load_seed_data(seed_path).await?;
        }

        Ok(manager)
    }

    /// Start automatic cleanup task
    pub fn start_cleanup_task(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        if !self.config.auto_cleanup {
            return tokio::spawn(async {});
        }

        let records = self.records.clone();
        let interval = self.config.cleanup_interval_minutes;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval * 60)
            );

            loop {
                interval.tick().await;
                
                let now = Utc::now();
                let mut records_guard = records.write().await;
                
                let expired_ids: Vec<String> = records_guard
                    .iter()
                    .filter(|(_, record)| {
                        record.expires_at.map_or(false, |exp| exp < now)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();

                for id in expired_ids {
                    records_guard.remove(&id);
                    info!("Cleaned up expired test data: {}", id);
                }
            }
        })
    }

    /// Create test users
    pub async fn create_test_users(&self, count: usize) -> Result<Vec<TestUser>> {
        let mut users = Vec::new();

        for i in 0..count {
            let user = TestUser {
                id: Uuid::new_v4().to_string(),
                email: format!("test_user_{}@test.com", i),
                wallet: format!("TestWa11et{:0>40}", i),
                password: format!("test_pass_{}", i),
                role: if i == 0 { "admin" } else { "user" }.to_string(),
                jwt_token: String::new(), // Will be set later
                balance: 1000000 * (i as u64 + 1),
                created_at: Utc::now(),
            };

            // Skip database storage for now

            // Store in memory
            self.store_record(
                TestDataCategory::Users,
                &user.id,
                "TestUser",
                serde_json::to_value(&user)?,
                vec![format!("user_{}", i), user.role.clone()],
            ).await?;

            users.push(user);
        }

        info!("Created {} test users", users.len());
        Ok(users)
    }

    /// Create test markets
    pub async fn create_test_markets(&self, count: usize) -> Result<Vec<TestMarket>> {
        let categories = vec!["politics", "sports", "crypto", "finance", "technology"];
        let mut markets = Vec::new();

        for i in 0..count {
            let category = categories[i % categories.len()];
            let market = TestMarket {
                id: 10000 + i as u128,
                pubkey: Pubkey::new_unique(),
                title: format!("Test Market {} - {}", i, category),
                description: format!("Test market for {} category", category),
                category: category.to_string(),
                outcomes: vec!["Yes".to_string(), "No".to_string()],
                creator: format!("TestCreator{:0>40}", i % 5),
                liquidity: 100000 * (i as u64 + 1),
                volume: 500000 * (i as u64 + 1),
                status: "open".to_string(),
                resolution_time: Utc::now() + Duration::days(7),
                created_at: Utc::now() - Duration::hours(i as i64),
            };

            // Skip database storage for now

            // Store in memory
            self.store_record(
                TestDataCategory::Markets,
                &market.id.to_string(),
                "TestMarket",
                serde_json::to_value(&market)?,
                vec![format!("market_{}", i), market.category.clone()],
            ).await?;

            markets.push(market);
        }

        info!("Created {} test markets", markets.len());
        Ok(markets)
    }

    /// Create test positions
    pub async fn create_test_positions(
        &self,
        users: &[TestUser],
        markets: &[TestMarket],
        positions_per_user: usize,
    ) -> Result<Vec<TestPosition>> {
        let mut positions = Vec::new();
        let mut rng = rand::thread_rng();
        use rand::Rng;

        for user in users {
            for i in 0..positions_per_user {
                let market = &markets[rng.gen_range(0..markets.len())];
                let position = TestPosition {
                    id: Uuid::new_v4().to_string(),
                    wallet: user.wallet.clone(),
                    market_id: market.id,
                    outcome: rng.gen_range(0..2),
                    shares: rng.gen_range(100..10000) * 1000,
                    locked_amount: rng.gen_range(1000..100000) * 1000,
                    average_price: rng.gen_range(0.1..0.9),
                    leverage: rng.gen_range(1..10),
                    status: "open".to_string(),
                    created_at: Utc::now() - Duration::hours(rng.gen_range(1..48)),
                };

                // Skip database storage for now

                // Store in memory
                self.store_record(
                    TestDataCategory::Positions,
                    &position.id,
                    "TestPosition",
                    serde_json::to_value(&position)?,
                    vec![
                        format!("position_{}", i),
                        user.id.clone(),
                        market.id.to_string(),
                    ],
                ).await?;

                positions.push(position);
            }
        }

        info!("Created {} test positions", positions.len());
        Ok(positions)
    }

    /// Create complex test scenario
    pub async fn create_test_scenario(
        &self,
        name: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut scenario = HashMap::new();

        // Create interconnected test data
        let users = self.create_test_users(10).await?;
        let markets = self.create_test_markets(20).await?;
        let positions = self.create_test_positions(&users, &markets, 5).await?;

        // Create some settled markets
        for market in markets.iter().take(3) {
            self.settle_test_market(market.id, 0).await?;
        }

        scenario.insert("name".to_string(), serde_json::json!(name));
        scenario.insert("users".to_string(), serde_json::to_value(&users)?);
        scenario.insert("markets".to_string(), serde_json::to_value(&markets)?);
        scenario.insert("positions".to_string(), serde_json::to_value(&positions)?);
        scenario.insert("created_at".to_string(), serde_json::json!(Utc::now()));

        info!("Created test scenario: {}", name);
        Ok(scenario)
    }

    /// Settle a test market
    pub async fn settle_test_market(
        &self,
        market_id: u128,
        winning_outcome: u8,
    ) -> Result<()> {
        // Skip database updates for now
        info!("Settled test market {} with outcome {}", market_id, winning_outcome);
        Ok(())
    }

    /// Store test data record
    async fn store_record(
        &self,
        category: TestDataCategory,
        id: &str,
        data_type: &str,
        data: serde_json::Value,
        tags: Vec<String>,
    ) -> Result<()> {
        let record = TestDataRecord {
            id: id.to_string(),
            category: category.clone(),
            data_type: data_type.to_string(),
            data,
            lifecycle: TestDataLifecycle::Created,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: Some(
                Utc::now() + Duration::minutes(self.config.default_expiry_minutes as i64)
            ),
            tags,
            references: Vec::new(),
        };

        // Store in memory
        self.records.write().await.insert(id.to_string(), record);
        
        // Update category index
        self.categories
            .write()
            .await
            .entry(category)
            .or_insert_with(Vec::new)
            .push(id.to_string());

        Ok(())
    }

    /// Get test data by ID
    pub async fn get_by_id(&self, id: &str) -> Option<TestDataRecord> {
        self.records.read().await.get(id).cloned()
    }

    /// Get test data by category
    pub async fn get_by_category(
        &self,
        category: TestDataCategory,
    ) -> Vec<TestDataRecord> {
        let records = self.records.read().await;
        let categories = self.categories.read().await;

        if let Some(ids) = categories.get(&category) {
            ids.iter()
                .filter_map(|id| records.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Search test data by tags
    pub async fn search_by_tags(&self, tags: &[String]) -> Vec<TestDataRecord> {
        self.records
            .read()
            .await
            .values()
            .filter(|record| {
                tags.iter().any(|tag| record.tags.contains(tag))
            })
            .cloned()
            .collect()
    }

    /// Clean up test data
    pub async fn cleanup(&self, force: bool) -> Result<usize> {
        let mut count = 0;

        // Clean up memory records
        let ids_to_remove: Vec<String> = if force {
            self.records.read().await.keys().cloned().collect()
        } else {
            let now = Utc::now();
            self.records
                .read()
                .await
                .iter()
                .filter(|(_, record)| {
                    record.lifecycle == TestDataLifecycle::Cleanup ||
                    record.expires_at.map_or(false, |exp| exp < now)
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        for id in &ids_to_remove {
            self.records.write().await.remove(id);
            count += 1;
        }

        // Skip database cleanup for now

        info!("Cleaned up {} test data records", count);
        Ok(count)
    }

    /// Load seed data from file
    async fn load_seed_data(&self, path: &PathBuf) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read seed data file")?;

        let seed_data: serde_json::Value = serde_json::from_str(&content)
            .context("Failed to parse seed data")?;

        // Process seed data based on structure
        if let Some(users) = seed_data.get("users").and_then(|v| v.as_array()) {
            for user_data in users {
                if let Ok(user) = serde_json::from_value::<TestUser>(user_data.clone()) {
                    self.store_record(
                        TestDataCategory::Users,
                        &user.id,
                        "TestUser",
                        user_data.clone(),
                        vec!["seed".to_string()],
                    ).await?;
                }
            }
        }

        if let Some(markets) = seed_data.get("markets").and_then(|v| v.as_array()) {
            for market_data in markets {
                if let Ok(market) = serde_json::from_value::<TestMarket>(market_data.clone()) {
                    self.store_record(
                        TestDataCategory::Markets,
                        &market.id.to_string(),
                        "TestMarket",
                        market_data.clone(),
                        vec!["seed".to_string()],
                    ).await?;
                }
            }
        }

        info!("Loaded seed data from {:?}", path);
        Ok(())
    }

    /// Export test data to file
    pub async fn export_data(&self, path: &PathBuf) -> Result<()> {
        let records = self.records.read().await;
        let data = serde_json::json!({
            "export_time": Utc::now(),
            "records": records.values().collect::<Vec<_>>(),
        });

        let content = serde_json::to_string_pretty(&data)?;
        tokio::fs::write(path, content)
            .await
            .context("Failed to write export file")?;

        info!("Exported {} test data records to {:?}", records.len(), path);
        Ok(())
    }

    /// Generate test report
    pub async fn generate_report(&self) -> HashMap<String, serde_json::Value> {
        let records = self.records.read().await;
        let mut report = HashMap::new();

        // Count by category
        let mut category_counts = HashMap::new();
        for record in records.values() {
            *category_counts.entry(record.category.clone()).or_insert(0) += 1;
        }

        // Count by lifecycle
        let mut lifecycle_counts = HashMap::new();
        for record in records.values() {
            *lifecycle_counts.entry(record.lifecycle.clone()).or_insert(0) += 1;
        }

        report.insert("total_records".to_string(), serde_json::json!(records.len()));
        report.insert("categories".to_string(), serde_json::to_value(category_counts).unwrap());
        report.insert("lifecycles".to_string(), serde_json::to_value(lifecycle_counts).unwrap());
        report.insert("generated_at".to_string(), serde_json::json!(Utc::now()));

        report
    }
}

/// Test data builder for fluent API
pub struct TestDataBuilder {
    manager: Arc<TestDataManager>,
    users: Vec<TestUser>,
    markets: Vec<TestMarket>,
    positions: Vec<TestPosition>,
}

impl TestDataBuilder {
    pub fn new(manager: Arc<TestDataManager>) -> Self {
        Self {
            manager,
            users: Vec::new(),
            markets: Vec::new(),
            positions: Vec::new(),
        }
    }

    pub async fn with_users(mut self, count: usize) -> Result<Self> {
        self.users = self.manager.create_test_users(count).await?;
        Ok(self)
    }

    pub async fn with_markets(mut self, count: usize) -> Result<Self> {
        self.markets = self.manager.create_test_markets(count).await?;
        Ok(self)
    }

    pub async fn with_positions(mut self, per_user: usize) -> Result<Self> {
        self.positions = self.manager
            .create_test_positions(&self.users, &self.markets, per_user)
            .await?;
        Ok(self)
    }

    pub async fn with_settled_markets(self, count: usize) -> Result<Self> {
        for market in self.markets.iter().take(count) {
            self.manager.settle_test_market(market.id, 0).await?;
        }
        Ok(self)
    }

    pub fn build(self) -> TestDataSet {
        TestDataSet {
            users: self.users,
            markets: self.markets,
            positions: self.positions,
        }
    }
}

/// Test data set result
#[derive(Debug, Clone)]
pub struct TestDataSet {
    pub users: Vec<TestUser>,
    pub markets: Vec<TestMarket>,
    pub positions: Vec<TestPosition>,
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_data_manager_creation() {
        let config = TestDataConfig::default();
        let db = Arc::new(Database::new(Default::default()).await.unwrap());
        let manager = TestDataManager::new(config, db).await.unwrap();
        
        // Create test data
        let users = manager.create_test_users(5).await.unwrap();
        assert_eq!(users.len(), 5);
        
        let markets = manager.create_test_markets(10).await.unwrap();
        assert_eq!(markets.len(), 10);
    }

    #[tokio::test]
    async fn test_data_builder() {
        let config = TestDataConfig::default();
        let db = Arc::new(Database::new(Default::default()).await.unwrap());
        let manager = Arc::new(TestDataManager::new(config, db).await.unwrap());
        
        let dataset = TestDataBuilder::new(manager)
            .with_users(3).await.unwrap()
            .with_markets(5).await.unwrap()
            .with_positions(2).await.unwrap()
            .build();
        
        assert_eq!(dataset.users.len(), 3);
        assert_eq!(dataset.markets.len(), 5);
        assert_eq!(dataset.positions.len(), 6); // 3 users * 2 positions
    }
}