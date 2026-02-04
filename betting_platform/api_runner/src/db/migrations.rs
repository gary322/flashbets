//! Database migrations

use anyhow::{Result, Context};
use deadpool_postgres::{Object, GenericClient};

/// Run all database migrations
pub async fn run_migrations(conn: &mut Object) -> Result<()> {
    // Create migrations table if it doesn't exist
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS migrations (
            id SERIAL PRIMARY KEY,
            version INTEGER NOT NULL UNIQUE,
            name VARCHAR(255) NOT NULL,
            applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        )
        "#,
        &[],
    ).await.context("Failed to create migrations table")?;
    
    // Get applied migrations
    let applied_migrations = get_applied_migrations(conn).await?;
    
    // Apply each migration in order
    let migrations = get_migrations();
    for migration in migrations {
        if !applied_migrations.contains(&migration.version) {
            apply_migration(conn, &migration).await?;
        }
    }
    
    Ok(())
}

/// Migration definition
struct Migration {
    version: i32,
    name: &'static str,
    up: &'static str,
}

/// Get all migrations
fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            name: "create_users_table",
            up: r#"
                CREATE TABLE users (
                    id BIGSERIAL PRIMARY KEY,
                    wallet_address VARCHAR(66) NOT NULL UNIQUE,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    last_login TIMESTAMP WITH TIME ZONE,
                    total_volume BIGINT DEFAULT 0,
                    total_trades INTEGER DEFAULT 0,
                    is_active BOOLEAN DEFAULT true
                );
                
                CREATE INDEX idx_users_wallet ON users(wallet_address);
                CREATE INDEX idx_users_created_at ON users(created_at);
            "#,
        },
        Migration {
            version: 2,
            name: "create_markets_table",
            up: r#"
                CREATE TABLE markets (
                    id BIGSERIAL PRIMARY KEY,
                    market_id VARCHAR(255) NOT NULL UNIQUE,
                    chain VARCHAR(50) NOT NULL,
                    title TEXT NOT NULL,
                    description TEXT,
                    creator VARCHAR(66) NOT NULL,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    end_time TIMESTAMP WITH TIME ZONE NOT NULL,
                    resolved BOOLEAN DEFAULT false,
                    winning_outcome SMALLINT,
                    total_volume BIGINT DEFAULT 0,
                    total_liquidity BIGINT DEFAULT 0,
                    metadata JSONB DEFAULT '{}'::jsonb
                );
                
                CREATE INDEX idx_markets_market_id ON markets(market_id);
                CREATE INDEX idx_markets_chain ON markets(chain);
                CREATE INDEX idx_markets_creator ON markets(creator);
                CREATE INDEX idx_markets_end_time ON markets(end_time);
                CREATE INDEX idx_markets_resolved ON markets(resolved);
            "#,
        },
        Migration {
            version: 3,
            name: "create_positions_table",
            up: r#"
                CREATE TABLE positions (
                    id BIGSERIAL PRIMARY KEY,
                    position_id VARCHAR(255) NOT NULL UNIQUE,
                    user_id BIGINT NOT NULL REFERENCES users(id),
                    market_id BIGINT NOT NULL REFERENCES markets(id),
                    outcome SMALLINT NOT NULL,
                    amount BIGINT NOT NULL,
                    leverage SMALLINT DEFAULT 1,
                    entry_price DOUBLE PRECISION NOT NULL,
                    exit_price DOUBLE PRECISION,
                    pnl BIGINT,
                    status VARCHAR(50) NOT NULL,
                    opened_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    closed_at TIMESTAMP WITH TIME ZONE,
                    metadata JSONB DEFAULT '{}'::jsonb
                );
                
                CREATE INDEX idx_positions_position_id ON positions(position_id);
                CREATE INDEX idx_positions_user_id ON positions(user_id);
                CREATE INDEX idx_positions_market_id ON positions(market_id);
                CREATE INDEX idx_positions_status ON positions(status);
                CREATE INDEX idx_positions_opened_at ON positions(opened_at);
            "#,
        },
        Migration {
            version: 4,
            name: "create_trades_table",
            up: r#"
                CREATE TABLE trades (
                    id BIGSERIAL PRIMARY KEY,
                    trade_id VARCHAR(255) NOT NULL UNIQUE,
                    user_id BIGINT NOT NULL REFERENCES users(id),
                    market_id BIGINT NOT NULL REFERENCES markets(id),
                    position_id BIGINT REFERENCES positions(id),
                    trade_type VARCHAR(50) NOT NULL,
                    outcome SMALLINT NOT NULL,
                    amount BIGINT NOT NULL,
                    price DOUBLE PRECISION NOT NULL,
                    fee BIGINT DEFAULT 0,
                    signature VARCHAR(255) NOT NULL,
                    status VARCHAR(50) NOT NULL,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    confirmed_at TIMESTAMP WITH TIME ZONE,
                    metadata JSONB DEFAULT '{}'::jsonb
                );
                
                CREATE INDEX idx_trades_trade_id ON trades(trade_id);
                CREATE INDEX idx_trades_user_id ON trades(user_id);
                CREATE INDEX idx_trades_market_id ON trades(market_id);
                CREATE INDEX idx_trades_position_id ON trades(position_id);
                CREATE INDEX idx_trades_signature ON trades(signature);
                CREATE INDEX idx_trades_created_at ON trades(created_at);
            "#,
        },
        Migration {
            version: 5,
            name: "create_settlements_table",
            up: r#"
                CREATE TABLE settlements (
                    id BIGSERIAL PRIMARY KEY,
                    market_id BIGINT NOT NULL REFERENCES markets(id),
                    settled_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    winning_outcome SMALLINT NOT NULL,
                    settlement_price DOUBLE PRECISION NOT NULL,
                    total_payout BIGINT NOT NULL,
                    oracle_source VARCHAR(255) NOT NULL,
                    oracle_data JSONB DEFAULT '{}'::jsonb,
                    signature VARCHAR(255) NOT NULL UNIQUE
                );
                
                CREATE INDEX idx_settlements_market_id ON settlements(market_id);
                CREATE INDEX idx_settlements_settled_at ON settlements(settled_at);
            "#,
        },
        Migration {
            version: 6,
            name: "create_audit_logs_table",
            up: r#"
                CREATE TABLE audit_logs (
                    id BIGSERIAL PRIMARY KEY,
                    user_id BIGINT REFERENCES users(id),
                    action VARCHAR(100) NOT NULL,
                    entity_type VARCHAR(50) NOT NULL,
                    entity_id VARCHAR(255) NOT NULL,
                    ip_address INET,
                    user_agent TEXT,
                    request_id VARCHAR(255) NOT NULL,
                    changes JSONB DEFAULT '{}'::jsonb,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                );
                
                CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
                CREATE INDEX idx_audit_logs_action ON audit_logs(action);
                CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id);
                CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);
            "#,
        },
        Migration {
            version: 7,
            name: "create_api_keys_table",
            up: r#"
                CREATE TABLE api_keys (
                    id BIGSERIAL PRIMARY KEY,
                    user_id BIGINT NOT NULL REFERENCES users(id),
                    key_hash VARCHAR(255) NOT NULL UNIQUE,
                    name VARCHAR(255) NOT NULL,
                    permissions TEXT[] DEFAULT ARRAY[]::TEXT[],
                    rate_limit INTEGER DEFAULT 1000,
                    expires_at TIMESTAMP WITH TIME ZONE,
                    last_used_at TIMESTAMP WITH TIME ZONE,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
                    is_active BOOLEAN DEFAULT true
                );
                
                CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
                CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
                CREATE INDEX idx_api_keys_expires_at ON api_keys(expires_at);
            "#,
        },
        Migration {
            version: 8,
            name: "create_price_history_table",
            up: r#"
                CREATE TABLE price_history (
                    id BIGSERIAL PRIMARY KEY,
                    market_id BIGINT NOT NULL REFERENCES markets(id),
                    outcome SMALLINT NOT NULL,
                    price DOUBLE PRECISION NOT NULL,
                    volume BIGINT DEFAULT 0,
                    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                );
                
                CREATE INDEX idx_price_history_market_outcome ON price_history(market_id, outcome);
                CREATE INDEX idx_price_history_timestamp ON price_history(timestamp);
                
                -- Create hypertable for time-series data (if using TimescaleDB)
                -- SELECT create_hypertable('price_history', 'timestamp');
            "#,
        },
        Migration {
            version: 9,
            name: "create_quantum_settlements_table",
            up: r#"
                CREATE TABLE IF NOT EXISTS quantum_settlements (
                    id BIGSERIAL PRIMARY KEY,
                    position_id VARCHAR(255) NOT NULL UNIQUE,
                    wallet VARCHAR(255) NOT NULL,
                    market_id BIGINT NOT NULL,
                    outcome SMALLINT NOT NULL,
                    amount BIGINT NOT NULL,
                    pnl BIGINT NOT NULL,
                    quantum_bonus DOUBLE PRECISION NOT NULL,
                    coherence_multiplier DOUBLE PRECISION DEFAULT 1.0,
                    settlement_time TIMESTAMP WITH TIME ZONE NOT NULL,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                );
                
                CREATE INDEX idx_quantum_settlements_wallet ON quantum_settlements(wallet);
                CREATE INDEX idx_quantum_settlements_market ON quantum_settlements(market_id);
                CREATE INDEX idx_quantum_settlements_time ON quantum_settlements(settlement_time);
                CREATE INDEX idx_quantum_settlements_position ON quantum_settlements(position_id);
            "#,
        },
    ]
}

/// Get list of applied migration versions
async fn get_applied_migrations(conn: &Object) -> Result<Vec<i32>> {
    let rows = conn.query(
        "SELECT version FROM migrations ORDER BY version",
        &[],
    ).await.context("Failed to query applied migrations")?;
    
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

/// Apply a single migration
async fn apply_migration(conn: &mut Object, migration: &Migration) -> Result<()> {
    println!("Applying migration {}: {}", migration.version, migration.name);
    
    // Start transaction
    let txn = conn.transaction().await
        .context("Failed to start transaction")?;
    
    // Execute migration
    txn.batch_execute(migration.up).await
        .context(format!("Failed to execute migration {}", migration.version))?;
    
    // Record migration
    txn.execute(
        "INSERT INTO migrations (version, name) VALUES ($1, $2)",
        &[&migration.version, &migration.name],
    ).await.context("Failed to record migration")?;
    
    // Commit
    txn.commit().await
        .context("Failed to commit migration")?;
    
    println!("Applied migration {}: {}", migration.version, migration.name);
    Ok(())
}