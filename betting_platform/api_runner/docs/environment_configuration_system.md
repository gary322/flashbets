# Environment Configuration System Documentation

## Overview

The Environment Configuration System provides centralized, type-safe configuration management with validation, hot reloading, and environment-specific overrides for the betting platform API.

## Architecture

### Core Components

1. **EnvironmentConfigService** (`environment_config.rs`)
   - Central service managing all configuration
   - Configuration loading and merging
   - Hot reloading with file watching
   - Runtime override support
   - Export and validation capabilities

2. **Configuration Endpoints** (`environment_config_endpoints.rs`)
   - REST API endpoints for configuration management
   - Admin-only access control
   - Configuration viewing and modification
   - Validation and diff utilities

3. **Configuration Structure**
   - Hierarchical configuration with typed sections
   - Environment-specific overrides
   - Feature flags and runtime toggles

## Configuration Structure

```rust
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
```

### Environment Types

- `Development` - Local development with debug features
- `Staging` - Pre-production testing
- `Production` - Live production environment
- `Test` - Automated testing environment

## Configuration Sources

Configuration is loaded from multiple sources in priority order:

1. **Default Configuration** - Hardcoded defaults in code
2. **Base Configuration File** - `config/config.default.toml`
3. **Environment-Specific File** - `config/config.{environment}.toml`
4. **Environment Variables** - Override specific values
5. **Runtime Overrides** - Dynamic changes via API

## Configuration Files

### Default Configuration (`config/config.default.toml`)

```toml
[server]
host = "127.0.0.1"
port = 8081
workers = 4  # Optional, defaults to CPU count
keep_alive = "75s"
request_timeout = "30s"
body_limit = 10485760  # 10MB

[database]
url = "postgresql://localhost/betting_platform"
max_connections = 100
min_connections = 10
connect_timeout = "30s"
idle_timeout = "10m"
max_lifetime = "30m"
enable_fallback = true

[redis]
url = "redis://localhost:6379"
pool_size = 20
timeout = "5s"
retry_attempts = 3
retry_delay = "100ms"

[solana]
rpc_url = "https://api.devnet.solana.com"
ws_url = "wss://api.devnet.solana.com"
commitment = "confirmed"
program_id = "11111111111111111111111111111111"
request_timeout = "30s"
max_retries = 3
retry_delay = "500ms"

[websocket]
max_connections = 10000
ping_interval = "30s"
pong_timeout = "10s"
message_buffer_size = 1000
broadcast_capacity = 10000

[security]
jwt_secret = "change-me-in-production"
jwt_expiry = "1h"
refresh_token_expiry = "168h"  # 7 days
bcrypt_cost = 12
rate_limit_requests = 100
rate_limit_window = "60s"
cors_origins = ["http://localhost:3000"]

[external_apis.polymarket]
base_url = "https://api.polymarket.com"
ws_url = "wss://ws.polymarket.com"
rate_limit = 10

[external_apis]
timeout = "30s"
max_retries = 3

[features]
enable_mock_services = true
enable_test_endpoints = true
enable_debug_logging = true
enable_metrics = true
enable_tracing = true
enable_circuit_breakers = true
enable_health_checks = true

[monitoring]
health_check_interval = "30s"
metrics_retention = "1h"
log_level = "info"
enable_performance_tracking = true

[performance]
cache_ttl = "5m"
query_timeout = "5s"
max_concurrent_requests = 1000
enable_compression = true
```

### Production Configuration (`config/config.production.toml`)

```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "${DATABASE_URL}"  # Use environment variable
max_connections = 200
min_connections = 50

[redis]
url = "${REDIS_URL}"

[solana]
rpc_url = "https://api.mainnet-beta.solana.com"
ws_url = "wss://api.mainnet-beta.solana.com"
program_id = "${SOLANA_PROGRAM_ID}"

[security]
jwt_secret = "${JWT_SECRET}"
cors_origins = ["https://betting-platform.com"]

[features]
enable_mock_services = false
enable_test_endpoints = false
enable_debug_logging = false

[monitoring]
log_level = "warn"
```

## Environment Variables

Key environment variables for configuration:

```bash
# Environment detection
ENVIRONMENT=production       # or development, staging, test
ENV=production              # Alternative
RUST_ENV=production         # Alternative

# Configuration directory
CONFIG_DIR=/path/to/config  # Default: ./config

# Configuration watching
CONFIG_WATCH_ENABLED=true   # Enable hot reloading

# Direct overrides
PORT=8080
DATABASE_URL=postgresql://user:pass@host/db
REDIS_URL=redis://host:6379
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
SOLANA_PROGRAM_ID=YourProgramId
JWT_SECRET=your-secret-key
MOCK_SERVICES_ENABLED=false
```

## API Endpoints

### Get Configuration
```http
GET /api/config?include_sensitive=false
Authorization: Bearer <admin-token>

Response:
{
  "status": "success",
  "data": {
    "environment": "production",
    "config": {
      "server": { ... },
      "database": { 
        "url": "postgresql://user:[REDACTED]@host/db"
      }
    },
    "overrides": {},
    "sources": {}
  }
}
```

### Get Specific Value
```http
GET /api/config/server.port
Authorization: Bearer <admin-token>

Response:
{
  "status": "success",
  "data": 8080
}
```

