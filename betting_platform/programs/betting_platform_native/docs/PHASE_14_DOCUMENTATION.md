# Phase 14 & 14.5: Advanced Trading Features and Monitoring System

## Table of Contents

1. [Overview](#overview)
2. [Advanced Order Types](#advanced-order-types)
3. [Monitoring System](#monitoring-system)
4. [Disaster Recovery](#disaster-recovery)
5. [Integration Guide](#integration-guide)
6. [API Reference](#api-reference)
7. [Testing Guide](#testing-guide)

## Overview

Phase 14 implements advanced trading features including Iceberg, TWAP, Peg orders, and Dark Pools. Phase 14.5 adds comprehensive monitoring and disaster recovery capabilities.

### Key Features

- **Advanced Orders**: Iceberg, TWAP, Peg, Dark Pool
- **Polymarket Integration**: All orders route through Polymarket
- **System Monitoring**: Real-time health, performance, and alerts
- **Disaster Recovery**: Checkpoint system and recovery modes

## Advanced Order Types

### Iceberg Orders

Splits large orders into smaller visible chunks to minimize market impact.

```rust
OrderType::Iceberg {
    display_size: 1000,      // 10% chunks
    total_size: 10000,
    randomization: 5,        // 0-10% randomization
}
```

**Features:**
- 10% default chunk size
- 0-10% randomization using deterministic seed
- MMT priority scoring: `stake * depth/32`

**Example:**
```rust
// Create 10,000 unit iceberg order showing 1,000 units
let order = AdvancedOrder {
    order_type: OrderType::Iceberg {
        display_size: 1000,
        total_size: 10000,
        randomization: 5,
    },
    // ... other fields
};
```

### TWAP Orders

Time-Weighted Average Price orders execute over a specified duration.

```rust
OrderType::TWAP {
    duration_slots: 100,     // 10 slots default
    slice_count: 10,
    min_slice_size: 100,
}
```

**Features:**
- Default 10 slot duration
- Equal-sized slices with time priority
- Automatic slice calculation

**Example:**
```rust
// Execute 10,000 units over 100 slots in 10 slices
let order = AdvancedOrder {
    order_type: OrderType::TWAP {
        duration_slots: 100,
        slice_count: 10,
        min_slice_size: 100,
    },
    // ... other fields
};
```

### Peg Orders

Orders that track a reference price with an offset.

```rust
OrderType::Peg {
    reference: PegReference::BestBid,
    offset: 100,  // +$1.00
    limit_price: Some(U64F64::from_num(105)),
}
```

**Reference Types:**
- `BestBid`: Track best bid price
- `BestAsk`: Track best ask price
- `MidPrice`: Track mid-market price
- `PolymarketPrice`: Track Polymarket price
- `VerseDerivedPrice`: Track verse-weighted price

**Example:**
```rust
// Buy at best bid + $1, max $105
let order = AdvancedOrder {
    order_type: OrderType::Peg {
        reference: PegReference::BestBid,
        offset: 100,  // +$1.00
        limit_price: Some(U64F64::from_num(105)),
    },
    side: Side::Buy,
    // ... other fields
};
```

### Dark Pool

Private order matching for large trades without revealing order details.

```rust
let dark_pool = DarkPool {
    pool_id: [1u8; 32],
    min_size: 1000,
    settlement_frequency: 150,  // Settle every 150 slots
    // ... other fields
};
```

**Features:**
- Volume aggregation without individual order disclosure
- VWAP crossing price calculation
- Size bucket obfuscation (Small/Medium/Large/Whale)
- Periodic settlement by keepers

## Monitoring System

### System Health Monitoring

Real-time tracking of system performance and health metrics.

```rust
pub struct SystemHealth {
    pub status: SystemStatus,
    pub current_tps: u32,
    pub coverage_ratio: U64F64,
    pub circuit_breaker_active: bool,
    // ... other metrics
}
```

**Health States:**
- `Healthy`: All systems operational
- `Degraded`: Some services impacted
- `Critical`: Major issues detected
- `Emergency`: Circuit breaker activated

**Key Metrics:**
- TPS (Transactions Per Second)
- CU (Compute Unit) usage
- Coverage ratio (vault/OI)
- API response times
- Service availability

### Performance Monitoring

Detailed per-operation metrics and performance tracking.

```rust
pub struct OperationMetrics {
    pub total_count: u64,
    pub success_count: u64,
    pub average_cu_usage: u32,
    pub p95_latency_ms: u32,
    // ... other metrics
}
```

**Tracked Operations:**
- `open_position`
- `close_position`
- `liquidation`
- `order_execution`
- `keeper_task`

**Performance Alerts:**
- High CU usage (>20k)
- Low success rate (<95%)
- High latency (p95 > 1000ms)
- Consecutive failures (>5)

### Alert System

Configurable alerts for various system conditions.

```rust
pub struct AlertConfiguration {
    pub coverage_warning_threshold: U64F64,    // Default: 1.5
    pub coverage_critical_threshold: U64F64,   // Default: 1.0
    pub api_deviation_warning_pct: u8,         // Default: 3%
    pub congestion_tps_threshold: u32,         // Default: 2500
    // ... other thresholds
}
```

**Alert Types:**
- Coverage alerts (low coverage)
- API deviation alerts
- Network congestion alerts
- Service outage alerts
- Performance alerts

## Disaster Recovery

### Recovery Modes

```rust
pub enum RecoveryMode {
    Normal,              // System operating normally
    PartialDegradation,  // Some services degraded
    FullRecovery,        // Full recovery in progress
    Emergency,           // Emergency mode - minimal operations
}
```

### Checkpoint System

Periodic snapshots for state recovery.

```rust
pub struct Checkpoint {
    pub checkpoint_id: u64,
    pub global_snapshot: GlobalSnapshot,
    pub positions_root: [u8; 32],
    pub orders_root: [u8; 32],
    pub verified: bool,
    // ... other fields
}
```

**Checkpoint Types:**
- `Scheduled`: Regular interval
- `Manual`: Admin triggered
- `PreUpgrade`: Before upgrades
- `Emergency`: During incidents

### Recovery Procedures

1. **Polymarket Outage** (>5 min):
   - Halt new orders
   - Allow closes only
   - Queue orders for later

2. **Low Coverage** (<1.0):
   - Trigger circuit breaker
   - Block new positions
   - Allow emergency withdrawals

3. **Solana Degradation**:
   - Switch to emergency mode
   - Restore from checkpoint
   - Gradual service restoration

## Integration Guide

### Setting Up Advanced Orders

```rust
// 1. Initialize order account
let order_account = Keypair::new();

// 2. Create advanced order
let order = AdvancedOrder {
    order_id: generate_order_id(),
    user: user_pubkey,
    market_id: market_id,
    order_type: OrderType::Iceberg { /* ... */ },
    // ... other fields
};

// 3. Submit to system
let ix = create_advanced_order_instruction(
    &program_id,
    &order_account.pubkey(),
    &user_pubkey,
    &order,
);
```

### Monitoring Integration

```rust
// 1. Subscribe to health updates
let health_account = get_system_health_account();

// 2. Check operation allowed
let allowed = HealthMonitor::check_operation_allowed(
    &health,
    "open_position"
)?;

// 3. React to alerts
match alert.alert_type {
    AlertType::CriticalCoverage => {
        // Handle critical coverage
    }
    // ... other alert types
}
```

## API Reference

### Instructions

#### Advanced Orders
- `create_iceberg_order`: Create iceberg order
- `execute_iceberg_slice`: Execute next slice
- `create_twap_order`: Create TWAP order
- `execute_twap_slice`: Execute TWAP slice
- `create_peg_order`: Create peg order
- `update_peg_order`: Update peg price
- `submit_dark_order`: Submit to dark pool
- `match_dark_pool`: Execute dark pool matching

#### Monitoring
- `update_system_health`: Update health metrics
- `trigger_circuit_breaker`: Emergency halt
- `record_operation_metric`: Track performance
- `check_alerts`: Process alert conditions

#### Recovery
- `initiate_recovery`: Start recovery process
- `create_checkpoint`: Create state snapshot
- `restore_from_checkpoint`: Restore state
- `complete_recovery`: Finalize recovery

### Account Structures

#### AdvancedOrder (Size: ~256 bytes)
```rust
pub struct AdvancedOrder {
    pub order_id: [u8; 32],
    pub user: Pubkey,
    pub market_id: [u8; 32],
    pub order_type: OrderType,
    pub side: Side,
    pub status: OrderStatus,
    // ... fields
}
```

#### SystemHealth (Size: ~256 bytes)
```rust
pub struct SystemHealth {
    pub status: SystemStatus,
    pub current_tps: u32,
    pub coverage_ratio: U64F64,
    pub circuit_breaker_active: bool,
    // ... fields
}
```

## Testing Guide

### Unit Tests

```bash
# Run all Phase 14 tests
cargo test --test phase_14_tests

# Run specific test
cargo test test_iceberg_order_execution
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_order_lifecycle() {
    // Create order
    let order = create_test_order();
    
    // Execute slices
    for i in 0..10 {
        execute_slice(&order, i).await?;
    }
    
    // Verify completion
    assert_eq!(order.status, OrderStatus::Filled);
}
```

### Performance Benchmarks

```bash
# Run benchmarks
cargo bench --bench phase_14_benchmarks

# Expected results:
# - Iceberg slice calculation: <1ms
# - TWAP timing calculation: <0.5ms
# - Peg price update: <0.5ms
# - Dark pool matching: <5ms
```

## Security Considerations

1. **Order Validation**
   - Minimum size enforcement
   - Randomization bounds checking
   - Time window validation

2. **Access Control**
   - User owns their orders
   - Keepers execute slices
   - Admin controls recovery

3. **Circuit Breakers**
   - Coverage < 1.0
   - API deviation > 5%
   - Polymarket outage > 5 min

4. **Recovery Safety**
   - Checkpoint verification
   - Authority validation
   - Progress tracking

## Deployment Checklist

- [ ] Deploy program with monitoring modules
- [ ] Initialize system health account
- [ ] Configure alert thresholds
- [ ] Set up keeper monitoring
- [ ] Create initial checkpoint
- [ ] Test circuit breakers
- [ ] Verify Polymarket integration
- [ ] Document emergency procedures

## Appendix

### Error Codes

| Code | Error | Description |
|------|-------|-------------|
| 6130 | InvalidRandomization | Randomization > 10% |
| 6131 | InvalidOrderType | Unsupported order type |
| 6132 | InvalidSliceCount | Invalid TWAP slices |
| 6133 | TWAPComplete | TWAP fully executed |
| 6134 | TWAPTooEarly | Too early for next slice |
| 6140 | InvalidAlertIndex | Alert not found |
| 6142 | UnauthorizedRecoveryAction | Not recovery authority |

### Constants

```rust
// Iceberg
pub const DEFAULT_DISPLAY_PERCENT: u64 = 1000; // 10%
pub const MAX_RANDOMIZATION: u64 = 1000;       // 10%

// TWAP
pub const DEFAULT_TWAP_DURATION: u64 = 10;     // 10 slots
pub const MIN_SLICE_SIZE_BPS: u64 = 100;       // 1%

// Monitoring
pub const MIGRATION_NOTICE_PERIOD: u64 = 21_600;  // ~2 hours
pub const MIGRATION_DURATION: u64 = 1_296_000;    // ~6 days
```

### Resources

- [Solana Program Library](https://github.com/solana-labs/solana-program-library)
- [Polymarket API Docs](https://docs.polymarket.com/)
- [Fixed-Point Math Guide](https://docs.rs/fixed/)
- [Monitoring Best Practices](https://solana.com/docs/monitoring)

---

*This documentation covers Phase 14 & 14.5 implementation. For questions or support, contact the development team.*