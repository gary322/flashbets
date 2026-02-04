//! Environment configuration with validation

use serde::{Deserialize, Serialize};
use std::env;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// Solana configuration
    pub solana: SolanaConfig,
    
    /// Authentication configuration
    pub auth: AuthConfig,
    
    /// Integration configuration
    pub integration: IntegrationConfig,
    
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub program_id: String,
    pub commitment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub bcrypt_cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub polymarket_enabled: bool,
    pub polymarket_api_key: Option<String>,
    pub polymarket_webhook_secret: Option<String>,
    pub kalshi_enabled: bool,
    pub kalshi_api_key: Option<String>,
    pub kalshi_api_secret: Option<String>,
    pub sync_interval_seconds: u64,
    pub max_price_deviation: f64,
    pub min_liquidity_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub global_rps: u32,
    pub per_ip_rps: u32,
    pub global_burst: u32,
    pub ip_burst: u32,
}

impl Config {
    /// Load configuration from environment
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Config {
            server: ServerConfig {
                host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("SERVER_PORT")
                    .unwrap_or_else(|_| "8081".to_string())
                    .parse()
                    .map_err(|_| ConfigError::InvalidPort)?,
                cors_origins: env::var("CORS_ORIGINS")
                    .unwrap_or_else(|_| "*".to_string())
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
                log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            },
            
            database: DatabaseConfig {
                url: env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "sqlite://betting_platform.db".to_string()),
                max_connections: env::var("DB_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .unwrap_or(100),
                min_connections: env::var("DB_MIN_CONNECTIONS")
                    .unwrap_or_else(|_| "2".to_string())
                    .parse()
                    .unwrap_or(2),
                connection_timeout: env::var("DB_CONNECTION_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            },
            
            solana: SolanaConfig {
                rpc_url: env::var("SOLANA_RPC_URL")
                    .unwrap_or_else(|_| "http://localhost:8899".to_string()),
                ws_url: env::var("SOLANA_WS_URL")
                    .unwrap_or_else(|_| "ws://localhost:8900".to_string()),
                program_id: env::var("PROGRAM_ID")
                    .map_err(|_| ConfigError::MissingRequired("PROGRAM_ID".to_string()))?,
                commitment: env::var("SOLANA_COMMITMENT")
                    .unwrap_or_else(|_| "confirmed".to_string()),
            },
            
            auth: AuthConfig {
                jwt_secret: std::env::var("JWT_SECRET")
                    .unwrap_or_else(|_| "your-secret-key-must-be-at-least-32-characters-long".to_string()),
                jwt_expiration_hours: env::var("JWT_EXPIRATION_HOURS")
                    .unwrap_or_else(|_| "24".to_string())
                    .parse()
                    .unwrap_or(24),
                bcrypt_cost: env::var("BCRYPT_COST")
                    .unwrap_or_else(|_| "12".to_string())
                    .parse()
                    .unwrap_or(12),
            },
            
            integration: IntegrationConfig {
                polymarket_enabled: env::var("POLYMARKET_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                polymarket_api_key: env::var("POLYMARKET_API_KEY").ok(),
                polymarket_webhook_secret: env::var("POLYMARKET_WEBHOOK_SECRET").ok(),
                kalshi_enabled: env::var("KALSHI_ENABLED")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                kalshi_api_key: env::var("KALSHI_API_KEY").ok(),
                kalshi_api_secret: env::var("KALSHI_API_SECRET").ok(),
                sync_interval_seconds: env::var("SYNC_INTERVAL_SECONDS")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .unwrap_or(300),
                max_price_deviation: env::var("MAX_PRICE_DEVIATION")
                    .unwrap_or_else(|_| "0.05".to_string())
                    .parse()
                    .unwrap_or(0.05),
                min_liquidity_usd: env::var("MIN_LIQUIDITY_USD")
                    .unwrap_or_else(|_| "10000.0".to_string())
                    .parse()
                    .unwrap_or(10_000.0),
            },
            
            rate_limit: RateLimitConfig {
                global_rps: env::var("RATE_LIMIT_GLOBAL_RPS")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
                per_ip_rps: env::var("RATE_LIMIT_PER_IP_RPS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
                global_burst: env::var("RATE_LIMIT_GLOBAL_BURST")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .unwrap_or(100),
                ip_burst: env::var("RATE_LIMIT_IP_BURST")
                    .unwrap_or_else(|_| "20".to_string())
                    .parse()
                    .unwrap_or(20),
            },
        })
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate server config
        if self.server.port == 0 {
            return Err(ConfigError::InvalidPort);
        }
        
        // Validate database config
        if self.database.max_connections < self.database.min_connections {
            return Err(ConfigError::InvalidConfig(
                "max_connections must be >= min_connections".to_string()
            ));
        }
        
        // Validate Solana program ID
        if self.solana.program_id.is_empty() {
            return Err(ConfigError::MissingRequired("program_id".to_string()));
        }
        
        // Validate JWT secret length
        if self.auth.jwt_secret.len() < 32 {
            return Err(ConfigError::InvalidConfig("JWT secret must be at least 32 characters".to_string()));
        }
        
        // Validate rate limits
        if self.rate_limit.global_rps == 0 || self.rate_limit.per_ip_rps == 0 {
            return Err(ConfigError::InvalidConfig(
                "Rate limits must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),
    
    #[error("Invalid port number")]
    InvalidPort,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Create example .env file
pub fn create_env_example() -> String {
    r#"# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8081
CORS_ORIGINS=http://localhost:3000,http://localhost:8080
LOG_LEVEL=info

# Database Configuration
DATABASE_URL=sqlite://betting_platform.db
DB_MAX_CONNECTIONS=100
DB_MIN_CONNECTIONS=2
DB_CONNECTION_TIMEOUT=30

# Solana Configuration
SOLANA_RPC_URL=http://localhost:8899
SOLANA_WS_URL=ws://localhost:8900
PROGRAM_ID=HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca
SOLANA_COMMITMENT=confirmed

# Authentication
JWT_SECRET=your-secret-key-must-be-at-least-32-characters-long
JWT_EXPIRATION_HOURS=24
BCRYPT_COST=12

# External Integrations
POLYMARKET_ENABLED=true
POLYMARKET_API_KEY=your-polymarket-api-key
KALSHI_ENABLED=false
KALSHI_API_KEY=your-kalshi-api-key
SYNC_INTERVAL_SECONDS=300

# Rate Limiting
RATE_LIMIT_GLOBAL_RPS=1000
RATE_LIMIT_PER_IP_RPS=10
RATE_LIMIT_GLOBAL_BURST=100
RATE_LIMIT_IP_BURST=20
"#.to_string()
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let mut config = Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8081,
                cors_origins: vec!["*".to_string()],
                log_level: "info".to_string(),
            },
            database: DatabaseConfig {
                url: "sqlite://test.db".to_string(),
                max_connections: 100,
                min_connections: 2,
                connection_timeout: 30,
            },
            solana: SolanaConfig {
                rpc_url: "http://localhost:8899".to_string(),
                ws_url: "ws://localhost:8900".to_string(),
                program_id: "TestProgramId123".to_string(),
                commitment: "confirmed".to_string(),
            },
            auth: AuthConfig {
                jwt_secret: "a-very-long-secret-key-for-testing-purposes-only".to_string(),
                jwt_expiration_hours: 24,
                bcrypt_cost: 12,
            },
            integration: IntegrationConfig {
                polymarket_enabled: true,
                polymarket_api_key: None,
                polymarket_webhook_secret: None,
                kalshi_enabled: false,
                kalshi_api_key: None,
                kalshi_api_secret: None,
                sync_interval_seconds: 300,
                max_price_deviation: 0.05,
                min_liquidity_usd: 10_000.0,
            },
            rate_limit: RateLimitConfig {
                global_rps: 1000,
                per_ip_rps: 10,
                global_burst: 100,
                ip_burst: 20,
            },
        };
        
        assert!(config.validate().is_ok());
        
        // Test invalid config
        config.auth.jwt_secret = "short".to_string();
        assert!(config.validate().is_err());
    }
}