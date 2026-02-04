//! State Management Service
//! 
//! Provides centralized state management with synchronization,
//! persistence, and atomic updates across the application.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Notify, broadcast};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use async_trait::async_trait;

use crate::{
    typed_errors::{AppError, ErrorKind, ErrorContext},
    platform::Timestamp,
};

/// State change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub key: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub timestamp: Timestamp,
    pub source: String,
}

/// State snapshot for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub timestamp: Timestamp,
    pub version: u64,
    pub data: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, String>,
}

/// State persistence trait
#[async_trait]
pub trait StatePersistence: Send + Sync {
    /// Save state snapshot
    async fn save_snapshot(&self, snapshot: &StateSnapshot) -> Result<(), AppError>;
    
    /// Load latest snapshot
    async fn load_latest_snapshot(&self) -> Result<Option<StateSnapshot>, AppError>;
    
    /// List snapshots
    async fn list_snapshots(&self, limit: usize) -> Result<Vec<StateSnapshot>, AppError>;
}

/// In-memory state persistence (for testing)
pub struct InMemoryPersistence {
    snapshots: Arc<RwLock<Vec<StateSnapshot>>>,
}

impl InMemoryPersistence {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl StatePersistence for InMemoryPersistence {
    async fn save_snapshot(&self, snapshot: &StateSnapshot) -> Result<(), AppError> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.push(snapshot.clone());
        Ok(())
    }
    
    async fn load_latest_snapshot(&self) -> Result<Option<StateSnapshot>, AppError> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.last().cloned())
    }
    
    async fn list_snapshots(&self, limit: usize) -> Result<Vec<StateSnapshot>, AppError> {
        let snapshots = self.snapshots.read().await;
        let start = snapshots.len().saturating_sub(limit);
        Ok(snapshots[start..].to_vec())
    }
}

/// State manager configuration
#[derive(Debug, Clone)]
pub struct StateManagerConfig {
    pub snapshot_interval: Duration,
    pub max_snapshots: usize,
    pub enable_persistence: bool,
    pub broadcast_changes: bool,
}

impl Default for StateManagerConfig {
    fn default() -> Self {
        Self {
            snapshot_interval: Duration::from_secs(300), // 5 minutes
            max_snapshots: 100,
            enable_persistence: true,
            broadcast_changes: true,
        }
    }
}

/// Centralized state management service
pub struct StateManager {
    /// Core state storage
    state: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    
    /// State metadata
    metadata: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
    
    /// Version counter for optimistic locking
    version: Arc<RwLock<u64>>,
    
    /// Change event broadcaster
    change_broadcaster: broadcast::Sender<StateChangeEvent>,
    
    /// Persistence layer
    persistence: Option<Box<dyn StatePersistence>>,
    
    /// Configuration
    config: StateManagerConfig,
    
    /// Last snapshot time
    last_snapshot: Arc<RwLock<Instant>>,
    
    /// Update notification
    update_notify: Arc<Notify>,
}

