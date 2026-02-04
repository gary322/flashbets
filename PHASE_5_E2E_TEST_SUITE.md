# Phase 5: End-to-End User Journey Test Suite

## Overview
Comprehensive test suite for the Native Solana betting platform covering all user journeys from onboarding to advanced trading strategies.

## Test Environment Setup

### Prerequisites
- Solana test validator running locally
- Test wallets with SOL and tokens
- Mock oracle data feeds
- Performance monitoring tools

### Test Configuration
```typescript
export const TEST_CONFIG = {
  VALIDATOR_URL: "http://localhost:8899",
  WEBSOCKET_URL: "ws://localhost:8900",
  COMMITMENT: "confirmed",
  AIRDROP_AMOUNT: 100, // SOL
  TEST_TIMEOUT: 60000, // 60 seconds
  PERFORMANCE_TARGETS: {
    CREATE_MARKET: 2000, // 2 seconds
    PLACE_BET: 500, // 500ms
    SETTLE_MARKET: 5000, // 5 seconds
    CHAIN_EXECUTION: 3000, // 3 seconds
  }
};
```

## User Journey 1: New User Onboarding

### Test ID: UJ-001
**Scenario**: First-time user creates wallet and places first bet

```typescript
describe("New User Onboarding Journey", () => {
  test("Complete onboarding flow", async () => {
    // 1. Create new wallet
    const wallet = await createTestWallet();
    expect(wallet.publicKey).toBeDefined();
    
    // 2. Airdrop test SOL
    const airdropSig = await connection.requestAirdrop(
      wallet.publicKey,
      TEST_CONFIG.AIRDROP_AMOUNT * LAMPORTS_PER_SOL
    );
    await connection.confirmTransaction(airdropSig);
    
    // 3. Connect to platform
    const session = await platform.connect(wallet);
    expect(session.isConnected).toBe(true);
    
    // 4. Browse markets
    const markets = await platform.getActiveMarkets();
    expect(markets.length).toBeGreaterThan(0);
    
    // 5. Select a binary market
    const binaryMarket = markets.find(m => m.outcomeCount === 2);
    expect(binaryMarket).toBeDefined();
    
    // 6. Place first bet
    const betResult = await platform.placeBet({
      market: binaryMarket.publicKey,
      outcome: 0,
      amount: 1 * LAMPORTS_PER_SOL,
      slippage: 0.01
    });
    
    expect(betResult.success).toBe(true);
    expect(betResult.txSignature).toBeDefined();
    
    // 7. Check position
    const position = await platform.getUserPosition(
      wallet.publicKey,
      binaryMarket.publicKey
    );
    expect(position.shares).toBeGreaterThan(0);
  });
});
```

## User Journey 2: Market Creator Flow

### Test ID: UJ-002
**Scenario**: User creates and manages a prediction market

```typescript
describe("Market Creator Journey", () => {
  test("Create and manage market lifecycle", async () => {
    const creator = await getTestWallet("creator");
    
    // 1. Create market proposal
    const proposal = {
      question: "Will BTC reach $100k by Dec 31?",
      outcomes: ["Yes", "No"],
      endTime: Date.now() + 7 * 24 * 60 * 60 * 1000, // 7 days
      category: "Crypto",
      oracleSource: "polymarket",
      initialLiquidity: 100 * LAMPORTS_PER_SOL
    };
    
    const createResult = await platform.createMarket(creator, proposal);
    expect(createResult.marketId).toBeDefined();
    
    // 2. Add initial liquidity
    const liquidityResult = await platform.addLiquidity({
      market: createResult.marketId,
      amount: proposal.initialLiquidity,
      weights: [0.5, 0.5] // Equal weights
    });
    expect(liquidityResult.lpTokens).toBeGreaterThan(0);
    
    // 3. Monitor market activity
    await sleep(5000); // Wait for trades
    const marketStats = await platform.getMarketStats(createResult.marketId);
    expect(marketStats.volume).toBeGreaterThan(0);
    expect(marketStats.tradersCount).toBeGreaterThan(0);
    
    // 4. Collect fees
    const feesClaimed = await platform.claimCreatorFees(
      creator,
      createResult.marketId
    );
    expect(feesClaimed).toBeGreaterThan(0);
    
    // 5. Resolve market (after end time)
    await fastForwardToEndTime();
    const resolveResult = await platform.resolveMarket(
      creator,
      createResult.marketId,
      0 // Yes wins
    );
    expect(resolveResult.status).toBe("resolved");
  });
});
```

