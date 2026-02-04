//! Feature flag system for runtime feature toggling
//! Supports multi-source configuration, percentage rollouts, and user targeting

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::{
    environment_config::EnvironmentConfigService,
    typed_errors::{AppError, ErrorContext, ErrorKind},
};

/// Feature flag status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlagStatus {
    Enabled,
    Disabled,
    Percentage(u8), // 0-100
}

/// Feature flag target type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    User,
    Group,
    IpRange,
    Market,
    Custom(String),
}

/// Feature flag target rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRule {
    pub target_type: TargetType,
    pub values: Vec<String>,
    pub enabled: bool,
}

/// Feature flag definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub name: String,
    pub description: String,
    pub status: FlagStatus,
    pub target_rules: Vec<TargetRule>,
    pub metadata: HashMap<String, JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Feature flag evaluation context
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    pub user_id: Option<String>,
    pub group_ids: Vec<String>,
    pub ip_address: Option<String>,
    pub market_id: Option<u128>,
    pub custom_attributes: HashMap<String, String>,
}

/// Feature flag provider trait
#[async_trait]
pub trait FeatureFlagProvider: Send + Sync {
    /// Get all feature flags
    async fn get_flags(&self) -> Result<Vec<FeatureFlag>>;
    
    /// Get a specific feature flag
    async fn get_flag(&self, name: &str) -> Result<Option<FeatureFlag>>;
    
    /// Update a feature flag
    async fn update_flag(&self, flag: &FeatureFlag) -> Result<()>;
    
    /// Delete a feature flag
    async fn delete_flag(&self, name: &str) -> Result<()>;
}

/// In-memory feature flag provider
pub struct InMemoryProvider {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        Self {
            flags: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn load_defaults(&self) {
        let mut flags = self.flags.write().await;
        
        // Default feature flags
        flags.insert(
            "new_trading_ui".to_string(),
            FeatureFlag {
                name: "new_trading_ui".to_string(),
                description: "New trading UI with enhanced features".to_string(),
                status: FlagStatus::Percentage(50),
                target_rules: vec![],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                expires_at: None,
            },
        );
        
        flags.insert(
            "quantum_trading".to_string(),
            FeatureFlag {
                name: "quantum_trading".to_string(),
                description: "Quantum position trading features".to_string(),
                status: FlagStatus::Disabled,
                target_rules: vec![
                    TargetRule {
                        target_type: TargetType::Group,
                        values: vec!["beta_testers".to_string()],
                        enabled: true,
                    },
                ],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                expires_at: None,
            },
        );
        
        flags.insert(
            "advanced_analytics".to_string(),
            FeatureFlag {
                name: "advanced_analytics".to_string(),
                description: "Advanced market analytics dashboard".to_string(),
                status: FlagStatus::Enabled,
                target_rules: vec![],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                expires_at: None,
            },
        );
        
        flags.insert(
            "maintenance_mode".to_string(),
            FeatureFlag {
                name: "maintenance_mode".to_string(),
                description: "System maintenance mode".to_string(),
                status: FlagStatus::Disabled,
                target_rules: vec![],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                expires_at: None,
            },
        );
    }
}

#[async_trait]
impl FeatureFlagProvider for InMemoryProvider {
    async fn get_flags(&self) -> Result<Vec<FeatureFlag>> {
        let flags = self.flags.read().await;
        Ok(flags.values().cloned().collect())
    }
    
    async fn get_flag(&self, name: &str) -> Result<Option<FeatureFlag>> {
        let flags = self.flags.read().await;
        Ok(flags.get(name).cloned())
    }
    
    async fn update_flag(&self, flag: &FeatureFlag) -> Result<()> {
        let mut flags = self.flags.write().await;
        flags.insert(flag.name.clone(), flag.clone());
        Ok(())
    }
    
    async fn delete_flag(&self, name: &str) -> Result<()> {
        let mut flags = self.flags.write().await;
        flags.remove(name);
        Ok(())
    }
}

/// Feature flag service
pub struct FeatureFlagService {
    providers: Vec<Box<dyn FeatureFlagProvider>>,
    cache: Arc<RwLock<HashMap<String, (FeatureFlag, DateTime<Utc>)>>>,
    cache_ttl: chrono::Duration,
    env_config: Option<Arc<EnvironmentConfigService>>,
}

impl FeatureFlagService {
    /// Create new feature flag service
    pub fn new(env_config: Option<Arc<EnvironmentConfigService>>) -> Self {
        Self {
            providers: vec![],
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: chrono::Duration::minutes(5),
            env_config,
        }
    }
    
