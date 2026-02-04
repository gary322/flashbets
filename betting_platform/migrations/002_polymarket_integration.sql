-- Polymarket Integration Schema
-- Version: 002
-- Date: 2025-08-06
-- Description: Tables for Polymarket CLOB integration, order tracking, and CTF operations

-- Enable additional extensions if needed
CREATE EXTENSION IF NOT EXISTS "btree_gist";

-- Create enums for Polymarket-specific types
CREATE TYPE polymarket_order_status AS ENUM (
    'pending',
    'open',
    'partially_filled',
    'filled',
    'cancelled',
    'expired',
    'failed'
);

CREATE TYPE polymarket_order_side AS ENUM ('buy', 'sell');
CREATE TYPE polymarket_order_type AS ENUM ('gtc', 'fok', 'ioc', 'post_only');
CREATE TYPE polymarket_chain AS ENUM ('polygon', 'ethereum');
CREATE TYPE polymarket_token_type AS ENUM ('usdc', 'ctf', 'outcome');

-- Polymarket Markets Mapping
-- Maps internal markets to Polymarket condition IDs
CREATE TABLE polymarket_markets (
    id BIGSERIAL PRIMARY KEY,
    internal_market_id BIGINT REFERENCES markets(id) ON DELETE CASCADE,
    condition_id VARCHAR(66) NOT NULL UNIQUE, -- 0x prefixed hex
    question_id VARCHAR(66),
    token_id VARCHAR(66) NOT NULL,
    clob_token_id VARCHAR(100), -- CLOB specific token ID
    outcome_prices JSONB DEFAULT '[]', -- Array of current prices
    liquidity NUMERIC(20, 6) DEFAULT 0,
    volume_24h NUMERIC(20, 6) DEFAULT 0,
    open_interest NUMERIC(20, 6) DEFAULT 0,
    last_price NUMERIC(10, 6),
    bid NUMERIC(10, 6),
    ask NUMERIC(10, 6),
    resolved BOOLEAN DEFAULT FALSE,
    winning_outcome INTEGER,
    resolution_time TIMESTAMP WITH TIME ZONE,
    payout_numerators JSONB, -- Array of payout values
    oracle_address VARCHAR(42),
    sync_enabled BOOLEAN DEFAULT TRUE,
    last_sync TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_polymarket_markets_condition ON polymarket_markets(condition_id);
CREATE INDEX idx_polymarket_markets_internal ON polymarket_markets(internal_market_id);
CREATE INDEX idx_polymarket_markets_sync ON polymarket_markets(sync_enabled, last_sync);

-- Polymarket Orders
-- Tracks all orders submitted to Polymarket CLOB
CREATE TABLE polymarket_orders (
    id BIGSERIAL PRIMARY KEY,
    order_id VARCHAR(100) UNIQUE NOT NULL, -- Polymarket order ID
    order_hash VARCHAR(66), -- Order hash from Polymarket
    internal_position_id UUID REFERENCES positions(position_id),
    wallet_address VARCHAR(42) NOT NULL,
    market_id BIGINT REFERENCES polymarket_markets(id) ON DELETE CASCADE,
    condition_id VARCHAR(66) NOT NULL,
    token_id VARCHAR(66) NOT NULL,
    side polymarket_order_side NOT NULL,
    order_type polymarket_order_type DEFAULT 'gtc',
    size NUMERIC(20, 6) NOT NULL, -- Original size
    price NUMERIC(10, 6) NOT NULL,
    filled_amount NUMERIC(20, 6) DEFAULT 0,
    remaining_amount NUMERIC(20, 6),
    average_fill_price NUMERIC(10, 6),
    status polymarket_order_status NOT NULL DEFAULT 'pending',
    fee_rate_bps INTEGER DEFAULT 0, -- Fee rate in basis points
    maker_amount VARCHAR(78), -- BigNumber as string
    taker_amount VARCHAR(78), -- BigNumber as string
    salt VARCHAR(78), -- Order salt
    expiration BIGINT, -- Unix timestamp
    nonce VARCHAR(78),
    signature TEXT NOT NULL,
    signature_type INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    submitted_at TIMESTAMP WITH TIME ZONE,
    filled_at TIMESTAMP WITH TIME ZONE,
    cancelled_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT,
    raw_order JSONB -- Store complete order data
);

CREATE INDEX idx_polymarket_orders_wallet ON polymarket_orders(wallet_address);
CREATE INDEX idx_polymarket_orders_market ON polymarket_orders(market_id);
CREATE INDEX idx_polymarket_orders_status ON polymarket_orders(status);
CREATE INDEX idx_polymarket_orders_created ON polymarket_orders(created_at DESC);
CREATE INDEX idx_polymarket_orders_condition ON polymarket_orders(condition_id);

-- Polymarket Trades
-- Records actual trades executed on Polymarket
CREATE TABLE polymarket_trades (
    id BIGSERIAL PRIMARY KEY,
    trade_id VARCHAR(100) UNIQUE NOT NULL,
    order_id VARCHAR(100) REFERENCES polymarket_orders(order_id),
    wallet_address VARCHAR(42) NOT NULL,
    market_id BIGINT REFERENCES polymarket_markets(id),
    condition_id VARCHAR(66) NOT NULL,
    token_id VARCHAR(66) NOT NULL,
    side polymarket_order_side NOT NULL,
    price NUMERIC(10, 6) NOT NULL,
    size NUMERIC(20, 6) NOT NULL,
    fee NUMERIC(20, 6) DEFAULT 0,
    fee_token VARCHAR(42), -- Token used for fee payment
    maker_address VARCHAR(42),
    taker_address VARCHAR(42),
    transaction_hash VARCHAR(66),
    block_number BIGINT,
    log_index INTEGER,
    executed_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_polymarket_trades_wallet ON polymarket_trades(wallet_address);
CREATE INDEX idx_polymarket_trades_order ON polymarket_trades(order_id);
CREATE INDEX idx_polymarket_trades_tx ON polymarket_trades(transaction_hash);
CREATE INDEX idx_polymarket_trades_executed ON polymarket_trades(executed_at DESC);

-- CTF Positions
-- Tracks Conditional Token Framework positions
CREATE TABLE polymarket_ctf_positions (
    id BIGSERIAL PRIMARY KEY,
    wallet_address VARCHAR(42) NOT NULL,
    condition_id VARCHAR(66) NOT NULL,
    collection_id VARCHAR(66), -- CTF collection ID
    position_id VARCHAR(78), -- Calculated position ID
    token_id VARCHAR(66),
    outcome_index INTEGER NOT NULL,
    balance NUMERIC(30, 0) DEFAULT 0, -- Current balance
    locked_balance NUMERIC(30, 0) DEFAULT 0, -- Locked in orders
    average_price NUMERIC(10, 6),
    total_bought NUMERIC(30, 0) DEFAULT 0,
    total_sold NUMERIC(30, 0) DEFAULT 0,
    realized_pnl NUMERIC(20, 6) DEFAULT 0,
    unrealized_pnl NUMERIC(20, 6) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(wallet_address, position_id)
);

CREATE INDEX idx_ctf_positions_wallet ON polymarket_ctf_positions(wallet_address);
CREATE INDEX idx_ctf_positions_condition ON polymarket_ctf_positions(condition_id);
CREATE INDEX idx_ctf_positions_balance ON polymarket_ctf_positions(balance) WHERE balance > 0;

-- CTF Operations
-- Tracks split, merge, and redeem operations
CREATE TABLE polymarket_ctf_operations (
    id BIGSERIAL PRIMARY KEY,
    operation_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
    operation_type VARCHAR(20) NOT NULL, -- 'split', 'merge', 'redeem'
    wallet_address VARCHAR(42) NOT NULL,
    condition_id VARCHAR(66) NOT NULL,
    amount NUMERIC(30, 0) NOT NULL,
    collateral_token VARCHAR(42), -- USDC address
    outcome_tokens JSONB, -- Array of outcome token amounts
    transaction_hash VARCHAR(66),
    block_number BIGINT,
    gas_used BIGINT,
    gas_price NUMERIC(20, 0),
    status VARCHAR(20) DEFAULT 'pending',
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    confirmed_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_ctf_operations_wallet ON polymarket_ctf_operations(wallet_address);
CREATE INDEX idx_ctf_operations_condition ON polymarket_ctf_operations(condition_id);
CREATE INDEX idx_ctf_operations_type ON polymarket_ctf_operations(operation_type);
CREATE INDEX idx_ctf_operations_tx ON polymarket_ctf_operations(transaction_hash);

-- Polymarket Balances
-- Tracks user balances on Polygon
CREATE TABLE polymarket_balances (
    id BIGSERIAL PRIMARY KEY,
    wallet_address VARCHAR(42) NOT NULL,
    token_type polymarket_token_type NOT NULL,
    token_address VARCHAR(42),
    chain polymarket_chain DEFAULT 'polygon',
    balance NUMERIC(30, 0) NOT NULL DEFAULT 0,
    locked_balance NUMERIC(30, 0) DEFAULT 0,
    pending_deposits NUMERIC(30, 0) DEFAULT 0,
    pending_withdrawals NUMERIC(30, 0) DEFAULT 0,
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(wallet_address, token_address, chain)
);

CREATE INDEX idx_polymarket_balances_wallet ON polymarket_balances(wallet_address);
CREATE INDEX idx_polymarket_balances_updated ON polymarket_balances(last_updated);

-- Deposit/Withdrawal History
CREATE TABLE polymarket_transfers (
    id BIGSERIAL PRIMARY KEY,
    transfer_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
    transfer_type VARCHAR(20) NOT NULL, -- 'deposit', 'withdrawal'
    wallet_address VARCHAR(42) NOT NULL,
    token_address VARCHAR(42) NOT NULL,
    chain polymarket_chain NOT NULL,
    amount NUMERIC(30, 0) NOT NULL,
    from_address VARCHAR(42),
    to_address VARCHAR(42),
    transaction_hash VARCHAR(66),
    block_number BIGINT,
    status VARCHAR(20) DEFAULT 'pending',
    confirmations INTEGER DEFAULT 0,
    fee NUMERIC(20, 0),
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    confirmed_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_transfers_wallet ON polymarket_transfers(wallet_address);
CREATE INDEX idx_transfers_type ON polymarket_transfers(transfer_type);
CREATE INDEX idx_transfers_status ON polymarket_transfers(status);
CREATE INDEX idx_transfers_tx ON polymarket_transfers(transaction_hash);

-- Market Maker Positions (for liquidity providers)
CREATE TABLE polymarket_mm_positions (
    id BIGSERIAL PRIMARY KEY,
    position_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
    wallet_address VARCHAR(42) NOT NULL,
    market_id BIGINT REFERENCES polymarket_markets(id),
    condition_id VARCHAR(66) NOT NULL,
    liquidity_shares NUMERIC(30, 18) DEFAULT 0,
    base_token_amount NUMERIC(30, 0) DEFAULT 0,
    outcome_token_amounts JSONB DEFAULT '[]',
    fee_earned NUMERIC(20, 6) DEFAULT 0,
    impermanent_loss NUMERIC(20, 6) DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_mm_positions_wallet ON polymarket_mm_positions(wallet_address);
CREATE INDEX idx_mm_positions_market ON polymarket_mm_positions(market_id);

-- Order Book Snapshots (for analysis)
CREATE TABLE polymarket_orderbook_snapshots (
    id BIGSERIAL PRIMARY KEY,
    market_id BIGINT REFERENCES polymarket_markets(id),
    condition_id VARCHAR(66) NOT NULL,
    token_id VARCHAR(66) NOT NULL,
    bids JSONB NOT NULL, -- Array of [price, size] pairs
    asks JSONB NOT NULL, -- Array of [price, size] pairs
    mid_price NUMERIC(10, 6),
    spread NUMERIC(10, 6),
    depth_10 NUMERIC(20, 6), -- Depth within 10% of mid
    imbalance NUMERIC(5, 4), -- Buy/sell imbalance ratio
    snapshot_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_orderbook_market ON polymarket_orderbook_snapshots(market_id);
CREATE INDEX idx_orderbook_time ON polymarket_orderbook_snapshots(snapshot_time DESC);

-- Price History (for charts)
CREATE TABLE polymarket_price_history (
    id BIGSERIAL PRIMARY KEY,
    market_id BIGINT REFERENCES polymarket_markets(id),
    condition_id VARCHAR(66) NOT NULL,
    token_id VARCHAR(66) NOT NULL,
    outcome_index INTEGER,
    price NUMERIC(10, 6) NOT NULL,
    volume NUMERIC(20, 6),
    trades_count INTEGER DEFAULT 0,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_price_history_market ON polymarket_price_history(market_id, timestamp DESC);
CREATE INDEX idx_price_history_condition ON polymarket_price_history(condition_id, timestamp DESC);

-- WebSocket Events Log
CREATE TABLE polymarket_ws_events (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,
    channel VARCHAR(50),
    market_id BIGINT REFERENCES polymarket_markets(id),
    order_id VARCHAR(100),
    data JSONB NOT NULL,
    processed BOOLEAN DEFAULT FALSE,
    error_message TEXT,
    received_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_ws_events_type ON polymarket_ws_events(event_type);
CREATE INDEX idx_ws_events_processed ON polymarket_ws_events(processed, received_at);

-- Audit Log for compliance
CREATE TABLE polymarket_audit_log (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID DEFAULT uuid_generate_v4(),
    user_address VARCHAR(42),
    action VARCHAR(100) NOT NULL,
    entity_type VARCHAR(50),
    entity_id VARCHAR(100),
    old_values JSONB,
    new_values JSONB,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_audit_user ON polymarket_audit_log(user_address);
CREATE INDEX idx_audit_action ON polymarket_audit_log(action);
CREATE INDEX idx_audit_created ON polymarket_audit_log(created_at DESC);

-- Functions and Triggers

-- Update timestamp trigger
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply update trigger to tables with updated_at
CREATE TRIGGER update_polymarket_markets_updated_at 
    BEFORE UPDATE ON polymarket_markets 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_polymarket_orders_updated_at 
    BEFORE UPDATE ON polymarket_orders 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_polymarket_ctf_positions_updated_at 
    BEFORE UPDATE ON polymarket_ctf_positions 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_polymarket_mm_positions_updated_at 
    BEFORE UPDATE ON polymarket_mm_positions 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Function to calculate position P&L
CREATE OR REPLACE FUNCTION calculate_position_pnl(
    p_position_id VARCHAR(78),
    p_current_price NUMERIC(10, 6)
) RETURNS TABLE (
    realized_pnl NUMERIC(20, 6),
    unrealized_pnl NUMERIC(20, 6),
    total_pnl NUMERIC(20, 6)
) AS $$
DECLARE
    v_position RECORD;
    v_unrealized NUMERIC(20, 6);
BEGIN
    SELECT * INTO v_position 
    FROM polymarket_ctf_positions 
    WHERE position_id = p_position_id;
    
    IF v_position IS NULL THEN
        RETURN QUERY SELECT 0::NUMERIC(20, 6), 0::NUMERIC(20, 6), 0::NUMERIC(20, 6);
        RETURN;
    END IF;
    
    -- Calculate unrealized P&L
    v_unrealized := (p_current_price - v_position.average_price) * v_position.balance;
    
    RETURN QUERY SELECT 
        v_position.realized_pnl,
        v_unrealized,
        v_position.realized_pnl + v_unrealized;
END;
$$ LANGUAGE plpgsql;

-- Function to get market depth
CREATE OR REPLACE FUNCTION get_market_depth(
    p_market_id BIGINT,
    p_depth_percent NUMERIC DEFAULT 10
) RETURNS TABLE (
    bid_depth NUMERIC(20, 6),
    ask_depth NUMERIC(20, 6),
    total_depth NUMERIC(20, 6)
) AS $$
DECLARE
    v_snapshot RECORD;
    v_mid_price NUMERIC(10, 6);
    v_bid_depth NUMERIC(20, 6) := 0;
    v_ask_depth NUMERIC(20, 6) := 0;
BEGIN
    -- Get latest snapshot
    SELECT * INTO v_snapshot
    FROM polymarket_orderbook_snapshots
    WHERE market_id = p_market_id
    ORDER BY snapshot_time DESC
    LIMIT 1;
    
    IF v_snapshot IS NULL THEN
        RETURN QUERY SELECT 0::NUMERIC(20, 6), 0::NUMERIC(20, 6), 0::NUMERIC(20, 6);
        RETURN;
    END IF;
    
    v_mid_price := v_snapshot.mid_price;
    
    -- Calculate bid depth within percentage
    SELECT COALESCE(SUM((level->>'size')::NUMERIC), 0) INTO v_bid_depth
    FROM jsonb_array_elements(v_snapshot.bids) AS level
    WHERE (level->>'price')::NUMERIC >= v_mid_price * (1 - p_depth_percent / 100);
    
    -- Calculate ask depth within percentage
    SELECT COALESCE(SUM((level->>'size')::NUMERIC), 0) INTO v_ask_depth
    FROM jsonb_array_elements(v_snapshot.asks) AS level
    WHERE (level->>'price')::NUMERIC <= v_mid_price * (1 + p_depth_percent / 100);
    
    RETURN QUERY SELECT v_bid_depth, v_ask_depth, v_bid_depth + v_ask_depth;
END;
$$ LANGUAGE plpgsql;

-- Views for common queries

-- Active orders view
CREATE VIEW v_polymarket_active_orders AS
SELECT 
    o.*,
    m.internal_market_id,
    m.liquidity,
    m.last_price
FROM polymarket_orders o
JOIN polymarket_markets m ON o.market_id = m.id
WHERE o.status IN ('pending', 'open', 'partially_filled');

-- User positions summary
CREATE VIEW v_polymarket_user_positions AS
SELECT 
    p.wallet_address,
    p.condition_id,
    m.internal_market_id,
    p.outcome_index,
    p.balance,
    p.average_price,
    p.realized_pnl,
    p.unrealized_pnl,
    m.last_price,
    m.resolved,
    m.winning_outcome
FROM polymarket_ctf_positions p
JOIN polymarket_markets m ON p.condition_id = m.condition_id
WHERE p.balance > 0;

-- Market statistics view
CREATE VIEW v_polymarket_market_stats AS
SELECT 
    m.*,
    COUNT(DISTINCT o.wallet_address) as unique_traders,
    COUNT(o.id) as total_orders,
    SUM(CASE WHEN o.status = 'filled' THEN 1 ELSE 0 END) as filled_orders,
    AVG(o.filled_amount) as avg_order_size
FROM polymarket_markets m
LEFT JOIN polymarket_orders o ON m.id = o.market_id
GROUP BY m.id;

-- Indexes for performance
CREATE INDEX idx_polymarket_orders_composite ON polymarket_orders(wallet_address, status, created_at DESC);
CREATE INDEX idx_polymarket_trades_composite ON polymarket_trades(wallet_address, executed_at DESC);
CREATE INDEX idx_ctf_positions_composite ON polymarket_ctf_positions(wallet_address, balance) WHERE balance > 0;

-- Add comments for documentation
COMMENT ON TABLE polymarket_markets IS 'Maps internal markets to Polymarket condition IDs and tracks market data';
COMMENT ON TABLE polymarket_orders IS 'Tracks all orders submitted to Polymarket CLOB';
COMMENT ON TABLE polymarket_ctf_positions IS 'Tracks user positions in Conditional Token Framework';
COMMENT ON TABLE polymarket_ctf_operations IS 'Logs all CTF operations (split, merge, redeem)';
COMMENT ON TABLE polymarket_balances IS 'Tracks user token balances on Polygon';
COMMENT ON TABLE polymarket_transfers IS 'Records deposit and withdrawal history';
COMMENT ON TABLE polymarket_orderbook_snapshots IS 'Periodic snapshots of order book state for analysis';
COMMENT ON TABLE polymarket_price_history IS 'Historical price data for charting';
COMMENT ON TABLE polymarket_ws_events IS 'Log of WebSocket events for processing and debugging';
COMMENT ON TABLE polymarket_audit_log IS 'Audit trail for compliance and debugging';