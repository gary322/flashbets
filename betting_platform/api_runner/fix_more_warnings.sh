#!/bin/bash
# Script to fix more compiler warnings

echo "Fixing unused variables by prefixing with underscore..."

# Fix websocket/real_events.rs unused variables
echo "Fixing websocket/real_events.rs..."
sed -i '' 's/position_id: String/_position_id: String/' src/websocket/real_events.rs
sed -i '' 's/market_id: u64/_market_id: u64/' src/websocket/real_events.rs
sed -i '' 's/creator: Pubkey/_creator: Pubkey/' src/websocket/real_events.rs
sed -i '' 's/timestamp: i64/_timestamp: i64/' src/websocket/real_events.rs

# Fix rpc_client.rs unused variables
echo "Fixing rpc_client.rs..."
sed -i '' 's/let order_type/let _order_type/' src/rpc_client.rs
sed -i '' 's/let instruction/let _instruction/' src/rpc_client.rs
sed -i '' 's/let recent_blockhash/let _recent_blockhash/' src/rpc_client.rs

# Fix risk_engine.rs unused variables
echo "Fixing risk_engine.rs..."
sed -i '' 's/positions: Vec<RiskPosition>/_positions: Vec<RiskPosition>/' src/risk_engine.rs

# Fix risk_engine_ext.rs unused variables
echo "Fixing risk_engine_ext.rs..."
sed -i '' 's/market_id: u64/_market_id: u64/' src/risk_engine_ext.rs

# Fix quantum_engine_ext.rs unused variables
echo "Fixing quantum_engine_ext.rs..."
sed -i '' 's/let collapse_strategy/let _collapse_strategy/' src/quantum_engine_ext.rs

# Fix integration/price_feed.rs unused variables
echo "Fixing integration/price_feed.rs..."
sed -i '' 's/let market_id/let _market_id/' src/integration/price_feed.rs

# Fix integration/polymarket_price_feed.rs unused variables
echo "Fixing integration/polymarket_price_feed.rs..."
sed -i '' 's/let tracked/let _tracked/' src/integration/polymarket_price_feed.rs

# Fix integration/kalshi.rs unused variables
echo "Fixing integration/kalshi.rs..."
sed -i '' 's/let callback/let _callback/' src/integration/kalshi.rs

# Fix security/security_logger.rs unused variables
echo "Fixing security/security_logger.rs..."
sed -i '' 's/let user_agent/let _user_agent/' src/security/security_logger.rs

# Fix additional unused imports
echo "Fixing more unused imports..."
sed -i '' '/use hyper::StatusCode;/d' src/response_types.rs
sed -i '' '/use axum::http::StatusCode;/d' src/auth_endpoints.rs
sed -i '' '/use std::collections::HashMap;/d' src/market_data_service.rs
sed -i '' '/use rust_decimal::Decimal;/d' src/trading_engine.rs
sed -i '' '/use std::str::FromStr;/d' src/solana_transaction_manager.rs
sed -i '' '/use rust_decimal::Decimal;/d' src/trade_execution_service.rs
sed -i '' '/use hyper::StatusCode;/d' src/typed_errors.rs
sed -i '' '/use std::collections::HashMap;/d' src/market_creation_service.rs
sed -i '' '/use rust_decimal::Decimal;/d' src/settlement_service.rs

# Fix unused Result types
echo "Fixing unused Result types..."
sed -i '' 's/Result<()>/anyhow::Result<()>/' src/integration/market_sync.rs 2>/dev/null || true
sed -i '' 's/-> Result {/-> anyhow::Result<()> {/' src/queue/worker.rs 2>/dev/null || true

# Fix mutable variables that don't need to be mutable
echo "Fixing unnecessary mutable variables..."
sed -i '' 's/let mut state = /let state = /' src/handlers/*.rs 2>/dev/null || true

# Fix dead code warnings
echo "Adding #[allow(dead_code)] to test utilities..."
find src -name "*.rs" -exec grep -l "^#\[cfg(test)\]" {} \; | while read file; do
    if ! grep -q "#\[allow(dead_code)\]" "$file"; then
        sed -i '' '/#\[cfg(test)\]/i\
#[allow(dead_code)]' "$file" 2>/dev/null || true
    fi
done

echo "Done fixing additional warnings!"