    /// Add a feature flag provider
    pub fn add_provider(&mut self, provider: Box<dyn FeatureFlagProvider>) {
        self.providers.push(provider);
    }
    
    /// Initialize with default provider
    pub async fn init_default(&mut self) {
        let provider = InMemoryProvider::new();
        provider.load_defaults().await;
        self.add_provider(Box::new(provider));
    }
    
    /// Check if a feature is enabled for a given context
    pub async fn is_enabled(
        &self,
        flag_name: &str,
        context: &EvaluationContext,
    ) -> Result<bool, AppError> {
        let ctx = ErrorContext::new("feature_flags", "is_enabled");
        
        // Check cache first
        if let Some(flag) = self.get_from_cache(flag_name).await {
            return Ok(self.evaluate_flag(&flag, context));
        }
        
        // Get from providers
        for provider in &self.providers {
            if let Some(flag) = provider.get_flag(flag_name).await.map_err(|e| {
                AppError::new(
                    ErrorKind::InternalError,
                    format!("Failed to get feature flag: {}", e),
                    ctx.clone(),
                )
            })? {
                // Cache the flag
                self.cache_flag(&flag).await;
                return Ok(self.evaluate_flag(&flag, context));
            }
        }
        
        // Flag not found - default to disabled
        debug!("Feature flag '{}' not found, defaulting to disabled", flag_name);
        Ok(false)
    }
    
    /// Get all feature flags
    pub async fn get_all_flags(&self) -> Result<Vec<FeatureFlag>, AppError> {
        let ctx = ErrorContext::new("feature_flags", "get_all_flags");
        
        let mut all_flags = HashMap::new();
        
        // Collect from all providers
        for provider in &self.providers {
            let flags = provider.get_flags().await.map_err(|e| {
                AppError::new(
                    ErrorKind::InternalError,
                    format!("Failed to get feature flags: {}", e),
                    ctx.clone(),
                )
            })?;
            
            for flag in flags {
                all_flags.insert(flag.name.clone(), flag);
            }
        }
        
        Ok(all_flags.into_values().collect())
    }
    
    /// Update a feature flag
    pub async fn update_flag(&self, flag: &FeatureFlag) -> Result<(), AppError> {
        let ctx = ErrorContext::new("feature_flags", "update_flag");
        
        // Update in all providers
        for provider in &self.providers {
            provider.update_flag(flag).await.map_err(|e| {
                AppError::new(
                    ErrorKind::InternalError,
                    format!("Failed to update feature flag: {}", e),
                    ctx.clone(),
                )
            })?;
        }
        
        // Invalidate cache
        self.invalidate_cache(&flag.name).await;
        
        info!("Updated feature flag: {}", flag.name);
        Ok(())
    }
    
    /// Delete a feature flag
    pub async fn delete_flag(&self, name: &str) -> Result<(), AppError> {
        let ctx = ErrorContext::new("feature_flags", "delete_flag");
        
        // Delete from all providers
        for provider in &self.providers {
            provider.delete_flag(name).await.map_err(|e| {
                AppError::new(
                    ErrorKind::InternalError,
                    format!("Failed to delete feature flag: {}", e),
                    ctx.clone(),
                )
            })?;
        }
        
        // Invalidate cache
        self.invalidate_cache(name).await;
        
        info!("Deleted feature flag: {}", name);
        Ok(())
    }
    