impl StateManager {
    /// Create new state manager
    pub fn new(config: StateManagerConfig) -> Self {
        let (tx, _) = broadcast::channel(1000);
        
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            version: Arc::new(RwLock::new(0)),
            change_broadcaster: tx,
            persistence: None,
            config,
            last_snapshot: Arc::new(RwLock::new(Instant::now())),
            update_notify: Arc::new(Notify::new()),
        }
    }
    
    /// Set persistence layer
    pub fn with_persistence(mut self, persistence: Box<dyn StatePersistence>) -> Self {
        self.persistence = Some(persistence);
        self
    }
    
    /// Get state value
    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, AppError> {
        let context = ErrorContext::new("state_manager", "get");
        
        let state = self.state.read().await;
        
        match state.get(key) {
            Some(value) => {
                let typed_value = serde_json::from_value(value.clone())
                    .map_err(|e| AppError::new(
                        ErrorKind::InternalError,
                        format!("Failed to deserialize state value: {}", e),
                        context,
                    ))?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }
    
    /// Set state value with atomic update
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: T,
        source: &str,
    ) -> Result<(), AppError> {
        let context = ErrorContext::new("state_manager", "set");
        
        let new_value = serde_json::to_value(&value)
            .map_err(|e| AppError::new(
                ErrorKind::InternalError,
                format!("Failed to serialize state value: {}", e),
                context,
            ))?;
        
        let mut state = self.state.write().await;
        let old_value = state.get(key).cloned();
        
        // Update state
        state.insert(key.to_string(), new_value.clone());
        
        // Increment version
        let mut version = self.version.write().await;
        *version += 1;
        
        drop(state);
        drop(version);
        
        // Broadcast change event
        if self.config.broadcast_changes {
            let event = StateChangeEvent {
                key: key.to_string(),
                old_value,
                new_value: Some(new_value),
                timestamp: Timestamp::now(),
                source: source.to_string(),
            };
            
            let _ = self.change_broadcaster.send(event);
        }
        
        // Notify waiters
        self.update_notify.notify_waiters();
        
        // Check if snapshot needed
        self.maybe_snapshot().await?;
        
        Ok(())
    }
    
    /// Remove state value
    pub async fn remove(&self, key: &str, source: &str) -> Result<(), AppError> {
        let mut state = self.state.write().await;
        let old_value = state.remove(key);
        
        // Increment version
        let mut version = self.version.write().await;
        *version += 1;
        
        drop(state);
        drop(version);
        
        // Broadcast change event
        if self.config.broadcast_changes && old_value.is_some() {
            let event = StateChangeEvent {
                key: key.to_string(),
                old_value,
                new_value: None,
                timestamp: Timestamp::now(),
                source: source.to_string(),
            };
            
            let _ = self.change_broadcaster.send(event);
        }
        
        // Notify waiters
        self.update_notify.notify_waiters();
        
        Ok(())
    }
    
    /// Atomic compare-and-swap operation
    pub async fn compare_and_swap<T: Serialize + serde::de::DeserializeOwned + PartialEq>(
        &self,
        key: &str,
        expected: Option<T>,
        new_value: Option<T>,
        source: &str,
    ) -> Result<bool, AppError> {
        let context = ErrorContext::new("state_manager", "compare_and_swap");
        
        let mut state = self.state.write().await;
        
        // Get current value
        let current = match state.get(key) {
            Some(v) => {
                let typed: T = serde_json::from_value(v.clone())
                    .map_err(|e| AppError::new(
                        ErrorKind::InternalError,
                        format!("Failed to deserialize current value: {}", e),
                        context.clone(),
                    ))?;
                Some(typed)
            }
            None => None,
        };
        
        // Check if current matches expected
        if current != expected {
            return Ok(false);
        }
        
        // Update value
        let new_value_for_event = match &new_value {
            Some(v) => {
                let json_value = serde_json::to_value(v)
                    .map_err(|e| AppError::new(
                        ErrorKind::InternalError,
                        format!("Failed to serialize new value: {}", e),
                        context,
                    ))?;
                state.insert(key.to_string(), json_value.clone());
                Some(json_value)
            }
            None => {
                state.remove(key);
                None
            }
        };
        
        // Increment version
        let mut version = self.version.write().await;
        *version += 1;
        
        drop(state);
        drop(version);
        
        // Broadcast change event
        if self.config.broadcast_changes {
            let event = StateChangeEvent {
                key: key.to_string(),
                old_value: expected.and_then(|v| serde_json::to_value(v).ok()),
                new_value: new_value_for_event,
                timestamp: Timestamp::now(),
                source: source.to_string(),
            };
            
            let _ = self.change_broadcaster.send(event);
        }
        
        // Notify waiters
        self.update_notify.notify_waiters();
        
        Ok(true)
    }
    
    /// Get all keys matching a prefix
    pub async fn get_keys_by_prefix(&self, prefix: &str) -> Vec<String> {
        let state = self.state.read().await;
        state.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
    
    /// Get state metadata
    pub async fn get_metadata(&self, key: &str) -> Option<HashMap<String, String>> {
        let metadata = self.metadata.read().await;
        metadata.get(key).cloned()
    }
    
    /// Set state metadata
    pub async fn set_metadata(&self, key: &str, metadata: HashMap<String, String>) {
        let mut meta_map = self.metadata.write().await;
        meta_map.insert(key.to_string(), metadata);
    }
    
    /// Subscribe to state changes
    pub fn subscribe(&self) -> broadcast::Receiver<StateChangeEvent> {
        self.change_broadcaster.subscribe()
    }
    
    /// Wait for next update
    pub async fn wait_for_update(&self) {
        self.update_notify.notified().await;
    }
    
    /// Get current version
    pub async fn get_version(&self) -> u64 {
        *self.version.read().await
    }
    
    /// Create state snapshot
    pub async fn create_snapshot(&self) -> Result<StateSnapshot, AppError> {
        let state = self.state.read().await;
        let version = *self.version.read().await;
        
        let snapshot = StateSnapshot {
            timestamp: Timestamp::now(),
            version,
            data: state.clone(),
            metadata: HashMap::new(),
        };
        
        // Save to persistence if enabled
        if self.config.enable_persistence {
            if let Some(persistence) = &self.persistence {
                persistence.save_snapshot(&snapshot).await?;
            }
        }
        
        Ok(snapshot)
    }
    
    /// Restore from snapshot
    pub async fn restore_snapshot(&self, snapshot: StateSnapshot) -> Result<(), AppError> {
        let mut state = self.state.write().await;
        let mut version = self.version.write().await;
        
        *state = snapshot.data;
        *version = snapshot.version;
        
        drop(state);
        drop(version);
        
        // Notify all waiters
        self.update_notify.notify_waiters();
        
        info!("State restored from snapshot version {}", snapshot.version);
        
        Ok(())
    }
    
    /// Check if snapshot is needed
    async fn maybe_snapshot(&self) -> Result<(), AppError> {
        if !self.config.enable_persistence || self.persistence.is_none() {
            return Ok(());
        }
        
        let mut last_snapshot = self.last_snapshot.write().await;
        
        if last_snapshot.elapsed() >= self.config.snapshot_interval {
            *last_snapshot = Instant::now();
            drop(last_snapshot);
            
            self.create_snapshot().await?;
            debug!("Created automatic state snapshot");
        }
        
        Ok(())
    }
    
    /// Get state statistics
    pub async fn get_stats(&self) -> StateStats {
        let state = self.state.read().await;
        let metadata = self.metadata.read().await;
        let version = *self.version.read().await;
        
        StateStats {
            total_keys: state.len(),
            total_size: state.values()
                .map(|v| v.to_string().len())
                .sum(),
            version,
            metadata_keys: metadata.len(),
        }
    }
}

/// State statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateStats {
    pub total_keys: usize,
    pub total_size: usize,
    pub version: u64,
    pub metadata_keys: usize,
}

