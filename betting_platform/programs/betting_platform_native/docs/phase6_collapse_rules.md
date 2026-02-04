# Phase 6: Collapse Rules Implementation

## Overview
This phase implements the market collapse rules as specified in the Mathematical Implementation Details. The collapse rules ensure markets resolve deterministically based on probabilities and time constraints.

## Implemented Features

### 1. Max Probability Collapse with Lexical ID Tiebreaker ✓
- **Location**: `/src/collapse/max_probability_collapse.rs`
- **Function**: `find_max_probability_outcome()`
- **Implementation**:
  ```rust
  // Find outcome with maximum probability
  // Uses lexical tiebreaker: if prices are equal, prefer lower outcome ID
  for (outcome, &price) in proposal.prices.iter().enumerate() {
      if price > max_price {
          max_price = price;
          max_outcome = outcome as u8;
      }
      // If price equals max_price, keep the lower outcome ID (lexical order)
  }
  ```

### 2. Time-based Collapse at settle_slot ✓
- **Location**: `/src/collapse/max_probability_collapse.rs`
- **Function**: `process_settle_slot_collapse()`
- **Implementation**:
  - Checks if current slot >= settle_slot
  - Automatically triggers collapse when condition is met
  - Resolves to outcome with highest probability
  - No manual intervention required

### 3. Price Clamp 2%/slot (PRICE_CLAMP_SLOT = 200) ✓
- **Location**: `/src/amm/constants.rs` and `/src/amm/helpers.rs`
- **Constant**: `PRICE_CLAMP_PER_SLOT_BPS: u16 = 200`
- **Function**: `validate_price_movement_per_slot()`
- **Implementation**:
  ```rust
  // Calculate max allowed change based on slots elapsed
  let max_change_bps = PRICE_CLAMP_PER_SLOT_BPS as u64 * slots_elapsed;
  
  // Check if price change exceeds allowed limit
  if price_change_bps > max_change_bps {
      return Err(BettingPlatformError::PriceManipulation.into());
  }
  ```

### 4. Flash Loan Prevention (halt if >5% over 4 slots) ✓
- **Location**: `/src/collapse/max_probability_collapse.rs`
- **Function**: `check_flash_loan_halt()`
- **Implementation**:
  ```rust
  const FLASH_LOAN_WINDOW: u64 = 4; // 4 slots
  const FLASH_LOAN_THRESHOLD_BPS: u64 = 500; // 5%
  
  // Halt if change exceeds 5% in 4 slots
  Ok(price_change_bps > FLASH_LOAN_THRESHOLD_BPS)
  ```

## Key Components

### CollapseType Enum
```rust
pub enum CollapseType {
    SettleSlot = 0,     // Normal time-based collapse
    MaxProbability = 1, // Max probability collapse
    Emergency = 2,      // Admin emergency collapse
}
```

### MarketCollapsed Event
```rust
define_event!(MarketCollapsed, EventType::MarketCollapsed, {
    proposal_id: [u8; 32],
    winning_outcome: u8,
    probability: u64,
    collapse_type: u8,
    timestamp: i64,
});
```

## Test Coverage

### Unit Tests
1. **test_max_probability_selection**: Verifies correct outcome selection with lexical tiebreaker
2. **test_flash_loan_detection**: Tests 5% price movement detection over 4 slots

## Security Considerations

1. **Keeper Authorization**: Only authorized keepers can trigger settle_slot collapse
2. **Time Verification**: Uses Solana Clock sysvar for secure time verification
3. **Price Manipulation Protection**: 2% per slot clamp prevents rapid price manipulation
4. **Flash Loan Protection**: Halts trading if suspicious price movements detected

## Integration Points

1. **AMM Integration**: Price clamping integrated into all AMM trade functions
2. **Event System**: MarketCollapsed events emitted for monitoring
3. **Resolution System**: Integrates with existing resolution mechanism

## Future Enhancements

1. Add configurable collapse parameters per market
2. Implement gradual collapse for smoother transitions
3. Add multi-sig requirements for emergency collapse
4. Enhanced flash loan detection with ML-based anomaly detection