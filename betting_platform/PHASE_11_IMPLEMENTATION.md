# Phase 11 & 11.5 Implementation Documentation

## Overview

This document provides comprehensive documentation for the implementation of Phase 11 (Attack Prevention & Circuit Breakers) and Phase 11.5 (Liquidation Priority System) from CLAUDE.md specifications.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                   Attack Detection Layer                      │
│  ┌─────────────┐ ┌─────────────┐ ┌──────────────────────┐  │
│  │Price Tracker│ │Volume Detect│ │Flash Loan Detection  │  │
│  └─────────────┘ └─────────────┘ └──────────────────────┘  │
│  ┌─────────────┐ ┌─────────────┐                            │
│  │Wash Trading │ │Cross-Verse  │                            │
│  │  Detection  │ │Correlation  │                            │
│  └─────────────┘ └─────────────┘                            │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Circuit Breaker System                      │
│  ┌─────────────┐ ┌─────────────┐ ┌──────────────────────┐  │
│  │  Coverage   │ │    Price    │ │      Volume          │  │
│  │   Breaker   │ │   Breaker   │ │      Breaker         │  │
│  └─────────────┘ └─────────────┘ └──────────────────────┘  │
│  ┌─────────────┐ ┌─────────────┐ ┌──────────────────────┐  │
│  │ Liquidation │ │ Congestion  │ │    Emergency         │  │
│  │   Cascade   │ │   Breaker   │ │    Shutdown          │  │
│  └─────────────┘ └─────────────┘ └──────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                Liquidation Priority System                    │
│  ┌─────────────┐ ┌─────────────┐ ┌──────────────────────┐  │
│  │  Priority   │ │   Partial   │ │   Staking Tier       │  │
│  │   Queue     │ │ Liquidation │ │   Protection         │  │
│  └─────────────┘ └─────────────┘ └──────────────────────┘  │
│  ┌─────────────┐ ┌─────────────┐                            │
│  │  Bootstrap  │ │   Keeper    │                            │
│  │ Protection  │ │  Incentives │                            │
│  └─────────────┘ └─────────────┘                            │
└─────────────────────────────────────────────────────────────┘
```

## Phase 11: Attack Prevention & Circuit Breakers

### 1. Attack Detection Core (`attack_detection.rs`)

The `AttackDetector` struct monitors trading activity for malicious patterns:

```rust
pub struct AttackDetector {
    pub detector_id: [u8; 32],
    pub risk_level: u8,
    pub detected_patterns: Vec<AttackPattern>,
    pub recent_trades: VecDeque<TradeSnapshot>,
    pub price_tracker: PriceMovementTracker,
    pub volume_detector: VolumeAnomalyDetector,
    pub flash_loan_detector: FlashLoanDetector,
    pub correlation_tracker: CrossVerseTracker,
    pub wash_trade_detector: WashTradeDetector,
    pub last_update_slot: u64,
}
```

#### Key Features:

1. **Price Manipulation Detection**
   - Enforces 2% max price change per slot
   - Enforces 5% cumulative change over 4 slots
   - Triggers `ClampPrice` action when violated

2. **Volume Anomaly Detection**
   - Tracks 7-day rolling average volume
   - Alerts when volume exceeds 3 standard deviations
   - Increases monitoring frequency

3. **Flash Loan Detection**
   - Identifies opposite trades in same slot
   - Checks for positions >10% of vault size
   - Triggers trade reversion

4. **Wash Trading Detection**
   - Tracks trader activity patterns
   - Identifies opposite trades within 10 slots
   - Applies penalty fees

5. **Risk Scoring**
   - Aggregates severity scores from all detectors
   - Maintains overall risk level (0-100)

### 2. Circuit Breaker Implementation (`circuit_breaker.rs`)

The `CircuitBreaker` struct provides multi-level protection:

```rust
pub struct CircuitBreaker {
    pub breaker_id: [u8; 32],
    pub state: BreakerState,
    pub coverage_breaker: CoverageBreaker,
    pub price_breaker: PriceBreaker,
    pub volume_breaker: VolumeBreaker,
    pub liquidation_breaker: LiquidationBreaker,
    pub congestion_breaker: CongestionBreaker,
    pub emergency_authority: Option<Pubkey>,
}
```

#### Breaker Types:

1. **Coverage Breaker**
   - Halts trading when coverage < 0.5
   - 1 hour halt duration
   - Critical severity

2. **Price Breaker**
   - Halts when cumulative price change > 5% over 4 slots
   - 1 hour halt duration
   - High severity

3. **Volume Breaker**
   - Halts when volume > 10x average
   - 30 minute halt duration
   - Medium severity

4. **Liquidation Cascade Breaker**
   - Halts at >50 liquidations per slot
   - Halts when liquidation volume >10% of OI
   - 1 hour halt duration

5. **Network Congestion Breaker**
   - Halts at >100 failed transactions per slot
   - 15 minute halt duration
   - High severity

6. **Emergency Shutdown**
   - One-time use authority
   - Authority burned after use
   - Permanent halt

#### State Management:

```rust
pub enum BreakerState {
    Active,
    Halted {
        start_slot: u64,
        expected_resume: u64,
        reason: HaltReason,
    },
    Cooldown {
        end_slot: u64,
    },
    EmergencyShutdown,
}
```

## Phase 11.5: Liquidation Priority System

### 1. Liquidation Queue Management (`liquidation_priority.rs`)

The `LiquidationQueue` maintains priority-ordered positions at risk:

```rust
pub struct LiquidationQueue {
    pub queue_id: [u8; 32],
    pub at_risk_positions: Vec<AtRiskPosition>,
    pub active_liquidations: Vec<ActiveLiquidation>,
    pub config: LiquidationConfig,
    pub metrics: LiquidationMetrics,
    pub keeper_rewards_pool: u64,
    pub last_update_slot: u64,
}
```

### 2. Priority Score Calculation

Priority scoring considers multiple factors:

```rust
pub fn calculate_priority_score(&self) -> u64 {
    // Base score from risk (0-100)
    let mut score = self.risk_score as u64 * 1_000_000;
    
    // Distance to liquidation factor
    let distance_factor = if self.distance_to_liquidation < 0.01 {
        1_000_000 // <1% from liquidation
    } else if self.distance_to_liquidation < 0.05 {
        500_000 // <5% from liquidation
    } else {
        100_000 // >5% from liquidation
    };
    
    // Staking tier protection (subtracted)
    let staking_protection = match self.staking_tier {
        StakingTier::None => 0,
        StakingTier::Bronze => 100_000,
        StakingTier::Silver => 200_000,
        StakingTier::Gold => 300_000,
        StakingTier::Platinum => 500_000,
    };
    
    // Bootstrap protection (subtracted)
    // Chain risk factor (added)
    // Time at risk factor (added)
}
```

### 3. Key Parameters (from CLAUDE.md)

- **Max Liquidation Per Slot**: 8% of position size
- **Keeper Rewards**: 5 basis points (0.05%)
- **Minimum Liquidation Size**: $10
- **Grace Period**: 180 slots (~1 minute)
- **Bootstrap Protection**: 50% more time before liquidation

### 4. Staking Tiers

```rust
pub enum StakingTier {
    None,        // 0 MMT
    Bronze,      // 100-1k MMT
    Silver,      // 1k-10k MMT
    Gold,        // 10k-100k MMT
    Platinum,    // 100k+ MMT
}
```

## Instruction Handlers

### Attack Detection Instructions

1. **initialize_attack_detector**: Sets up detector with default thresholds
2. **process_trade**: Analyzes trade for attack patterns
3. **update_volume_baseline**: Updates volume statistics
4. **reset_detector**: Clears detector state (authority only)

### Circuit Breaker Instructions

1. **initialize_circuit_breaker**: Sets up breaker with CLAUDE.md parameters
2. **check_breakers**: Evaluates all breaker conditions
3. **emergency_shutdown**: One-time emergency halt
4. **update_breaker_config**: Adjusts breaker parameters

### Liquidation Priority Instructions

1. **initialize_liquidation_queue**: Creates priority queue
2. **update_at_risk_position**: Adds/updates position in queue
3. **process_liquidation**: Executes liquidations by priority
4. **claim_keeper_rewards**: Distributes keeper incentives

## Security Considerations

1. **Immutability**: Emergency authorities are burned after use
2. **Partial Liquidation**: Prevents cascade by limiting to 8% per slot
3. **Cooldown Periods**: Prevents rapid triggering/resuming
4. **Multi-Factor Detection**: Reduces false positives
5. **Staking Protection**: Incentivizes long-term participation

## Testing Coverage

### Attack Detection Tests
- Price manipulation (single slot and cumulative)
- Volume anomaly detection
- Flash loan detection
- Wash trading identification
- Risk level calculation

### Circuit Breaker Tests
- Coverage breaker trigger
- Price volatility detection
- Liquidation cascade prevention
- Network congestion handling
- Emergency shutdown mechanism
- Cooldown period management

### Liquidation Priority Tests
- Staking tier determination
- Risk score calculation
- Priority queue ordering
- Partial liquidation limits
- Keeper reward calculation
- Metrics tracking

## Compliance with CLAUDE.md

All implementations strictly follow CLAUDE.md specifications:

✅ 2% price change per slot limit  
✅ 5% cumulative change over 4 slots triggering halt  
✅ 8% maximum liquidation per slot  
✅ 5bp keeper rewards  
✅ Coverage-based halting at <0.5  
✅ Staking tier protection system  
✅ Bootstrap trader protection  
✅ Immutable design with burned authorities  
✅ Comprehensive attack detection mechanisms  
✅ Multi-level circuit breaker system  

## Future Enhancements

1. **Machine Learning Integration**: Pattern recognition for novel attacks
2. **Cross-Chain Monitoring**: Detect coordinated cross-chain attacks
3. **Dynamic Threshold Adjustment**: Self-tuning based on market conditions
4. **Decentralized Keeper Network**: Distributed liquidation execution
5. **Insurance Fund Integration**: Additional protection layer

## Conclusion

The implementation provides robust protection against various attack vectors while maintaining fair liquidation prioritization. The system balances security with market efficiency, protecting legitimate traders while preventing malicious activity.