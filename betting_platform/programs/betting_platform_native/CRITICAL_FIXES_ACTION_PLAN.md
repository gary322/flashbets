# Critical Compilation Fixes Action Plan

## Immediate Actions Required

### 1. Missing Credits Module Issue
**Problem**: Tests import `state::credits_accounts::{UserCredits, CreditMap}` but this module doesn't exist.

**Solution Options**:
1. Create the missing credits_accounts module in state/
2. Update tests to use existing credit structures from credits/ module
3. Check if credits functionality exists elsewhere

### 2. Instruction Variant Naming Issues
**Quick Fix Script**:
```bash
#!/bin/bash
# Fix instruction names in all test files

# Backup tests first
cp -r tests tests_backup

# Fix instruction variant names
find tests -name "*.rs" -type f -exec sed -i \
  -e 's/InitializeLMSR/InitializeLmsrMarket/g' \
  -e 's/InitializePMAMM/InitializePmammMarket/g' \
  -e 's/InitializeL2AMM/InitializeL2AmmMarket/g' \
  -e 's/ExecuteAutoChain/AutoChain/g' {} \;

echo "Fixed instruction variant names"
```

### 3. GlobalConfigPDA Field Mapping
**Quick Fix Script**:
```bash
#!/bin/bash
# Fix GlobalConfigPDA field references

find tests -name "*.rs" -type f -exec sed -i \
  -e 's/coverage_percentage/coverage/g' \
  -e 's/\.admin/\.update_authority/g' \
  -e 's/fee_percentage/fee_base/g' \
  -e 's/total_oracle_fee/vault/g' \
  -e 's/total_coverage/coverage/g' \
  -e 's/last_update/last_update_slot/g' {} \;

echo "Fixed GlobalConfigPDA field names"
```

### 4. Missing Methods Implementation

#### Position::try_from_slice
Add to src/state/accounts.rs:
```rust
impl Position {
    pub fn try_from_slice(data: &[u8]) -> Result<Self, ProgramError> {
        Self::deserialize(&mut &data[..])
            .map_err(|_| ProgramError::InvalidAccountData)
    }
}
```

#### LiquidationQueue::sort_by_priority
Add to src/liquidation/queue.rs:
```rust
impl LiquidationQueue {
    pub fn sort_by_priority(&mut self) {
        self.positions.sort_by(|a, b| {
            b.risk_score.cmp(&a.risk_score)
                .then(a.entry_time.cmp(&b.entry_time))
        });
    }
}
```

### 5. Missing Instruction Variants

**For InitializeUserCredits and DepositCredits**:
Either:
1. Add these variants to BettingPlatformInstruction enum
2. Update tests to use existing credit-related instructions

### 6. Import Resolution

Fix ambiguous glob imports by being explicit:
```rust
// Instead of:
pub use accounts::*;
pub use security_accounts::*;

// Use:
pub use accounts::{GlobalConfigPDA, VersePDA, Position, /* specific items */};
pub use security_accounts::{CircuitBreaker, /* specific items */};
```

## Execution Order

1. **First**: Run the quick fix scripts for instruction and field names (5 minutes)
2. **Second**: Add missing try_from_slice methods (10 minutes)
3. **Third**: Resolve credits module issue (20 minutes)
4. **Fourth**: Fix remaining missing methods (30 minutes)
5. **Fifth**: Clean up imports (15 minutes)

## Expected Results

After these fixes:
- Error count should drop from ~370 to ~100
- Most test files should compile
- Core functionality tests can begin running

## Validation

After each step, run:
```bash
cargo test --no-run 2>&1 | grep -c "error\[E"
```

Track error count reduction to ensure progress.