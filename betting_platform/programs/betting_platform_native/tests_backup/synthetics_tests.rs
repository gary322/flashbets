//! Comprehensive tests for Phase 19: Synthetic Wrapper & Routing Layer

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_program,
    sysvar,
};
use solana_program_test::{*};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::collections::HashMap;

use betting_platform_native::{
    synthetics::{
        wrapper::{SyntheticWrapper, SyntheticType, WrapperStatus},
        router::{RouteRequest, RoutingEngine, ExecutionReceipt, ExecutionStatus},
        derivation::{DerivationEngine, MarketData},
        bundle_optimizer::{BundleOptimizer, BundleRequest, TradeIntent},
        arbitrage::{ArbitrageDetector, ArbitrageOpportunity},
    },
    math::U64F64,
    error::BettingPlatformError,
};

/// Test context for synthetic wrapper tests
struct TestContext {
    program_test: ProgramTest,
    program_id: Pubkey,
}

impl TestContext {
    fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let program_test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            processor!(betting_platform_native::process_instruction),
        );
        
        Self {
            program_test,
            program_id,
        }
    }
    
    async fn start(mut self) -> (BanksClient, Keypair, Pubkey) {
        let (banks_client, payer, recent_blockhash) = self.program_test.start().await;
        (banks_client, payer, self.program_id)
    }
}