/// State synchronization service for distributed systems
pub struct StateSyncService {
    local_manager: Arc<StateManager>,
    sync_interval: Duration,
}

impl StateSyncService {
    pub fn new(local_manager: Arc<StateManager>, sync_interval: Duration) -> Self {
        Self {
            local_manager,
            sync_interval,
        }
    }
    
    /// Start synchronization loop
    pub async fn start(&self) {
        let mut interval = tokio::time::interval(self.sync_interval);
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.sync_state().await {
                error!("State synchronization failed: {}", e);
            }
        }
    }
    
    /// Synchronize state with peers
    async fn sync_state(&self) -> Result<(), AppError> {
        // This would implement actual state synchronization logic
        // For now, just log
        debug!("State synchronization check");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_state_manager_basic() {
        let manager = StateManager::new(StateManagerConfig::default());
        
        // Set value
        manager.set("test_key", "test_value", "test").await.unwrap();
        
        // Get value
        let value: Option<String> = manager.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));
        
        // Remove value
        manager.remove("test_key", "test").await.unwrap();
        
        // Verify removed
        let value: Option<String> = manager.get("test_key").await.unwrap();
        assert_eq!(value, None);
    }
    
    #[tokio::test]
    async fn test_compare_and_swap() {
        let manager = StateManager::new(StateManagerConfig::default());
        
        // Set initial value
        manager.set("counter", 0u64, "test").await.unwrap();
        
        // Successful CAS
        let success = manager.compare_and_swap(
            "counter",
            Some(0u64),
            Some(1u64),
            "test"
        ).await.unwrap();
        assert!(success);
        
        // Failed CAS
        let success = manager.compare_and_swap(
            "counter",
            Some(0u64),
            Some(2u64),
            "test"
        ).await.unwrap();
        assert!(!success);
        
        // Verify value
        let value: Option<u64> = manager.get("counter").await.unwrap();
        assert_eq!(value, Some(1));
    }
    
    #[tokio::test]
    async fn test_state_events() {
        let manager = StateManager::new(StateManagerConfig::default());
        let mut subscriber = manager.subscribe();
        
        // Set value
        manager.set("event_test", "value1", "test").await.unwrap();
        
        // Receive event
        let event = subscriber.recv().await.unwrap();
        assert_eq!(event.key, "event_test");
        assert_eq!(event.source, "test");
    }
}