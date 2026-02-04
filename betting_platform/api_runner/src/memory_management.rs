//! Memory management utilities for preventing leaks

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Configuration for memory management
pub struct MemoryConfig {
    /// Maximum WebSocket connections
    pub max_websocket_connections: usize,
    /// Maximum broadcast channel size
    pub broadcast_channel_size: usize,
    /// Cache cleanup interval
    pub cache_cleanup_interval: Duration,
    /// Connection idle timeout
    pub connection_idle_timeout: Duration,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_websocket_connections: 5000,
            broadcast_channel_size: 1000, // Increased from 100
            cache_cleanup_interval: Duration::from_secs(300), // 5 minutes
            connection_idle_timeout: Duration::from_secs(900), // 15 minutes
        }
    }
}

/// WebSocket connection tracker to prevent connection leaks
pub struct ConnectionTracker {
    connections: Arc<RwLock<std::collections::HashMap<uuid::Uuid, std::time::Instant>>>,
    max_connections: usize,
    idle_timeout: Duration,
}

impl ConnectionTracker {
    pub fn new(max_connections: usize, idle_timeout: Duration) -> Self {
        Self {
            connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
            max_connections,
            idle_timeout,
        }
    }
    
    /// Register a new connection
    pub async fn register(&self) -> Option<uuid::Uuid> {
        let mut connections = self.connections.write().await;
        
        // Check if we've reached the limit
        if connections.len() >= self.max_connections {
            return None;
        }
        
        let id = uuid::Uuid::new_v4();
        connections.insert(id, std::time::Instant::now());
        Some(id)
    }
    
    /// Unregister a connection
    pub async fn unregister(&self, id: uuid::Uuid) {
        let mut connections = self.connections.write().await;
        connections.remove(&id);
    }
    
    /// Update connection activity
    pub async fn touch(&self, id: uuid::Uuid) {
        let mut connections = self.connections.write().await;
        if let Some(last_seen) = connections.get_mut(&id) {
            *last_seen = std::time::Instant::now();
        }
    }
    
    /// Clean up idle connections
    pub async fn cleanup_idle(&self) -> usize {
        let mut connections = self.connections.write().await;
        let now = std::time::Instant::now();
        let before = connections.len();
        
        connections.retain(|_, last_seen| {
            now.duration_since(*last_seen) < self.idle_timeout
        });
        
        before - connections.len()
    }
    
    /// Get current connection count
    pub async fn count(&self) -> usize {
        self.connections.read().await.len()
    }
}

/// Start background cleanup tasks
pub fn start_cleanup_tasks(
    connection_tracker: Arc<ConnectionTracker>,
    cleanup_interval: Duration,
) {
    // Spawn connection cleanup task
    tokio::spawn(async move {
        let mut interval = interval(cleanup_interval);
        
        loop {
            interval.tick().await;
            let cleaned = connection_tracker.cleanup_idle().await;
            if cleaned > 0 {
                tracing::info!("Cleaned up {} idle WebSocket connections", cleaned);
            }
        }
    });
}

/// Memory usage monitor
pub async fn log_memory_stats() {
    // This is a placeholder for actual memory monitoring
    // In production, you might use jemalloc stats or system metrics
    tracing::debug!(
        "Memory stats - WebSocket broadcast channel optimized to prevent overflow"
    );
}