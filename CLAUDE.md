FLASH BETS MODULE - MODULAR ADDITION TO EXISTING PLATFORM

This is a MODULAR ADDITION to the platform for short-term/flash bets. The existing platform handles Polymarket-based long and mid-term bets. This flash betting module will be built:
- WITHOUT CHANGING ANY OF THE CURRENT CODE
- In a NEW SUB-DIRECTORY called "flash_bets"
- Alongside and on top of the current model
- Using Native Solana (NO ANCHOR for existing, but Anchor allowed for new flash module)
- Production-grade implementation only (NO MOCKS, NO PLACEHOLDERS)

## Overview

The platform currently uses Polymarket for long and mid-term bets, but this model doesn't work for very short-term flash bets. This module adds support for sub-minute betting markets with instant resolution through the ZK Flash Markets Module for Sub-Minute Bets, built as a modular parallel program ID ("mv-flash") alongside the existing Polymarket implementation.

## Architecture Overview (Modular, Parallel ID with CPI)

This module deploys as a new program ID alongside the main (Polymarket-focused) one. It CPIs into the main for shared state (e.g., VersePDA for grouping, vault for lev/coverage, quantum for collapse). 

### Key Architecture Components

```
┌────────────────────────┐   (DraftKings API: Live Odds/Resolves <1min, 200/hour limit)
│ Flash Ingestor         │   (Batch 5/min <3.33/min safe)
└──────────┬─────────────┘
             │
             ▼
┌────────────────────────┐
│ Flash Classifier       │   (Auto: Title "Q1 Goal?" → Flash Sub-Verse ID, CPI Link to Main Parent)
└──────────┬─────────────┘
             │ (Flash PDA: <5min Life)
             ▼
┌────────────────────────┐
│ ZK Flash Resolver      │   (Prove Outcome <10s, Mirror DraftKings Sole Source)
└──────────┬─────────────┘
             │
             ▼
┌────────────────────────┐    ┌────────────────────────┐   (CPI to Main for Shared State)
│ Micro-Tau Hybrid AMM   │◄───►│ Auto-Chainer (500x Eff)│   (Tau=0.0001/s for Vol, Chain in Flash)
└──────────┬─────────────┘    └──────────┬─────────────┘
             │                                 │
             ▼                                 ▼
┌────────────────────────┐    ┌────────────────────────┐
│ Bundled Router (to API)│    │ Shared Vault/Leverage  │   (100x Raw from Main, +5x Mult)
└──────────┬─────────────┘    └──────────┬─────────────┘
             │                                 │
             ▼                                 ▼
┌────────────────────────┐    ┌────────────────────────┐
│ Sharded Execution      │    │ UI Extension (Live Mode)│   (Blur with Ticker/Chaining)
└────────────────────────┘    └────────────────────────┘
```

### Key Flows
- Ingest DraftKings live odds (e.g., "Next Goal Yes 30% prob")
- Auto-create/link flash sub-verse to parent (CPI to main VersePDA)
- Trade with chaining (3 steps for 5x mult on 100x = 500x eff)
- Bundle route to DraftKings POST /bet (money sent to original)
- ZK prove resolution <10s (mirror DraftKings as sole source)
- Auto-collapse/refund

## Technical Implementation

