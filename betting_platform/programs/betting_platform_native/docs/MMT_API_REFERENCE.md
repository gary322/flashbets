# MMT Token System - API Reference

## Instructions

### Core MMT Instructions

#### InitializeMMTPDAs
Initializes all MMT-related Program Derived Addresses.

**Accounts:**
- `initializer` (signer, writable) - Authority initializing the system
- `mmt_config` (writable) - MMT configuration PDA
- `mmt_mint` (writable) - MMT token mint PDA
- `treasury` (writable) - Treasury token account PDA
- `reserved_vault` (writable) - Reserved vault token account PDA
- `staking_pool` (writable) - Staking pool PDA
- `stake_vault` (writable) - Stake vault token account PDA
- `maker_registry` (writable) - Maker registry PDA
- `season_emission` (writable) - Season emission PDA
- `system_program` - System program
- `token_program` - SPL Token program
- `rent` - Rent sysvar

**Data:** None

**Errors:**
- `Unauthorized` - Initializer is not authorized
- `InvalidSeeds` - PDA derivation failed

---

#### StakeMMT
Stakes MMT tokens with optional lock period.

**Accounts:**
- `staker` (signer, writable) - User staking tokens
- `user_stake` (writable) - User's stake account PDA
- `staking_pool` (writable) - Staking pool PDA
- `user_token` (writable) - User's MMT token account
- `stake_vault` (writable) - Stake vault token account
- `mmt_config` - MMT configuration
- `token_program` - SPL Token program

**Data:**
```rust
{
    amount: u64,                    // Amount to stake
    lock_period_slots: Option<u64>, // Optional lock period in slots
}
```

**Errors:**
- `InsufficientBalance` - User has insufficient MMT
- `BelowMinimumStake` - Amount below 100 MMT minimum
- `MathOverflow` - Arithmetic overflow

---

#### UnstakeMMT
Unstakes MMT tokens from the staking pool.

**Accounts:**
- `staker` (signer, writable) - User unstaking tokens
- `user_stake` (writable) - User's stake account PDA
- `staking_pool` (writable) - Staking pool PDA
- `user_token` (writable) - User's MMT token account
- `stake_vault` (writable) - Stake vault token account
- `mmt_config` - MMT configuration
- `token_program` - SPL Token program

**Data:**
```rust
{
    amount: u64, // Amount to unstake
}
```

**Errors:**
- `InsufficientStake` - User has insufficient staked amount
- `StillLocked` - Tokens are still in lock period
- `MathOverflow` - Arithmetic overflow

---

#### ClaimStakingRewards
Claims accumulated staking rewards.

**Accounts:**
- `staker` (signer, writable) - User claiming rewards
- `user_stake` (writable) - User's stake account PDA
- `staking_pool` - Staking pool PDA
- `user_token` (writable) - User's MMT token account
- `treasury` (writable) - Treasury token account
- `mmt_config` - MMT configuration
- `token_program` - SPL Token program

**Data:** None

**Errors:**
- `NoRewardsToClaim` - No pending rewards
- `InvalidAccount` - Invalid stake account

---

### Maker Reward Instructions

#### InitializeMakerAccount
Initializes a maker account for tracking rewards.

**Accounts:**
- `maker` (signer, writable) - Market maker
- `maker_account` (writable) - Maker account PDA
- `maker_registry` (writable) - Maker registry PDA
- `system_program` - System program

**Data:** None

**Errors:**
- `AccountAlreadyInitialized` - Maker account exists

---

#### RecordMakerTrade
Records a trade with spread improvement for rewards.

**Accounts:**
- `maker` (signer) - Market maker
- `maker_account` (writable) - Maker account PDA
- `season_emission` (writable) - Current season emission
- `mmt_config` - MMT configuration

**Data:**
```rust
{
    notional: u64,              // Trade notional value
    spread_improvement_bp: u16, // Spread improvement in basis points
}
```

**Errors:**
- `InsufficientSpreadImprovement` - Improvement < 1bp
- `InvalidNotional` - Zero or excessive notional
- `ExceedsSeasonAllocation` - Would exceed season allocation

---

#### ClaimMakerRewards
Claims accumulated maker rewards.

**Accounts:**
- `maker` (signer) - Market maker
- `maker_account` (writable) - Maker account PDA
- `maker_token` (writable) - Maker's MMT token account
- `treasury` (writable) - Treasury token account
- `token_program` - SPL Token program

**Data:** None

**Errors:**
- `NoRewardsToClaim` - No pending rewards
- `InvalidAccount` - Invalid maker account

---

### Distribution Instructions

#### DistributeEmission
Distributes MMT tokens from season allocation.

