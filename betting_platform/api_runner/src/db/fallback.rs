//! Fallback database implementation for graceful degradation

use anyhow::{Result, anyhow};
use deadpool_postgres::{Object, Pool};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A fallback database wrapper that can operate without a real connection
pub struct FallbackDatabase {
    /// The real database pool, if available
    pool: Option<Pool>,
    /// Track if we're in degraded mode
    degraded_mode: Arc<Mutex<bool>>,
}

impl FallbackDatabase {
    /// Create a new fallback database
    pub async fn new(config: super::DatabaseConfig) -> Result<Self> {
        // Try to create a real database connection
        match super::Database::new(config).await {
            Ok(db) => {
                // We have a real database
                Ok(Self {
                    pool: Some(db.pool),
                    degraded_mode: Arc::new(Mutex::new(false)),
                })
            }
            Err(e) => {
                tracing::error!("Database connection failed: {}. Running in degraded mode.", e);
                // No database available, run in degraded mode
                Ok(Self {
                    pool: None,
                    degraded_mode: Arc::new(Mutex::new(true)),
                })
            }
        }
    }
    
    /// Get a connection from the pool if available
    pub async fn get_connection(&self) -> Result<Object> {
        match &self.pool {
            Some(pool) => {
                pool.get().await
                    .map_err(|e| anyhow!("Failed to get database connection: {}", e))
            }
            None => {
                Err(anyhow!("Database unavailable - running in degraded mode"))
            }
        }
    }
    
    /// Check if we're in degraded mode
    pub async fn is_degraded(&self) -> bool {
        *self.degraded_mode.lock().await
    }
    
    /// Try to run migrations if database is available
    pub async fn run_migrations(&self) -> Result<()> {
        match self.get_connection().await {
            Ok(mut conn) => {
                super::migrations_production::run_migrations(&mut conn).await
            }
            Err(_) => {
                tracing::warn!("Cannot run migrations - database unavailable");
                Ok(())
            }
        }
    }
    
    /// Get the connection pool (if available)
    pub fn get_pool(&self) -> Result<&deadpool_postgres::Pool> {
        self.pool.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database pool not available in degraded mode"))
    }
    
    /// Get pool status
    pub fn pool_status(&self) -> super::PoolStatus {
        match &self.pool {
            Some(pool) => {
                let status = pool.status();
                super::PoolStatus {
                    size: status.size as u32,
                    available: status.available as u32,
                    waiting: status.waiting as u32,
                }
            }
            None => {
                // Return zeros when in degraded mode
                super::PoolStatus {
                    size: 0,
                    available: 0,
                    waiting: 0,
                }
            }
        }
    }
}