### Set Configuration Override
```http
POST /api/config/override
Authorization: Bearer <admin-token>
Content-Type: application/json

{
  "key": "server.workers",
  "value": 8,
  "reason": "Increased for high load"
}
```

### Reload Configuration
```http
POST /api/config/reload
Authorization: Bearer <admin-token>
```

### Export Configuration
```http
GET /api/config/export?format=toml
Authorization: Bearer <admin-token>

Response: TOML formatted configuration
```

### Get Configuration Diff
```http
GET /api/config/diff
Authorization: Bearer <admin-token>

Response:
{
  "status": "success",
  "data": {
    "changes": [
      {
        "key": "server.port",
        "current": 8080,
        "default": 8081
      }
    ],
    "total_changes": 1
  }
}
```

### Validate Configuration
```http
GET /api/config/validate
Authorization: Bearer <admin-token>

Response:
{
  "status": "success",
  "data": {
    "valid": true,
    "results": [
      {
        "component": "database",
        "valid": true,
        "message": null
      }
    ]
  }
}
```

## Usage Examples

### Initialize Configuration Service

```rust
use std::path::PathBuf;
use environment_config::EnvironmentConfigService;

// Initialize with default config directory
let config_dir = PathBuf::from("config");
let config_service = EnvironmentConfigService::new(config_dir)?;

// Enable file watching
let config_arc = Arc::new(config_service);
config_arc.clone().watch_for_changes().await;
```

### Access Configuration Values

```rust
// Get entire configuration
let config = config_service.get_config().await;
println!("Server port: {}", config.server.port);

// Get specific value
let port: u16 = config_service.get("server.port").await?;

// Check feature flag
let mock_enabled = config_service.get::<bool>("features.enable_mock_services").await?;
```

### Runtime Configuration Changes

```rust
// Set override
config_service.set_override(
    "server.workers",
    serde_json::json!(8)
).await?;

// Reload from disk
config_service.reload().await?;

// Export current configuration
let toml_config = config_service.export(ConfigFormat::Toml).await?;
```

## Configuration Validation

### Built-in Validators

1. **RequiredFieldsValidator**
   - Ensures critical fields are present
   - Validates database URL, program ID, etc.

2. **ProductionReadinessValidator**
   - Checks production-specific requirements
   - Validates security settings
   - Ensures test features are disabled

### Custom Validators

```rust
use environment_config::{ConfigValidator, Config, ConfigError};

struct MyValidator;

impl ConfigValidator for MyValidator {
    fn validate(&self, config: &Config) -> Result<(), ConfigError> {
        if config.server.port < 1024 && config.environment == Environment::Production {
            return Err(ConfigError::InvalidValue(
                "Production must use port >= 1024".to_string()
            ));
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "my_validator"
    }
}

// Register validator
config_service.register_validator(Box::new(MyValidator));
```

## Hot Reloading

The configuration service supports automatic reloading when files change:

1. File watcher monitors configuration directory
2. Changes trigger automatic reload
3. Validation runs before applying changes
4. Failed reloads keep existing configuration

```rust
// Enable watching (usually done at startup)
config_service.watch_for_changes().await;

// Files are automatically reloaded on change
// Check logs for reload status
```

## Security Considerations

1. **Sensitive Data Protection**
   - Passwords and secrets are redacted in API responses
   - Use `include_sensitive=true` to view (admin only)
   - Environment variables for production secrets

2. **Access Control**
   - All configuration endpoints require admin role
   - Changes are logged with user information
   - Audit trail for configuration modifications

3. **Production Safety**
   - Validation prevents dangerous production configs
   - Some settings cannot be changed at runtime
   - Rollback capability through reload

## Best Practices

1. **Environment-Specific Files**
   - Keep defaults in `config.default.toml`
   - Override only necessary values per environment
   - Use environment variables for secrets

2. **Configuration Structure**
   - Group related settings together
   - Use consistent naming conventions
   - Document non-obvious settings

3. **Deployment**
   - Version control configuration files
   - Exclude sensitive production configs
   - Use CI/CD for configuration deployment

4. **Monitoring**
   - Watch configuration reload logs
   - Monitor validation failures
   - Alert on configuration errors

## Troubleshooting

### Common Issues

1. **Configuration not loading**
   - Check CONFIG_DIR environment variable
   - Verify file permissions
   - Check TOML syntax errors

2. **Hot reload not working**
   - Ensure CONFIG_WATCH_ENABLED=true
   - Check file watcher logs
   - Verify filesystem events are supported

3. **Validation failures**
   - Review validation error messages
   - Check environment-specific requirements
   - Ensure all required fields are set

### Debug Mode

Enable debug logging for configuration:

```bash
RUST_LOG=betting_platform_api::environment_config=debug cargo run
```

## Migration Guide

### From Environment Variables

```bash
# Old approach
export SERVER_PORT=8080
export DB_URL=postgresql://...

# New approach with config file
[server]
port = 8080

[database]
url = "postgresql://..."
```

### From Hardcoded Values

```rust
// Old
const PORT: u16 = 8080;

// New
let port = config_service.get::<u16>("server.port").await?;
```

## Performance Considerations

1. **Caching**
   - Configuration is cached in memory
   - Minimal overhead for access
   - Reload is atomic

2. **File Watching**
   - Low overhead using OS notifications
   - Debounced to prevent rapid reloads
   - Can be disabled in production

3. **Validation**
   - Runs only on load/reload
   - Fast path for common validations
   - Async validators supported