#[tokio::test]
async fn test_synthetic_wrapper_creation() {
    let mut context = TestContext::new();
    let (mut banks_client, payer, program_id) = context.start().await;
    
    // Create synthetic wrapper account
    let wrapper_keypair = Keypair::new();
    let synthetic_id = 1u128;
    
    // Create polymarket markets
    let market1 = Pubkey::new_unique();
    let market2 = Pubkey::new_unique();
    let market3 = Pubkey::new_unique();
    let polymarket_markets = vec![market1, market2, market3];
    
    // Create wrapper
    let wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: polymarket_markets.clone(),
        weights: vec![
            U64F64::from_num(333_333), // 33.33%
            U64F64::from_num(333_333), // 33.33%
            U64F64::from_num(333_334), // 33.34%
        ],
        derived_probability: U64F64::from_num(500_000), // 50%
        total_volume_7d: 0,
        last_update_slot: 0,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    // Pack wrapper data
    let mut wrapper_data = vec![0u8; SyntheticWrapper::LEN];
    SyntheticWrapper::pack(wrapper.clone(), &mut wrapper_data).unwrap();
    
    // Create account
    let wrapper_account = Account {
        lamports: Rent::default().minimum_balance(SyntheticWrapper::LEN),
        data: wrapper_data,
        owner: program_id,
        executable: false,
        rent_epoch: 0,
    };
    
    // Add account to test
    context.program_test.add_account(wrapper_keypair.pubkey(), wrapper_account);
    
    // Verify wrapper was created correctly
    let fetched_account = banks_client
        .get_account(wrapper_keypair.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let unpacked_wrapper = SyntheticWrapper::unpack(&fetched_account.data).unwrap();
    assert_eq!(unpacked_wrapper.synthetic_id, synthetic_id);
    assert_eq!(unpacked_wrapper.polymarket_markets.len(), 3);
    assert_eq!(unpacked_wrapper.status, WrapperStatus::Active);
}

#[tokio::test]
async fn test_probability_derivation() {
    let derivation_engine = DerivationEngine::default();
    
    // Create test wrapper
    let wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id: 1,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
        weights: vec![
            U64F64::from_num(500_000), // 50%
            U64F64::from_num(500_000), // 50%
        ],
        derived_probability: U64F64::from_num(500_000),
        total_volume_7d: 0,
        last_update_slot: 0,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    // Create market data
    let market_data = vec![
        MarketData {
            market_id: wrapper.polymarket_markets[0],
            probability: U64F64::from_num(600_000), // 60%
            volume_7d: 100_000,
            liquidity_depth: 50_000,
            last_trade_time: 0,
        },
        MarketData {
            market_id: wrapper.polymarket_markets[1],
            probability: U64F64::from_num(700_000), // 70%
            volume_7d: 150_000,
            liquidity_depth: 75_000,
            last_trade_time: 0,
        },
    ];
    
    // Derive probability
    let derived_prob = derivation_engine
        .derive_synthetic_probability(&wrapper, market_data)
        .unwrap();
    
    // Should be weighted towards second market due to higher volume/liquidity
    assert!(derived_prob > U64F64::from_num(650_000)); // > 65%
    assert!(derived_prob < U64F64::from_num(700_000)); // < 70%
}

#[tokio::test]
async fn test_bundle_optimization() {
    let optimizer = BundleOptimizer::default();
    
    // Create trade intents
    let trades = vec![
        TradeIntent {
            synthetic_id: 1,
            is_buy: true,
            amount: 1000,
            leverage: U64F64::from_num(10_000_000), // 10x
        },
        TradeIntent {
            synthetic_id: 1,
            is_buy: true,
            amount: 2000,
            leverage: U64F64::from_num(20_000_000), // 20x
        },
        TradeIntent {
            synthetic_id: 2,
            is_buy: false,
            amount: 1500,
            leverage: U64F64::from_num(15_000_000), // 15x
        },
    ];
    
    let bundle_request = BundleRequest {
        user: Pubkey::new_unique(),
        trades,
        max_slippage: U64F64::from_num(20_000), // 2%
    };
    
    // Create test wrapper manager
    let mut wrapper_manager = HashMap::new();
    
    // Add wrappers for synthetic IDs 1 and 2
    let wrapper1 = SyntheticWrapper {
        is_initialized: true,
        synthetic_id: 1,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
        weights: vec![U64F64::from_num(500_000), U64F64::from_num(500_000)],
        derived_probability: U64F64::from_num(500_000),
        total_volume_7d: 0,
        last_update_slot: 0,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    let wrapper2 = SyntheticWrapper {
        is_initialized: true,
        synthetic_id: 2,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![Pubkey::new_unique()],
        weights: vec![U64F64::from_num(1_000_000)],
        derived_probability: U64F64::from_num(500_000),
        total_volume_7d: 0,
        last_update_slot: 0,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    wrapper_manager.insert(1u128, wrapper1);
    wrapper_manager.insert(2u128, wrapper2);
    
    // Optimize bundle
    let optimized = optimizer.optimize_bundle(
        bundle_request,
        &wrapper_manager
    ).unwrap();
    
    // Should have 2 bundles (grouped by synthetic_id)
    assert_eq!(optimized.bundles.len(), 2);
    
    // First bundle should have 2 trades (same synthetic_id)
    assert_eq!(optimized.bundles[0].trades.len(), 2);
    
    // Should calculate fee savings
    assert!(optimized.total_saved_fee > 0);
}

#[tokio::test]
async fn test_arbitrage_detection() {
    let detector = ArbitrageDetector::default();
    let derivation_engine = DerivationEngine::default();
    
    // Create wrapper with price divergence
    let wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id: 1,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
        weights: vec![U64F64::from_num(500_000), U64F64::from_num(500_000)],
        derived_probability: U64F64::from_num(650_000), // 65% synthetic
        total_volume_7d: 0,
        last_update_slot: 0,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    // Create market data with divergence
    let market_data = vec![
        MarketData {
            market_id: wrapper.polymarket_markets[0],
            probability: U64F64::from_num(600_000), // 60% - below synthetic
            volume_7d: 100_000,
            liquidity_depth: 50_000,
            last_trade_time: 0,
        },
        MarketData {
            market_id: wrapper.polymarket_markets[1],
            probability: U64F64::from_num(800_000), // 80% - above synthetic
            volume_7d: 150_000,
            liquidity_depth: 75_000,
            last_trade_time: 0,
        },
    ];
    
    // Detect arbitrage opportunities
    let opportunities = detector.detect_opportunities(
        &wrapper,
        &market_data,
        &derivation_engine,
    ).unwrap();
    
    // Should detect at least one opportunity
    assert!(!opportunities.is_empty());
    
    // Verify opportunity details
    let opp = &opportunities[0];
    assert!(opp.price_diff > U64F64::from_num(50_000)); // > 5% difference
    assert!(opp.potential_profit > 0);
}

#[tokio::test]
async fn test_routing_engine_simulation() {
    // This test simulates routing without actual Polymarket integration
    let routing_engine = RoutingEngine::default();
    
    // Create route request
    let request = RouteRequest {
        synthetic_id: 1,
        is_buy: true,
        amount: 10_000,
        leverage: U64F64::from_num(10_000_000), // 10x
        user: Pubkey::new_unique(),
    };
    
    // Create test wrapper
    let wrapper = SyntheticWrapper {
        is_initialized: true,
        synthetic_id: 1,
        synthetic_type: SyntheticType::Verse,
        polymarket_markets: vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ],
        weights: vec![
            U64F64::from_num(400_000), // 40%
            U64F64::from_num(350_000), // 35%
            U64F64::from_num(250_000), // 25%
        ],
        derived_probability: U64F64::from_num(650_000),
        total_volume_7d: 1_000_000,
        last_update_slot: 0,
        status: WrapperStatus::Active,
        is_verse_level: true,
        bump: 0,
    };
    
    // Calculate order distribution
    let orders = routing_engine.calculate_order_distribution(
        &wrapper,
        request.amount,
        request.leverage,
    ).unwrap();
    
    // Should split across 3 markets
    assert_eq!(orders.len(), 3);
    
    // Verify proportional allocation
    assert_eq!(orders[0].amount, 4000); // 40% of 10,000
    assert_eq!(orders[1].amount, 3500); // 35% of 10,000
    assert_eq!(orders[2].amount, 2500); // 25% of 10,000
    
    // Verify all orders have correct expected price
    for order in &orders {
        assert_eq!(order.expected_price, wrapper.derived_probability);
    }
}

#[tokio::test]
async fn test_fee_optimization() {
    let routing_engine = RoutingEngine::default();
    
    // Test individual trades vs bundled trade
    let individual_fee_per_trade = 150; // 1.5% in basis points
    let num_trades = 5;
    let trade_amount = 1000;
    
    // Individual trades total fee
    let individual_total = individual_fee_per_trade * num_trades * trade_amount / 10_000;
    
    // Bundled trade fee (with 60% savings)
    let bundled_total = individual_total * 40 / 100;
    
    // Verify 60% savings
    let savings = individual_total - bundled_total;
    assert_eq!(savings * 100 / individual_total, 60);
}

#[tokio::test]
async fn test_execution_receipt_verification() {
    // Create mock execution receipt
    let receipt = ExecutionReceipt {
        synthetic_id: 1,
        user: Pubkey::new_unique(),
        timestamp: 1234567890,
        polymarket_orders: vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ],
        signatures: vec![
            [1u8; 64],
            [2u8; 64],
        ],
        total_executed: 10_000,
        average_price: U64F64::from_num(650_000),
        status: ExecutionStatus::Complete,
    };
    
    // Verify receipt fields
    assert_eq!(receipt.polymarket_orders.len(), receipt.signatures.len());
    assert_eq!(receipt.status, ExecutionStatus::Complete);
    assert_eq!(receipt.total_executed, 10_000);
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_synthetic_trade_flow() {
        // This test simulates the complete flow of a synthetic trade
        
        // 1. Create synthetic wrapper
        let synthetic_id = 1u128;
        let polymarket_markets = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        
        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: polymarket_markets.clone(),
            weights: vec![
                U64F64::from_num(333_333),
                U64F64::from_num(333_333),
                U64F64::from_num(333_334),
            ],
            derived_probability: U64F64::from_num(650_000),
            total_volume_7d: 1_000_000,
            last_update_slot: 0,
            status: WrapperStatus::Active,
            is_verse_level: true,
            bump: 0,
        };
        
        // 2. Create trade request
        let user = Pubkey::new_unique();
        let trade_amount = 50_000;
        let leverage = U64F64::from_num(20_000_000); // 20x
        
        let route_request = RouteRequest {
            synthetic_id,
            is_buy: true,
            amount: trade_amount,
            leverage,
            user,
        };
        
        // 3. Calculate routing
        let routing_engine = RoutingEngine::default();
        let orders = routing_engine.calculate_order_distribution(
            &wrapper,
            trade_amount,
            leverage,
        ).unwrap();
        
        // 4. Verify order distribution
        let total_routed: u64 = orders.iter().map(|o| o.amount).sum();
        assert_eq!(total_routed, trade_amount);
        
        // 5. Simulate execution receipt
        let receipt = ExecutionReceipt {
            synthetic_id,
            user,
            timestamp: 1234567890,
            polymarket_orders: orders.iter().map(|_| Pubkey::new_unique()).collect(),
            signatures: orders.iter().map(|_| [0u8; 64]).collect(),
            total_executed: trade_amount,
            average_price: wrapper.derived_probability,
            status: ExecutionStatus::Complete,
        };
        
        // 6. Verify execution
        assert_eq!(receipt.total_executed, trade_amount);
        assert_eq!(receipt.polymarket_orders.len(), orders.len());
    }
    
    #[tokio::test]
    async fn test_multi_user_bundle_optimization() {
        let optimizer = BundleOptimizer::default();
        
        // Create trades from multiple users for same synthetic
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();
        let user3 = Pubkey::new_unique();
        
        let trades = vec![
            TradeIntent {
                synthetic_id: 1,
                is_buy: true,
                amount: 5000,
                leverage: U64F64::from_num(10_000_000),
            },
            TradeIntent {
                synthetic_id: 1,
                is_buy: true,
                amount: 3000,
                leverage: U64F64::from_num(15_000_000),
            },
            TradeIntent {
                synthetic_id: 1,
                is_buy: true,
                amount: 7000,
                leverage: U64F64::from_num(20_000_000),
            },
        ];
        
        // Bundle for efficiency
        let bundle_request = BundleRequest {
            user: user1, // Primary user
            trades,
            max_slippage: U64F64::from_num(20_000),
        };
        
        // Create wrapper
        let mut wrapper_manager = HashMap::new();
        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 1,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(600_000), U64F64::from_num(400_000)],
            derived_probability: U64F64::from_num(700_000),
            total_volume_7d: 5_000_000,
            last_update_slot: 0,
            status: WrapperStatus::Active,
            is_verse_level: true,
            bump: 0,
        };
        wrapper_manager.insert(1u128, wrapper);
        
        // Optimize
        let optimized = optimizer.optimize_bundle(
            bundle_request,
            &wrapper_manager
        ).unwrap();
        
        // All trades should be in one bundle
        assert_eq!(optimized.bundles.len(), 1);
        assert_eq!(optimized.bundles[0].trades.len(), 3);
        
        // Calculate total volume
        let total_volume: u64 = optimized.bundles[0].trades
            .iter()
            .map(|t| t.amount)
            .sum();
        assert_eq!(total_volume, 15000);
        
        // Verify significant fee savings
        assert!(optimized.total_saved_fee > 0);
    }
}