**Accounts:**
- `authority` (signer) - Distribution authority
- `season_emission` (writable) - Season emission PDA
- `mmt_config` - MMT configuration
- `distribution_record` (writable) - Distribution record PDA
- `recipient_token` (writable) - Recipient's token account
- `treasury` (writable) - Treasury token account
- `token_program` - SPL Token program

**Data:**
```rust
{
    distribution_type: u8, // Type of distribution (0-4)
    amount: u64,          // Amount to distribute
    distribution_id: u64, // Unique distribution ID
}
```

**Distribution Types:**
- 0: MakerReward
- 1: StakingReward
- 2: EarlyTraderBonus
- 3: VaultSeed
- 4: Airdrop

**Errors:**
- `Unauthorized` - Not authorized to distribute
- `ExceedsSeasonAllocation` - Would exceed allocation
- `SeasonNotActive` - Season has ended
- `InvalidDistributionType` - Unknown distribution type

---

#### TransitionSeason
Transitions to the next season.

**Accounts:**
- `authority` (signer) - Transition authority
- `mmt_config` (writable) - MMT configuration
- `current_season` - Current season emission
- `next_season` (writable) - Next season emission PDA
- `system_program` - System program

**Data:** None

**Errors:**
- `SeasonStillActive` - Current season not ended
- `InvalidSeasonProgression` - Invalid season number

---

### Early Trader Instructions

#### RegisterEarlyTrader
Registers as an early trader for 2x rewards.

**Accounts:**
- `trader` (signer, writable) - Trader registering
- `early_trader_registry` (writable) - Registry PDA
- `maker_account` (writable) - Maker account PDA
- `system_program` - System program

**Data:**
```rust
{
    season: u8, // Season number
}
```

**Errors:**
- `EarlyTraderLimitReached` - 100 trader limit reached
- `AlreadyRegistered` - Trader already registered
- `InvalidSeason` - Invalid season number

---

### Table Instructions

#### InitializeNormalTables
Initializes the normal distribution tables.

**Accounts:**
- `authority` (signer, writable) - Initialization authority
- `tables` (writable) - Tables PDA
- `system_program` - System program

**Data:** None

**Errors:**
- `AccountAlreadyInitialized` - Tables already initialized

---

#### PopulateTablesChunk
Populates a chunk of table values.

**Accounts:**
- `authority` (signer) - Population authority
- `tables` (writable) - Tables PDA

**Data:**
```rust
{
    start_index: usize,      // Starting index
    values: Vec<TableValues>, // Table values to populate
}
```

**TableValues Structure:**
```rust
{
    x: i32,       // x value in hundredths
    cdf: U64F64,  // Φ(x)
    pdf: U64F64,  // φ(x)
    erf: U64F64,  // erf(x)
}
```

**Errors:**
- `TablesAlreadyPopulated` - Tables already populated
- `InvalidTableIndex` - Index out of bounds
- `InvalidTableData` - Invalid values

---

## Account Structures

### MMTConfig
```rust
pub struct MMTConfig {
    pub discriminator: [u8; 8],      // Account discriminator
    pub mint: Pubkey,                // MMT mint address
    pub authority: Pubkey,           // Config authority
    pub total_supply: u64,           // 100M total supply
    pub circulating_supply: u64,     // Current circulating
    pub season_allocation: u64,      // 10M per season
    pub current_season: u8,          // Current season number
    pub season_start_slot: u64,      // Season start slot
    pub season_emitted: u64,         // Emitted this season
    pub locked_supply: u64,          // 90M locked
    pub treasury: Pubkey,            // Treasury address
    pub reserved_vault: Pubkey,      // Reserved vault
    pub staking_pool: Pubkey,        // Staking pool
    pub bump: u8,                    // PDA bump
}
```

### StakingPool
```rust
pub struct StakingPool {
    pub discriminator: [u8; 8],      // Account discriminator
    pub total_staked: u64,           // Total MMT staked
    pub total_shares: u128,          // Total pool shares
    pub reward_per_share: u128,      // Accumulated rewards
    pub last_update_slot: u64,       // Last update slot
    pub minimum_stake: u64,          // 100 MMT minimum
    pub rebate_percentage_base: u16, // 1500 (15%)
    pub stake_vault: Pubkey,         // Vault address
    pub total_distributed_rewards: u64, // Total distributed
    pub bump: u8,                    // PDA bump
}
```

### UserStake
```rust
pub struct UserStake {
    pub discriminator: [u8; 8],      // Account discriminator
    pub owner: Pubkey,               // Stake owner
    pub staked_amount: u64,          // Amount staked
    pub shares: u128,                // Pool shares
    pub reward_debt: u128,           // Reward debt
    pub last_stake_slot: u64,        // Last stake slot
    pub lock_end_slot: Option<u64>,  // Lock end (if any)
    pub accumulated_rewards: u64,     // Total rewards earned
    pub bump: u8,                    // PDA bump
}
```

