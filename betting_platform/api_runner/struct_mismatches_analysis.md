# Struct Mismatches Analysis

## Overview
This document maps the discrepancies between test code expectations and actual struct definitions in `src/types.rs`.

## 1. Market Struct

### Actual Definition (types.rs)
```rust
pub struct Market {
    pub id: u128,
    pub title: String,
    pub description: String,
    pub creator: Pubkey,
    pub outcomes: Vec<MarketOutcome>,
    pub amm_type: AmmType,
    pub total_liquidity: u64,
    pub total_volume: u64,
    pub resolution_time: i64,
    pub resolved: bool,
    pub winning_outcome: Option<u8>,
    pub created_at: i64,
    pub verse_id: Option<u128>,
}
```

### Test Expectation (rpc_client.rs:369)
```rust
Market {
    id,
    title: title.to_string(),
    description: format!("Description for {}", title),
    category: "Test".to_string(),        // ❌ MISSING in actual
    status: MarketStatus::Active,        // ❌ MISSING in actual
    fee_rate: 250,                       // ❌ MISSING in actual
    locked_liquidity: 100000,            // ❌ WRONG NAME (total_liquidity)
    total_yes_bets: 50000,              // ❌ MISSING in actual
    total_no_bets: 50000,               // ❌ MISSING in actual
    created_at: chrono::Utc::now().timestamp(),
    ends_at: chrono::Utc::now().timestamp() + 86400,  // ❌ WRONG NAME (resolution_time)
    resolved: false,
    outcome: None,                       // ❌ WRONG NAME (winning_outcome)
    resolver: Pubkey::new_unique(),      // ❌ WRONG NAME (creator)
}
```

### Missing Fields in Test
- `outcomes: Vec<MarketOutcome>`
- `amm_type: AmmType`
- `total_volume: u64`
- `verse_id: Option<u128>`

## 2. Position Struct

### Actual Definition (types.rs)
```rust
pub struct Position {
    pub owner: Pubkey,
    pub market_id: u128,
    pub outcome: u8,
    pub size: u64,
    pub leverage: u32,
    pub entry_price: u64,
    pub liquidation_price: u64,
    pub is_long: bool,
    pub collateral: u64,
    pub created_at: i64,
}
```

### PositionInfo Struct (used in tests)
```rust
pub struct PositionInfo {
    pub position: Pubkey,
    pub market_id: u128,
    pub amount: u64,       // Maps to size
    pub outcome: u8,
    pub leverage: u32,
    pub entry_price: f64,  // ❌ WRONG TYPE (u64 vs f64)
    pub current_price: f64,
    pub pnl: i128,
    pub status: PositionStatus,
    pub created_at: i64,
    pub updated_at: i64,
}
```

## 3. WsMessage Enum

### Actual Definition (types.rs)
```rust
pub enum WsMessage {
    MarketUpdate {
        market_id: u128,
        yes_price: f64,
        no_price: f64,
        volume: u64,
    },
    PositionUpdate {
        position_id: String,
        pnl: f64,
        current_price: f64,
    },
    Notification {
        title: String,
        message: String,
        level: String,
    },
}
```

### Test Expectations
Tests may be expecting different variants or field names.

## 4. Test Utility Issues

### test_utils.rs
- `create_test_market`: Uses non-existent fields
- `create_position`: Needs to match actual Position struct
- `create_position_info`: Already correct, just used for testing

### quantum_engine.rs Tests
- Calling non-existent methods like `store_position`
- Actual API methods:
  - `create_quantum_position(wallet: String, states: Vec<QuantumState>, entanglement_group: Option<String>) -> Result<String>`
  - `measure_quantum_position(position_id: &str) -> Result<QuantumMeasurement>`
  - `get_quantum_position(position_id: &str) -> Result<QuantumPosition>`
  - `get_wallet_positions(wallet: &str) -> Result<Vec<QuantumPosition>>`
  - `get_market_quantum_states(market_id: u128) -> Result<Vec<QuantumState>>`
  - `get_measurements() -> Result<Vec<QuantumMeasurement>>`
  - `calculate_quantum_metrics(wallet: &str) -> Result<QuantumPortfolioMetrics>`

### risk_engine.rs Tests
- `create_position_info` helper is correct
- Tests are properly using PositionInfo struct

### websocket.rs Tests
- Need to update message types to match actual WsMessage enum

## Summary of Required Changes

### Priority 1: Market Factory
1. Remove: category, status, fee_rate, total_yes_bets, total_no_bets
2. Rename: locked_liquidity → total_liquidity, ends_at → resolution_time, outcome → winning_outcome, resolver → creator
3. Add: outcomes (Vec<MarketOutcome>), amm_type, total_volume, verse_id

### Priority 2: Position/PositionInfo
- Tests correctly use PositionInfo which is defined properly
- No changes needed for risk_engine tests

### Priority 3: WebSocket Messages
- Update test messages to use correct WsMessage variants

### Priority 4: Method Signatures
- Analyze quantum_engine.rs to find correct method names
- Update test calls accordingly