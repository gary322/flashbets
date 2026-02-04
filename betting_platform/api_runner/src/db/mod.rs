//! Database module for PostgreSQL integration
//! 
//! Provides database connection pooling, migrations, and data access layer

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub mod models;
pub mod queries;
pub mod migrations;
pub mod migrations_production;
pub mod market_queries;
pub mod fallback;
pub mod pool_optimization;
pub mod polymarket_repository;

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://betting_user:betting_pass@localhost/betting_platform".to_string(),
            max_connections: 100,
            min_connections: 10,
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
        }
    }
}

/// Database connection pool using deadpool-postgres
pub struct Database {
    pub(crate) pool: deadpool_postgres::Pool,
}

impl Database {
    /// Create a new database connection pool
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        let pg_config = config.url.parse::<tokio_postgres::Config>()
            .context("Failed to parse database URL")?;
        
        let manager_config = deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        };
        
        let manager = deadpool_postgres::Manager::from_config(
            pg_config,
            tokio_postgres::NoTls,
            manager_config,
        );
        
        let pool = deadpool_postgres::Pool::builder(manager)
            .max_size(config.max_connections as usize)
            .runtime(deadpool_postgres::Runtime::Tokio1)
            .wait_timeout(Some(config.connection_timeout))
            .create_timeout(Some(config.connection_timeout))
            .recycle_timeout(Some(config.idle_timeout))
            .build()
            .context("Failed to create database pool")?;
        
        // Test the connection
        let _ = pool.get().await
            .context("Failed to establish database connection")?;
        
        Ok(Self { pool })
    }
    
    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<deadpool_postgres::Object> {
        self.pool.get().await
            .context("Failed to get database connection from pool")
    }
    
    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        
        // Use production migrations if the old schema doesn't exist
        match conn.query_opt("SELECT 1 FROM information_schema.tables WHERE table_name = 'migrations'", &[]).await {
            Ok(Some(_)) => {
                // Old migrations table exists, check if we need to migrate
                let count: i64 = conn.query_one("SELECT COUNT(*) FROM migrations", &[])
                    .await?.get(0);
                if count > 0 {
                    // Clear old migrations and use production schema
                    conn.execute("DROP TABLE IF EXISTS migrations CASCADE", &[]).await?;
                    migrations_production::run_migrations(&mut conn).await
                } else {
                    migrations_production::run_migrations(&mut conn).await
                }
            },
            _ => {
                // No old migrations, use production schema
                migrations_production::run_migrations(&mut conn).await
            }
        }
    }
    
    /// Get pool statistics
    pub fn pool_status(&self) -> PoolStatus {
        let status = self.pool.status();
        PoolStatus {
            size: status.size as u32,
            available: status.available as u32,
            waiting: status.waiting as u32,
        }
    }
}

/// Pool status information
#[derive(Debug, Clone, Serialize)]
pub struct PoolStatus {
    pub size: u32,
    pub available: u32,
    pub waiting: u32,
}

/// Transaction helper for atomic operations
pub struct Transaction<'a> {
    txn: deadpool_postgres::Transaction<'a>,
}

impl<'a> Transaction<'a> {
    /// Create a new transaction
    pub async fn begin(conn: &'a mut deadpool_postgres::Object) -> Result<Self> {
        let txn = conn.transaction().await
            .context("Failed to begin transaction")?;
        Ok(Self { txn })
    }
    
    /// Commit the transaction
    pub async fn commit(self) -> Result<()> {
        self.txn.commit().await
            .context("Failed to commit transaction")
    }
    
    /// Rollback the transaction
    pub async fn rollback(self) -> Result<()> {
        self.txn.rollback().await
            .context("Failed to rollback transaction")
    }
    
    /// Get a reference to the underlying transaction
    pub fn as_ref(&self) -> &deadpool_postgres::Transaction<'a> {
        &self.txn
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 100);
        assert_eq!(config.min_connections, 10);
    }
}