### MakerAccount
```rust
pub struct MakerAccount {
    pub discriminator: [u8; 8],      // Account discriminator
    pub owner: Pubkey,               // Maker address
    pub total_volume: u64,           // Total volume traded
    pub spread_improvements: u64,     // Total bp improved
    pub trades_count: u32,           // Number of trades
    pub average_spread_improvement_bp: u64, // Average improvement
    pub pending_rewards: u64,         // Unclaimed rewards
    pub total_rewards_claimed: u64,   // Total claimed
    pub is_early_trader: bool,       // Early trader flag
    pub last_trade_slot: u64,        // Last trade slot
    pub bump: u8,                    // PDA bump
}
```

### NormalDistributionTables
```rust
pub struct NormalDistributionTables {
    pub discriminator: [u8; 8],      // Account discriminator
    pub is_initialized: bool,        // Initialization flag
    pub version: u8,                 // Table version
    pub min_x: i32,                  // -400 (-4.0)
    pub max_x: i32,                  // 400 (4.0)
    pub step: i32,                   // 1 (0.01)
    pub table_size: usize,           // 801
    pub cdf_table: Vec<u64>,         // CDF values
    pub pdf_table: Vec<u64>,         // PDF values
    pub erf_table: Vec<u64>,         // erf values
}
```

## Error Codes

| Error | Code | Description |
|-------|------|-------------|
| `InvalidInput` | 6000 | Invalid input parameter |
| `MathOverflow` | 6001 | Arithmetic overflow |
| `InsufficientBalance` | 6002 | Insufficient token balance |
| `Unauthorized` | 6003 | Unauthorized operation |
| `AccountAlreadyInitialized` | 6004 | Account already exists |
| `InvalidAccount` | 6005 | Invalid or corrupt account |
| `BelowMinimumStake` | 6006 | Below 100 MMT minimum |
| `StillLocked` | 6007 | Tokens still locked |
| `NoRewardsToClaim` | 6008 | No pending rewards |
| `InsufficientSpreadImprovement` | 6009 | < 1bp improvement |
| `ExceedsSeasonAllocation` | 6010 | Exceeds season limit |
| `SeasonNotActive` | 6011 | Season has ended |
| `SeasonStillActive` | 6012 | Season not ended |
| `EarlyTraderLimitReached` | 6013 | 100 trader limit |
| `AlreadyRegistered` | 6014 | Already registered |
| `TablesNotInitialized` | 6015 | Tables not ready |
| `InvalidTableIndex` | 6016 | Index out of bounds |
| `SecurityCheckFailed` | 6017 | Security validation failed |

## Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `TOTAL_SUPPLY` | 100,000,000 × 10^9 | Total MMT supply |
| `CURRENT_SEASON_ALLOCATION` | 10,000,000 × 10^9 | Per season allocation |
| `RESERVED_ALLOCATION` | 90,000,000 × 10^9 | Locked supply |
| `DECIMALS` | 9 | Token decimals |
| `SEASON_DURATION_SLOTS` | 38,880,000 | ~6 months |
| `MIN_STAKE_AMOUNT` | 100 × 10^9 | Minimum stake |
| `STAKING_REBATE_BASIS_POINTS` | 1500 | 15% rebate |
| `MIN_SPREAD_IMPROVEMENT_BP` | 1 | Minimum 1bp |
| `EARLY_TRADER_LIMIT` | 100 | Max early traders |
| `TABLE_SIZE` | 801 | CDF/PDF table points |

## PDA Seeds

| PDA | Seeds | Description |
|-----|-------|-------------|
| MMT Config | `[b"mmt_config"]` | Main configuration |
| MMT Mint | `[b"mmt_mint"]` | Token mint |
| Treasury | `[b"mmt_treasury"]` | Treasury vault |
| Reserved Vault | `[b"reserved_vault"]` | Locked tokens |
| Staking Pool | `[b"staking_pool"]` | Staking pool |
| Stake Vault | `[b"stake_vault"]` | Staked tokens |
| User Stake | `[b"user_stake", user_pubkey]` | User stake account |
| Maker Account | `[b"maker_account", maker_pubkey]` | Maker account |
| Early Traders | `[b"early_traders", season]` | Early trader list |
| Season | `[b"season", season_number]` | Season emission |
| Distribution | `[b"distribution", id.to_le_bytes()]` | Distribution record |
| Normal Tables | `[b"normal_tables"]` | CDF/PDF tables |