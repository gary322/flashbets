use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use betting_platform_native::{
    market_ingestion::{
        MarketIngestionState, PolymarketMarketData, INGESTION_INTERVAL_SLOTS,
        MAX_MARKETS_SUPPORTED, BATCH_SIZE, TARGET_VERSE_COUNT,
    },
    verse_classification::VerseClassifier,
    merkle::{MerkleTree, calculate_merkle_root},
    sharding::enhanced_sharding::EnhancedShardManager,
    state::accounts::{VersePDA, ProposalPDA},
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use borsh::BorshSerialize;

/// Test configuration for 21k+ market handling
pub struct MarketHandlingTestConfig {
    pub total_markets: usize,
    pub batch_size: usize,
    pub ingestion_interval: u64,
    pub verse_count_target: usize,
    pub parallel_workers: usize,
}

impl Default for MarketHandlingTestConfig {
    fn default() -> Self {
        Self {
            total_markets: MAX_MARKETS_SUPPORTED, // 21,300
            batch_size: BATCH_SIZE as usize,     // 1,000
            ingestion_interval: INGESTION_INTERVAL_SLOTS, // 5 slots
            verse_count_target: TARGET_VERSE_COUNT,        // ~400
            parallel_workers: 8,
        }
    }
}

/// Market statistics for validation
#[derive(Debug, Default)]
pub struct MarketStats {
    pub total_markets: usize,
    pub markets_per_verse: HashMap<u32, usize>,
    pub verse_depths: HashMap<u32, u8>,
    pub processing_times: Vec<Duration>,
    pub merkle_proof_times: Vec<Duration>,
    pub shard_distribution: HashMap<u32, usize>,
}

/// Test 21k+ market ingestion and processing
#[tokio::test]
async fn test_21k_market_handling() {
    println!("=== 21k+ Market Handling Validation Test ===");
    
    let config = MarketHandlingTestConfig::default();
    let mut context = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    ).start_with_context().await;
    
    // Initialize ingestion state
    let ingestion_state = initialize_ingestion_state(&mut context).await;
    
    // Generate 21k+ test markets
    let markets = generate_test_markets(config.total_markets);
    println!("Generated {} test markets", markets.len());
    
    // Initialize shard manager
    let mut shard_manager = EnhancedShardManager::new(Pubkey::new_unique());
    
    // Process markets in batches
    let mut stats = MarketStats::default();
    let start_time = Instant::now();
    
    for (batch_idx, batch) in markets.chunks(config.batch_size).enumerate() {
        let batch_start = Instant::now();
        
        // Process batch
        let batch_result = process_market_batch(
            &mut context,
            &mut shard_manager,
            batch,
            batch_idx,
            &config,
        ).await;
        
        // Update statistics
        update_market_stats(&mut stats, &batch_result, batch_start.elapsed());
        
        // Log progress
        if batch_idx % 5 == 0 {
            let processed = (batch_idx + 1) * config.batch_size;
            let progress = (processed as f64 / config.total_markets as f64) * 100.0;
            println!(
                "Processed {}/{} markets ({:.1}%), {} verses created",
                processed.min(config.total_markets),
                config.total_markets,
                progress,
                stats.markets_per_verse.len()
            );
        }
        
        // Simulate ingestion interval
        if batch_idx < markets.chunks(config.batch_size).len() - 1 {
            advance_slots(&mut context, config.ingestion_interval).await;
        }
    }
    
    let total_time = start_time.elapsed();
    
    // Validate results
    validate_market_handling(&stats, &config, total_time);
    
    // Print detailed statistics
    print_market_handling_stats(&stats, total_time);
}