## User Journey 3: Professional Trader - Arbitrage

### Test ID: UJ-003
**Scenario**: Trader executes cross-market arbitrage

```typescript
describe("Arbitrage Trading Journey", () => {
  test("Execute profitable arbitrage", async () => {
    const trader = await getTestWallet("arbitrageur");
    const detector = new ArbitrageDetector();
    
    // 1. Monitor multiple markets
    const markets = await platform.getMarketsWithSameEvent();
    expect(markets.length).toBeGreaterThanOrEqual(2);
    
    // 2. Detect price discrepancy
    const opportunity = await detector.findOpportunity(markets);
    expect(opportunity).toBeDefined();
    expect(opportunity.profitBps).toBeGreaterThan(900); // 9% minimum
    
    // 3. Calculate optimal size
    const optimalSize = detector.calculateOptimalSize(
      opportunity,
      trader.balance
    );
    expect(optimalSize).toBeLessThan(trader.balance * 0.2); // Risk limit
    
    // 4. Execute atomic arbitrage
    const arbResult = await platform.executeArbitrage({
      buyMarket: opportunity.buyMarket,
      sellMarket: opportunity.sellMarket,
      outcome: opportunity.outcome,
      size: optimalSize,
      minProfit: opportunity.expectedProfit * 0.9 // 10% slippage
    });
    
    expect(arbResult.success).toBe(true);
    expect(arbResult.actualProfit).toBeGreaterThan(0);
    
    // 5. Verify positions neutralized
    const positions = await platform.getUserPositions(trader.publicKey);
    const netExposure = positions.reduce((sum, p) => sum + p.size, 0);
    expect(Math.abs(netExposure)).toBeLessThan(100); // Near zero
    
    // 6. Check daily profit
    const dailyPnL = await platform.getDailyPnL(trader.publicKey);
    expect(dailyPnL).toBeGreaterThan(500 * LAMPORTS_PER_SOL); // $500+
  });
});
```

## User Journey 4: Liquidity Provider

### Test ID: UJ-004
**Scenario**: User provides liquidity and earns fees

```typescript
describe("Liquidity Provider Journey", () => {
  test("LP lifecycle with fee collection", async () => {
    const lp = await getTestWallet("liquidityProvider");
    
    // 1. Find high-volume market
    const markets = await platform.getMarketsByVolume();
    const topMarket = markets[0];
    expect(topMarket.volume24h).toBeGreaterThan(10000 * LAMPORTS_PER_SOL);
    
    // 2. Add concentrated liquidity
    const liquidityParams = {
      market: topMarket.publicKey,
      amount: 1000 * LAMPORTS_PER_SOL,
      range: {
        lower: 0.3,
        upper: 0.7
      },
      lockPeriod: 30 * 24 * 60 * 60 // 30 days for bonus
    };
    
    const addResult = await platform.addConcentratedLiquidity(
      lp,
      liquidityParams
    );
    expect(addResult.lpTokens).toBeGreaterThan(0);
    expect(addResult.lockBonus).toBe(1.5); // 50% bonus
    
    // 3. Monitor fee accrual
    await simulateTradingActivity(topMarket, 100); // 100 trades
    
    const fees = await platform.getAccruedFees(lp.publicKey, topMarket.publicKey);
    expect(fees.totalFees).toBeGreaterThan(0);
    expect(fees.apr).toBeGreaterThan(0.05); // 5%+ APR
    
    // 4. Claim fees
    const claimResult = await platform.claimLPFees(lp, topMarket.publicKey);
    expect(claimResult.amount).toBe(fees.claimable);
    
    // 5. Remove liquidity after lock
    await fastForward(30 * 24 * 60 * 60);
    const removeResult = await platform.removeLiquidity(
      lp,
      topMarket.publicKey,
      addResult.lpTokens
    );
    expect(removeResult.returned).toBeGreaterThan(liquidityParams.amount);
  });
});
```

