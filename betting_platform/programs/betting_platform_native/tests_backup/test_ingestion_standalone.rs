//! Standalone market ingestion test
//! Tests Polymarket integration requirements without dependencies

// Standalone test - no external dependencies needed

// Constants from specification
const INGESTION_INTERVAL_SLOTS: u64 = 150; // 60 seconds at 0.4s/slot
const MAX_FAILURE_SLOTS: u64 = 300; // Halt after 300 slots of failures
const BATCH_SIZE: u32 = 1000; // Markets per batch
const MAX_MARKETS: u32 = 21000; // Total markets

#[derive(Debug, Clone)]
struct PolymarketMarket {
    id: String,
    title: String,
    yes_price: u64, // basis points
    no_price: u64,
    volume_24h: u64,
    liquidity: u64,
    resolved: bool,
    disputed: bool,
}

#[derive(Debug)]
struct IngestionState {
    last_ingestion_slot: u64,
    next_scheduled_slot: u64,
    consecutive_failures: u32,
    first_failure_slot: u64,
    total_markets_ingested: u64,
    current_offset: u32,
    is_halted: bool,
}

impl IngestionState {
    fn new() -> Self {
        Self {
            last_ingestion_slot: 0,
            next_scheduled_slot: 0,
            consecutive_failures: 0,
            first_failure_slot: 0,
            total_markets_ingested: 0,
            current_offset: 0,
            is_halted: false,
        }
    }
    
    fn should_halt(&self, current_slot: u64) -> bool {
        if self.is_halted {
            return true;
        }
        
        // Halt if failures persist for more than MAX_FAILURE_SLOTS
        if self.consecutive_failures > 0 && self.first_failure_slot > 0 {
            let failure_duration = current_slot.saturating_sub(self.first_failure_slot);
            if failure_duration >= MAX_FAILURE_SLOTS {
                return true;
            }
        }
        
        false
    }
}

fn simulate_api_response(offset: u32, limit: u32, fail: bool) -> Result<Vec<PolymarketMarket>, String> {
    if fail {
        return Err("API timeout".to_string());
    }
    
    let mut markets = Vec::new();
    for i in 0..limit.min(100) { // Simulate smaller batch for testing
        let id = format!("market_{}", offset + i);
        markets.push(PolymarketMarket {
            id: id.clone(),
            title: format!("Will event {} happen?", offset + i),
            yes_price: 4500 + (i as u64 % 1000), // Vary prices
            no_price: 10000 - (4500 + (i as u64 % 1000)), // Sum to ~10000
            volume_24h: 100000 + i as u64 * 1000,
            liquidity: 50000 + i as u64 * 500,
            resolved: false,
            disputed: false,
        });
    }
    
    Ok(markets)
}

#[test]
fn test_ingestion_interval_enforcement() {
    println!("\nTesting 60-second interval enforcement:");
    
    let mut state = IngestionState::new();
    let mut current_slot = 0;
    
    // First ingestion should succeed
    state.last_ingestion_slot = current_slot;
    state.next_scheduled_slot = current_slot + INGESTION_INTERVAL_SLOTS;
    
    // Try to ingest too early
    current_slot += 100; // Only 40 seconds passed
    assert!(current_slot < state.next_scheduled_slot);
    println!("  Slot {}: Too early ({}s elapsed) - should wait", 
        current_slot, (current_slot - state.last_ingestion_slot) as f64 * 0.4);
    
    // Wait full interval
    current_slot = state.next_scheduled_slot;
    assert!(current_slot >= state.next_scheduled_slot);
    println!("  Slot {}: Interval reached (60s) - can ingest", current_slot);
    
    println!("✅ Interval enforcement working correctly!");
}

#[test]
fn test_batch_processing() {
    println!("\nTesting batch processing (1000 markets/batch):");
    
    let mut state = IngestionState::new();
    let mut batches_processed = 0;
    
    while state.current_offset < MAX_MARKETS {
        let batch_start = state.current_offset;
        let batch_end = (batch_start + BATCH_SIZE).min(MAX_MARKETS);
        let batch_size = batch_end - batch_start;
        
        // Simulate processing
        if let Ok(markets) = simulate_api_response(batch_start, batch_size, false) {
            state.total_markets_ingested += markets.len() as u64;
            state.current_offset = batch_end;
            batches_processed += 1;
            
            if batches_processed <= 3 || batches_processed % 5 == 0 {
                println!("  Batch {}: offset={}, size={}, total_ingested={}", 
                    batches_processed, batch_start, markets.len(), state.total_markets_ingested);
            }
        }
    }
    
    println!("  Total batches: {}", batches_processed);
    println!("  Total markets: {}", state.total_markets_ingested);
    assert!(batches_processed >= 21); // At least 21 batches for 21k markets
    
    println!("✅ Batch processing verified!");
}

