# Compilation Error Fix Guide

## Priority 1: Instruction Enum Mismatches

### Problem
Tests reference instruction variants that don't exist:
- `InitializeUserCredits` → Not found
- `DepositCredits` → Not found  
- `InitializeLMSR` → Actually `InitializeLmsrMarket`
- `InitializePMAMM` → Actually `InitializePmammMarket`
- `InitializeL2AMM` → Actually `InitializeL2AmmMarket`
- `ExecuteAutoChain` → Actually `AutoChain`

### Fix Strategy
Either update tests to use correct names OR add missing instructions if they represent actual missing functionality.

## Priority 2: Struct Field Mismatches

### GlobalConfigPDA Issues
Tests expect these fields that don't exist:
- `coverage_percentage` → Use `coverage` field instead
- `admin` → Use `update_authority` field instead
- `fee_percentage` → Use `fee_base` field instead
- `oracle_fee_percentage` → Not present, may need to add
- `total_oracle_fee` → Not present in current struct
- `total_coverage` → Use `coverage` field instead
- `last_update` → Use `last_update_slot` field instead

### VersePDA Issues
Tests expect:
- `id` → Not present
- `created_at` → Not present
- `children_merkle_root` → Not present

### AdvancedOrder Issues
Tests expect:
- `expiry_slot` → Not present
- `mmt_stake_score` → Not present
- `priority_fee` → Not present

## Priority 3: Missing Methods

### Critical Missing Methods
1. `LiquidationQueue::sort_by_priority()` - Needed for liquidation processing
2. `AntiMEVProtection::compute_order_hash()` - Needed for MEV protection
3. `AntiMEVProtection::commit_order()` - Needed for MEV protection
4. `Position::try_from_slice()` - Needed for deserialization
5. `AdvancedOracleAggregator::filter_outliers()` - Needed for oracle operations

## Priority 4: Type Import Issues

### Common Missing Imports
- `AMMType::L2Norm` variant doesn't exist
- Various discriminator imports are ambiguous
- Missing trait implementations (Debug, Clone, etc.)

## Recommended Fix Order

### Step 1: Fix Instruction Names (Quick Win)
Create a mapping function or update all test files:
```rust
// In tests, replace:
BettingPlatformInstruction::InitializeLMSR
// With:
BettingPlatformInstruction::InitializeLmsrMarket
```

### Step 2: Add Missing Struct Fields
Either:
- Add missing fields to structs (if functionality needed), OR
- Update tests to use existing fields

### Step 3: Implement Missing Methods
Priority order:
1. Deserialization methods (`try_from_slice`)
2. Core business logic (liquidation queue sorting)
3. Security features (MEV protection)

### Step 4: Fix Import Issues
- Resolve ambiguous glob imports
- Add missing trait derivations
- Fix module visibility

## Quick Fixes Available

### 1. Instruction Name Replacements
```bash
# Run these replacements across test files:
sed -i 's/InitializeLMSR/InitializeLmsrMarket/g' tests/*.rs
sed -i 's/InitializePMAMM/InitializePmammMarket/g' tests/*.rs
sed -i 's/InitializeL2AMM/InitializeL2AmmMarket/g' tests/*.rs
sed -i 's/ExecuteAutoChain/AutoChain/g' tests/*.rs
```

### 2. Field Name Replacements
```bash
# Update field references:
sed -i 's/coverage_percentage/coverage/g' tests/*.rs
sed -i 's/\.admin/\.update_authority/g' tests/*.rs
sed -i 's/fee_percentage/fee_base/g' tests/*.rs
sed -i 's/last_update/last_update_slot/g' tests/*.rs
```

### 3. Add Missing Trait Implementations
Many structs need:
```rust
#[derive(Debug, Clone)]
```

## Next Steps

1. Run the quick fixes above
2. Add missing instruction variants or update tests
3. Implement missing methods
4. Run `cargo test --no-run` to verify fixes
5. Continue with remaining errors

This systematic approach should reduce errors from ~370 to under 100.