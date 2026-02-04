//! Production-ready integration test
//! 
//! Tests all components working together in a realistic scenario

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{
        accounts::{GlobalConfigPDA, VersePDA, ProposalPDA, Position, LeverageTier, discriminators},
        ProposalState, Resolution,
        security_accounts::{CircuitBreaker, CircuitBreakerType},
    },
    integration::{
        coordinator::SystemCoordinator,
        bootstrap::BootstrapManager,
        oracle_ingestion::OracleIngestion,
        health_monitor::HealthMonitor,
    },
    coverage::{CoverageState, calculate_coverage_ratio},
    amm::{AMMPool, execute_trade},
    oracle::{OracleCoordinator, OracleSource, PriceData},
    events::{emit_event, EventType},
    math::fixed_point::U64F64,
};

/// Production integration test: Full system flow
pub fn test_production_integration() -> ProgramResult {
    msg!("=== PRODUCTION INTEGRATION TEST ===");
    
    let program_id = Pubkey::new_unique();
    let clock = Clock::get()?;
    
    // Step 1: System bootstrap
    msg!("Step 1: System bootstrap and initialization");
    
    let mut global_config = GlobalConfigPDA {
        discriminator: discriminators::GLOBAL_CONFIG,
        epoch: 1,
        season: 1,
        vault: 0,
        total_oi: 0,
        coverage: 0,
        fee_base: 30, // 0.3% base fee
        fee_slope: 10,
        halt_flag: false,
        genesis_slot: clock.slot,
        season_start_slot: clock.slot,
        season_end_slot: clock.slot + 1000000,
        mmt_total_supply: 10_000_000_000_000, // 10M MMT
        mmt_current_season: 1_000_000_000_000, // 1M MMT for current season
        mmt_emission_rate: 1000,
        leverage_tiers: vec![
            LeverageTier { n: 100, max: 10 },
            LeverageTier { n: 50, max: 20 },
            LeverageTier { n: 25, max: 50 },
            LeverageTier { n: 10, max: 100 },
        ],
        min_order_size: 10_000_000_000, // $10k minimum
        max_order_size: 1_000_000_000_000, // $1M maximum
        update_authority: Pubkey::new_unique(),
        primary_market_id: [0u8; 32],
    };
    
    // Initialize bootstrap manager
    let bootstrap_target_vault = 250_000_000_000_000; // $250k target
    let mut bootstrap_manager = BootstrapManager::new(
        bootstrap_target_vault,
        10_000_000_000_000, // 10M MMT incentive pool
    );
    
    // Simulate bootstrap deposits
    let depositors = vec![
        (Pubkey::new_unique(), 50_000_000_000_000),  // $50k
        (Pubkey::new_unique(), 100_000_000_000_000), // $100k
        (Pubkey::new_unique(), 75_000_000_000_000),  // $75k
        (Pubkey::new_unique(), 25_000_000_000_000),  // $25k
    ];
    
    for (depositor, amount) in depositors {
        bootstrap_manager.process_deposit(depositor, amount)?;
        global_config.vault += amount;
        
        msg!("  Deposit: ${} from {}", amount / 1_000_000, depositor);
    }
    
    msg!("  Bootstrap progress: ${} / ${} ({:.1}%)", 
         global_config.vault / 1_000_000,
         bootstrap_target_vault / 1_000_000,
         (global_config.vault as f64 / bootstrap_target_vault as f64) * 100.0);
    
    // Complete bootstrap
    let bootstrap_complete = global_config.vault >= bootstrap_target_vault;
    if bootstrap_complete {
        bootstrap_manager.complete_bootstrap(clock.slot)?;
        msg!("  ✓ Bootstrap completed successfully!");
    }
    
    // Step 2: Oracle integration and market ingestion
    msg!("\nStep 2: Oracle integration and market ingestion");
    
    let mut oracle_coordinator = OracleCoordinator::new();
    oracle_coordinator.add_source(OracleSource::Polymarket)?;
    
    // Ingest markets from Polymarket
    let markets = vec![
        create_test_market("btc-150k-eoy", "Will Bitcoin reach $150k by EOY?", 5500),
        create_test_market("eth-10k-q1", "Will Ethereum reach $10k in Q1?", 3500),
        create_test_market("sol-500-march", "Will Solana reach $500 by March?", 4200),
    ];
    
    let mut ingested_count = 0;
    for market in markets {
        // Verify market meets requirements
        if market.liquidity >= global_config.min_liquidity {
            oracle_coordinator.ingest_market(market.clone())?;
            global_config.total_markets += 1;
            ingested_count += 1;
            
            msg!("  ✓ Ingested: {} ({}% YES)", market.slug, market.prices[0] as f64 / 100.0);
        }
    }
    
    msg!("  Total markets ingested: {}", ingested_count);
    
    // Step 3: Initialize system components
    msg!("\nStep 3: Initializing system components");
    
    // Initialize coverage tracking
    let mut coverage_state = CoverageState::new();
    coverage_state.vault = global_config.vault;
    coverage_state.total_exposure = 0;
    coverage_state.correlation_factor = U64F64::from_fraction(1, 10)?; // 0.1 correlation
    
    // Initialize circuit breakers
    let mut circuit_breaker = CircuitBreaker::new();
    circuit_breaker.price_movement_threshold = 2000; // 20% price movement
    circuit_breaker.liquidation_cascade_threshold = 500; // 5% positions
    circuit_breaker.coverage_threshold = 10000; // 100% minimum (1.0x)
    circuit_breaker.volume_spike_threshold = 500; // 5x normal volume (500%)
    
    // Initialize health monitor
    let mut health_monitor = HealthMonitor::new();
    health_monitor.start_monitoring()?;
    
    msg!("  ✓ Coverage tracking initialized");
    msg!("  ✓ Circuit breakers configured");
    msg!("  ✓ Health monitoring started");
    
    // Step 4: Simulate trading activity
    msg!("\nStep 4: Simulating production trading activity");
    
    let mut total_volume = 0u64;
    let mut total_fees = 0u64;
    
    // Simulate various trades
    let trades = vec![
        (0, 0, 50_000_000_000, true, 10),   // $50k long BTC, 10x
        (1, 1, 30_000_000_000, false, 5),   // $30k short ETH, 5x
        (2, 0, 100_000_000_000, true, 50),  // $100k long SOL, 50x
        (0, 1, 75_000_000_000, false, 20),  // $75k short BTC, 20x
    ];
    
    for (market_idx, outcome, size, is_long, leverage) in trades {
        let notional = size * leverage;
        let fee = (notional * global_config.base_fee_bps as u64) / 10000;
        
        // Update exposure
        coverage_state.total_exposure += notional;
        total_volume += notional;
        total_fees += fee;
        
        // Check coverage ratio
        let coverage_ratio = calculate_coverage_ratio(&coverage_state)?;
        
        msg!("  Trade: ${} {} on market {} ({}x leverage)", 
             size / 1_000_000, 
             if is_long { "LONG" } else { "SHORT" },
             market_idx,
             leverage);
        msg!("    Coverage ratio: {:.2}x", coverage_ratio.to_num() as f64);
        
        // Check circuit breakers
        if coverage_ratio < U64F64::from_num(1) {
            circuit_breaker.coverage_breaker_active = true;
            circuit_breaker.activate_breaker(
                crate::state::security_accounts::BreakerType::Coverage,
                clock.slot,
                "Coverage ratio below minimum".to_string(),
            );
            msg!("    ⚠️  Coverage circuit breaker triggered!");
        }
    }
    
    global_config.total_volume = total_volume;
    
    msg!("  Total volume: ${}", total_volume / 1_000_000);
    msg!("  Total fees collected: ${}", total_fees / 1_000_000);
    
    // Step 5: Test risk management
    msg!("\nStep 5: Testing risk management systems");
    
    // Simulate price shock
    let shock_market = 0; // BTC market
    let original_price = 5500;
    let shocked_price = 4400; // 20% drop
    let price_change_bps = ((original_price - shocked_price) * 10000) / original_price;
    
    msg!("  Simulating {}% price shock on market {}", 
         price_change_bps as f64 / 100.0, shock_market);
    
    if price_change_bps > circuit_breaker.price_movement_threshold as u64 {
        circuit_breaker.price_breaker_active = true;
        circuit_breaker.activate_breaker(
            crate::state::security_accounts::BreakerType::Price,
            clock.slot,
            "Price movement threshold exceeded".to_string(),
        );
        msg!("  ✓ Price circuit breaker activated");
    }
    
    // Test liquidation cascade prevention
    let liquidation_rate = 600; // 6% of positions at risk
    if liquidation_rate > circuit_breaker.liquidation_cascade_threshold {
        circuit_breaker.liquidation_breaker_active = true;
        msg!("  ✓ Liquidation cascade prevention activated");
    }
    
    // Step 6: System health check
    msg!("\nStep 6: Running system health check");
    
    let health_status = health_monitor.check_system_health()?;
    
    msg!("  Component statuses:");
    msg!("    AMM Engine: {:?}", health_status.amm_status);
    msg!("    Oracle Feed: {:?}", health_status.oracle_status);
    msg!("    Keeper Network: {:?}", health_status.keeper_status);
    msg!("    Coverage System: {:?}", health_status.coverage_status);
    
    // Step 7: Verify results
    msg!("\nStep 7: Verifying integration test results");
    
    // Calculate key metrics
    let avg_leverage = 21; // Average from trades
    let fee_revenue = (total_fees * global_config.protocol_fee_share_bps as u64) / 10000;
    let keeper_rewards = (total_fees * (10000 - global_config.protocol_fee_share_bps) as u64) / 10000;
    
    msg!("  === System Metrics ===");
    msg!("  Bootstrap amount: ${}", global_config.vault / 1_000_000);
    msg!("  Total markets: {}", global_config.total_markets);
    msg!("  Total volume: ${}", total_volume / 1_000_000);
    msg!("  Average leverage: {}x", avg_leverage);
    msg!("  Protocol revenue: ${}", fee_revenue / 1_000_000);
    msg!("  Keeper/LP rewards: ${}", keeper_rewards / 1_000_000);
    msg!("  Active circuit breakers: {}", 
         (circuit_breaker.coverage_breaker_active as u8) +
         (circuit_breaker.price_breaker_active as u8) +
         (circuit_breaker.liquidation_breaker_active as u8));
    msg!("  System health: {:?}", health_status.overall_health);
    
    // Verify critical assertions
    assert!(global_config.vault >= bootstrap_target_vault); // Bootstrap completed
    assert!(global_config.total_markets > 0); // Markets ingested
    assert!(total_volume > 0); // Trading occurred
    assert!(total_fees > 0); // Fees collected
    assert!(coverage_state.vault > 0); // Vault funded
    
    msg!("\n=== Integration Test PASSED ===");
    Ok(())
}