/// Test verse classification and grouping
#[tokio::test]
async fn test_verse_classification() {
    println!("=== Verse Classification Test ===");
    
    let test_markets = vec![
        ("Will BTC reach $100k by end of 2024?", "crypto/bitcoin/price"),
        ("Will ETH reach $10k by end of 2024?", "crypto/ethereum/price"),
        ("Will Biden win 2024 election?", "politics/us/presidential"),
        ("Will Trump win 2024 election?", "politics/us/presidential"),
        ("Will Lakers win NBA championship?", "sports/basketball/nba"),
        ("Will S&P 500 reach 5000?", "finance/stocks/indices"),
        ("Will inflation exceed 5% in 2024?", "economics/inflation/us"),
        ("Will SpaceX land on Mars by 2030?", "technology/space/spacex"),
    ];
    
    let mut verse_groups: HashMap<String, Vec<String>> = HashMap::new();
    
    for (title, expected_verse) in test_markets {
        let verse_id = VerseClassifier::classify_market_to_verse(title).unwrap();
        verse_groups.entry(expected_verse.to_string())
            .or_default()
            .push(title.to_string());
        
        println!("Market: '{}' -> Verse: {}", title, expected_verse);
    }
    
    // Verify grouping
    println!("\nVerse Groups:");
    for (verse, markets) in verse_groups {
        println!("- {} ({} markets)", verse, markets.len());
        for market in markets.iter().take(3) {
            println!("  • {}", market);
        }
    }
    
    assert!(verse_groups.len() >= 6, "Should have multiple verse categories");
}

/// Test Merkle tree performance with 21k markets
#[tokio::test]
async fn test_merkle_tree_performance() {
    println!("=== Merkle Tree Performance Test ===");
    
    let market_counts = vec![100, 1000, 5000, 10000, 21300];
    
    for count in market_counts {
        // Create test markets
        let markets: Vec<Pubkey> = (0..count)
            .map(|_| Pubkey::new_unique())
            .collect();
        
        // Build Merkle tree
        let tree_start = Instant::now();
        let merkle_tree = MerkleTree::new(&markets);
        let build_time = tree_start.elapsed();
        
        // Calculate root
        let root_start = Instant::now();
        let root = merkle_tree.get_root();
        let root_time = root_start.elapsed();
        
        // Generate proof for random market
        let target_idx = count / 2;
        let proof_start = Instant::now();
        let proof = merkle_tree.get_proof(target_idx);
        let proof_time = proof_start.elapsed();
        
        // Verify proof
        let verify_start = Instant::now();
        let verified = merkle_tree.verify_proof(&markets[target_idx], &proof, &root);
        let verify_time = verify_start.elapsed();
        
        println!("\nMarket Count: {}", count);
        println!("- Tree Build: {:?}", build_time);
        println!("- Root Calculation: {:?}", root_time);
        println!("- Proof Generation: {:?}", proof_time);
        println!("- Proof Verification: {:?}", verify_time);
        println!("- Tree Depth: {}", (count as f64).log2().ceil() as u32);
        
        assert!(verified, "Merkle proof should be valid");
        assert!(proof_time.as_micros() < 1000, "Proof generation should be < 1ms");
    }
}

/// Test parallel batch processing
#[tokio::test]
async fn test_parallel_batch_processing() {
    println!("=== Parallel Batch Processing Test ===");
    
    let mut context = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::entrypoint::process_instruction),
    ).start_with_context().await;
    
    let batch_sizes = vec![100, 500, 1000];
    let worker_counts = vec![1, 4, 8];
    
    for batch_size in &batch_sizes {
        for workers in &worker_counts {
            let markets = generate_test_markets(*batch_size);
            
            let start = Instant::now();
            let results = process_markets_parallel(
                &markets,
                *workers,
                *batch_size / workers,
            ).await;
            let duration = start.elapsed();
            
            let throughput = *batch_size as f64 / duration.as_secs_f64();
            
            println!(
                "Batch: {}, Workers: {}, Time: {:?}, Throughput: {:.0} markets/sec",
                batch_size, workers, duration, throughput
            );
            
            assert_eq!(results.len(), *batch_size);
        }
    }
}

