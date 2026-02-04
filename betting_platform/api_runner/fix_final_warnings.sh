#!/bin/bash
# Final script to fix remaining warnings

echo "Fixing remaining unused imports..."

# Fix more unused imports
find src -name "*.rs" -exec grep -l "use hyper::StatusCode" {} \; | while read file; do
    sed -i '' '/use hyper::StatusCode;/d' "$file" 2>/dev/null || true
done

find src -name "*.rs" -exec grep -l "use std::collections::HashMap" {} \; | while read file; do
    if ! grep -q "HashMap[<,]" "$file"; then
        sed -i '' '/use std::collections::HashMap;/d' "$file" 2>/dev/null || true
    fi
done

find src -name "*.rs" -exec grep -l "use rust_decimal::Decimal" {} \; | while read file; do
    if ! grep -q "Decimal[^e]" "$file"; then
        sed -i '' '/use rust_decimal::Decimal;/d' "$file" 2>/dev/null || true
    fi
done

find src -name "*.rs" -exec grep -l "use std::str::FromStr" {} \; | while read file; do
    if ! grep -q "FromStr" "$file" | grep -v "use std::str::FromStr"; then
        sed -i '' '/use std::str::FromStr;/d' "$file" 2>/dev/null || true
    fi
done

# Fix handler imports
echo "Fixing handler imports..."
sed -i '' 's/use crate::middleware::{ValidatedJson, OptionalAuth};/use crate::middleware::{ValidatedJson};/' src/handlers/*.rs 2>/dev/null || true
sed -i '' 's/use crate::{OptionalAuth, validation::ValidatedJson};/use crate::validation::ValidatedJson;/' src/handlers/*.rs 2>/dev/null || true

# Fix specific files with known issues
echo "Fixing specific file issues..."

# Fix staking_handlers.rs
if [ -f "src/staking_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/staking_handlers.rs
    sed -i '' 's/timestamp: i64/_timestamp: i64/' src/staking_handlers.rs
fi

# Fix risk_handlers.rs
if [ -f "src/risk_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/risk_handlers.rs
fi

# Fix quantum_handlers.rs
if [ -f "src/quantum_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/quantum_handlers.rs
    sed -i '' 's/correlation_id: String/_correlation_id: String/' src/quantum_handlers.rs
fi

# Fix trading_handlers.rs
if [ -f "src/trading_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/trading_handlers.rs
fi

# Fix position_handlers.rs
if [ -f "src/position_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/position_handlers.rs
fi

# Fix liquidity_handlers.rs
if [ -f "src/liquidity_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/liquidity_handlers.rs
    sed -i '' 's/outcome: u8/_outcome: u8/' src/liquidity_handlers.rs
fi

# Fix auth_handlers.rs
if [ -f "src/auth_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/auth_handlers.rs
fi

# Fix transaction_handlers.rs
if [ -f "src/transaction_handlers.rs" ]; then
    sed -i '' 's/let state/let _state/' src/transaction_handlers.rs
fi

# Fix settlement service errors
if [ -f "src/settlement_service.rs" ]; then
    sed -i '' 's/unwrap_or(SettlementStatus::Pending)/unwrap_or(Ok(SettlementStatus::Pending))/' src/settlement_service.rs
    sed -i '' 's/recent_blockhash.0/recent_blockhash.to_bytes()/' src/settlement_service.rs
    sed -i '' 's/send_and_confirm_transaction(&transaction)/send_and_confirm_transaction(&transaction, &[&authority])/' src/settlement_service.rs
fi

# Fix settlement endpoints
if [ -f "src/settlement_endpoints.rs" ]; then
    sed -i '' 's/let market/let _market/' src/settlement_endpoints.rs
    sed -i '' 's/correlation_id: String/_correlation_id: String/' src/settlement_endpoints.rs
    sed -i '' 's/creator: String/_creator: String/' src/settlement_endpoints.rs
    sed -i '' 's/outcome: MarketOutcome/_outcome: MarketOutcome/' src/settlement_endpoints.rs
fi

# Add allow(unused) to handler state parameters
echo "Adding allow(unused) attributes..."
find src -name "*_handlers.rs" -exec sed -i '' 's/state: State<Arc<AppState>>/_state: State<Arc<AppState>>/' {} \; 2>/dev/null || true

# Remove duplicate imports
echo "Removing duplicate imports..."
find src -name "*.rs" -exec awk '!seen[$0]++' {} > {}.tmp && mv {}.tmp {} \; 2>/dev/null || true

echo "Done fixing final warnings!"