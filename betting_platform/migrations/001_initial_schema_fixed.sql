-- Initial Schema for Betting Platform
-- Version: 001
-- Date: 2025-08-04

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Create custom types
CREATE TYPE market_status AS ENUM ('active', 'closed', 'resolved', 'disputed');
CREATE TYPE market_type AS ENUM ('binary', 'multiple', 'scalar', 'quantum');
CREATE TYPE position_status AS ENUM ('open', 'closed', 'liquidated', 'settled');
CREATE TYPE trade_side AS ENUM ('yes', 'no', 'long', 'short');
CREATE TYPE settlement_status AS ENUM ('pending', 'processing', 'completed', 'failed');

-- Markets table
CREATE TABLE markets (
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

-- Create indexes for markets
CREATE INDEX idx_markets_status ON markets(status);
CREATE INDEX idx_markets_end_time ON markets(end_time);
CREATE INDEX idx_markets_created_at ON markets(created_at DESC);

-- Positions table
CREATE TABLE positions (
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

-- Create indexes for positions
CREATE INDEX idx_positions_wallet ON positions(wallet_address);
CREATE INDEX idx_positions_market ON positions(market_id);
CREATE INDEX idx_positions_status ON positions(status);
CREATE INDEX idx_positions_opened_at ON positions(opened_at DESC);

-- Trades table
CREATE TABLE trades (
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

-- Create indexes for trades
CREATE INDEX idx_trades_wallet ON trades(wallet_address);
CREATE INDEX idx_trades_market ON trades(market_id);
CREATE INDEX idx_trades_executed_at ON trades(executed_at DESC);

-- Settlements table
CREATE TABLE settlements (
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

-- Create indexes for settlements
CREATE INDEX idx_settlements_wallet ON settlements(wallet_address);
CREATE INDEX idx_settlements_status ON settlements(status);
CREATE INDEX idx_settlements_created_at ON settlements(created_at DESC);

-- User wallets table
CREATE TABLE user_wallets (
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

-- Create indexes for user_wallets
CREATE INDEX idx_user_wallets_address ON user_wallets(wallet_address);
CREATE INDEX idx_user_wallets_volume ON user_wallets(total_volume DESC);

-- Liquidity pools table
CREATE TABLE liquidity_pools (
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

-- Create indexes for liquidity_pools
CREATE INDEX idx_liquidity_pools_market ON liquidity_pools(market_id);

-- Verses table
CREATE TABLE verses (
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

-- Create indexes for verses
CREATE INDEX idx_verses_category ON verses(category);
CREATE INDEX idx_verses_relevance ON verses(relevance_score DESC);

-- Quantum positions table
CREATE TABLE quantum_positions (
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

-- Create indexes for quantum_positions
CREATE INDEX idx_quantum_positions_wallet ON quantum_positions(wallet_address);
CREATE INDEX idx_quantum_positions_market ON quantum_positions(market_id);

-- Audit log table
CREATE TABLE audit_logs (
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

-- Create indexes for audit_logs
CREATE INDEX idx_audit_logs_wallet ON audit_logs(wallet_address);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_event_type ON audit_logs(event_type);

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply updated_at triggers
CREATE TRIGGER update_markets_updated_at BEFORE UPDATE ON markets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_liquidity_pools_updated_at BEFORE UPDATE ON liquidity_pools
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create performance indexes
CREATE INDEX idx_markets_volume ON markets(total_volume DESC);
CREATE INDEX idx_positions_pnl ON positions(pnl DESC) WHERE status = 'closed';
CREATE INDEX idx_trades_composite ON trades(market_id, executed_at DESC);
CREATE INDEX idx_user_wallets_active ON user_wallets(last_active DESC);

-- Create materialized view for market statistics
CREATE MATERIALIZED VIEW market_statistics AS
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
    SUM(CASE WHEN p.pnl > 0 THEN 1 ELSE 0 END)::FLOAT / NULLIF(COUNT(CASE WHEN p.status = 'closed' THEN 1 END), 0) as win_rate
FROM markets m
LEFT JOIN positions p ON m.id = p.market_id
GROUP BY m.id, m.market_id, m.question, m.status, m.total_volume;

-- Create index on materialized view
CREATE INDEX idx_market_statistics_volume ON market_statistics(total_volume DESC);

-- Add comments for documentation
COMMENT ON TABLE markets IS 'Core markets table storing all prediction market data';
COMMENT ON TABLE positions IS 'User positions in markets including leveraged positions';
COMMENT ON TABLE trades IS 'Individual trades that make up positions';
COMMENT ON TABLE settlements IS 'Settlement records for resolved markets';
COMMENT ON TABLE user_wallets IS 'Aggregated user statistics and metadata';
COMMENT ON TABLE liquidity_pools IS 'AMM liquidity pools for each market';
COMMENT ON TABLE verses IS 'Verse catalog for market theming';
COMMENT ON TABLE quantum_positions IS 'Quantum betting positions with superposition states';
COMMENT ON TABLE audit_logs IS 'Comprehensive audit trail for all system actions';