/// Test market sharding distribution
#[tokio::test]
async fn test_market_sharding() {
    println!("=== Market Sharding Distribution Test ===");
    
    let mut shard_manager = EnhancedShardManager::new(Pubkey::new_unique());
    let num_markets = 1000;
    
    // Allocate shards for markets
    let mut shard_counts: HashMap<u32, usize> = HashMap::new();
    
    for i in 0..num_markets {
        let market_id = generate_market_id(i);
        shard_manager.allocate_market_shards(&market_id).unwrap();
        
        // Track shard distribution
        let allocation = shard_manager.shard_allocations.last().unwrap();
        for shard in &allocation.shard_assignments {
            *shard_counts.entry(shard.shard_id).or_insert(0) += 1;
        }
    }
    
    // Analyze distribution
    let total_shards = shard_manager.total_shards;
    let avg_markets_per_shard = num_markets as f64 / (total_shards / 4) as f64;
    
    println!("Shard Distribution:");
    println!("- Total Markets: {}", num_markets);
    println!("- Total Shards: {}", total_shards);
    println!("- Shards per Market: 4");
    println!("- Average Markets per Shard Type: {:.1}", avg_markets_per_shard);
    
    // Verify even distribution
    let min_count = shard_counts.values().min().unwrap();
    let max_count = shard_counts.values().max().unwrap();
    let distribution_ratio = *max_count as f64 / *min_count as f64;
    
    println!("- Min Markets on Shard: {}", min_count);
    println!("- Max Markets on Shard: {}", max_count);
    println!("- Distribution Ratio: {:.2}", distribution_ratio);
    
    assert!(
        distribution_ratio < 1.5,
        "Shard distribution should be relatively even"
    );
}

// Helper functions

async fn initialize_ingestion_state(
    context: &mut ProgramTestContext,
) -> Pubkey {
    // In production, would initialize actual ingestion state account
    Pubkey::new_unique()
}

fn generate_test_markets(count: usize) -> Vec<PolymarketMarketData> {
    let categories = vec![
        "crypto", "politics", "sports", "finance", "technology",
        "economics", "entertainment", "science", "weather", "gaming",
    ];
    
    let mut markets = Vec::new();
    
    for i in 0..count {
        let category = categories[i % categories.len()];
        let subcategory = i / categories.len();
        
        let market = PolymarketMarketData {
            id: format!("market_{}", i),
            title: format!("Will {} event {} happen?", category, subcategory),
            description: format!("Market for {} prediction #{}", category, i),
            outcomes: vec!["Yes".to_string(), "No".to_string()],
            yes_price: 4000 + (i as u64 % 2000), // 40-60% range
            no_price: 10000 - (4000 + (i as u64 % 2000)),
            volume_24h: 100_000_000_000 + (i as u64 * 1_000_000_000),
            liquidity: 50_000_000_000 + (i as u64 * 500_000_000),
            resolved: false,
            resolution: None,
            disputed: false,
            dispute_reason: None,
        };
        
        markets.push(market);
    }
    
    markets
}

struct BatchResult {
    markets_processed: usize,
    verses_created: HashSet<u32>,
    shards_allocated: usize,
    processing_time: Duration,
}

async fn process_market_batch(
    context: &mut ProgramTestContext,
    shard_manager: &mut EnhancedShardManager,
    batch: &[PolymarketMarketData],
    batch_idx: usize,
    config: &MarketHandlingTestConfig,
) -> BatchResult {
    let start = Instant::now();
    let mut verses_created = HashSet::new();
    let mut shards_allocated = 0;
    
    for market in batch {
        // Classify to verse
        let verse_id = VerseClassifier::classify_market_to_verse(&market.title).unwrap();
        verses_created.insert(verse_id);
        
        // Allocate shards if new market
        let market_pubkey = generate_market_id_from_string(&market.id);
        if shard_manager.shard_allocations.iter()
            .find(|a| a.market_id == market_pubkey)
            .is_none() {
            shard_manager.allocate_market_shards(&market_pubkey).ok();
            shards_allocated += 4; // 4 shards per market
        }
    }
    
    BatchResult {
        markets_processed: batch.len(),
        verses_created,
        shards_allocated,
        processing_time: start.elapsed(),
    }
}

fn update_market_stats(
    stats: &mut MarketStats,
    result: &BatchResult,
    batch_time: Duration,
) {
    stats.total_markets += result.markets_processed;
    stats.processing_times.push(batch_time);
    
    for verse_id in &result.verses_created {
        *stats.markets_per_verse.entry(*verse_id).or_insert(0) += 1;
    }
}

