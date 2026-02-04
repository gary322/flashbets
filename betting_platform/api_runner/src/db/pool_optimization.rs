//! Connection pool optimization for high-load scenarios

use std::time::Duration;

/// Optimized database configuration for 2000+ concurrent users
#[derive(Debug, Clone)]
pub struct OptimizedPoolConfig {
    /// Maximum connections in the pool
    pub max_connections: u32,
    /// Minimum idle connections to maintain
    pub min_idle: u32,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Idle timeout before connection is closed
    pub idle_timeout: Duration,
    /// Maximum lifetime of a connection
    pub max_lifetime: Duration,
}

impl OptimizedPoolConfig {
    /// Create configuration optimized for high load (2000+ users)
    pub fn high_load() -> Self {
        Self {
            // PostgreSQL default max_connections is often 100
            // For 2000+ users, we need efficient connection pooling
            // Using 200 connections with multiplexing
            max_connections: 200,
            
            // Keep 50 connections ready for immediate use
            min_idle: 50,
            
            // Quick timeout to fail fast under load
            connection_timeout: Duration::from_secs(5),
            
            // Shorter idle timeout to recycle connections faster
            idle_timeout: Duration::from_secs(300), // 5 minutes
            
            // Shorter max lifetime to prevent stale connections
            max_lifetime: Duration::from_secs(900), // 15 minutes
        }
    }
    
    /// Create configuration for medium load (500-2000 users)
    pub fn medium_load() -> Self {
        Self {
            max_connections: 100,
            min_idle: 20,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(600), // 10 minutes
            max_lifetime: Duration::from_secs(1800), // 30 minutes
        }
    }
    
    /// Create configuration for low load (<500 users)
    pub fn low_load() -> Self {
        Self {
            max_connections: 50,
            min_idle: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(900), // 15 minutes
            max_lifetime: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Calculate optimal pool size based on system resources
pub fn calculate_optimal_pool_size() -> u32 {
    // Formula: connections = ((core_count * 2) + effective_spindle_count)
    // For SSDs, effective_spindle_count = 1
    let cpu_count = num_cpus::get() as u32;
    let base_connections = (cpu_count * 2) + 1;
    
    // Scale up for high-performance systems
    // But cap at PostgreSQL's typical limits
    base_connections.min(200).max(50)
}

/// Pool configuration recommendations based on load testing
pub struct PoolRecommendations;

impl PoolRecommendations {
    pub fn get_recommendations(expected_concurrent_users: u32) -> String {
        let config = if expected_concurrent_users > 2000 {
            OptimizedPoolConfig::high_load()
        } else if expected_concurrent_users > 500 {
            OptimizedPoolConfig::medium_load()
        } else {
            OptimizedPoolConfig::low_load()
        };
        
        format!(
            "Recommended configuration for {} concurrent users:\n\
             - Max connections: {}\n\
             - Min idle connections: {}\n\
             - Connection timeout: {:?}\n\
             - Idle timeout: {:?}\n\
             - Max lifetime: {:?}\n\
             - Estimated memory usage: {} MB\n\
             - PostgreSQL max_connections should be at least: {}",
            expected_concurrent_users,
            config.max_connections,
            config.min_idle,
            config.connection_timeout,
            config.idle_timeout,
            config.max_lifetime,
            config.max_connections * 5, // ~5MB per connection
            config.max_connections + 50 // Leave room for other connections
        )
    }
}