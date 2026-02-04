//! Production-ready database migrations with comprehensive schema

use anyhow::{Result, Context};
use deadpool_postgres::{Object, GenericClient};

/// Run all database migrations
pub async fn run_migrations(conn: &mut Object) -> Result<()> {
    // Create migrations table if it doesn't exist
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            id SERIAL PRIMARY KEY,
            version INTEGER NOT NULL UNIQUE,
            name VARCHAR(255) NOT NULL,
            checksum VARCHAR(64) NOT NULL,
            executed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
            execution_time_ms INTEGER
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
            let start = std::time::Instant::now();
            apply_migration(conn, &migration).await?;
            let elapsed = start.elapsed().as_millis() as i32;
            
            // Update execution time
            conn.execute(
                "UPDATE schema_migrations SET execution_time_ms = $1 WHERE version = $2",
                &[&elapsed, &migration.version],
            ).await?;
        }
    }
    
    Ok(())
}

/// Migration definition
struct Migration {
    version: i32,
    name: &'static str,
    up: &'static str,
    checksum: &'static str,
}

/// Get all migrations
fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            name: "create_extensions",
            checksum: "a1b2c3d4",
            up: r#"
                -- Enable required extensions
                CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
                CREATE EXTENSION IF NOT EXISTS "pgcrypto";
                CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";
            "#,
        },
        Migration {
            version: 2,
            name: "create_custom_types",
            checksum: "e5f6g7h8",
            up: r#"
                -- Create custom types for type safety
                DO $$ BEGIN
                    CREATE TYPE market_status AS ENUM ('active', 'closed', 'resolved', 'disputed');
                EXCEPTION
                    WHEN duplicate_object THEN null;
                END $$;
                
                DO $$ BEGIN
                    CREATE TYPE market_type AS ENUM ('binary', 'multiple', 'scalar', 'quantum');
                EXCEPTION
                    WHEN duplicate_object THEN null;
                END $$;
                
                DO $$ BEGIN
                    CREATE TYPE position_status AS ENUM ('open', 'closed', 'liquidated', 'settled');
                EXCEPTION
                    WHEN duplicate_object THEN null;
                END $$;
                
                DO $$ BEGIN
                    CREATE TYPE trade_side AS ENUM ('yes', 'no', 'long', 'short');
                EXCEPTION
                    WHEN duplicate_object THEN null;
                END $$;
                
                DO $$ BEGIN
                    CREATE TYPE settlement_status AS ENUM ('pending', 'processing', 'completed', 'failed');
                EXCEPTION
                    WHEN duplicate_object THEN null;
                END $$;
            "#,
        },
        Migration {
            version: 3,
            name: "create_markets_table",
            checksum: "i9j0k1l2",
            up: r#"
                -- Markets table with comprehensive fields
                CREATE TABLE IF NOT EXISTS markets (
                    id BIGSERIAL PRIMARY KEY,
                    market_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    question TEXT NOT NULL,
                    description TEXT,
                    outcomes JSONB NOT NULL,
                    market_type market_type NOT NULL DEFAULT 'binary',
                    status market_status NOT NULL DEFAULT 'active',
                    end_time TIMESTAMP WITH TIME ZONE NOT NULL,
                    resolution_time TIMESTAMP WITH TIME ZONE,
                    resolution_outcome INTEGER,
                    total_volume NUMERIC(20, 0) DEFAULT 0,
                    total_liquidity NUMERIC(20, 0) DEFAULT 0,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    metadata JSONB DEFAULT '{}',
                    CONSTRAINT check_end_time CHECK (end_time > created_at)
                );
                
                -- Create indexes
                CREATE INDEX IF NOT EXISTS idx_markets_status ON markets(status);
                CREATE INDEX IF NOT EXISTS idx_markets_end_time ON markets(end_time);
                CREATE INDEX IF NOT EXISTS idx_markets_created_at ON markets(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_markets_volume ON markets(total_volume DESC);
            "#,
        },
        Migration {
            version: 4,
            name: "create_positions_table",
            checksum: "m3n4o5p6",
            up: r#"
                -- Positions table with leverage support
                CREATE TABLE IF NOT EXISTS positions (
                    id BIGSERIAL PRIMARY KEY,
                    position_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    wallet_address TEXT NOT NULL,
                    market_id BIGINT REFERENCES markets(id) ON DELETE CASCADE,
                    outcome INTEGER NOT NULL,
                    side trade_side NOT NULL,
                    amount NUMERIC(20, 0) NOT NULL,
                    leverage INTEGER DEFAULT 1,
                    entry_price NUMERIC(10, 4) NOT NULL,
                    exit_price NUMERIC(10, 4),
                    margin_used NUMERIC(20, 0),
                    pnl NUMERIC(20, 0),
                    status position_status NOT NULL DEFAULT 'open',
                    opened_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    closed_at TIMESTAMP WITH TIME ZONE,
                    liquidation_price NUMERIC(10, 4)
                );
                
                -- Create indexes
                CREATE INDEX IF NOT EXISTS idx_positions_wallet ON positions(wallet_address);
                CREATE INDEX IF NOT EXISTS idx_positions_market ON positions(market_id);
                CREATE INDEX IF NOT EXISTS idx_positions_status ON positions(status);
                CREATE INDEX IF NOT EXISTS idx_positions_opened_at ON positions(opened_at DESC);
                CREATE INDEX IF NOT EXISTS idx_positions_pnl ON positions(pnl DESC) WHERE status = 'closed';
            "#,
        },
        Migration {
            version: 5,
            name: "create_trades_table",
            checksum: "q7r8s9t0",
            up: r#"
                -- Trades table for transaction history
                CREATE TABLE IF NOT EXISTS trades (
                    id BIGSERIAL PRIMARY KEY,
                    trade_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    position_id UUID REFERENCES positions(position_id) ON DELETE CASCADE,
                    wallet_address TEXT NOT NULL,
                    market_id BIGINT REFERENCES markets(id) ON DELETE CASCADE,
                    outcome INTEGER NOT NULL,
                    side trade_side NOT NULL,
                    amount NUMERIC(20, 0) NOT NULL,
                    price NUMERIC(10, 4) NOT NULL,
                    fee NUMERIC(20, 0) DEFAULT 0,
                    signature TEXT,
                    executed_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );
                
                -- Create indexes
                CREATE INDEX IF NOT EXISTS idx_trades_wallet ON trades(wallet_address);
                CREATE INDEX IF NOT EXISTS idx_trades_market ON trades(market_id);
                CREATE INDEX IF NOT EXISTS idx_trades_executed_at ON trades(executed_at DESC);
                CREATE INDEX IF NOT EXISTS idx_trades_composite ON trades(market_id, executed_at DESC);
            "#,
        },
        Migration {
            version: 6,
            name: "create_user_wallets_table",
            checksum: "u1v2w3x4",
            up: r#"
                -- User wallets with aggregated stats
                CREATE TABLE IF NOT EXISTS user_wallets (
                    id BIGSERIAL PRIMARY KEY,
                    wallet_address TEXT UNIQUE NOT NULL,
                    total_volume NUMERIC(20, 0) DEFAULT 0,
                    total_pnl NUMERIC(20, 0) DEFAULT 0,
                    win_rate NUMERIC(5, 2) DEFAULT 0,
                    positions_opened INTEGER DEFAULT 0,
                    positions_won INTEGER DEFAULT 0,
                    last_active TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    metadata JSONB DEFAULT '{}'
                );
                
                -- Create indexes
                CREATE INDEX IF NOT EXISTS idx_user_wallets_address ON user_wallets(wallet_address);
                CREATE INDEX IF NOT EXISTS idx_user_wallets_volume ON user_wallets(total_volume DESC);
                CREATE INDEX IF NOT EXISTS idx_user_wallets_active ON user_wallets(last_active DESC);
            "#,
        },
        Migration {
            version: 7,
            name: "create_liquidity_pools_table",
            checksum: "y5z6a7b8",
            up: r#"
                -- Liquidity pools for AMM
                CREATE TABLE IF NOT EXISTS liquidity_pools (
                    id BIGSERIAL PRIMARY KEY,
                    pool_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    market_id BIGINT REFERENCES markets(id) ON DELETE CASCADE,
                    total_liquidity NUMERIC(20, 0) DEFAULT 0,
                    yes_liquidity NUMERIC(20, 0) DEFAULT 0,
                    no_liquidity NUMERIC(20, 0) DEFAULT 0,
                    fee_rate NUMERIC(5, 4) DEFAULT 0.003,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );
                
                CREATE INDEX IF NOT EXISTS idx_liquidity_pools_market ON liquidity_pools(market_id);
            "#,
        },
        Migration {
            version: 8,
            name: "create_verses_table",
            checksum: "c9d0e1f2",
            up: r#"
                -- Verses catalog
                CREATE TABLE IF NOT EXISTS verses (
                    id BIGSERIAL PRIMARY KEY,
                    verse_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    category TEXT NOT NULL,
                    subcategory TEXT,
                    text TEXT NOT NULL,
                    author TEXT,
                    relevance_score NUMERIC(3, 2) DEFAULT 0.5,
                    usage_count INTEGER DEFAULT 0,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );
                
                CREATE INDEX IF NOT EXISTS idx_verses_category ON verses(category);
                CREATE INDEX IF NOT EXISTS idx_verses_relevance ON verses(relevance_score DESC);
            "#,
        },
        Migration {
            version: 9,
            name: "create_quantum_positions_table",
            checksum: "g3h4i5j6",
            up: r#"
                -- Quantum positions with superposition
                CREATE TABLE IF NOT EXISTS quantum_positions (
                    id BIGSERIAL PRIMARY KEY,
                    quantum_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    position_id UUID REFERENCES positions(position_id) ON DELETE CASCADE,
                    wallet_address TEXT NOT NULL,
                    market_id BIGINT REFERENCES markets(id) ON DELETE CASCADE,
                    quantum_states JSONB NOT NULL,
                    entanglement_level INTEGER DEFAULT 1,
                    superposition_weights JSONB NOT NULL,
                    collapsed BOOLEAN DEFAULT FALSE,
                    collapsed_outcome INTEGER,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    collapsed_at TIMESTAMP WITH TIME ZONE
                );
                
                CREATE INDEX IF NOT EXISTS idx_quantum_positions_wallet ON quantum_positions(wallet_address);
                CREATE INDEX IF NOT EXISTS idx_quantum_positions_market ON quantum_positions(market_id);
            "#,
        },
        Migration {
            version: 10,
            name: "create_settlements_table",
            checksum: "k7l8m9n0",
            up: r#"
                -- Settlements with enhanced tracking
                CREATE TABLE IF NOT EXISTS settlements (
                    id BIGSERIAL PRIMARY KEY,
                    settlement_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    market_id BIGINT REFERENCES markets(id) ON DELETE CASCADE,
                    position_id UUID REFERENCES positions(position_id) ON DELETE CASCADE,
                    wallet_address TEXT NOT NULL,
                    amount NUMERIC(20, 0) NOT NULL,
                    status settlement_status NOT NULL DEFAULT 'pending',
                    signature TEXT,
                    error_message TEXT,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    processed_at TIMESTAMP WITH TIME ZONE
                );
                
                CREATE INDEX IF NOT EXISTS idx_settlements_wallet ON settlements(wallet_address);
                CREATE INDEX IF NOT EXISTS idx_settlements_status ON settlements(status);
                CREATE INDEX IF NOT EXISTS idx_settlements_created_at ON settlements(created_at DESC);
            "#,
        },
        Migration {
            version: 11,
            name: "create_audit_logs_table",
            checksum: "o1p2q3r4",
            up: r#"
                -- Comprehensive audit trail
                CREATE TABLE IF NOT EXISTS audit_logs (
                    id BIGSERIAL PRIMARY KEY,
                    event_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
                    event_type TEXT NOT NULL,
                    entity_type TEXT NOT NULL,
                    entity_id TEXT NOT NULL,
                    wallet_address TEXT,
                    action TEXT NOT NULL,
                    details JSONB DEFAULT '{}',
                    ip_address INET,
                    user_agent TEXT,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );
                
                CREATE INDEX IF NOT EXISTS idx_audit_logs_wallet ON audit_logs(wallet_address);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_audit_logs_event_type ON audit_logs(event_type);
            "#,
        },
        Migration {
            version: 12,
            name: "create_triggers_and_functions",
            checksum: "s5t6u7v8",
            up: r#"
                -- Create updated_at trigger function
                CREATE OR REPLACE FUNCTION update_updated_at_column()
                RETURNS TRIGGER AS $$
                BEGIN
                    NEW.updated_at = CURRENT_TIMESTAMP;
                    RETURN NEW;
                END;
                $$ language 'plpgsql';
                
                -- Apply triggers
                DROP TRIGGER IF EXISTS update_markets_updated_at ON markets;
                CREATE TRIGGER update_markets_updated_at 
                    BEFORE UPDATE ON markets
                    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
                
                DROP TRIGGER IF EXISTS update_liquidity_pools_updated_at ON liquidity_pools;
                CREATE TRIGGER update_liquidity_pools_updated_at 
                    BEFORE UPDATE ON liquidity_pools
                    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
            "#,
        },
        Migration {
            version: 13,
            name: "create_market_statistics_view",
            checksum: "w9x0y1z2",
            up: r#"
                -- Create materialized view for performance
                CREATE MATERIALIZED VIEW IF NOT EXISTS market_statistics AS
                SELECT 
                    m.id,
                    m.market_id,
                    m.question,
                    m.status,
                    m.total_volume,
                    COUNT(DISTINCT p.wallet_address) as unique_traders,
                    COUNT(p.id) as total_positions,
                    AVG(p.amount) as avg_position_size,
                    MAX(p.amount) as max_position_size,
                    SUM(CASE WHEN p.pnl > 0 THEN 1 ELSE 0 END)::FLOAT / 
                        NULLIF(COUNT(CASE WHEN p.status = 'closed' THEN 1 END), 0) as win_rate
                FROM markets m
                LEFT JOIN positions p ON m.id = p.market_id
                GROUP BY m.id, m.market_id, m.question, m.status, m.total_volume;
                
                CREATE UNIQUE INDEX IF NOT EXISTS idx_market_statistics_id 
                    ON market_statistics(id);
                CREATE INDEX IF NOT EXISTS idx_market_statistics_volume 
                    ON market_statistics(total_volume DESC);
            "#,
        },
        Migration {
            version: 14,
            name: "create_polymarket_orders_table",
            checksum: "pm1ordersv1",
            up: r#"
                -- Polymarket orders table (used by /api/orders/* endpoints)
                CREATE TABLE IF NOT EXISTS polymarket_orders (
                    id BIGSERIAL PRIMARY KEY,
                    order_id TEXT UNIQUE NOT NULL,
                    order_hash TEXT,
                    market_id TEXT NOT NULL,
                    user_address TEXT NOT NULL,
                    side SMALLINT NOT NULL,
                    outcome SMALLINT,
                    amount TEXT NOT NULL,
                    price TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );

                CREATE INDEX IF NOT EXISTS idx_polymarket_orders_user ON polymarket_orders(user_address);
                CREATE INDEX IF NOT EXISTS idx_polymarket_orders_market ON polymarket_orders(market_id);
                CREATE INDEX IF NOT EXISTS idx_polymarket_orders_status ON polymarket_orders(status);
                CREATE INDEX IF NOT EXISTS idx_polymarket_orders_created_at ON polymarket_orders(created_at DESC);
            "#,
        },
    ]
}

/// Get list of applied migration versions
async fn get_applied_migrations(conn: &Object) -> Result<Vec<i32>> {
    let rows = conn.query(
        "SELECT version FROM schema_migrations ORDER BY version",
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
        "INSERT INTO schema_migrations (version, name, checksum) VALUES ($1, $2, $3)",
        &[&migration.version, &migration.name, &migration.checksum],
    ).await.context("Failed to record migration")?;
    
    // Commit
    txn.commit().await
        .context("Failed to commit migration")?;
    
    println!("Applied migration {}: {}", migration.version, migration.name);
    Ok(())
}