### Tech Stack
- **Program**: Rust/Anchor for program (Solana v1.18+ for ZK)
- **ZK Proofs**: ark-groth16 + ark-circom (not snarkjs-solana which doesn't exist)
- **Keepers**: Node.js/axios for API ingestion
- **UI**: Next.js extension (add /live page)

### Program Structure

#### Directory Layout
```
/betting_platform/
  /flash_bets/               # New module directory
    /programs/
      /mv-flash/             # Flash program
        Cargo.toml
        Anchor.toml
        /src/
          lib.rs             # Entry point with CPI
          /state/
            mod.rs           # Flash PDAs
          /instructions/
            flash.rs         # Create/Trade/Resolve
          /amm/
            pm_amm.rs        # Micro-tau AMM
          /utils/
            mod.rs           # Helpers
          /tests/
            flash.test.rs    # Tests
    /keepers/
      /src/
        ingestor.ts          # API ingestion
        resolver.ts          # ZK resolution
        providers.ts         # Multi-provider adapters
        sse_proxy.js         # SSE proxy layer
    /ui/
      /components/
        FlashTicker.tsx      # Live ticker
        LiveMode.tsx         # Mode toggle
    /docs/
      FLASH_BETS.md          # This documentation
```

### Core Implementation Details

#### 1. Flash Verse PDAs
```rust
#[account]
pub struct FlashVerse {
    pub id: u128,
    pub parent_id: u128,      // CPI link to main Verse
    pub tau: f64,             // Micro-tau for short T
    pub settle_slot: u64,
    pub rule: u8,             // For quantum flash
}
```

#### 2. Micro-Tau Formula
- Base formula: `tau = 0.0001 * (time_left / 60.0)`
- For 30-second market: `tau = 0.0001 * 0.5 = 0.00005`
- Sport-specific values:
  - Soccer: `tau = 0.00015` (45s base)
  - Basketball: `tau = 0.0004` (24s shot clock)
  - Tennis: `tau = 0.0002` (30s between points)

#### 3. Leverage Calculation
- Effective leverage: `base * ∏(mult_i * (1 + (mult_i - 1) * tau))`
- With multipliers `[1.5, 1.2, 1.1]` and `tau=0.0001`
- Results in ~500x effective leverage through chaining

#### 4. ZK Circuit Design
Inputs:
- `outcome`: u8 (e.g., goal=1)
- `timestamp`: u64 slot
- `game_id`: u128 hash
- `odds_at_bet`: u64 (fixed point)

Output:
- `hash = keccak(outcome + timestamp + game_id + odds) == expected`

## API Integration Strategy

### Primary Provider: DraftKings
- Endpoint: `/v1/odds/live` (unofficial, ~60 calls/min limit)
- Contest model mapping to flash markets
- Points/salary ratio as odds proxy

### Secondary Providers
1. **FanDuel**: `/fixtures/live` (500/10s limits)
2. **BetMGM**: `/events/live` (100/s)
3. **Caesars**: SSE for props (200/min)
4. **PointsBet**: GraphQL markets (50/s)

### Rate Limiting & Failover
```typescript
// Exponential backoff
async function backoffRetry(fn, maxRetries = 5, initialDelay = 100) {
  let delay = initialDelay;
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn();
    } catch (e) {
      if (e.response?.status !== 429) throw e;
      await new Promise(r => setTimeout(r, delay));
      delay *= 2;
    }
  }
  throw new Error('Max retries exceeded');
}

// Circuit breaker
let failureCount = 0;
async function callApi(endpoint) {
  if (failureCount >= 5) {
    if (Date.now() - lastFailure < 60000) throw new Error('Circuit Open');
    failureCount = 4;  // Half-open test
  }
  try {
    const res = await backoffRetry(() => axios.get(endpoint));
    failureCount = 0;
    return res;
  } catch (e) {
    failureCount++;
    lastFailure = Date.now();
    throw e;
  }
}
```

### Data Normalization
Universal ID format: `PROVIDER:SPORT:EVENT:MARKET:TIMESTAMP`

```typescript
function normalizeOdds(provider, value) {
  switch (provider) {
    case 'DraftKings': 
      return value.points / value.salary / avg_ratio;
    case 'FanDuel': 
      return convertAmerican(value);  // -110 -> 0.5238
    case 'European': 
      return 1 / value;  // 1.91 -> 0.5236
    default: 
      return value;
  }
}
```

### Live Data Synchronization
- Adaptive polling: 2s for active games, 60s for inactive
- Long-polling with 4s timeout
- SSE proxy layer for push-like updates

## Resolution & Timing

### ZK Proof Generation
- Off-chain proof generation: ~2s
- On-chain verification: ~3s
- Total resolution time: <10s

### Edge Cases
- Late proof (>10s): Accept with 10% penalty if within grace period
- Proof failure: Fallback to raw DraftKings data, halt future flash
- Network congestion: 10 slot grace period with degraded odds

## State Management

### Flash PDA Lifecycle
1. Create on market ingestion (time_left < 5min)
2. Active trading period
3. Resolution via ZK proof
4. Archive to IPFS before deletion
5. Delete PDA to reclaim rent

### Historical Data
- Archive all flash PDAs to IPFS before deletion
- Store hash on-chain for verification
- Query via API: `/api/flash/[id]/archive`

## UI Extensions

### Minimal Changes
- "Live Mode" toggle in dashboard
- Flash ticker with real-time odds
- Countdown timers for active markets
- Auto-chain preview with leverage display

### New Components
```tsx
// Flash ticker component
<FlashTicker>
  "Next Goal? 30% Yes" // Updates <1s from SSE
</FlashTicker>

// Live mode toggle
<LiveModeToggle 
  onToggle={(isLive) => setProgramId(isLive ? FLASH_ID : MAIN_ID)}
/>
```

## Testing Strategy

### Local Setup
- Solana test-validator for program testing
- Mock API responses for provider simulation
- Local ZK prover setup (<10s end-to-end)

### Test Coverage
1. Flash verse creation and CPI linking
2. Micro-tau AMM convergence
3. ZK proof generation/verification
4. Multi-provider failover
5. Leverage chaining atomicity
6. Load testing (100 games/day, 1k flash verses)

## Performance Targets

- **Resolution**: <10 seconds
- **API Updates**: <5 seconds
- **Transaction Cost**: <50k CU per trade
- **State Size**: ~83KB for 1k flash PDAs
- **Uptime**: 99.9% with provider redundancy

## Risk Management

### Provider Failures
- Automatic failover to secondary providers
- Consensus requirement (3+ providers agree)
- Halt markets if insufficient providers

### Geographic Restrictions
- Off-chain IP geolocation checks
- Route to legal providers by jurisdiction
- No on-chain geo-fencing (handled by keepers)

## Implementation Priorities

1. **API Rate Limiting & Failover** - Core for <5s updates
2. **Data Normalization** - Essential for aggregation
3. **Live Data Sync** - Poll/SSE proxy infrastructure
4. **Multi-Provider Aggregation** - Redundancy at scale
5. **ZK Proof Implementation** - After data flow established

## Money-Making Mechanisms

- **500x Effective Leverage**: 100x base * 5x chaining multiplier
- **Micro-tau Efficiency**: +25% on short volatility
- **Multi-provider Arbitrage**: +15% on odds discrepancies
- **Flash Verse Depth Bonuses**: +10% per hierarchy level
- **Quick Resolution Cycles**: 6 bets/min for +600% hourly potential