## User Journey 5: Chain Leverage Trader

### Test ID: UJ-005
**Scenario**: Execute 3-step chain for maximum leverage

```typescript
describe("Chain Leverage Journey", () => {
  test("Execute 3-step chain with 180% leverage", async () => {
    const trader = await getTestWallet("chainTrader");
    
    // 1. Identify chain opportunity
    const chainBuilder = new ChainBuilder();
    const opportunities = await chainBuilder.findChainOpportunities();
    const bestChain = opportunities[0];
    expect(bestChain.steps.length).toBe(3);
    expect(bestChain.totalLeverage).toBeGreaterThanOrEqual(180);
    
    // 2. Validate chain feasibility
    const validation = await chainBuilder.validateChain(bestChain);
    expect(validation.isValid).toBe(true);
    expect(validation.estimatedGas).toBeLessThan(20000); // CU limit
    
    // 3. Execute chain atomically
    const chainResult = await platform.executeChain({
      chain: bestChain,
      initialDeposit: 100 * LAMPORTS_PER_SOL,
      slippageTolerance: 0.02,
      deadline: Date.now() + 60000
    });
    
    expect(chainResult.success).toBe(true);
    expect(chainResult.finalPosition).toBeGreaterThan(
      100 * LAMPORTS_PER_SOL * 2.5 // 150%+ profit
    );
    
    // 4. Monitor chain performance
    const chainStats = await platform.getChainStats(chainResult.chainId);
    expect(chainStats.executionTime).toBeLessThan(3000); // 3 seconds
    expect(chainStats.gasUsed).toBeLessThan(18000);
    
    // 5. Exit strategy
    const exitPlan = await chainBuilder.createExitStrategy(
      chainResult.positions
    );
    const exitResult = await platform.executeExitPlan(trader, exitPlan);
    expect(exitResult.totalProfit).toBeGreaterThan(
      100 * LAMPORTS_PER_SOL * 1.8
    );
  });
});
```

## User Journey 6: Mobile User

### Test ID: UJ-006
**Scenario**: Complete mobile app user flow

```typescript
describe("Mobile User Journey", () => {
  test("Mobile wallet connection and trading", async () => {
    // 1. Initialize WalletConnect
    const wcClient = await WalletConnect.init({
      projectId: TEST_CONFIG.WC_PROJECT_ID,
      metadata: {
        name: "Betting Platform",
        description: "Native Solana Prediction Markets",
        url: "https://betting.app",
        icons: ["https://betting.app/icon.png"]
      }
    });
    
    // 2. Connect mobile wallet
    const { uri } = await wcClient.connect();
    const session = await simulateMobileWalletApproval(uri);
    expect(session.namespaces.solana).toBeDefined();
    
    // 3. Browse markets with gestures
    const mobileUI = new MobileUISimulator();
    await mobileUI.swipeUp(); // Open markets
    await mobileUI.pinchToZoom(1.5); // Zoom chart
    
    // 4. Place bet with gesture
    await mobileUI.longPress({ x: 200, y: 300 }); // Select outcome
    await mobileUI.swipeRight(); // Confirm bet
    
    const betResult = await waitForTransaction();
    expect(betResult.success).toBe(true);
    
    // 5. Check position in portfolio
    await mobileUI.navigateToPortfolio();
    const positions = await mobileUI.getDisplayedPositions();
    expect(positions.length).toBeGreaterThan(0);
    
    // 6. Setup price alerts
    const alertResult = await platform.createPriceAlert({
      market: positions[0].market,
      threshold: 0.8,
      type: "above"
    });
    expect(alertResult.alertId).toBeDefined();
  });
});
```