    /// Evaluate a feature flag against context
    fn evaluate_flag(&self, flag: &FeatureFlag, context: &EvaluationContext) -> bool {
        // Check expiration
        if let Some(expires_at) = flag.expires_at {
            if Utc::now() > expires_at {
                debug!("Feature flag '{}' has expired", flag.name);
                return false;
            }
        }
        
        // Check target rules first
        for rule in &flag.target_rules {
            if self.matches_rule(rule, context) {
                return rule.enabled;
            }
        }
        
        // Apply status
        match flag.status {
            FlagStatus::Enabled => true,
            FlagStatus::Disabled => false,
            FlagStatus::Percentage(pct) => {
                // Use user ID or IP for consistent bucketing
                let bucket_key = context.user_id.as_ref()
                    .or(context.ip_address.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("anonymous");
                
                let hash = self.hash_string(&format!("{}-{}", flag.name, bucket_key));
                (hash % 100) < pct as u64
            }
        }
    }
    
    /// Check if context matches a target rule
    fn matches_rule(&self, rule: &TargetRule, context: &EvaluationContext) -> bool {
        match &rule.target_type {
            TargetType::User => {
                if let Some(user_id) = &context.user_id {
                    rule.values.contains(user_id)
                } else {
                    false
                }
            }
            TargetType::Group => {
                context.group_ids.iter().any(|g| rule.values.contains(g))
            }
            TargetType::IpRange => {
                if let Some(ip) = &context.ip_address {
                    rule.values.iter().any(|range| self.ip_in_range(ip, range))
                } else {
                    false
                }
            }
            TargetType::Market => {
                if let Some(market_id) = context.market_id {
                    rule.values.contains(&market_id.to_string())
                } else {
                    false
                }
            }
            TargetType::Custom(attr_name) => {
                if let Some(value) = context.custom_attributes.get(attr_name) {
                    rule.values.contains(value)
                } else {
                    false
                }
            }
        }
    }
    
    /// Check if IP is in CIDR range
    fn ip_in_range(&self, ip: &str, range: &str) -> bool {
        // Simplified implementation - in production use proper CIDR matching
        ip.starts_with(range.split('/').next().unwrap_or(""))
    }
    
    /// Simple hash function for consistent bucketing
    fn hash_string(&self, s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Get flag from cache
    async fn get_from_cache(&self, name: &str) -> Option<FeatureFlag> {
        let cache = self.cache.read().await;
        if let Some((flag, cached_at)) = cache.get(name) {
            if Utc::now() - *cached_at < self.cache_ttl {
                return Some(flag.clone());
            }
        }
        None
    }
    
    /// Cache a flag
    async fn cache_flag(&self, flag: &FeatureFlag) {
        let mut cache = self.cache.write().await;
        cache.insert(flag.name.clone(), (flag.clone(), Utc::now()));
    }
    
    /// Invalidate cache entry
    async fn invalidate_cache(&self, name: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(name);
    }
    
    /// Clear entire cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Feature flag guard for protecting endpoints
pub struct FeatureFlagGuard {
    flag_name: String,
    service: Arc<FeatureFlagService>,
}

impl FeatureFlagGuard {
    pub fn new(flag_name: String, service: Arc<FeatureFlagService>) -> Self {
        Self { flag_name, service }
    }
    
    pub async fn check(&self, context: &EvaluationContext) -> Result<(), AppError> {
        if !self.service.is_enabled(&self.flag_name, context).await? {
            Err(AppError::new(
                ErrorKind::FeatureDisabled,
                format!("Feature '{}' is not enabled", self.flag_name),
                ErrorContext::new("feature_flags", "guard"),
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_feature_flag_evaluation() {
        let mut service = FeatureFlagService::new(None);
        service.init_default().await;
        
        let context = EvaluationContext {
            user_id: Some("user123".to_string()),
            ..Default::default()
        };
        
        // Test enabled flag
        assert!(service.is_enabled("advanced_analytics", &context).await.unwrap());
        
        // Test disabled flag
        assert!(!service.is_enabled("maintenance_mode", &context).await.unwrap());
    }
    
    #[tokio::test]
    async fn test_percentage_rollout() {
        let mut service = FeatureFlagService::new(None);
        service.init_default().await;
        
        let mut enabled_count = 0;
        for i in 0..100 {
            let context = EvaluationContext {
                user_id: Some(format!("user{}", i)),
                ..Default::default()
            };
            
            if service.is_enabled("new_trading_ui", &context).await.unwrap() {
                enabled_count += 1;
            }
        }
        
        // Should be roughly 50% (with some variance)
        assert!(enabled_count > 30 && enabled_count < 70);
    }
    
    #[tokio::test]
    async fn test_target_rules() {
        let mut service = FeatureFlagService::new(None);
        service.init_default().await;
        
        // Beta tester should have access
        let beta_context = EvaluationContext {
            user_id: Some("beta_user".to_string()),
            group_ids: vec!["beta_testers".to_string()],
            ..Default::default()
        };
        assert!(service.is_enabled("quantum_trading", &beta_context).await.unwrap());
        
        // Regular user should not have access
        let regular_context = EvaluationContext {
            user_id: Some("regular_user".to_string()),
            ..Default::default()
        };
        assert!(!service.is_enabled("quantum_trading", &regular_context).await.unwrap());
    }
}