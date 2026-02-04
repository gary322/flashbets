# Native Solana Betting Platform - Smart Contracts Inventory

## Total Smart Contract Instructions: 92

The betting platform consists of **92 distinct smart contract instructions** organized into the following categories:

## 1. Core System Instructions (5)
- `Initialize` - Initialize global configuration
- `InitializeGenesis` - Initialize genesis parameters  
- `InitializeMmt` - Initialize MMT token
- `GenesisAtomic` - Atomic genesis initialization
- `EmergencyHalt` - Emergency halt system

## 2. Trading Instructions (2)
- `OpenPosition` - Open a new trading position
- `ClosePosition` - Close an existing position

## 3. Fee & Basic Liquidation Instructions (2)
- `DistributeFees` - Distribute trading fees
- `PartialLiquidate` - Partial liquidation of positions

## 4. Chain Execution Instructions (2)
- `AutoChain` - Execute automated chain strategies
- `UnwindChain` - Unwind chain positions

## 5. Safety & Monitoring Instructions (2)
- `CheckCircuitBreakers` - Check circuit breaker conditions
- `MonitorPositionHealth` - Monitor position health status

## 6. AMM Instructions (11)
### LMSR AMM
- `InitializeLmsrMarket` - Initialize LMSR market
- `ExecuteLmsrTrade` - Execute LMSR trade

### PM-AMM 
- `InitializePmammMarket` - Initialize PM-AMM market
- `ExecutePmammTrade` - Execute PM-AMM trade

### L2-AMM
- `InitializeL2AmmMarket` - Initialize L2 AMM market
- `ExecuteL2Trade` - Execute L2 trade
- `UpdateDistribution` - Update distribution weights
- `ResolveContinuous` - Resolve continuous market
- `ClaimContinuous` - Claim winnings from continuous market

### Hybrid AMM
- `InitializeHybridAmm` - Initialize hybrid AMM
- `ExecuteHybridTrade` - Execute hybrid trade

## 7. Advanced Trading Instructions (10)
### Iceberg Orders
- `PlaceIcebergOrder` - Place iceberg order
- `ExecuteIcebergFill` - Execute iceberg fill
- `CancelIcebergOrder` - Cancel iceberg order

### TWAP Orders
- `PlaceTwapOrder` - Place TWAP order
- `ExecuteTwapInterval` - Execute TWAP interval
- `CancelTwapOrder` - Cancel TWAP order

### Dark Pool
- `InitializeDarkPool` - Initialize dark pool
- `PlaceDarkOrder` - Place dark order
- `MatchDarkOrders` - Match dark orders

### Stop Loss
- `CreateAutoStopLoss` - Create auto stop loss

## 8. Security & MEV Protection Instructions (8)
- `InitializeCircuitBreaker` - Initialize circuit breaker
- `InitializeAttackDetector` - Initialize attack detector
- `UpdateAttackStatus` - Update attack detection status
- `ResetAttackDetector` - Reset attack detector
- `ApplyFlashLoanFee` - Apply flash loan fee
- `InitializePriorityQueue` - Initialize priority queue
- `SubmitPriorityTrade` - Submit priority trade
- `ProcessPriorityBatch` - Process priority batch

## 9. Liquidation System Instructions (6)
- `InitializeLiquidationQueue` - Initialize liquidation queue
- `MarkForLiquidation` - Mark position for liquidation
- `ExecuteLiquidation` - Execute liquidation
- `ProcessLiquidationQueue` - Process liquidation queue
- `UnifiedLiquidate` - Unified liquidation handler
- `ChainLiquidate` - Chain position liquidation

## 10. Keeper Network Instructions (8)
- `RegisterKeeper` - Register new keeper
- `UpdateKeeperHealth` - Update keeper health
- `ExecuteKeeperLiquidation` - Execute keeper liquidation
- `ClaimKeeperRewards` - Claim keeper rewards
- `InitializeResolution` - Initialize resolution
- `ProcessResolution` - Process market resolution
- `DisputeResolution` - Dispute resolution
- `UpdatePriceCache` - Update price cache

## 11. MMT Token Instructions (10)
- `StakeMmt` - Stake MMT tokens
- `UnstakeMmt` - Unstake MMT tokens
- `ClaimMmtRewards` - Claim MMT rewards
- `UpdateStakingTiers` - Update staking tiers
- `ProcessMakerRewards` - Process maker rewards
- `DistributeMmt` - Distribute MMT tokens
- `ClaimEarlyTraderBonus` - Claim early trader bonus
- `CreateVestingSchedule` - Create vesting schedule
- `ClaimVestedTokens` - Claim vested tokens
- `ProcessPrelaunchAirdrop` - Process prelaunch airdrop

## 12. Oracle & Resolution Instructions (5)
- `UpdatePolymarketOracle` - Update Polymarket oracle
- `FetchBatchPrices` - Fetch batch prices
- `InitializeMedianOracle` - Initialize median oracle
- `ProcessDisputeEvidence` - Process dispute evidence
- `FinalizeResolution` - Finalize resolution

## 13. Bootstrap Phase Instructions (4)
- `InitializeBootstrap` - Initialize bootstrap phase
- `DepositBootstrap` - Deposit to bootstrap
- `ClaimBootstrapRewards` - Claim bootstrap rewards
- `TransitionFromBootstrap` - Transition from bootstrap

## 14. Credits & Refund Instructions (3)
- `LockCredits` - Lock credits for position
- `UnlockCredits` - Unlock credits
- `ProcessRefund` - Process refund

## 15. Cross-Margin Instructions (2)
- `EnableCrossMargin` - Enable cross margin
- `UpdateCrossMarginSettings` - Update cross margin settings

## Key Features of the Smart Contract Architecture:

1. **Modular Design**: Each instruction handles a specific function
2. **Native Solana**: All contracts use native Solana program patterns
3. **Type Safety**: Strong typing with Borsh serialization
4. **Error Handling**: Comprehensive error types for each operation
5. **Access Control**: Proper authority checks on all operations
6. **State Management**: Efficient PDA-based state storage
7. **Composability**: Instructions can be combined for complex operations

## Contract Sizes & Complexity:
- **Largest Contracts**: AMM implementations (LMSR, PM-AMM, L2-AMM)
- **Most Complex**: Chain execution and liquidation systems
- **Most Critical**: Trading, liquidation, and oracle instructions
- **Security-Focused**: MEV protection and attack detection

This represents a comprehensive DeFi platform with all necessary components for prediction markets, AMMs, liquidations, and token economics.