//! Environment Configuration Management
//! 
//! Centralized configuration management with validation,
//! hot reloading, and environment-specific overrides.

use std::{
    collections::HashMap,
    env,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::{
    typed_errors::{AppError, ErrorKind, ErrorContext},
    platform::{PlatformPath, Timestamp},
};

type Result<T> = std::result::Result<T, AppError>;

/// Environment types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Staging,
    Production,
    Test,
}

impl Environment {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "production" | "prod" => Environment::Production,
            "staging" | "stage" => Environment::Staging,
            "test" | "testing" => Environment::Test,
            _ => Environment::Development,
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Development => "development",
            Environment::Staging => "staging",
            Environment::Production => "production",
            Environment::Test => "test",
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub environment: Environment,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub solana: SolanaConfig,
    pub websocket: WebSocketConfig,
    pub security: SecurityConfig,
    pub external_apis: ExternalApisConfig,
    pub features: FeatureFlags,
    pub monitoring: MonitoringConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
    #[serde(with = "humantime_serde")]
    pub keep_alive: Duration,
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
    pub body_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    #[serde(with = "humantime_serde")]
    pub connect_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub idle_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub max_lifetime: Duration,
    pub enable_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    pub retry_attempts: u32,
    #[serde(with = "humantime_serde")]
    pub retry_delay: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub commitment: String,
    pub program_id: String,
    #[serde(with = "humantime_serde")]
    pub request_timeout: Duration,
    pub max_retries: u32,
    #[serde(with = "humantime_serde")]
    pub retry_delay: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub max_connections: usize,
    #[serde(with = "humantime_serde")]
    pub ping_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub pong_timeout: Duration,
    pub message_buffer_size: usize,
    pub broadcast_capacity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    #[serde(with = "humantime_serde")]
    pub jwt_expiry: Duration,
    #[serde(with = "humantime_serde")]
    pub refresh_token_expiry: Duration,
    pub bcrypt_cost: u32,
    pub rate_limit_requests: u32,
    #[serde(with = "humantime_serde")]
    pub rate_limit_window: Duration,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalApisConfig {
    pub polymarket: PolymarketConfig,
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub ws_url: String,
    pub rate_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_mock_services: bool,
    pub enable_test_endpoints: bool,
    pub enable_debug_logging: bool,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub enable_circuit_breakers: bool,
    pub enable_health_checks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(with = "humantime_serde")]
    pub health_check_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub metrics_retention: Duration,
    pub log_level: String,
    pub enable_performance_tracking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    #[serde(with = "humantime_serde")]
    pub cache_ttl: Duration,
    #[serde(with = "humantime_serde")]
    pub query_timeout: Duration,
    pub max_concurrent_requests: usize,
    pub enable_compression: bool,
}

/// Configuration validation errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),
    
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
    
    #[error("Configuration file error: {0}")]
    FileError(String),
    
    #[error("Environment variable error: {0}")]
    EnvError(String),
}

/// Configuration source priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    Default = 0,
    ConfigFile = 1,
    EnvironmentSpecificFile = 2,
    EnvironmentVariable = 3,
    RuntimeOverride = 4,
}

/// Configuration value with source tracking
#[derive(Debug, Clone)]
pub struct ConfigValue<T> {
    pub value: T,
    pub source: ConfigSource,
    pub key: String,
}

/// Environment configuration service
pub struct EnvironmentConfigService {
    config: Arc<RwLock<Config>>,
    config_dir: PathBuf,
    environment: Environment,
    overrides: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    validators: Vec<Box<dyn ConfigValidator>>,
}

/// Configuration validator trait
pub trait ConfigValidator: Send + Sync {
    fn validate(&self, config: &Config) -> std::result::Result<(), ConfigError>;
    fn name(&self) -> &str;
}

