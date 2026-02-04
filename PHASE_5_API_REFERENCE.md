# Native Solana Betting Platform - API Reference

## Overview
Complete API reference for the Native Solana betting platform, covering all program instructions, account structures, and client SDK methods.

## Table of Contents
1. [Program Instructions](#program-instructions)
2. [Account Structures](#account-structures)
3. [Client SDK](#client-sdk)
4. [Error Codes](#error-codes)
5. [Constants](#constants)

---

## Program Instructions

### Market Management

#### CreateMarket
Creates a new prediction market.

```rust
pub fn create_market(
    ctx: Context<CreateMarket>,
    market_params: MarketParams,
) -> Result<()>
```

**Accounts:**
- `market` - Market account PDA (uninitialized)
- `creator` - Market creator (signer)
- `oracle_feed` - Oracle price feed account
- `system_program` - System program

**Parameters:**
```rust
pub struct MarketParams {
    pub question: String,           // Max 200 chars
    pub outcomes: Vec<String>,      // 2-8 outcomes, max 50 chars each
    pub end_time: i64,             // Unix timestamp
    pub resolution_time: i64,      // Unix timestamp
    pub category: String,          // Market category
    pub oracle_source: String,     // "polymarket" only
    pub min_bet: u64,             // Minimum bet amount
    pub max_bet: u64,             // Maximum bet amount
    pub fee_bps: u16,             // Fee in basis points (3-28)
}
```

#### InitializeAMM
Initializes AMM for a market based on outcome count.

```rust
pub fn initialize_amm(
    ctx: Context<InitializeAMM>,
    liquidity_amount: u64,
    weights: Vec<u16>,
) -> Result<()>
```

**AMM Selection Logic:**
- N=1: LMSR (Logarithmic Market Scoring Rule)
- N>1: PM-AMM (Prediction Market AMM)
- Continuous: L2-AMM (Layer 2 AMM)

### Trading Operations

#### PlaceBet
Places a bet on a market outcome.

```rust
pub fn place_bet(
    ctx: Context<PlaceBet>,
    outcome: u8,
    amount: u64,
    slippage_tolerance: u16,
    deadline: Option<i64>,
) -> Result<()>
```

**Accounts:**
- `market` - Market account
- `user` - User placing bet (signer)
- `user_position` - User's position account (PDA)
- `user_token_account` - User's token account
- `market_vault` - Market's token vault
- `amm_state` - AMM state account
- `oracle_feed` - Oracle for price validation

**CU Usage:** 15,000-18,000 (optimized with lookup tables)

#### ExitPosition
Exits a position by selling shares.

```rust
pub fn exit_position(
    ctx: Context<ExitPosition>,
    shares_to_sell: u64,
    min_output: u64,
) -> Result<()>
```

### Liquidity Operations

#### AddLiquidity
Adds liquidity to market AMM.

```rust
pub fn add_liquidity(
    ctx: Context<AddLiquidity>,
    amount: u64,
    weights: Option<Vec<u16>>,
    lock_period: Option<u64>,
) -> Result<()>
```

**Lock Period Bonuses:**
- No lock: 1.0x
- 30 days: 1.25x
- 90 days: 1.5x

#### RemoveLiquidity
Removes liquidity from market.

```rust
pub fn remove_liquidity(
    ctx: Context<RemoveLiquidity>,
    lp_tokens: u64,
    min_amounts: Vec<u64>,
) -> Result<()>
```

### Chain Execution

#### ExecuteChain
Executes multi-step conditional trades.

```rust
pub fn execute_chain(
    ctx: Context<ExecuteChain>,
    chain_params: ChainParams,
) -> Result<()>
```

**Parameters:**
```rust
pub struct ChainParams {
    pub steps: Vec<ChainStep>,      // Max 3 steps (CPI depth limit)
    pub initial_amount: u64,
    pub min_final_amount: u64,
    pub deadline: i64,
}

pub struct ChainStep {
    pub action: ChainAction,
    pub market: Pubkey,
    pub outcome: u8,
    pub condition: Option<ChainCondition>,
}
```

**CPI Depth:** Maximum 4 levels enforced

### MMT Staking

#### StakeMMT
Stakes MMT tokens for fee rebates.

```rust
pub fn stake_mmt(
    ctx: Context<StakeMMT>,
    amount: u64,
    lock_period: Option<u64>,
) -> Result<()>
```

**Rebate Rate:** 15% of trading fees

#### ClaimRebates
Claims accumulated fee rebates.

```rust
pub fn claim_rebates(
    ctx: Context<ClaimRebates>,
) -> Result<()>
```

### Oracle Operations

#### UpdateOraclePrice
Updates oracle price feed (authorized only).

```rust
pub fn update_oracle_price(
    ctx: Context<UpdateOraclePrice>,
    price: u64,
    confidence: u64,
    timestamp: i64,
) -> Result<()>
```

### Settlement

#### SettleMarket
Settles a market with final outcome.

```rust
pub fn settle_market(
    ctx: Context<SettleMarket>,
    winning_outcome: u8,
) -> Result<()>
```

#### ClaimWinnings
Claims winnings from settled market.

```rust
pub fn claim_winnings(
    ctx: Context<ClaimWinnings>,
) -> Result<()>
```

---

## Account Structures

### Market Account
```rust
pub struct Market {
    pub discriminator: [u8; 8],      // Account discriminator
    pub creator: Pubkey,             // Market creator
    pub market_id: [u8; 32],         // Unique market ID
    pub question: [u8; 200],         // Market question
    pub outcomes: [[u8; 50]; 8],     // Outcome names
    pub outcome_count: u8,           // Number of outcomes
    pub status: MarketStatus,        // Current status
    pub end_time: i64,              // Trading end time
    pub resolution_time: i64,       // Resolution deadline
    pub winning_outcome: Option<u8>, // Settled outcome
    pub total_volume: u64,          // Total trading volume
    pub total_liquidity: u64,       // Total liquidity
    pub fee_collected: u64,         // Fees collected
    pub oracle_feed: Pubkey,        // Oracle account
    pub amm_type: AMMType,          // AMM variant
    pub created_at: i64,            // Creation timestamp
}

impl Market {
    pub const LEN: usize = 520;
    pub const DISCRIMINATOR: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
}
```

### Position Account
```rust
pub struct Position {
    pub discriminator: [u8; 8],
    pub owner: Pubkey,              // Position owner
    pub market: Pubkey,             // Market address
    pub position_id: [u8; 32],      // Unique position ID
    pub outcome: u8,                // Outcome index
    pub shares: u64,                // Number of shares
    pub entry_price: u64,           // Average entry price
    pub invested: u64,              // Total invested
    pub leverage: u8,               // Leverage used
    pub is_long: bool,              // Direction
    pub created_at: i64,            // Creation time
    pub updated_at: i64,            // Last update
}

impl Position {
    pub const LEN: usize = 256;
}
```

### AMM State Accounts

#### LMSR Market
```rust
pub struct LSMRMarket {
    pub discriminator: [u8; 8],
    pub b: u64,                     // Liquidity parameter
    pub outcomes: Vec<u64>,         // Outcome quantities
    pub total_shares: u64,          // Total shares issued
}
```

#### PM-AMM Market
```rust
pub struct PMAMMMarket {
    pub discriminator: [u8; 8],
    pub reserves: Vec<u64>,         // Token reserves
    pub weights: Vec<u16>,          // Pool weights
    pub swap_fee: u16,              // Swap fee bps
    pub total_liquidity: u64,       // Total liquidity
}
```

### Staking Account
```rust
pub struct StakeAccount {
    pub discriminator: [u8; 8],
    pub owner: Pubkey,              // Staker address
    pub amount_staked: u64,         // MMT staked
    pub stake_timestamp: i64,       // Stake time
    pub lock_end_slot: Option<u64>, // Lock end
    pub lock_multiplier: u64,       // Lock bonus
    pub accumulated_rewards: u64,    // Pending rewards
    pub rebate_percentage: u64,     // Fee rebate %
}
```

---

## Client SDK

### TypeScript/JavaScript

#### Installation
```bash
npm install @betting-platform/sdk
```

#### Initialization
```typescript
import { BettingPlatform, Connection } from '@betting-platform/sdk';

const connection = new Connection('https://api.mainnet-beta.solana.com');
const platform = new BettingPlatform(connection, wallet);
```

#### Market Operations
```typescript
// Create market
const market = await platform.createMarket({
  question: "Will BTC reach $100k by Dec 31?",
  outcomes: ["Yes", "No"],
  endTime: new Date('2024-12-31').getTime() / 1000,
  initialLiquidity: 100 * LAMPORTS_PER_SOL,
});

// Get market data
const marketData = await platform.getMarket(marketPubkey);

// Get all markets
const markets = await platform.getAllMarkets({
  status: 'active',
  category: 'crypto',
  sortBy: 'volume',
  limit: 20,
});
```

#### Trading
```typescript
// Place bet
const bet = await platform.placeBet({
  market: marketPubkey,
  outcome: 0, // "Yes"
  amount: 10 * LAMPORTS_PER_SOL,
  slippage: 0.01, // 1%
});

// Get position
const position = await platform.getPosition(wallet.publicKey, marketPubkey);

// Exit position
const exit = await platform.exitPosition({
  market: marketPubkey,
  shares: position.shares / 2, // Sell half
  minOutput: 5 * LAMPORTS_PER_SOL,
});
```

#### Chain Execution
```typescript
// Build and execute chain
const chain = platform.chainBuilder()
  .step1({
    action: 'bet',
    market: marketA,
    outcome: 0,
    leverage: 10,
  })
  .step2({
    action: 'liquidity',
    market: marketB,
    condition: {
      type: 'price_above',
      threshold: 0.7,
    },
  })
  .step3({
    action: 'stake',
    lockPeriod: 30 * 24 * 60 * 60,
  })
  .build();

const result = await platform.executeChain(chain, {
  initialAmount: 100 * LAMPORTS_PER_SOL,
});
```

#### Liquidity Provision
```typescript
// Add liquidity
const lp = await platform.addLiquidity({
  market: marketPubkey,
  amount: 1000 * LAMPORTS_PER_SOL,
  weights: [0.5, 0.5], // Equal weight
  lockPeriod: 30 * 24 * 60 * 60, // 30 days
});

// Remove liquidity
const removal = await platform.removeLiquidity({
  market: marketPubkey,
  lpTokens: lp.tokens,
});
```

#### Staking
```typescript
// Stake MMT
const stake = await platform.stakeMMT({
  amount: 10000 * LAMPORTS_PER_SOL,
  lockPeriod: 90 * 24 * 60 * 60, // 90 days for 1.5x
});

// Claim rebates
const rebates = await platform.claimRebates();
```

### Rust Client

```rust
use betting_platform_sdk::{BettingClient, MarketParams};
use solana_client::rpc_client::RpcClient;
use solana_sdk::signer::Signer;

// Initialize client
let rpc = RpcClient::new("https://api.mainnet-beta.solana.com");
let client = BettingClient::new(rpc, wallet);

// Create market
let market = client.create_market(MarketParams {
    question: "Will ETH reach $5k?".to_string(),
    outcomes: vec!["Yes".to_string(), "No".to_string()],
    end_time: 1735689600,
    // ...
})?;

// Place bet
let position = client.place_bet(
    &market.pubkey,
    0, // outcome
    1_000_000_000, // 1 SOL
    100, // 1% slippage
)?;
```

---

## Error Codes

### Trading Errors (6000-6099)
- `6000` - InvalidMarketStatus
- `6001` - MarketExpired  
- `6002` - InvalidOutcome
- `6003` - BetTooSmall
- `6004` - BetTooLarge
- `6005` - InsufficientLiquidity
- `6006` - SlippageExceeded
- `6007` - DeadlineExceeded

### Account Errors (6100-6199)
- `6100` - AccountNotInitialized
- `6101` - AccountAlreadyInitialized
- `6102` - InvalidAccountOwner
- `6103` - InvalidAccountData
- `6104` - AccountMismatch

### Oracle Errors (6200-6299)
- `6200` - InvalidOracle
- `6201` - OracleOffline
- `6202` - StalePriceData
- `6203` - PriceConfidenceTooLow

### Math Errors (6300-6399)
- `6300` - MathOverflow
- `6301` - DivisionByZero
- `6302` - InvalidPercentage
- `6303` - NegativeResult

### Chain Errors (6400-6499)
- `6400` - ChainDepthExceeded
- `6401` - ChainConditionFailed
- `6402` - ChainTimeout
- `6403` - InvalidChainStep

### Security Errors (6500-6599)
- `6500` - FlashLoanDetected
- `6501` - SandwichAttackDetected
- `6502` - UnauthorizedAccess
- `6503` - SuspiciousActivity

---

## Constants

### System Constants
```rust
pub const MAX_OUTCOMES: usize = 8;
pub const MAX_QUESTION_LEN: usize = 200;
pub const MAX_OUTCOME_LEN: usize = 50;
pub const MAX_CHAIN_DEPTH: u8 = 4;
pub const MAX_CPI_DEPTH: u8 = 4;
```

### Fee Constants
```rust
pub const MIN_FEE_BPS: u16 = 3;      // 0.03%
pub const MAX_FEE_BPS: u16 = 28;     // 0.28%
pub const FLASH_LOAN_FEE_BPS: u16 = 200; // 2%
pub const STAKING_REBATE_BPS: u16 = 1500; // 15%
```

### Staking Constants
```rust
pub const MIN_STAKE_AMOUNT: u64 = 100_000_000; // 100 MMT
pub const LOCK_PERIOD_30_DAYS: u64 = 2_592_000; // slots
pub const LOCK_PERIOD_90_DAYS: u64 = 7_776_000; // slots
pub const LOCK_MULTIPLIER_30_DAYS: u64 = 12500; // 1.25x
pub const LOCK_MULTIPLIER_90_DAYS: u64 = 15000; // 1.5x
```

### Performance Targets
```rust
pub const TARGET_CU_PER_TRADE: u64 = 20_000;
pub const MAX_BATCH_CU: u64 = 180_000;
pub const TARGET_TPS: u64 = 5_000;
pub const SHARDS_PER_MARKET: u8 = 4;
```

---

## WebSocket Events

### Subscribe to Market Updates
```typescript
platform.subscribeToMarket(marketPubkey, (update) => {
  console.log('Price update:', update.prices);
  console.log('Volume:', update.volume);
});
```

### Subscribe to Position Changes
```typescript
platform.subscribeToPosition(positionPubkey, (position) => {
  console.log('Position value:', position.currentValue);
  console.log('PnL:', position.pnl);
});
```

---

## Rate Limits

### RPC Endpoints
- Public: 10 requests/second
- Authenticated: 100 requests/second
- WebSocket: 1000 messages/minute

### Transaction Limits
- Max transactions per block: 50
- Max positions per user: 100
- Max markets per creator: 20

---

## Changelog

### v1.0.0 (Launch)
- Native Solana implementation
- LMSR/PM-AMM support
- Chain execution
- MMT staking
- Mobile SDK

### Planned Updates
- v1.1.0: Additional oracle sources
- v1.2.0: Advanced order types
- v1.3.0: Cross-chain bridges