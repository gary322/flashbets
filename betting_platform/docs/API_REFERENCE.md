# Betting Platform API Reference

## Table of Contents
1. [Overview](#overview)
2. [Program Instructions](#program-instructions)
3. [Account Structures](#account-structures)
4. [Error Codes](#error-codes)
5. [Integration Examples](#integration-examples)
6. [Rate Limits](#rate-limits)

## Overview

The Betting Platform is a native Solana program that provides prediction market functionality with advanced features including:
- PM-AMM with Newton-Raphson solver
- Coverage-based liquidation
- Multi-oracle support
- Chain positions
- MMT tokenomics

**Program ID**: `Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4`

## Program Instructions

### 1. Initialize Global Config

Initializes the global configuration for the platform.

```rust
pub fn initialize_global_config(
    accounts: InitializeGlobalConfigAccounts,
    params: InitializeGlobalConfigParams,
) -> ProgramResult
```

**Accounts:**
- `authority` - Admin authority (signer)
- `global_config` - Global config PDA (mut)
- `treasury` - Treasury account
- `insurance_fund` - Insurance fund account
- `system_program` - System program

**Parameters:**
```rust
pub struct InitializeGlobalConfigParams {
    pub protocol_fee_bps: u16,        // Protocol fee in basis points (max: 1000)
    pub liquidation_fee_bps: u16,     // Liquidation fee (max: 1000)
    pub min_leverage: u8,             // Minimum leverage (1)
    pub max_leverage: u8,             // Maximum leverage (100)
    pub emergency_authority: Pubkey,   // Emergency pause authority
}
```

### 2. Create Market

Creates a new prediction market.

```rust
pub fn create_market(
    accounts: CreateMarketAccounts,
    params: CreateMarketParams,
) -> ProgramResult
```

**Accounts:**
- `creator` - Market creator (signer)
- `proposal` - Proposal PDA (mut)
- `amm_pool` - AMM pool PDA (mut)
- `creator_stake` - Creator's MMT stake account
- `global_config` - Global config
- `system_program` - System program

**Parameters:**
```rust
pub struct CreateMarketParams {
    pub market_id: [u8; 32],          // Unique market identifier
    pub verse_id: u64,                // Verse identifier
    pub outcomes: u8,                 // Number of outcomes (2-8)
    pub initial_liquidity: u64,       // Initial liquidity amount
    pub amm_type: AMMType,            // LMSR or CPMM
    pub settle_slot: u64,             // Settlement slot
    pub min_stake: u64,               // Minimum creator stake
}
```

### 3. Open Position

Opens a new trading position.

```rust
pub fn open_position(
    accounts: OpenPositionAccounts,
    params: OpenPositionParams,
) -> ProgramResult
```

**Accounts:**
- `trader` - Trader account (signer)
- `position` - Position PDA (mut)
- `proposal` - Market proposal
- `user_map` - User map PDA (mut)
- `vault` - Protocol vault (mut)
- `oracle` - Oracle account
- `global_config` - Global config
- `system_program` - System program

**Parameters:**
```rust
pub struct OpenPositionParams {
    pub market_id: [u8; 32],
    pub outcome: u8,              // Outcome to bet on
    pub size: u64,                // Position size in lamports
    pub leverage: u8,             // Leverage (1-100)
    pub is_long: bool,            // Long or short
    pub max_slippage_bps: u16,    // Max slippage tolerance
}
```

### 4. Close Position

Closes an existing position.

```rust
pub fn close_position(
    accounts: ClosePositionAccounts,
    params: ClosePositionParams,
) -> ProgramResult
```

**Accounts:**
- `trader` - Position owner (signer)
- `position` - Position account (mut)
- `proposal` - Market proposal (mut)
- `vault` - Protocol vault (mut)
- `oracle` - Oracle account
- `system_program` - System program

**Parameters:**
```rust
pub struct ClosePositionParams {
    pub position_id: [u8; 32],
    pub size_to_close: Option<u64>,  // None = close full position
}
```

### 5. Liquidate Position

Liquidates an unhealthy position.

```rust
pub fn liquidate_position(
    accounts: LiquidatePositionAccounts,
    params: LiquidatePositionParams,
) -> ProgramResult
```

**Accounts:**
- `keeper` - Registered keeper (signer)
- `position` - Position to liquidate (mut)
- `proposal` - Market proposal (mut)
- `position_owner` - Position owner (mut)
- `keeper_reward` - Keeper reward account (mut)
- `insurance_fund` - Insurance fund (mut)
- `oracle` - Oracle account
- `global_config` - Global config

**Parameters:**
```rust
pub struct LiquidatePositionParams {
    pub position_id: [u8; 32],
    pub liquidation_type: LiquidationType,  // Partial/Full/Emergency
}
```

### 6. Update Oracle Price

Updates oracle price feed.

```rust
pub fn update_oracle_price(
    accounts: UpdateOraclePriceAccounts,
    params: UpdateOraclePriceParams,
) -> ProgramResult
```

**Accounts:**
- `oracle_authority` - Oracle authority (signer)
- `oracle` - Oracle PDA (mut)
- `proposal` - Market proposal (mut)
- `clock` - Clock sysvar

**Parameters:**
```rust
pub struct UpdateOraclePriceParams {
    pub market_id: [u8; 32],
    pub prices: Vec<u64>,         // Prices for each outcome
    pub confidence: u8,           // Confidence level (0-100)
    pub source: String,           // Oracle source identifier
}
```

### 7. Stake MMT

Stakes MMT tokens for rewards and tier progression.

```rust
pub fn stake_mmt(
    accounts: StakeMMTAccounts,
    params: StakeMMTParams,
) -> ProgramResult
```

**Accounts:**
- `staker` - Token owner (signer)
- `stake_account` - Stake PDA (mut)
- `mmt_account` - User's MMT token account (mut)
- `stake_vault` - MMT stake vault (mut)
- `mmt_state` - MMT state PDA (mut)
- `token_program` - Token program

**Parameters:**
```rust
pub struct StakeMMTParams {
    pub amount: u64,              // Amount to stake
    pub lock_duration: u64,       // Lock duration in seconds
}
```

### 8. Create Chain Position

Creates a multi-leg chain position.

```rust
pub fn create_chain_position(
    accounts: CreateChainPositionAccounts,
    params: CreateChainPositionParams,
) -> ProgramResult
```

**Accounts:**
- `trader` - Trader (signer)
- `chain_position` - Chain position PDA (mut)
- `proposals` - Array of proposal accounts
- `vault` - Protocol vault (mut)
- `system_program` - System program

**Parameters:**
```rust
pub struct CreateChainPositionParams {
    pub legs: Vec<ChainLeg>,      // 2-8 legs
    pub total_size: u64,          // Total position size
}

pub struct ChainLeg {
    pub market_id: [u8; 32],
    pub outcome: u8,
    pub allocation_bps: u16,      // Allocation in basis points
}
```

## Account Structures

### GlobalConfigPDA

Global platform configuration.

```rust
pub struct GlobalConfigPDA {
    pub discriminator: [u8; 8],
    pub admin_authority: Pubkey,
    pub emergency_authority: Pubkey,
    pub treasury: Pubkey,
    pub insurance_fund: Pubkey,
    pub protocol_fee_bps: u16,
    pub liquidation_fee_bps: u16,
    pub flash_loan_fee_bps: u16,
    pub min_leverage: u8,
    pub max_leverage: u8,
    pub is_paused: bool,
    pub total_markets: u64,
    pub total_volume: u128,
    pub total_fees_collected: u128,
    pub mmt_mint: Pubkey,
}
```

### ProposalPDA

Market proposal state.

```rust
pub struct ProposalPDA {
    pub discriminator: [u8; 8],
    pub proposal_id: [u8; 32],
    pub verse_id: [u8; 32],
    pub market_id: [u8; 32],
    pub creator: Pubkey,
    pub amm_type: AMMType,
    pub outcomes: u8,
    pub prices: Vec<u64>,          // Current prices per outcome
    pub volumes: Vec<u64>,         // Volume per outcome
    pub liquidity_depth: u64,
    pub state: ProposalState,
    pub settle_slot: u64,
    pub resolution: Option<u8>,
    pub created_at: i64,
}
```

### Position

User position state.

```rust
pub struct Position {
    pub discriminator: [u8; 8],
    pub user: Pubkey,
    pub proposal_id: u128,
    pub position_id: [u8; 32],
    pub outcome: u8,
    pub size: u64,
    pub notional: u64,
    pub leverage: u64,
    pub entry_price: u64,
    pub liquidation_price: u64,
    pub is_long: bool,
    pub created_at: i64,
    pub is_closed: bool,
    pub margin: u64,
    pub partial_liq_accumulator: u64,
}
```

### OraclePrice

Oracle price data.

```rust
pub struct OraclePrice {
    pub discriminator: [u8; 8],
    pub oracle_id: Pubkey,
    pub market_id: [u8; 32],
    pub prices: Vec<u64>,
    pub confidence: u8,
    pub timestamp: i64,
    pub source: String,
    pub is_stale: bool,
}
```

## Error Codes

Common error codes and their meanings:

| Code | Name | Description |
|------|------|-------------|
| 6000 | InvalidInstruction | Invalid instruction data |
| 6001 | InvalidAccountData | Invalid account data |
| 6002 | AccountNotFound | Required account not provided |
| 6003 | Unauthorized | Unauthorized access attempt |
| 6004 | MathOverflow | Arithmetic overflow |
| 6005 | InsufficientFunds | Insufficient funds for operation |
| 6010 | MarketPaused | Market is paused |
| 6011 | InvalidLeverage | Leverage outside allowed range |
| 6012 | PositionTooSmall | Position size below minimum |
| 6020 | OracleStale | Oracle data is stale |
| 6021 | OracleSpreadTooHigh | Oracle price spread exceeds limit |
| 6030 | LiquidationNotRequired | Position is healthy |
| 6095 | CircuitBreakerTriggered | Circuit breaker activated |

## Integration Examples

### JavaScript/TypeScript

```typescript
import { Connection, PublicKey, Transaction } from '@solana/web3.js';
import { BettingPlatform } from '@betting-platform/sdk';

// Initialize connection
const connection = new Connection('https://api.mainnet-beta.solana.com');
const program = new BettingPlatform(connection);

// Open a position
async function openPosition(
  trader: PublicKey,
  marketId: Buffer,
  outcome: number,
  size: bigint,
  leverage: number
) {
  const ix = await program.createOpenPositionInstruction({
    trader,
    marketId,
    outcome,
    size,
    leverage,
    isLong: true,
    maxSlippageBps: 100, // 1%
  });
  
  const tx = new Transaction().add(ix);
  // Sign and send transaction
}

// Get market data
async function getMarketData(marketId: Buffer) {
  const proposalPDA = await program.getProposalPDA(marketId);
  const proposal = await program.getProposal(proposalPDA);
  
  return {
    prices: proposal.prices,
    volumes: proposal.volumes,
    liquidity: proposal.liquidityDepth,
    state: proposal.state,
  };
}
```

### Rust

```rust
use solana_program::pubkey::Pubkey;
use betting_platform::{
    instruction::{open_position, OpenPositionParams},
    state::ProposalPDA,
};

// Open position
let params = OpenPositionParams {
    market_id: [0u8; 32],
    outcome: 0,
    size: 1_000_000_000, // 1 SOL
    leverage: 10,
    is_long: true,
    max_slippage_bps: 100,
};

let ix = open_position(
    &program_id,
    &trader,
    &proposal_pda,
    &params,
)?;

// Get market data
let proposal_account = client.get_account(&proposal_pda)?;
let proposal = ProposalPDA::try_from_slice(&proposal_account.data)?;
```

## Rate Limits

The platform enforces the following rate limits:

### Market Data
- **Endpoint**: Get market prices/volumes
- **Limit**: 50 requests per 10 seconds
- **Scope**: Per IP address

### Order Data
- **Endpoint**: Position operations
- **Limit**: 500 requests per 10 seconds
- **Scope**: Per user account

### Oracle Updates
- **Endpoint**: Oracle price updates
- **Limit**: 100 updates per minute
- **Scope**: Per oracle authority

### Rate Limit Headers
```
X-RateLimit-Limit: 50
X-RateLimit-Remaining: 45
X-RateLimit-Reset: 1642435200
```

### Handling Rate Limits

When rate limited, the API returns:
- Status Code: 429
- Error: "Rate limit exceeded"
- Retry-After: Seconds until reset

Recommended approach:
1. Implement exponential backoff
2. Use request coalescing
3. Cache frequently accessed data
4. Consider WebSocket subscriptions for real-time data

---

For more detailed integration examples and SDK documentation, visit: https://docs.betting-platform.io