## User Journey 7: MMT Staker

### Test ID: UJ-007
**Scenario**: Stake MMT tokens and earn rebates

```typescript
describe("MMT Staking Journey", () => {
  test("Stake MMT for fee rebates", async () => {
    const staker = await getTestWallet("staker");
    
    // 1. Acquire MMT tokens
    const mmtAmount = 10000 * LAMPORTS_PER_SOL; // 10k MMT
    await platform.swapForMMT(staker, mmtAmount);
    
    // 2. Stake with 90-day lock
    const stakeResult = await platform.stakeMMT({
      amount: mmtAmount,
      lockPeriod: 90 * 24 * 60 * 60, // 90 days
    });
    expect(stakeResult.lockMultiplier).toBe(1.5); // 50% bonus
    
    // 3. Trade and track rebates
    const trades = [];
    for (let i = 0; i < 10; i++) {
      const trade = await platform.placeBet({
        market: getRandomMarket(),
        outcome: 0,
        amount: 100 * LAMPORTS_PER_SOL
      });
      trades.push(trade);
    }
    
    // 4. Check rebate accrual
    const rebateStats = await platform.getRebateStats(staker.publicKey);
    expect(rebateStats.totalRebates).toBeGreaterThan(0);
    expect(rebateStats.effectiveFeeRate).toBeLessThan(0.00255); // 15% off
    
    // 5. Claim rebates
    const claimResult = await platform.claimRebates(staker);
    expect(claimResult.amount).toBe(rebateStats.claimable);
    
    // 6. Check staking tier
    const tier = await platform.getStakingTier(staker.publicKey);
    expect(tier).toBe("Gold"); // 10k MMT = Gold
  });
});
```

## Performance Test Suite

### Test ID: PT-001
**Scenario**: Verify performance targets

```typescript
describe("Performance Tests", () => {
  test("Transaction performance targets", async () => {
    const perfMonitor = new PerformanceMonitor();
    
    // 1. Market creation < 2s
    const createStart = Date.now();
    await platform.createMarket(creator, marketParams);
    const createTime = Date.now() - createStart;
    expect(createTime).toBeLessThan(2000);
    
    // 2. Bet placement < 500ms
    const betStart = Date.now();
    await platform.placeBet(betParams);
    const betTime = Date.now() - betStart;
    expect(betTime).toBeLessThan(500);
    
    // 3. Bulk operations - 8 outcomes in one tx
    const bulkStart = Date.now();
    await platform.placeBulkBets(Array(8).fill(betParams));
    const bulkTime = Date.now() - bulkStart;
    expect(bulkTime).toBeLessThan(2000);
    
    // 4. CU usage verification
    const cuStats = await perfMonitor.getCUStats();
    expect(cuStats.avgPerTrade).toBeLessThan(18000);
    expect(cuStats.maxPerTrade).toBeLessThan(20000);
  });
  
  test("Throughput targets", async () => {
    // 1. Setup 4 shards
    const shards = await platform.getActiveShards();
    expect(shards.length).toBe(4);
    
    // 2. Generate 5000 transactions
    const txGenerator = new TransactionGenerator();
    const txs = await txGenerator.generateRealisticLoad(5000);
    
    // 3. Submit and measure TPS
    const startTime = Date.now();
    const results = await platform.submitBulkTransactions(txs);
    const duration = (Date.now() - startTime) / 1000;
    
    const successCount = results.filter(r => r.success).length;
    const tps = successCount / duration;
    
    expect(tps).toBeGreaterThan(5000); // 5k TPS target
    expect(results.filter(r => !r.success).length).toBeLessThan(50); // <1% failure
  });
});
```

