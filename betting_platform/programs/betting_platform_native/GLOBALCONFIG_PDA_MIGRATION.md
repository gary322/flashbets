# GlobalConfigPDA Migration Documentation

## Overview
This document details the comprehensive migration of GlobalConfigPDA struct instances throughout the codebase to match the updated struct definition in `src/state/accounts.rs`.

## Original vs Updated GlobalConfigPDA Structure

### Fields Removed
- `admin` - replaced with `update_authority`
- `fee_percentage` - replaced with `fee_base`
- `oracle_fee_percentage` - replaced with `fee_slope`
- `coverage_ratio` - functionality moved to `coverage` field
- `last_update_slot` - no longer needed
- `bootstrap_phase` - removed
- `bootstrap_start_slot` - removed
- `bootstrap_target_vault` - removed
- `bootstrap_current_vault` - replaced with `vault`
- `min_leverage` - moved to leverage_tiers
- `max_leverage` - moved to leverage_tiers
- `min_liquidity` - replaced with `min_order_size`
- `base_fee_bps` - replaced with `fee_base`
- `protocol_fee_share_bps` - removed
- `oracle_sources` - removed
- `keeper_reward_bps` - removed
- `min_coverage_ratio` - removed
- `emergency_mode` - replaced with `halt_flag`
- `total_markets` - removed
- `total_volume` - removed
- `mmt_mint` - removed
- `total_vault` - consolidated into `vault`
- `total_coverage` - consolidated into `coverage`

### New Fields Added
- `discriminator: [u8; 8]` - Account discriminator
- `epoch: u64` - Current epoch number
- `season: u64` - Current season number
- `vault: u128` - Total vault balance
- `total_oi: u128` - Total open interest
- `coverage: u128` - Coverage ratio (vault / total_oi)
- `fee_base: u32` - Base fee in basis points
- `fee_slope: u32` - Fee slope for dynamic pricing
- `halt_flag: bool` - System halt flag
- `genesis_slot: u64` - Genesis slot
- `season_start_slot: u64` - Season start slot
- `season_end_slot: u64` - Season end slot
- `mmt_total_supply: u64` - MMT total supply
- `mmt_current_season: u64` - MMT allocation for current season
- `mmt_emission_rate: u64` - MMT emission rate per slot
- `leverage_tiers: Vec<LeverageTier>` - Leverage tiers configuration
- `min_order_size: u64` - Minimum order size
- `max_order_size: u64` - Maximum order size
- `update_authority: Pubkey` - Update authority (replaces admin)
- `primary_market_id: [u8; 32]` - Primary market ID for system-wide events

## Files Modified

### Test Files Updated
1. **tests/test_security_phase6.rs**
   - Updated GlobalConfigPDA instantiation with correct fields
   - Added LeverageTier import
   - Fixed discriminator to use correct value

2. **tests/test_user_journey_phase7.rs**
   - Removed duplicate `vault` field
   - Updated all fields to match new structure
   - Added LeverageTier import

3. **src/tests/production_security_test.rs**
   - Updated imports to include LeverageTier
   - Changed `admin` references to `update_authority`
   - Changed `emergency_mode` to `halt_flag`
   - Updated GlobalConfigPDA instantiation

4. **src/tests/auto_chain_tests.rs**
   - Removed Default::default() usage
   - Added proper imports (Pack, BorshSerialize)
   - Changed GlobalConfigPDA::SIZE to GlobalConfigPDA::LEN
   - Created full struct instantiation

5. **src/tests/production_integration_test.rs**
   - Updated imports to include accounts module
   - Removed bootstrap-related field references
   - Updated vault balance references
   - Created bootstrap_target_vault as local variable

6. **tests/test_market_ingestion_e2e.rs**
   - Replaced GlobalConfigPDA::default() with full instantiation
   - Added proper imports for LeverageTier and discriminators

## Standard GlobalConfigPDA Instantiation Template

```rust
use betting_platform_native::state::accounts::{GlobalConfigPDA, LeverageTier, discriminators};

let global_config = GlobalConfigPDA {
    discriminator: discriminators::GLOBAL_CONFIG,
    epoch: 1,
    season: 1,
    vault: 0,
    total_oi: 0,
    coverage: 0,
    fee_base: 30,
    fee_slope: 10,
    halt_flag: false,
    genesis_slot: 0,
    season_start_slot: 0,
    season_end_slot: 1000000,
    mmt_total_supply: 1000000000,
    mmt_current_season: 100000000,
    mmt_emission_rate: 1000,
    leverage_tiers: vec![
        LeverageTier { n: 100, max: 10 },
        LeverageTier { n: 50, max: 20 },
        LeverageTier { n: 25, max: 50 },
        LeverageTier { n: 10, max: 100 },
    ],
    min_order_size: 1000,
    max_order_size: 1000000,
    update_authority: Pubkey::new_unique(),
    primary_market_id: [0u8; 32],
};
```

## Migration Patterns

### Authority Changes
```rust
// Before
validate_authority(&authority, &global_config.admin)

// After
validate_authority(&authority, &global_config.update_authority)
```

### Emergency Mode Changes
```rust
// Before
global_config.emergency_mode = true;

// After
global_config.halt_flag = true;
```

### Bootstrap Vault Changes
```rust
// Before
global_config.bootstrap_current_vault += amount;

// After
global_config.vault += amount;
```

### Size Constant Changes
```rust
// Before
GlobalConfigPDA::SIZE

// After
GlobalConfigPDA::LEN
```

## Build Status
All changes have been implemented and the project builds successfully. The GlobalConfigPDA struct is now consistent across all test files and matches the production struct definition.

## Testing Recommendations
1. Run all unit tests to ensure GlobalConfigPDA operations work correctly
2. Verify serialization/deserialization with the new struct format
3. Test migration of existing GlobalConfigPDA accounts if applicable
4. Ensure all PDA derivations still work correctly

## Notes
- The discriminator value `[159, 213, 171, 84, 129, 36, 178, 94]` is defined in `discriminators::GLOBAL_CONFIG`
- LeverageTier struct must be imported when creating GlobalConfigPDA instances
- The Pack trait must be in scope to access the LEN constant
- BorshSerialize must be imported for try_to_vec() method