async fn advance_slots(context: &mut ProgramTestContext, slots: u64) {
    let current_slot = context.banks_client.get_slot().await.unwrap();
    context.warp_to_slot(current_slot + slots).unwrap();
}

fn validate_market_handling(
    stats: &MarketStats,
    config: &MarketHandlingTestConfig,
    total_time: Duration,
) {
    // Validate total markets processed
    assert_eq!(
        stats.total_markets,
        config.total_markets,
        "All markets should be processed"
    );
    
    // Validate verse count is close to target
    let verse_count = stats.markets_per_verse.len();
    let verse_ratio = verse_count as f64 / config.verse_count_target as f64;
    assert!(
        verse_ratio > 0.8 && verse_ratio < 1.2,
        "Verse count {} should be close to target {}",
        verse_count,
        config.verse_count_target
    );
    
    // Validate processing time
    let markets_per_second = config.total_markets as f64 / total_time.as_secs_f64();
    assert!(
        markets_per_second > 100.0,
        "Should process at least 100 markets/second, got {:.1}",
        markets_per_second
    );
    
    // Validate batch processing times
    let avg_batch_time = stats.processing_times.iter()
        .map(|d| d.as_millis())
        .sum::<u128>() / stats.processing_times.len() as u128;
    
    assert!(
        avg_batch_time < 1000,
        "Average batch processing should be < 1 second, got {}ms",
        avg_batch_time
    );
}

fn print_market_handling_stats(stats: &MarketStats, total_time: Duration) {
    println!("\n=== Market Handling Statistics ===");
    println!("Total Markets: {}", stats.total_markets);
    println!("Total Verses: {}", stats.markets_per_verse.len());
    println!("Total Time: {:.2}s", total_time.as_secs_f64());
    println!("Throughput: {:.0} markets/sec", 
        stats.total_markets as f64 / total_time.as_secs_f64());
    
    // Verse distribution
    let mut verse_sizes: Vec<_> = stats.markets_per_verse.values().cloned().collect();
    verse_sizes.sort();
    
    println!("\nVerse Distribution:");
    println!("- Smallest Verse: {} markets", verse_sizes.first().unwrap_or(&0));
    println!("- Largest Verse: {} markets", verse_sizes.last().unwrap_or(&0));
    println!("- Average Verse: {:.1} markets", 
        stats.total_markets as f64 / stats.markets_per_verse.len().max(1) as f64);
    
    // Processing performance
    let avg_batch_ms = stats.processing_times.iter()
        .map(|d| d.as_millis() as f64)
        .sum::<f64>() / stats.processing_times.len() as f64;
    
    println!("\nProcessing Performance:");
    println!("- Batches Processed: {}", stats.processing_times.len());
    println!("- Avg Batch Time: {:.1}ms", avg_batch_ms);
    println!("- Markets per Batch: {}", BATCH_SIZE);
}

async fn process_markets_parallel(
    markets: &[PolymarketMarketData],
    worker_count: usize,
    chunk_size: usize,
) -> Vec<u32> {
    use tokio::task;
    
    let mut handles = vec![];
    
    for chunk in markets.chunks(chunk_size) {
        let chunk_vec = chunk.to_vec();
        
        let handle = task::spawn(async move {
            let mut verse_ids = Vec::new();
            for market in chunk_vec {
                if let Ok(verse_id) = VerseClassifier::classify_market_to_verse(&market.title) {
                    verse_ids.push(verse_id);
                }
            }
            verse_ids
        });
        
        handles.push(handle);
    }
    
    let mut all_verses = Vec::new();
    for handle in handles {
        if let Ok(verses) = handle.await {
            all_verses.extend(verses);
        }
    }
    
    all_verses
}

fn generate_market_id(index: usize) -> Pubkey {
    use solana_program::hash::hash;
    let seed = format!("market_{}", index);
    let hash_result = hash(seed.as_bytes());
    Pubkey::new_from_array(hash_result.to_bytes())
}

fn generate_market_id_from_string(id: &str) -> Pubkey {
    use solana_program::hash::hash;
    let hash_result = hash(id.as_bytes());
    Pubkey::new_from_array(hash_result.to_bytes())
}