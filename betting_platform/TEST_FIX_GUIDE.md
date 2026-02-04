# Test Compilation Fix Guide

## Current Status
- Main build: ✅ Compiles successfully
- Test build: 🚧 171 errors remaining

## Common Error Patterns & Fixes

### 1. Import Issues
**Error**: `unresolved import`
**Fix**: Check module hierarchy and update import paths
```rust
// Wrong
use crate::state::recovery::RecoveryState;
// Correct
use crate::coverage::recovery::RecoveryState;
```

### 2. Missing Struct Fields
**Error**: `missing field in initializer`
**Fix**: Add required fields with appropriate defaults
```rust
Position {
    // ... existing fields ...
    cross_margin_enabled: false,
    cross_verse_enabled: false,
    entry_funding_index: Some(U64F64::from_num(0)),
    collateral: margin, // or appropriate value
}
```

### 3. Type Mismatches
**Error**: `mismatched types`
**Fix**: Use correct types, especially for Option fields
```rust
// Wrong
entry_funding_index: 0,
// Correct
entry_funding_index: Some(U64F64::from_num(0)),
```

### 4. Missing Derives
**Error**: `trait bound not satisfied`
**Fix**: Add required derive macros
```rust
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct YourStruct { ... }
```

### 5. Enum Variant Mismatches
**Error**: `no variant named X found`
**Fix**: Use correct instruction enum variants
```rust
// Check instruction.rs for actual variants
BettingPlatformInstruction::ProcessBootstrapDeposit { amount }
// Not: Deposit, DepositCredits, etc.
```

## Systematic Approach

### Step 1: Group Similar Errors
```bash
cargo test --no-run --lib 2>&1 | grep -E "error\[E[0-9]+\]" | sort | uniq -c
```

### Step 2: Fix by Category
1. First fix all import errors
2. Then fix missing struct fields
3. Then fix type mismatches
4. Finally fix trait bounds

### Step 3: Create Fix Scripts
```bash
# Example: Fix all entry_funding_index fields
find src -name "*.rs" -exec grep -l "entry_funding_index: 0" {} \; | \
  xargs sed -i '' 's/entry_funding_index: 0/entry_funding_index: Some(U64F64::from_num(0))/g'
```

### Step 4: Verify Progress
```bash
# Check error count after each fix
cargo test --no-run --lib 2>&1 | grep -E "error\[E[0-9]+\]" | wc -l
```

## Quick Fixes for Common Issues

### Missing U64F64 Import
```rust
use crate::math::U64F64;
```

### Missing Position Fields
```rust
collateral: 0,
entry_funding_index: Some(U64F64::from_num(0)),
cross_margin_enabled: false,
```

### Missing ProposalPDA Fields
```rust
funding_state: None,
settled_at: None,
status: ProposalState::Active,
total_volume: 0,
```

## Testing Strategy Once Compiled

1. **Unit Tests**: Test individual functions
2. **Integration Tests**: Test module interactions
3. **E2E Tests**: Test full user journeys
4. **Performance Tests**: Verify CU usage
5. **Security Tests**: Test attack scenarios

## Final Verification
```bash
# Run all tests
cargo test --all

# Check coverage
cargo tarpaulin --out Html

# Run clippy
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check
```