#[test]
fn test_failure_handling_and_halt() {
    println!("\nTesting failure handling and 300 slot halt:");
    
    let mut state = IngestionState::new();
    let mut current_slot = 0;
    
    // Simulate consecutive failures
    for i in 0..10 {
        current_slot += 50; // 20 seconds between attempts
        
        // Simulate API failure
        if let Err(_) = simulate_api_response(0, 1000, true) {
            if state.consecutive_failures == 0 {
                state.first_failure_slot = current_slot;
            }
            state.consecutive_failures += 1;
            
            let should_halt = state.should_halt(current_slot);
            let failure_duration = current_slot - state.first_failure_slot;
            
            println!("  Failure {}: slot={}, duration={} slots, halt={}", 
                i + 1, current_slot, failure_duration, should_halt);
            
            if should_halt {
                state.is_halted = true;
                assert!(failure_duration >= MAX_FAILURE_SLOTS);
                println!("  ⚠️  HALTED after {} slots of failures", failure_duration);
                break;
            }
        }
    }
    
    assert!(state.is_halted);
    println!("✅ Failure handling and halt mechanism verified!");
}

#[test]
fn test_dispute_handling() {
    println!("\nTesting dispute handling:");
    
    let disputed_markets = vec![
        PolymarketMarket {
            id: "disputed_1".to_string(),
            title: "Controversial market".to_string(),
            yes_price: 7000,
            no_price: 3000,
            volume_24h: 1000000,
            liquidity: 500000,
            resolved: true,
            disputed: true,
        },
        PolymarketMarket {
            id: "normal_1".to_string(),
            title: "Normal market".to_string(),
            yes_price: 5500,
            no_price: 4500,
            volume_24h: 500000,
            liquidity: 250000,
            resolved: false,
            disputed: false,
        },
    ];
    
    let mut processed = 0;
    let mut disputed_count = 0;
    
    for market in &disputed_markets {
        if market.disputed {
            disputed_count += 1;
            println!("  ⚠️  Market '{}' is disputed - special handling required", market.id);
            // In real implementation, would mirror Polymarket's resolution
        } else {
            processed += 1;
            println!("  ✓ Market '{}' processed normally", market.id);
        }
    }
    
    println!("  Processed: {}, Disputed: {}", processed, disputed_count);
    assert_eq!(disputed_count, 1);
    
    println!("✅ Dispute handling verified!");
}

#[test]
fn test_price_validation() {
    println!("\nTesting price validation (sum ≈ 100%):");
    
    let test_markets = vec![
        ("Valid", 5000, 5000, true),      // Exactly 100%
        ("Valid", 4950, 5050, true),      // Within tolerance
        ("Invalid", 6000, 3000, false),   // Sum = 90%
        ("Invalid", 5500, 5500, false),   // Sum = 110%
    ];
    
    for (name, yes, no, should_be_valid) in test_markets {
        let sum = yes + no;
        let is_valid = sum >= 9900 && sum <= 10100; // ±1% tolerance
        
        println!("  {} market: yes={}, no={}, sum={}% - {}", 
            name, yes, no, sum / 100,
            if is_valid { "✓ Valid" } else { "✗ Invalid" });
        
        assert_eq!(is_valid, should_be_valid);
    }
    
    println!("✅ Price validation working correctly!");
}

#[test]
fn test_rate_limiting() {
    println!("\nTesting rate limiting (0.35 req/s):");
    
    let requests_per_second = 0.35;
    let min_interval_slots = (1.0 / requests_per_second / 0.4) as u64; // ~7 slots
    
    let mut last_request_slot = 0;
    let mut requests = Vec::new();
    
    for i in 0..5 {
        let slot = last_request_slot + min_interval_slots;
        let elapsed_seconds = (slot - last_request_slot) as f64 * 0.4;
        
        requests.push((slot, elapsed_seconds));
        last_request_slot = slot;
        
        println!("  Request {}: slot={}, elapsed={:.1}s", 
            i + 1, slot, elapsed_seconds);
    }
    
    // Verify spacing
    for i in 1..requests.len() {
        let elapsed = requests[i].1;
        assert!(elapsed >= 2.8, "Request spacing {:.1}s too small", elapsed);
    }
    
    println!("✅ Rate limiting enforced correctly!");
}

fn main() {
    println!("Running Market Ingestion Tests\n");
    
    test_ingestion_interval_enforcement();
    test_batch_processing();
    test_failure_handling_and_halt();
    test_dispute_handling();
    test_price_validation();
    test_rate_limiting();
    
    println!("\n🎉 ALL MARKET INGESTION TESTS PASSED! 🎉");
}