impl EnvironmentConfigService {
    /// Create new configuration service
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        let environment = Self::detect_environment();
        let config = Self::load_config(&config_dir, environment)?;
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_dir,
            environment,
            overrides: Arc::new(RwLock::new(HashMap::new())),
            validators: Vec::new(),
        })
    }
    
    /// Detect current environment
    fn detect_environment() -> Environment {
        env::var("ENVIRONMENT")
            .or_else(|_| env::var("ENV"))
            .or_else(|_| env::var("RUST_ENV"))
            .map(|e| Environment::from_str(&e))
            .unwrap_or(Environment::Development)
    }
    
    /// Load configuration from files and environment
    fn load_config(config_dir: &Path, environment: Environment) -> Result<Config> {
        let context = crate::typed_errors::ErrorContext::new("environment_config", "load_config");
        
        // Load default configuration
        let default_path = config_dir.join("config.default.toml");
        let mut config = if default_path.exists() {
            let content = fs::read_to_string(&default_path)
                .map_err(|e| AppError::new(
                    ErrorKind::ConfigurationError,
                    &format!("Failed to read default config: {}", e),
                    context.clone(),
                ))?;
            
            toml::from_str::<Config>(&content)
                .map_err(|e| AppError::new(
                    ErrorKind::ConfigurationError,
                    &format!("Failed to parse default config: {}", e),
                    context.clone(),
                ))?
        } else {
            Self::default_config()
        };
        
        // Load environment-specific configuration
        let env_path = config_dir.join(format!("config.{}.toml", environment.as_str()));
        if env_path.exists() {
            let content = fs::read_to_string(&env_path)
                .map_err(|e| AppError::new(
                    ErrorKind::ConfigurationError,
                    &format!("Failed to read {} config: {}", environment.as_str(), e),
                    context.clone(),
                ))?;
            
            let env_config: toml::Value = toml::from_str(&content)
                .map_err(|e| AppError::new(
                    ErrorKind::ConfigurationError,
                    &format!("Failed to parse {} config: {}", environment.as_str(), e),
                    context.clone(),
                ))?;
            
            // Merge environment config into default
            config = Self::merge_configs(config, env_config)?;
        }
        
        // Apply environment variable overrides
        config = Self::apply_env_overrides(config)?;

        // Always reflect the detected runtime environment, regardless of what config files specify.
        config.environment = environment;
        
        // Validate final configuration
        Self::validate_config(&config)?;
        
        Ok(config)
    }
    
    /// Default configuration
    fn default_config() -> Config {
        Config {
            environment: Environment::Development,
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8081,
                workers: None,
                keep_alive: Duration::from_secs(75),
                request_timeout: Duration::from_secs(30),
                body_limit: 10 * 1024 * 1024, // 10MB
            },
            database: DatabaseConfig {
                url: "postgresql://localhost/betting_platform".to_string(),
                max_connections: 100,
                min_connections: 10,
                connect_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(600),
                max_lifetime: Duration::from_secs(1800),
                enable_fallback: true,
            },
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                pool_size: 20,
                timeout: Duration::from_secs(5),
                retry_attempts: 3,
                retry_delay: Duration::from_millis(100),
            },
            solana: SolanaConfig {
                rpc_url: "https://api.devnet.solana.com".to_string(),
                ws_url: "wss://api.devnet.solana.com".to_string(),
                commitment: "confirmed".to_string(),
                program_id: "11111111111111111111111111111111".to_string(),
                request_timeout: Duration::from_secs(30),
                max_retries: 3,
                retry_delay: Duration::from_millis(500),
            },
            websocket: WebSocketConfig {
                max_connections: 10000,
                ping_interval: Duration::from_secs(30),
                pong_timeout: Duration::from_secs(10),
                message_buffer_size: 1000,
                broadcast_capacity: 10000,
            },
            security: SecurityConfig {
                jwt_secret: "change-me-in-production".to_string(),
                jwt_expiry: Duration::from_secs(3600),
                refresh_token_expiry: Duration::from_secs(86400 * 7),
                bcrypt_cost: 12,
                rate_limit_requests: 100,
                rate_limit_window: Duration::from_secs(60),
                cors_origins: vec!["http://localhost:3000".to_string()],
            },
            external_apis: ExternalApisConfig {
                polymarket: PolymarketConfig {
                    api_key: None,
                    base_url: "https://api.polymarket.com".to_string(),
                    ws_url: "wss://ws.polymarket.com".to_string(),
                    rate_limit: 10,
                },
                timeout: Duration::from_secs(30),
                max_retries: 3,
            },
            features: FeatureFlags {
                enable_mock_services: true,
                enable_test_endpoints: true,
                enable_debug_logging: true,
                enable_metrics: true,
                enable_tracing: true,
                enable_circuit_breakers: true,
                enable_health_checks: true,
            },
            monitoring: MonitoringConfig {
                health_check_interval: Duration::from_secs(30),
                metrics_retention: Duration::from_secs(3600),
                log_level: "info".to_string(),
                enable_performance_tracking: true,
            },
            performance: PerformanceConfig {
                cache_ttl: Duration::from_secs(300),
                query_timeout: Duration::from_secs(5),
                max_concurrent_requests: 1000,
                enable_compression: true,
            },
        }
    }
    
    /// Merge configurations
    fn merge_configs(base: Config, overlay: toml::Value) -> Result<Config> {
        let context = crate::typed_errors::ErrorContext::new("environment_config", "merge_configs");
        
        // Convert base config to Value
        let mut base_value = toml::Value::try_from(base)
            .map_err(|e| AppError::new(
                ErrorKind::ConfigurationError,
                &format!("Failed to convert config to value: {}", e),
                context.clone(),
            ))?;
        
        // Merge overlay into base
        if let (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) = 
            (&mut base_value, overlay) {
            Self::merge_tables(base_table, overlay_table);
        }
        
        // Convert back to Config
        let merged: Config = base_value.try_into()
            .map_err(|e| AppError::new(
                ErrorKind::ConfigurationError,
                &format!("Failed to convert merged config: {}", e),
                context,
            ))?;
        
        Ok(merged)
    }
    
    /// Recursively merge TOML tables
    fn merge_tables(base: &mut toml::map::Map<String, toml::Value>, overlay: toml::map::Map<String, toml::Value>) {
        for (key, value) in overlay {
            match (base.get_mut(&key), value) {
                (Some(toml::Value::Table(base_table)), toml::Value::Table(overlay_table)) => {
                    Self::merge_tables(base_table, overlay_table);
                }
                (_, value) => {
                    base.insert(key, value);
                }
            }
        }
    }
    
    /// Apply environment variable overrides
    fn apply_env_overrides(mut config: Config) -> Result<Config> {
        // Server configuration
        if let Ok(port) = env::var("PORT") {
            config.server.port = port.parse().unwrap_or(config.server.port);
        }
        
        // Database configuration
        if let Ok(db_url) = env::var("DATABASE_URL") {
            config.database.url = db_url;
        }
        
        // Redis configuration
        if let Ok(redis_url) = env::var("REDIS_URL") {
            config.redis.url = redis_url;
        }
        
        // Solana configuration
        if let Ok(rpc_url) = env::var("SOLANA_RPC_URL") {
            config.solana.rpc_url = rpc_url;
        }
        if let Ok(program_id) = env::var("SOLANA_PROGRAM_ID") {
            config.solana.program_id = program_id;
        }
        
        // Security configuration
        if let Ok(jwt_secret) = env::var("JWT_SECRET") {
            config.security.jwt_secret = jwt_secret;
        }
        
        // Feature flags
        if let Ok(mock_enabled) = env::var("MOCK_SERVICES_ENABLED") {
            config.features.enable_mock_services = mock_enabled == "true";
        }
        
        Ok(config)
    }
    
    /// Validate configuration
    fn validate_config(config: &Config) -> Result<()> {
        let context = crate::typed_errors::ErrorContext::new("environment_config", "validate_config");
        
        // Validate required fields
        if config.security.jwt_secret == "change-me-in-production" && 
           config.environment == Environment::Production {
            return Err(AppError::new(
                ErrorKind::ConfigurationError,
                "JWT secret must be changed in production",
                context,
            ));
        }
        
        if config.database.max_connections < config.database.min_connections {
            return Err(AppError::new(
                ErrorKind::ConfigurationError,
                "Database max_connections must be >= min_connections",
                context,
            ));
        }
        
        if config.server.port == 0 {
            return Err(AppError::new(
                ErrorKind::ConfigurationError,
                "Server port must be specified",
                context,
            ));
        }
        
        Ok(())
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }
    
    /// Get specific configuration value
    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let context = crate::typed_errors::ErrorContext::new("environment_config", "get");
        let config = self.config.read().await;
        
        // Convert config to JSON value for path traversal
        let value = serde_json::to_value(&*config)
            .map_err(|e| AppError::new(
                ErrorKind::ConfigurationError,
                &format!("Failed to serialize config: {}", e),
                context.clone(),
            ))?;
        
        // Traverse path
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = &value;
        
        for part in parts {
            current = current.get(part)
                .ok_or_else(|| AppError::new(
                    ErrorKind::ConfigurationError,
                    &format!("Configuration key not found: {}", path),
                    context.clone(),
                ))?;
        }
        
        // Deserialize final value
        serde_json::from_value(current.clone())
            .map_err(|e| AppError::new(
                ErrorKind::ConfigurationError,
                &format!("Failed to deserialize config value: {}", e),
                context,
            ))
    }
    
    /// Set configuration override
    pub async fn set_override(&self, path: &str, value: serde_json::Value) -> Result<()> {
        {
            let mut overrides = self.overrides.write().await;
            overrides.insert(path.to_string(), value);
        }
        
        // Reload configuration with new override
        self.reload().await?;
        
        Ok(())
    }
    
    /// Reload configuration from disk
    pub async fn reload(&self) -> Result<()> {
        let new_config = Self::load_config(&self.config_dir, self.environment)?;
        
        // Apply runtime overrides
        let overrides = self.overrides.read().await;
        let mut config = new_config;
        
        Self::apply_overrides(&mut config, &overrides)?;
        
        // Validate with registered validators
        for validator in &self.validators {
            validator.validate(&config)
                .map_err(|e| AppError::new(
                    ErrorKind::ConfigurationError,
                    &format!("Validation failed ({}): {}", validator.name(), e),
                    ErrorContext::new("environment_config", "reload"),
                ))?;
        }
        
        *self.config.write().await = config;
        
        info!("Configuration reloaded successfully");
        Ok(())
    }

    fn apply_overrides(config: &mut Config, overrides: &HashMap<String, serde_json::Value>) -> Result<()> {
        if overrides.is_empty() {
            return Ok(());
        }

        let context = ErrorContext::new("environment_config", "apply_overrides");

        let mut json = serde_json::to_value(&*config).map_err(|e| {
            AppError::new(
                ErrorKind::ConfigurationError,
                &format!("Failed to serialize config for overrides: {}", e),
                context.clone(),
            )
        })?;

        for (path, value) in overrides {
            Self::set_json_value_at_path(&mut json, path, value.clone())?;
        }

        *config = serde_json::from_value(json).map_err(|e| {
            AppError::new(
                ErrorKind::ConfigurationError,
                &format!("Failed to deserialize config after overrides: {}", e),
                context,
            )
        })?;

        Ok(())
    }

    fn set_json_value_at_path(root: &mut serde_json::Value, path: &str, value: serde_json::Value) -> Result<()> {
        let context = ErrorContext::new("environment_config", "set_override");
        let parts: Vec<&str> = path.split('.').collect();

        let mut current = root;
        for (idx, part) in parts.iter().enumerate() {
            let is_last = idx == parts.len() - 1;

            if is_last {
                match current {
                    serde_json::Value::Object(map) => {
                        map.insert((*part).to_string(), value);
                        return Ok(());
                    }
                    _ => {
                        return Err(AppError::new(
                            ErrorKind::ConfigurationError,
                            &format!("Override path does not point to an object: {}", path),
                            context,
                        ));
                    }
                }
            }

            current = current
                .get_mut(*part)
                .ok_or_else(|| {
                    AppError::new(
                        ErrorKind::ConfigurationError,
                        &format!("Override path not found: {}", path),
                        context.clone(),
                    )
                })?;
        }

        Ok(())
    }
    
    /// Register configuration validator
    pub fn register_validator(&mut self, validator: Box<dyn ConfigValidator>) {
        self.validators.push(validator);
    }
    
    /// Watch configuration files for changes
    pub async fn watch_for_changes(self: Arc<Self>) {
        use notify::{Watcher, RecursiveMode, Config};
        use std::sync::mpsc::channel;
        
        let (tx, rx) = channel();
        
        let mut watcher = match notify::RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                error!("Failed to create config watcher: {}", e);
                return;
            }
        };
        
        let watch_path = PlatformPath::ensure_absolute(&self.config_dir).unwrap_or_else(|_| self.config_dir.clone());
        if let Err(e) = watcher.watch(&watch_path, RecursiveMode::NonRecursive) {
            error!("Failed to watch config directory: {}", e);
            return;
        }
        
        info!("Watching configuration directory: {:?}", self.config_dir);
        
        // Handle file change events
        tokio::spawn(async move {
            loop {
                match rx.recv() {
                    Ok(event) => {
                        info!("Configuration file changed: {:?}", event);
                        if let Err(e) = self.reload().await {
                            error!("Failed to reload configuration: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Config watcher error: {}", e);
                        break;
                    }
                }
            }
        });
    }
    
    /// Export current configuration
    pub async fn export(&self, format: ConfigFormat) -> std::result::Result<String, crate::typed_errors::AppError> {
        let context = crate::typed_errors::ErrorContext::new("environment_config", "export");
        let config = self.config.read().await;
        
        match format {
            ConfigFormat::Toml => {
                toml::to_string(&*config)
                    .map_err(|e| AppError::new(
                        ErrorKind::ConfigurationError,
                        &format!("Failed to export config as TOML: {}", e),
                        context,
                    ))
            }
            ConfigFormat::Json => {
                serde_json::to_string_pretty(&*config)
                    .map_err(|e| AppError::new(
                        ErrorKind::ConfigurationError,
                        &format!("Failed to export config as JSON: {}", e),
                        context,
                    ))
            }
            ConfigFormat::Yaml => {
                serde_yaml::to_string(&*config)
                    .map_err(|e| AppError::new(
                        ErrorKind::ConfigurationError,
                        &format!("Failed to export config as YAML: {}", e),
                        context,
                    ))
            }
        }
    }
    
    /// Get configuration diff between current and default
    pub async fn get_diff(&self) -> HashMap<String, (serde_json::Value, serde_json::Value)> {
        let current = self.config.read().await;
        let default = Self::default_config();
        
        let current_json = serde_json::to_value(&*current).unwrap();
        let default_json = serde_json::to_value(&default).unwrap();
        
        let mut diff = HashMap::new();
        Self::compute_diff(&current_json, &default_json, String::new(), &mut diff);
        
        diff
    }
    
    /// Compute configuration diff recursively
    fn compute_diff(
        current: &serde_json::Value,
        default: &serde_json::Value,
        path: String,
        diff: &mut HashMap<String, (serde_json::Value, serde_json::Value)>,
    ) {
        match (current, default) {
            (serde_json::Value::Object(c), serde_json::Value::Object(d)) => {
                for (key, current_val) in c {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    
                    if let Some(default_val) = d.get(key) {
                        if current_val != default_val {
                            Self::compute_diff(current_val, default_val, new_path, diff);
                        }
                    } else {
                        diff.insert(new_path, (current_val.clone(), serde_json::Value::Null));
                    }
                }
            }
            _ => {
                if current != default {
                    diff.insert(path, (current.clone(), default.clone()));
                }
            }
        }
    }
}

