# Environment Variables Documentation

This document lists all environment variables used by the Betting Platform API, organized by category.

## Table of Contents
- [Core Configuration](#core-configuration)
- [Server Configuration](#server-configuration)
- [Solana Configuration](#solana-configuration)
- [Database Configuration](#database-configuration)
- [Redis & Caching](#redis--caching)
- [Security & Authentication](#security--authentication)
- [External Integrations](#external-integrations)
- [Queue System](#queue-system)
- [Rate Limiting](#rate-limiting)
- [Auto-Funding](#auto-funding)
- [Logging & Monitoring](#logging--monitoring)

## Core Configuration

### `RPC_URL`
- **Description**: Solana RPC endpoint URL
- **Default**: `http://localhost:8899`
- **Example**: `https://api.mainnet-beta.solana.com`
- **Required**: No

### `PROGRAM_ID`
- **Description**: Solana program ID for the betting platform smart contract
- **Default**: `HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca`
- **Example**: `BettingProgramV2111111111111111111111111111`
- **Required**: No

## Server Configuration

### `SERVER_HOST`
- **Description**: Host address for the API server
- **Default**: `127.0.0.1`
- **Example**: `0.0.0.0`
- **Required**: No

### `SERVER_PORT`
- **Description**: Port number for the API server
- **Default**: `8081`
- **Example**: `3000`
- **Required**: No

### `CORS_ORIGINS`
- **Description**: Comma-separated list of allowed CORS origins
- **Default**: `http://localhost:3000,http://localhost:3001`
- **Example**: `https://app.example.com,https://beta.example.com`
- **Required**: No

## Solana Configuration

### `SOLANA_RPC_URL`
- **Description**: Primary Solana RPC endpoint (alternative to RPC_URL)
- **Default**: `http://localhost:8899`
- **Example**: `https://api.mainnet-beta.solana.com`
- **Required**: No

### `SOLANA_WS_URL`
- **Description**: Solana WebSocket endpoint for real-time updates
- **Default**: `ws://localhost:8900`
- **Example**: `wss://api.mainnet-beta.solana.com`
- **Required**: No

### `SOLANA_COMMITMENT`
- **Description**: Solana commitment level for transactions
- **Default**: `confirmed`
- **Options**: `processed`, `confirmed`, `finalized`
- **Required**: No

## Database Configuration

### `DATABASE_URL`
- **Description**: PostgreSQL connection string
- **Default**: `postgresql://betting_user:betting_pass@localhost/betting_platform`
- **Example**: `postgresql://user:pass@db.example.com:5432/mydb?sslmode=require`
- **Required**: No

### `DB_MAX_CONNECTIONS`
- **Description**: Maximum number of database connections in the pool
- **Default**: `100`
- **Example**: `50`
- **Required**: No

### `DB_MIN_CONNECTIONS`
- **Description**: Minimum number of database connections in the pool
- **Default**: `10`
- **Example**: `5`
- **Required**: No

### `DB_CONNECTION_TIMEOUT`
- **Description**: Database connection timeout in seconds
- **Default**: `30`
- **Example**: `60`
- **Required**: No

## Redis & Caching

### `REDIS_URL`
- **Description**: Redis connection URL for caching and queues
- **Default**: `redis://localhost:6379`
- **Example**: `redis://user:pass@redis.example.com:6379/0`
- **Required**: No

### `CACHE_ENABLED`
- **Description**: Enable/disable caching system
- **Default**: `true`
- **Options**: `true`, `false`
- **Required**: No

### `CACHE_TTL`
- **Description**: Default cache time-to-live in seconds
- **Default**: `300` (5 minutes)
- **Example**: `600`
- **Required**: No

## Security & Authentication

### `JWT_SECRET`
- **Description**: Secret key for JWT token signing (CRITICAL - use strong random value)
- **Default**: None - MUST be set in production
- **Example**: `your-256-bit-secret-key-here-change-this-in-production`
- **Required**: **Yes** in production

### `JWT_EXPIRATION_HOURS`
- **Description**: JWT token expiration time in hours
- **Default**: `24`
- **Example**: `72`
- **Required**: No

### `BCRYPT_COST`
- **Description**: BCrypt hashing cost factor
- **Default**: `12`
- **Range**: `4-31` (higher = more secure but slower)
- **Required**: No

### `SECURITY_LOG_TO_FILE`
- **Description**: Enable security event logging to file
- **Default**: `true`
- **Options**: `true`, `false`
- **Required**: No

### `SECURITY_LOG_PATH`
- **Description**: Path to security log file
- **Default**: `logs/security.log`
- **Example**: `/var/log/betting-platform/security.log`
- **Required**: No

### `SECURITY_LOG_MAX_SIZE`
- **Description**: Maximum security log file size in bytes
- **Default**: `104857600` (100MB)
- **Example**: `52428800` (50MB)
- **Required**: No

### `SECURITY_LOG_RETENTION_DAYS`
- **Description**: Number of days to retain security logs
- **Default**: `90`
- **Example**: `30`
- **Required**: No

### `SECURITY_ALERT_WEBHOOK`
- **Description**: Webhook URL for critical security alerts
- **Default**: None
- **Example**: `https://hooks.slack.com/services/xxx/yyy/zzz`
- **Required**: No

## External Integrations

### Polymarket Integration

### `POLYMARKET_ENABLED`
- **Description**: Enable Polymarket integration
- **Default**: `true`
- **Options**: `true`, `false`
- **Required**: No

### `POLYMARKET_API_KEY`
- **Description**: Polymarket API key for authenticated endpoints
- **Default**: None
- **Example**: `pk_live_xxx`
- **Required**: No (but needed for full functionality)

### `POLYMARKET_WEBHOOK_SECRET`
- **Description**: Secret for validating Polymarket webhooks
- **Default**: None
- **Example**: `whsec_xxx`
- **Required**: No

### Kalshi Integration

### `KALSHI_ENABLED`
- **Description**: Enable Kalshi integration
- **Default**: `true`
- **Options**: `true`, `false`
- **Required**: No

### `KALSHI_API_KEY`
- **Description**: Kalshi API key
- **Default**: None
- **Example**: `kalshi_xxx`
- **Required**: No

### `KALSHI_API_SECRET`
- **Description**: Kalshi API secret
- **Default**: None
- **Example**: `kalshi_secret_xxx`
- **Required**: No

### Market Sync Configuration

### `SYNC_INTERVAL_SECONDS`
- **Description**: Interval for syncing external market data
- **Default**: `60`
- **Example**: `300` (5 minutes)
- **Required**: No

### `MAX_PRICE_DEVIATION`
- **Description**: Maximum allowed price deviation for market sync
- **Default**: `0.05` (5%)
- **Example**: `0.1` (10%)
- **Required**: No

### `MIN_LIQUIDITY_USD`
- **Description**: Minimum liquidity required for market sync
- **Default**: `10000` ($10,000)
- **Example**: `50000`
- **Required**: No

### Polygon Configuration

### `POLYGON_RPC_URL`
- **Description**: Polygon network RPC endpoint
- **Default**: `https://polygon-rpc.com`
- **Example**: `https://polygon-mainnet.infura.io/v3/YOUR-PROJECT-ID`
- **Required**: No

### `POLYGON_API_KEY`
- **Description**: API key for Polygon services
- **Default**: None
- **Example**: `your-polygon-api-key`
- **Required**: No

## Queue System

### `QUEUE_ENABLED`
- **Description**: Enable message queue system
- **Default**: `true`
- **Options**: `true`, `false`
- **Required**: No

### `QUEUE_WORKERS`
- **Description**: Number of queue worker threads
- **Default**: `4`
- **Example**: `8`
- **Required**: No

### `WEBHOOK_SECRET`
- **Description**: Secret for signing outgoing webhooks
- **Default**: `default-webhook-secret`
- **Example**: `your-webhook-secret-key`
- **Required**: No (but should be set in production)

## Rate Limiting

### `RATE_LIMIT_GLOBAL_RPS`
- **Description**: Global requests per second limit
- **Default**: `1000`
- **Example**: `5000`
- **Required**: No

### `RATE_LIMIT_PER_IP_RPS`
- **Description**: Per-IP requests per second limit
- **Default**: `100`
- **Example**: `50`
- **Required**: No

### `RATE_LIMIT_GLOBAL_BURST`
- **Description**: Global burst capacity
- **Default**: `2000`
- **Example**: `10000`
- **Required**: No

### `RATE_LIMIT_IP_BURST`
- **Description**: Per-IP burst capacity
- **Default**: `200`
- **Example**: `100`
- **Required**: No

## Auto-Funding

### `ENABLE_AUTO_FUNDING`
- **Description**: Enable automatic wallet funding for demo accounts
- **Default**: `false`
- **Options**: `true`, `false`
- **Required**: No

### `FUNDING_SOURCE_KEY`
- **Description**: Private key for funding source wallet (base58 encoded)
- **Default**: None
- **Example**: `5KJxxxxx...` (keep secret!)
- **Required**: Only if auto-funding is enabled

## Logging & Monitoring

### `LOG_LEVEL`
- **Description**: Application log level
- **Default**: `info`
- **Options**: `trace`, `debug`, `info`, `warn`, `error`
- **Required**: No

### `RUST_LOG`
- **Description**: Rust logging configuration (overrides LOG_LEVEL)
- **Default**: None
- **Example**: `betting_platform_api=debug,tower_http=debug`
- **Required**: No

## Production Checklist

When deploying to production, ensure these critical variables are set:

1. **`JWT_SECRET`** - Generate a strong random secret
2. **`DATABASE_URL`** - Point to production database with SSL
3. **`REDIS_URL`** - Point to production Redis instance
4. **`PROGRAM_ID`** - Use your deployed Solana program
5. **`RPC_URL`** - Use a reliable Solana RPC provider
6. **`WEBHOOK_SECRET`** - Set a strong webhook secret
7. **`SECURITY_ALERT_WEBHOOK`** - Configure security alerts

## Example `.env` File

```bash
# Core
RPC_URL=https://api.mainnet-beta.solana.com
PROGRAM_ID=YourProgramID11111111111111111111111111111

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
CORS_ORIGINS=https://app.yourdomain.com

# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/betting_platform
DB_MAX_CONNECTIONS=50

# Redis
REDIS_URL=redis://localhost:6379
CACHE_ENABLED=true

# Security (CHANGE THESE!)
JWT_SECRET=your-very-long-random-secret-key-minimum-256-bits
SECURITY_ALERT_WEBHOOK=https://hooks.slack.com/services/xxx/yyy/zzz

# External APIs
POLYMARKET_ENABLED=true
POLYMARKET_API_KEY=pk_live_xxx

# Queue
QUEUE_ENABLED=true
QUEUE_WORKERS=4

# Logging
LOG_LEVEL=info
```

## Security Notes

1. **Never commit `.env` files to version control**
2. Use environment-specific files: `.env.development`, `.env.production`
3. Rotate secrets regularly, especially `JWT_SECRET`
4. Use secret management services in production (AWS Secrets Manager, HashiCorp Vault, etc.)
5. Ensure database URLs use SSL in production
6. Monitor security logs regularly