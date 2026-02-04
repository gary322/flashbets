# Betting Platform Integration Guide

## Table of Contents
1. [Getting Started](#getting-started)
2. [SDK Installation](#sdk-installation)
3. [Authentication](#authentication)
4. [Core Workflows](#core-workflows)
5. [WebSocket Subscriptions](#websocket-subscriptions)
6. [Best Practices](#best-practices)
7. [Testing](#testing)
8. [Troubleshooting](#troubleshooting)

## Getting Started

This guide will help you integrate the Betting Platform into your application. The platform provides prediction market functionality on Solana with advanced features like chain positions and coverage-based liquidation.

### Prerequisites
- Node.js 18+ or Rust 1.70+
- Solana CLI tools
- Basic understanding of Solana programs
- Wallet integration (Phantom, Solflare, etc.)

### Architecture Overview
```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│   Your App  │────▶│  SDK/Client  │────▶│   Solana    │
│  (Frontend) │     │  Libraries   │     │   Program   │
└─────────────┘     └──────────────┘     └─────────────┘
                            │
                            ▼
                    ┌──────────────┐
                    │   WebSocket  │
                    │   Updates    │
                    └──────────────┘
```

## SDK Installation

### JavaScript/TypeScript

```bash
npm install @betting-platform/sdk @solana/web3.js
```

### Rust

```toml
[dependencies]
betting-platform-sdk = "1.0"
solana-sdk = "1.17"
```

### Python

```bash
pip install betting-platform-sdk solana
```

## Authentication

### Wallet Connection

```typescript
import { BettingPlatformSDK } from '@betting-platform/sdk';
import { Connection, PublicKey } from '@solana/web3.js';

// Initialize SDK
const connection = new Connection('https://api.mainnet-beta.solana.com');
const sdk = new BettingPlatformSDK({
  connection,
  programId: new PublicKey('Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4'),
  commitment: 'confirmed',
});

// Connect wallet (example with Phantom)
const connectWallet = async () => {
  const { solana } = window;
  if (solana?.isPhantom) {
    const response = await solana.connect();
    sdk.setWallet(response.publicKey);
    return response.publicKey;
  }
  throw new Error('Phantom wallet not found');
};
```

### Keypair Authentication (Backend)

```typescript
import { Keypair } from '@solana/web3.js';
import fs from 'fs';

// Load keypair from file
const keypairData = JSON.parse(fs.readFileSync('path/to/keypair.json', 'utf-8'));
const keypair = Keypair.fromSecretKey(new Uint8Array(keypairData));

sdk.setWallet(keypair);
```

## Core Workflows

### 1. Market Discovery

Find and filter available markets:

```typescript
// Get all active markets
const markets = await sdk.getActiveMarkets({
  limit: 20,
  offset: 0,
  filters: {
    minLiquidity: 10000 * 1e9, // 10k SOL
    maxSettleSlot: Date.now() / 1000 + 86400 * 30, // Within 30 days
    outcomes: 2, // Binary markets only
  }
});

// Get specific market
const market = await sdk.getMarket(marketId);
console.log({
  id: market.marketId,
  outcomes: market.outcomes,
  prices: market.prices.map(p => p / 1e6), // Convert to decimals
  liquidity: market.liquidityDepth / 1e9, // Convert to SOL
  volume: market.volumes.reduce((a, b) => a + b, 0n) / 1e9,
});
```

### 2. Trading Flow

Complete trading workflow from market selection to position management:

```typescript
// Step 1: Calculate position parameters
const tradeParams = await sdk.calculateTradeParameters({
  marketId,
  outcome: 0, // YES outcome
  size: 100 * 1e9, // 100 SOL
  leverage: 10,
  isLong: true,
});

console.log({
  entryPrice: tradeParams.entryPrice,
  priceImpact: tradeParams.priceImpact,
  margin: tradeParams.marginRequired,
  liquidationPrice: tradeParams.liquidationPrice,
  fees: tradeParams.fees,
});

// Step 2: Check slippage
if (tradeParams.priceImpact > 100) { // 1% in bps
  console.warn('High price impact detected');
}

// Step 3: Open position
const signature = await sdk.openPosition({
  marketId,
  outcome: 0,
  size: 100 * 1e9,
  leverage: 10,
  isLong: true,
  maxSlippageBps: 100, // 1% max slippage
});

// Step 4: Monitor position
const position = await sdk.getPosition(signature.positionId);
const subscription = sdk.subscribeToPosition(position.id, (update) => {
  console.log({
    pnl: update.unrealizedPnl,
    marginRatio: update.marginRatio,
    liquidationDistance: update.liquidationDistance,
  });
});

// Step 5: Close position
const closeSignature = await sdk.closePosition({
  positionId: position.id,
  size: position.size, // Full close
});
```

### 3. Chain Positions

Create multi-leg positions across correlated markets:

```typescript
// Define chain legs
const chainLegs = [
  {
    marketId: btcMarketId,
    outcome: 0, // BTC UP
    allocationBps: 5000, // 50%
  },
  {
    marketId: ethMarketId,
    outcome: 0, // ETH UP
    allocationBps: 3000, // 30%
  },
  {
    marketId: solMarketId,
    outcome: 0, // SOL UP
    allocationBps: 2000, // 20%
  },
];

// Validate chain
const validation = await sdk.validateChainPosition(chainLegs);
if (!validation.isValid) {
  throw new Error(validation.errors.join(', '));
}

// Create chain position
const chainTx = await sdk.createChainPosition({
  legs: chainLegs,
  totalSize: 1000 * 1e9, // 1000 SOL total
});

// Monitor chain execution
const chainStatus = await sdk.getChainPosition(chainTx.chainId);
console.log({
  executed: chainStatus.executedLegs,
  pending: chainStatus.pendingLegs,
  totalPnl: chainStatus.totalPnl,
});
```

### 4. Liquidation Protection

Monitor and protect positions from liquidation:

```typescript
// Set up liquidation monitoring
const monitor = sdk.createLiquidationMonitor({
  checkInterval: 5000, // 5 seconds
  warningThreshold: 0.2, // Warn at 20% margin ratio
  criticalThreshold: 0.1, // Critical at 10%
});

monitor.on('warning', async (position) => {
  console.warn(`Position ${position.id} approaching liquidation`);
  
  // Option 1: Add margin
  await sdk.addMargin({
    positionId: position.id,
    amount: 10 * 1e9, // Add 10 SOL
  });
  
  // Option 2: Reduce position
  await sdk.reducePosition({
    positionId: position.id,
    reductionBps: 3000, // Reduce by 30%
  });
});

monitor.on('critical', async (position) => {
  // Emergency close
  await sdk.closePosition({
    positionId: position.id,
    urgency: 'immediate',
  });
});

// Start monitoring
monitor.start([positionId1, positionId2]);
```

### 5. MMT Staking

Stake MMT tokens for rewards and tier benefits:

```typescript
// Check current tier
const stakeInfo = await sdk.getStakeInfo(wallet.publicKey);
console.log({
  tier: stakeInfo.tier, // Bronze, Silver, Gold, Diamond
  staked: stakeInfo.amount / 1e6, // MMT has 6 decimals
  rewards: stakeInfo.unclaimedRewards / 1e6,
  apy: stakeInfo.currentApy,
});

// Stake MMT
const stakeTx = await sdk.stakeMmt({
  amount: 10000 * 1e6, // 10k MMT
  lockDuration: 30 * 86400, // 30 days
});

// Claim rewards
const claimTx = await sdk.claimRewards();

// Check tier benefits
const benefits = await sdk.getTierBenefits(stakeInfo.tier);
console.log({
  maxLeverage: benefits.maxLeverage,
  feeDiscount: benefits.feeDiscountBps,
  rewardMultiplier: benefits.rewardMultiplier,
});
```

## WebSocket Subscriptions

Real-time updates for markets and positions:

```typescript
// Market updates
const marketSub = sdk.subscribeToMarket(marketId, (update) => {
  console.log({
    prices: update.prices,
    volume24h: update.volume24h,
    trades: update.recentTrades,
  });
});

// Order book updates
const bookSub = sdk.subscribeToOrderBook(marketId, (book) => {
  console.log({
    bids: book.bids.slice(0, 5),
    asks: book.asks.slice(0, 5),
    spread: book.spread,
  });
});

// Account updates
const accountSub = sdk.subscribeToAccount(wallet.publicKey, (account) => {
  console.log({
    positions: account.openPositions,
    orders: account.pendingOrders,
    pnl: account.totalPnl,
  });
});

// Cleanup
marketSub.unsubscribe();
bookSub.unsubscribe();
accountSub.unsubscribe();
```

## Best Practices

### 1. Error Handling

```typescript
try {
  await sdk.openPosition(params);
} catch (error) {
  if (error.code === 'INSUFFICIENT_FUNDS') {
    // Handle insufficient balance
  } else if (error.code === 'MARKET_PAUSED') {
    // Handle paused market
  } else if (error.code === 'SLIPPAGE_EXCEEDED') {
    // Handle high slippage
  } else {
    // Generic error handling
    console.error('Transaction failed:', error);
  }
}
```

### 2. Transaction Management

```typescript
// Use priority fees for time-sensitive operations
const tx = await sdk.openPosition({
  ...params,
  priorityFee: 10000, // microlamports
});

// Implement retry logic
const executeWithRetry = async (fn, maxRetries = 3) => {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn();
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      await new Promise(r => setTimeout(r, 1000 * Math.pow(2, i)));
    }
  }
};
```

### 3. Rate Limiting

```typescript
import { RateLimiter } from '@betting-platform/sdk';

const limiter = new RateLimiter({
  marketData: { limit: 50, window: 10000 }, // 50 per 10s
  trading: { limit: 500, window: 10000 }, // 500 per 10s
});

// Wrap API calls
const getMarketData = limiter.wrap('marketData', async (id) => {
  return await sdk.getMarket(id);
});
```

### 4. Caching

```typescript
import { CacheManager } from '@betting-platform/sdk';

const cache = new CacheManager({
  ttl: 5000, // 5 seconds
  maxSize: 1000,
});

const getCachedMarket = async (marketId) => {
  return cache.getOrFetch(
    `market:${marketId}`,
    () => sdk.getMarket(marketId)
  );
};
```

## Testing

### 1. Devnet Testing

```typescript
// Switch to devnet
const devnetSdk = new BettingPlatformSDK({
  connection: new Connection('https://api.devnet.solana.com'),
  programId: new PublicKey('DEVNET_PROGRAM_ID'),
});

// Request test tokens
await devnetSdk.requestAirdrop(wallet.publicKey, 100 * 1e9);
```

### 2. Mock Testing

```typescript
import { MockBettingPlatform } from '@betting-platform/sdk/mock';

const mockSdk = new MockBettingPlatform({
  initialMarkets: 10,
  priceVolatility: 0.02, // 2% volatility
});

// Test with mock data
const position = await mockSdk.openPosition({
  marketId: 'mock-market-1',
  size: 100 * 1e9,
  leverage: 10,
});
```

### 3. Integration Tests

```typescript
describe('Trading Flow', () => {
  it('should open and close position', async () => {
    const market = await sdk.getActiveMarkets({ limit: 1 });
    
    const openTx = await sdk.openPosition({
      marketId: market[0].id,
      outcome: 0,
      size: 10 * 1e9,
      leverage: 5,
    });
    
    expect(openTx.signature).toBeDefined();
    
    const position = await sdk.getPosition(openTx.positionId);
    expect(position.size).toBe(10 * 1e9);
    
    const closeTx = await sdk.closePosition({
      positionId: position.id,
    });
    
    expect(closeTx.signature).toBeDefined();
  });
});
```

## Troubleshooting

### Common Issues

#### 1. Transaction Failures
```typescript
// Check simulation
const simulation = await connection.simulateTransaction(tx);
if (simulation.value.err) {
  console.error('Simulation failed:', simulation.value.logs);
}

// Common solutions:
// - Increase SOL balance for fees
// - Check account ownership
// - Verify PDA derivation
// - Ensure market is active
```

#### 2. WebSocket Disconnections
```typescript
const ws = sdk.createWebSocket({
  reconnect: true,
  reconnectDelay: 1000,
  maxReconnectAttempts: 5,
});

ws.on('disconnect', () => {
  console.log('WebSocket disconnected, attempting reconnect...');
});

ws.on('reconnect', (attempt) => {
  console.log(`Reconnected after ${attempt} attempts`);
});
```

#### 3. Rate Limit Errors
```typescript
sdk.on('rateLimit', (error) => {
  const retryAfter = error.headers['retry-after'];
  console.log(`Rate limited, retry after ${retryAfter}s`);
});
```

### Debug Mode

Enable detailed logging:

```typescript
const sdk = new BettingPlatformSDK({
  // ... config
  debug: true,
  logger: console,
});

// Custom logger
sdk.setLogger({
  debug: (msg, data) => myLogger.debug(msg, data),
  info: (msg, data) => myLogger.info(msg, data),
  warn: (msg, data) => myLogger.warn(msg, data),
  error: (msg, data) => myLogger.error(msg, data),
});
```

### Support Resources

- **Documentation**: https://docs.betting-platform.io
- **API Reference**: https://api.betting-platform.io/docs
- **Discord**: https://discord.gg/betting-platform
- **GitHub**: https://github.com/betting-platform
- **Support Email**: developers@betting-platform.io

---

For advanced integration patterns and examples, check our GitHub repository.