/// Configuration export formats
#[derive(Debug, Clone, Copy)]
pub enum ConfigFormat {
    Toml,
    Json,
    Yaml,
}

/// Built-in validators
pub struct RequiredFieldsValidator;

impl ConfigValidator for RequiredFieldsValidator {
    fn validate(&self, config: &Config) -> std::result::Result<(), ConfigError> {
        if config.database.url.is_empty() {
            return Err(ConfigError::MissingRequired("database.url".to_string()));
        }
        
        if config.solana.program_id.is_empty() {
            return Err(ConfigError::MissingRequired("solana.program_id".to_string()));
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "required_fields"
    }
}

pub struct ProductionReadinessValidator;

impl ConfigValidator for ProductionReadinessValidator {
    fn validate(&self, config: &Config) -> std::result::Result<(), ConfigError> {
        if config.environment != Environment::Production {
            return Ok(());
        }
        
        // Check production-specific requirements
        if config.features.enable_test_endpoints {
            return Err(ConfigError::InvalidValue(
                "Test endpoints must be disabled in production".to_string()
            ));
        }
        
        if config.features.enable_debug_logging {
            warn!("Debug logging is enabled in production");
        }
        
        if config.security.cors_origins.contains(&"*".to_string()) {
            return Err(ConfigError::InvalidValue(
                "CORS wildcard not allowed in production".to_string()
            ));
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "production_readiness"
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        backups: Vec<(&'static str, Option<String>)>,
    }

    impl EnvRestore {
        fn set(vars: &[(&'static str, Option<&'static str>)]) -> Self {
            let backups = vars
                .iter()
                .map(|(key, _)| (*key, env::var(key).ok()))
                .collect::<Vec<_>>();

            for (key, value) in vars {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }

            Self { backups }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in self.backups.drain(..) {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }
    
    #[test]
    fn test_environment_detection() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore::set(&[
            ("ENVIRONMENT", Some("production")),
            ("ENV", None),
            ("RUST_ENV", None),
        ]);

        assert_eq!(EnvironmentConfigService::detect_environment(), Environment::Production);

        let _restore = EnvRestore::set(&[
            ("ENVIRONMENT", None),
            ("ENV", Some("staging")),
            ("RUST_ENV", None),
        ]);
        assert_eq!(EnvironmentConfigService::detect_environment(), Environment::Staging);
    }
    
    #[tokio::test]
    async fn test_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_service = {
            let _lock = ENV_LOCK.lock().unwrap();
            let _restore = EnvRestore::set(&[
                ("ENVIRONMENT", None),
                ("ENV", None),
                ("RUST_ENV", None),
            ]);
            EnvironmentConfigService::new(temp_dir.path().to_path_buf()).unwrap()
        };
        
        let config = config_service.get_config().await;
        assert_eq!(config.environment, Environment::Development);
        assert_eq!(config.server.port, 8081);
    }
    
    #[tokio::test]
    async fn test_config_override() {
        let temp_dir = TempDir::new().unwrap();
        let config_service = {
            let _lock = ENV_LOCK.lock().unwrap();
            let _restore = EnvRestore::set(&[
                ("ENVIRONMENT", None),
                ("ENV", None),
                ("RUST_ENV", None),
            ]);
            EnvironmentConfigService::new(temp_dir.path().to_path_buf()).unwrap()
        };
        
        config_service.set_override(
            "server.port",
            serde_json::json!(9000)
        ).await.unwrap();
        
        let port: u16 = config_service.get("server.port").await.unwrap();
        assert_eq!(port, 9000);
    }
}