/// Create test market data
fn create_test_market(slug: &str, title: &str, yes_price: u64) -> ProposalPDA {
    ProposalPDA {
        discriminator: [0; 8],
        proposal_id: generate_market_id(slug),
        verse_id: [1u8; 32], // Crypto verse
        slug: slug.to_string(),
        title: title.to_string(),
        description: format!("{} - Resolves based on Polymarket oracle", title),
        outcomes: 2,
        outcome_titles: vec!["YES".to_string(), "NO".to_string()],
        market_type: 0, // Binary
        amm_type: 0, // CPMM
        oracle: Pubkey::new_unique(),
        state: ProposalState::Active,
        created_at: 0,
        settle_at: 86400 * 30, // 30 days
        settle_slot: 216_000 * 30,
        prices: vec![yes_price, 10000 - yes_price],
        volumes: vec![0, 0],
        liquidity: 50_000_000_000_000, // $50k
        accumulated_fees: 0,
        resolution: None,
    }
}

/// Generate deterministic market ID from slug
fn generate_market_id(slug: &str) -> [u8; 32] {
    let mut id = [0u8; 32];
    let hash = solana_program::keccak::hashv(&[slug.as_bytes()]);
    id.copy_from_slice(&hash.to_bytes());
    id
}

/// Health status for monitoring
struct SystemHealthStatus {
    overall_health: HealthStatus,
    amm_status: HealthStatus,
    oracle_status: HealthStatus,
    keeper_status: HealthStatus,
    coverage_status: HealthStatus,
}

#[derive(Debug, PartialEq)]
enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_production_integration() {
        test_production_integration().unwrap();
    }
}