## Error Handling Tests

### Test ID: EH-001
**Scenario**: Graceful error handling

```typescript
describe("Error Handling", () => {
  test("Handle various error scenarios", async () => {
    // 1. Insufficient balance
    await expect(
      platform.placeBet({
        amount: 1000000 * LAMPORTS_PER_SOL, // More than balance
        ...betParams
      })
    ).rejects.toThrow("Insufficient balance");
    
    // 2. Invalid market
    await expect(
      platform.placeBet({
        market: Keypair.generate().publicKey,
        ...betParams
      })
    ).rejects.toThrow("Market not found");
    
    // 3. Slippage protection
    const volatile = await createVolatileMarket();
    await expect(
      platform.placeBet({
        market: volatile.publicKey,
        slippage: 0.001, // 0.1% too tight
        ...betParams
      })
    ).rejects.toThrow("Slippage tolerance exceeded");
    
    // 4. Chain depth limit
    const deepChain = createChainWithDepth(5);
    await expect(
      platform.executeChain(deepChain)
    ).rejects.toThrow("Max chain depth exceeded");
    
    // 5. Oracle timeout
    const staleMarket = await createMarketWithStaleOracle();
    await expect(
      platform.settleMarket(staleMarket)
    ).rejects.toThrow("Oracle data too old");
  });
});
```

## Test Execution Summary

```typescript
async function runComprehensiveTests() {
  console.log("Starting E2E Test Suite...\n");
  
  const testSuites = [
    { name: "User Onboarding", tests: ["UJ-001"] },
    { name: "Market Creation", tests: ["UJ-002"] },
    { name: "Arbitrage Trading", tests: ["UJ-003"] },
    { name: "Liquidity Provision", tests: ["UJ-004"] },
    { name: "Chain Leverage", tests: ["UJ-005"] },
    { name: "Mobile Experience", tests: ["UJ-006"] },
    { name: "MMT Staking", tests: ["UJ-007"] },
    { name: "Performance", tests: ["PT-001"] },
    { name: "Error Handling", tests: ["EH-001"] }
  ];
  
  const results = {
    total: 0,
    passed: 0,
    failed: 0,
    skipped: 0,
    duration: 0
  };
  
  const startTime = Date.now();
  
  for (const suite of testSuites) {
    console.log(`\nRunning ${suite.name}...`);
    
    for (const testId of suite.tests) {
      try {
        await runTest(testId);
        results.passed++;
        console.log(`✅ ${testId} PASSED`);
      } catch (error) {
        results.failed++;
        console.log(`❌ ${testId} FAILED: ${error.message}`);
      }
      results.total++;
    }
  }
  
  results.duration = Date.now() - startTime;
  
  console.log("\n=== Test Results ===");
  console.log(`Total: ${results.total}`);
  console.log(`Passed: ${results.passed} (${(results.passed/results.total*100).toFixed(1)}%)`);
  console.log(`Failed: ${results.failed}`);
  console.log(`Duration: ${(results.duration/1000).toFixed(1)}s`);
  
  return results.failed === 0;
}

// Run the tests
runComprehensiveTests().then(success => {
  process.exit(success ? 0 : 1);
});
```

## Coverage Report

### Code Coverage Targets
- **Overall**: 95%+
- **Core Logic**: 100%
- **AMM Operations**: 98%
- **Chain Execution**: 95%
- **Error Paths**: 90%

### User Journey Coverage
- ✅ New user onboarding
- ✅ Market creation & management
- ✅ Professional trading (arbitrage, chains)
- ✅ Liquidity provision
- ✅ Mobile experience
- ✅ Staking & rewards
- ✅ Error scenarios

## Next Steps
1. Run tests in CI/CD pipeline
2. Add stress testing for edge cases
3. Implement continuous monitoring
4. Create regression test suite
5. Document test maintenance procedures