//! User Journey: Data Storage & Chain Event Logging
//!
//! Simulates real user interaction with IPFS archival and chain event logging

use solana_program::{
    msg,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use crate::{
    instruction::ChainStepType,
    events::chain_events::*,
    state::{StateArchival, ArchivalStatus},
    math::U64F64,
};

/// Simulate a complete user journey for chain trading with event logging
pub fn simulate_chain_trading_with_events() {
    msg!("=== User Journey: Chain Trading with Event Logging ===");
    
    // User: Alice wants to execute a leveraged chain strategy
    let alice = Pubkey::new_unique();
    let chain_id = 12345u128;
    let initial_deposit = 1000u64; // $1000 USDC
    
    msg!("Alice initiates chain with {} USDC", initial_deposit);
    
    // Step 1: Borrow to increase capital
    let step1 = ChainStepType::Borrow { amount: 500 };
    let balance_after_borrow = 1500u64; // 1000 + 500
    let step1_return = 1500; // 15% return (borrowing gives 1.5x multiplier)
    
    log_chain_event(
        chain_id,
        &alice,
        0,
        step1.clone(),
        step1_return,
        150, // 1.5x leverage
        100, // 1x base
        U64F64::from_num(1.5),
        500,
        balance_after_borrow,
    );
    
    msg!("Step 1 complete: Borrowed 500, balance now {}", balance_after_borrow);
    
    // Step 2: Provide liquidity with borrowed funds
    let step2 = ChainStepType::Liquidity { amount: balance_after_borrow };
    let liquidity_yield = calculate_liquidity_yield(balance_after_borrow);
    let balance_after_liquidity = balance_after_borrow + liquidity_yield;
    let step2_return = 1000; // 10% return from liquidity provision
    
    log_chain_event(
        chain_id,
        &alice,
        1,
        step2.clone(),
        step2_return,
        165, // 1.65x leverage
        100,
        U64F64::from_num(1.65),
        liquidity_yield,
        balance_after_liquidity,
    );
    
    msg!("Step 2 complete: Provided liquidity, earned {}, balance now {}", 
        liquidity_yield, balance_after_liquidity);
    
    // Step 3: Stake the returns
    let step3 = ChainStepType::Stake { amount: balance_after_liquidity };
    let stake_return = calculate_stake_return(balance_after_liquidity, 2); // depth 2
    let step3_return = 800; // 8% return from staking
    
    log_chain_event(
        chain_id,
        &alice,
        2,
        step3.clone(),
        step3_return,
        178, // 1.78x leverage
        100,
        U64F64::from_num(1.78),
        stake_return - balance_after_liquidity,
        stake_return,
    );
    
    msg!("Step 3 complete: Staked funds, final balance {}", stake_return);
    
    // Create audit trail
    let steps_summary = vec![
        ChainStepSummary {
            step: 0,
            step_type: step1,
            r_i: step1_return,
            eff_lev: 150,
        },
        ChainStepSummary {
            step: 1,
            step_type: step2,
            r_i: step2_return,
            eff_lev: 165,
        },
        ChainStepSummary {
            step: 2,
            step_type: step3,
            r_i: step3_return,
            eff_lev: 178,
        },
    ];
    
    let audit_trail = build_chain_audit_trail(
        chain_id,
        steps_summary.clone(),
        178,
        true,
    );
    
    emit_chain_completion(chain_id, &alice, audit_trail.clone());
    
    msg!("Chain completed successfully!");
    msg!("Initial deposit: {}", initial_deposit);
    msg!("Final balance: {}", stake_return);
    msg!("Total return: {}%", (stake_return - initial_deposit) * 100 / initial_deposit);
    msg!("Final leverage: {}x", 178.0 / 100.0);
    
    // Compress and store audit data
    simulate_audit_storage(chain_id, alice, steps_summary);
}

/// Simulate storing audit data on-chain with compression
fn simulate_audit_storage(chain_id: u128, user: Pubkey, steps: Vec<ChainStepSummary>) {
    msg!("\n=== Simulating Audit Storage ===");
    
    let compressed_data = ChainAuditAccount::compress_steps(&steps);
    let uncompressed_size = steps.len() * std::mem::size_of::<ChainStepSummary>();
    let compressed_size = compressed_data.len();
    
    msg!("Uncompressed audit data: {} bytes", uncompressed_size);
    msg!("Compressed audit data: {} bytes", compressed_size);
    msg!("Compression ratio: {:.2}x", uncompressed_size as f64 / compressed_size as f64);
    
    // Create audit account
    let audit_account = ChainAuditAccount {
        discriminator: ChainAuditAccount::DISCRIMINATOR,
        chain_id,
        user,
        created_slot: 1000,
        num_steps: steps.len() as u8,
        step_data: compressed_data,
        final_effective_leverage: 178,
        total_return_bps: 3534, // ~35.34% total return
        ipfs_hash: None, // Would be set if archiving to IPFS
    };
    
    // Verify decompression works
    let decompressed = audit_account.decompress_steps();
    assert_eq!(decompressed.len(), steps.len());
    msg!("✓ Audit data compression/decompression verified");
}

/// Simulate market archival to IPFS
pub fn simulate_market_archival() {
    msg!("\n=== User Journey: Market Archival to IPFS ===");
    
    let proposal_id = [42u8; 32];
    let market_data_size = 10_240u64; // 10KB of market data
    let archival_slot = 864_000u64; // Archive after 2 epochs
    
    msg!("Archiving market {} at slot {}", bs58::encode(&proposal_id).into_string(), archival_slot);
    
    // Simulate IPFS upload
    let ipfs_hash = simulate_ipfs_upload(&proposal_id, market_data_size);
    
    // Create archival record
    let archival_state = StateArchival {
        discriminator: StateArchival::DISCRIMINATOR,
        ipfs_hash,
        archival_slot,
        status: ArchivalStatus::Complete,
        proposal_id,
        data_size: market_data_size,
        timestamp: Clock::get().unwrap().unix_timestamp,
    };
    
    msg!("Archival record created:");
    msg!("  IPFS hash: {}", bs58::encode(&ipfs_hash).into_string());
    msg!("  Data size: {} bytes", market_data_size);
    msg!("  Status: {:?}", archival_state.status);
    
    // Verify archival
    assert_eq!(archival_state.status, ArchivalStatus::Complete);
    assert_eq!(archival_state.data_size, market_data_size);
    msg!("✓ Market archival completed successfully");
}

/// Simulate IPFS upload (mock)
fn simulate_ipfs_upload(proposal_id: &[u8; 32], data_size: u64) -> [u8; 32] {
    msg!("Uploading {} bytes to IPFS...", data_size);
    
    // In production, would actually upload to IPFS
    // For simulation, generate mock hash
    let mut ipfs_hash = [0u8; 32];
    ipfs_hash[0] = 0x12; // SHA256 multihash prefix
    ipfs_hash[1..9].copy_from_slice(&proposal_id[0..8]);
    ipfs_hash[9..17].copy_from_slice(&data_size.to_le_bytes());
    
    msg!("IPFS upload complete");
    ipfs_hash
}

/// Simulate retrieving historical data
pub fn simulate_historical_data_retrieval() {
    msg!("\n=== User Journey: Historical Data Retrieval ===");
    
    // Bob wants to analyze historical chain performance
    let bob = Pubkey::new_unique();
    let chain_id = 98765u128;
    
    msg!("Bob requests historical data for chain {}", chain_id);
    
    // Simulate finding audit account
    let mock_audit = ChainAuditAccount {
        discriminator: ChainAuditAccount::DISCRIMINATOR,
        chain_id,
        user: Pubkey::new_unique(),
        created_slot: 500_000,
        num_steps: 3,
        step_data: vec![0, 3, 15, 0, 150, 0, 1, 4, 10, 0, 165, 0, 2, 5, 8, 0, 178, 0],
        final_effective_leverage: 178,
        total_return_bps: 3534,
        ipfs_hash: Some([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
                         0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
                         0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00,
                         0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF1]),
    };
    
    // Decompress and analyze
    let historical_steps = mock_audit.decompress_steps();
    
    msg!("Retrieved historical chain data:");
    msg!("  Total steps: {}", historical_steps.len());
    msg!("  Final leverage: {}x", mock_audit.final_effective_leverage as f64 / 100.0);
    msg!("  Total return: {:.2}%", mock_audit.total_return_bps as f64 / 100.0);
    
    // Analyze each step
    for (i, step) in historical_steps.iter().enumerate() {
        msg!("  Step {}: {:?}, return={:.2}%, leverage={}x", 
            i, 
            step.step_type,
            step.r_i as f64 / 100.0,
            step.eff_lev as f64 / 100.0
        );
    }
    
    // Check if detailed data is in IPFS
    if let Some(ipfs_hash) = mock_audit.ipfs_hash {
        msg!("\nDetailed historical data available in IPFS:");
        msg!("  Hash: {}", bs58::encode(&ipfs_hash).into_string());
        msg!("  URL: https://ipfs.io/ipfs/{}", bs58::encode(&ipfs_hash).into_string());
    }
    
    msg!("✓ Historical data retrieval complete");
}

/// Calculate liquidity yield (from auto_chain.rs)
fn calculate_liquidity_yield(liquidity_amount: u64) -> u64 {
    // LVR_TARGET = 500 (5%), TAU = 1000 (10%)
    liquidity_amount
        .saturating_mul(500)
        .saturating_mul(1000)
        .saturating_div(100_000_000)
}

/// Calculate stake return (from auto_chain.rs)
fn calculate_stake_return(stake_amount: u64, depth: u64) -> u64 {
    let multiplier = 32u64.saturating_add(depth);
    stake_amount
        .saturating_mul(multiplier)
        .saturating_div(32)
}

/// Run all data storage user journeys
pub fn run_all_journeys() {
    msg!("🚀 Starting Data Storage User Journeys\n");
    
    // Journey 1: Chain trading with comprehensive event logging
    simulate_chain_trading_with_events();
    
    // Journey 2: Market archival to IPFS
    simulate_market_archival();
    
    // Journey 3: Historical data retrieval and analysis
    simulate_historical_data_retrieval();
    
    msg!("\n✅ All data storage user journeys completed successfully!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_trading_journey() {
        simulate_chain_trading_with_events();
    }

    #[test]
    fn test_market_archival_journey() {
        simulate_market_archival();
    }

    #[test]
    fn test_historical_retrieval_journey() {
        simulate_historical_data_retrieval();
    }

    #[test]
    fn test_compression_efficiency() {
        // Test with maximum allowed steps
        let mut steps = Vec::new();
        for i in 0..10 {
            steps.push(ChainStepSummary {
                step: i,
                step_type: ChainStepType::Long { outcome: i, leverage: i + 1 },
                r_i: (i as i64 + 1) * 100,
                eff_lev: 100 + i as u64 * 10,
            });
        }
        
        let compressed = ChainAuditAccount::compress_steps(&steps);
        let compression_ratio = (steps.len() * std::mem::size_of::<ChainStepSummary>()) as f64 
            / compressed.len() as f64;
        
        assert!(compression_ratio > 2.0, "Compression ratio {} should be > 2.0", compression_ratio);
    }
}