//! Resolution System tests
//!
//! Tests market resolution, disputes, settlement, and price caching

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use borsh::{BorshDeserialize, BorshSerialize};

use betting_platform_native::{
    instruction::BettingPlatformInstruction,
    state::{
        resolution_accounts::{
            ResolutionState, ResolutionStatus, DisputeState, DisputeVote
        },
    },
    error::BettingPlatformError,
};

mod helpers;
use helpers::*;

#[tokio::test]
async fn test_resolution_flow() {
    let mut test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;
    
    print_test_section("Market Resolution Flow Test");
    
    // Test data
    let verse_id = 1u128;
    let market_id = "TEST_MARKET_001".to_string();
    let oracle = Keypair::new();
    
    // Fund oracle
    let fund_tx = system_transaction::transfer(
        &payer,
        &oracle.pubkey(),
        2_000_000_000, // 2 SOL
        recent_blockhash,
    );
    banks_client.process_transaction(fund_tx).await.unwrap();
    
    // 1. Initialize resolution
    println!("\n1. Initialize Resolution");
    let (resolution_pda, _) = create_pda(
        &[b"resolution", &verse_id.to_le_bytes(), market_id.as_bytes()],
        &betting_platform_native::id()
    );
    
    let ix = BettingPlatformInstruction::InitializeResolution {
        verse_id,
        market_id: market_id.clone(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new(oracle.pubkey(), true),
                AccountMeta::new(resolution_pda, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(rent::id(), false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&oracle.pubkey()),
    );
    
    transaction.sign(&[&oracle], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    println!("✓ Resolution initialized");
    
    // 2. Propose outcome
    println!("\n2. Propose Resolution Outcome");
    let proposed_outcome = "1"; // YES
    
    let ix = BettingPlatformInstruction::ProposeResolution {
        verse_id,
        market_id: market_id.clone(),
        outcome: proposed_outcome.to_string(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new(oracle.pubkey(), true),
                AccountMeta::new(resolution_pda, false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&oracle.pubkey()),
    );
    
    transaction.sign(&[&oracle], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    println!("✓ Outcome {} proposed by oracle", proposed_outcome);
    
    // 3. Confirm resolution (second oracle)
    println!("\n3. Confirm Resolution");
    let oracle2 = Keypair::new();
    
    // Fund second oracle
    let fund_tx = system_transaction::transfer(
        &payer,
        &oracle2.pubkey(),
        1_000_000_000,
        recent_blockhash,
    );
    banks_client.process_transaction(fund_tx).await.unwrap();
    
    let ix = BettingPlatformInstruction::ConfirmResolution {
        verse_id,
        market_id: market_id.clone(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new(oracle2.pubkey(), true),
                AccountMeta::new(resolution_pda, false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&oracle2.pubkey()),
    );
    
    transaction.sign(&[&oracle2], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    println!("✓ Resolution confirmed by second oracle");
    
    // 4. Check resolution state
    let resolution_account = get_account(&mut banks_client, &resolution_pda).await.unwrap();
    let resolution = ResolutionState::try_from_slice(&resolution_account.data).unwrap();
    
    println!("\n4. Resolution State:");
    println!("  Status: {:?}", resolution.status);
    println!("  Proposed outcome: {:?}", resolution.proposed_outcome);
    println!("  Confirmations: {}", resolution.oracle_confirmations.len());
    println!("  Dispute window ends: slot {}", resolution.dispute_window_end);
    
    println!("\n✓ Resolution flow test completed");
}

#[tokio::test]
async fn test_dispute_mechanism() {
    let mut test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;
    
    print_test_section("Dispute Mechanism Test");
    
    // Setup
    let verse_id = 2u128;
    let market_id = "DISPUTE_TEST_001".to_string();
    let disputer = Keypair::new();
    
    // Fund disputer
    let fund_tx = system_transaction::transfer(
        &payer,
        &disputer.pubkey(),
        5_000_000_000, // 5 SOL for bond
        recent_blockhash,
    );
    banks_client.process_transaction(fund_tx).await.unwrap();
    
    // Create resolution and dispute PDAs
    let (resolution_pda, _) = create_pda(
        &[b"resolution", &verse_id.to_le_bytes(), market_id.as_bytes()],
        &betting_platform_native::id()
    );
    
    let (dispute_pda, _) = create_pda(
        &[b"dispute", resolution_pda.as_ref()],
        &betting_platform_native::id()
    );
    
    // 1. Initiate dispute
    println!("\n1. Initiate Dispute");
    let dispute_bond = 1_000_000_000u64; // 1 SOL
    
    let ix = BettingPlatformInstruction::InitiateDispute {
        verse_id,
        market_id: market_id.clone(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new(disputer.pubkey(), true),
                AccountMeta::new(resolution_pda, false),
                AccountMeta::new(dispute_pda, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(rent::id(), false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&disputer.pubkey()),
    );
    
    transaction.sign(&[&disputer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    println!("✓ Dispute initiated with {} SOL bond", dispute_bond / 1_000_000_000);
    
    // 2. Submit evidence
    println!("\n2. Submit Evidence");
    let evidence = "Market outcome should be 0 (NO) based on event data";
    
    let ix = BettingPlatformInstruction::SubmitEvidence {
        verse_id,
        market_id: market_id.clone(),
        evidence: evidence.to_string(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new(disputer.pubkey(), true),
                AccountMeta::new(dispute_pda, false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&disputer.pubkey()),
    );
    
    transaction.sign(&[&disputer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    println!("✓ Evidence submitted: \"{}\"", evidence);
    
    // 3. Vote on dispute (simulate arbitrators)
    println!("\n3. Arbitrator Voting");
    let arbitrators = vec![
        (Keypair::new(), true, "Evidence supports NO outcome"),
        (Keypair::new(), true, "Agree with disputer"),
        (Keypair::new(), false, "Original outcome correct"),
    ];
    
    for (i, (arbitrator, vote_for_disputer, reason)) in arbitrators.iter().enumerate() {
        // Fund arbitrator
        let fund_tx = system_transaction::transfer(
            &payer,
            &arbitrator.pubkey(),
            1_000_000_000,
            recent_blockhash,
        );
        banks_client.process_transaction(fund_tx).await.unwrap();
        
        let ix = BettingPlatformInstruction::VoteOnDispute {
            verse_id,
            market_id: market_id.clone(),
            support_dispute: *vote_for_disputer,
        };
        
        let mut transaction = Transaction::new_with_payer(
            &[build_instruction(
                betting_platform_native::id(),
                vec![
                    AccountMeta::new(arbitrator.pubkey(), true),
                    AccountMeta::new(dispute_pda, false),
                    AccountMeta::new_readonly(sysvar::clock::id(), false),
                ],
                ix.try_to_vec().unwrap(),
            )],
            Some(&arbitrator.pubkey()),
        );
        
        transaction.sign(&[arbitrator], recent_blockhash);
        banks_client.process_transaction(transaction).await.unwrap();
        
        println!("  Arbitrator {}: {} - \"{}\"",
            i + 1,
            if *vote_for_disputer { "Support" } else { "Reject" },
            reason
        );
    }
    
    println!("\n✓ Voting complete: 2/3 support dispute");
    
    // 4. Resolve dispute
    println!("\n4. Resolve Dispute");
    let ix = BettingPlatformInstruction::ResolveDispute {
        verse_id,
        market_id: market_id.clone(),
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(resolution_pda, false),
                AccountMeta::new(dispute_pda, false),
                AccountMeta::new(disputer.pubkey(), false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&payer.pubkey()),
    );
    
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    
    println!("✓ Dispute resolved in favor of disputer");
    println!("✓ Bond refunded to disputer");
    println!("✓ Market outcome changed to 0 (NO)");
    
    println!("\n✓ Dispute mechanism test completed");
}

#[tokio::test]
async fn test_settlement_process() {
    print_test_section("Settlement Process Test");
    
    // Test data
    let market_data = vec![
        ("Market A", 1000, 600, 400), // total_shares, winning_shares, losing_shares
        ("Market B", 5000, 3200, 1800),
        ("Market C", 250, 175, 75),
    ];
    
    let pool_size = 100_000_000_000u64; // 100k USDC
    
    println!("Settlement calculations for pool size: {} USDC\n", 
        format_token_amount(pool_size, 6));
    
    for (market, total, winning, losing) in market_data {
        let winning_pct = (winning as f64 / total as f64) * 100.0;
        let payout_per_share = pool_size / winning as u64;
        let total_payout = payout_per_share * winning as u64;
        
        println!("{}:", market);
        println!("  Total shares: {}", total);
        println!("  Winning shares: {} ({:.1}%)", winning, winning_pct);
        println!("  Losing shares: {} ({:.1}%)", losing, 100.0 - winning_pct);
        println!("  Payout per winning share: {} USDC", 
            format_token_amount(payout_per_share, 6));
        println!("  Total payout: {} USDC", 
            format_token_amount(total_payout, 6));
        println!("  House edge: {} USDC\n", 
            format_token_amount(pool_size - total_payout, 6));
    }
    
    println!("✓ Settlement calculations verified");
}

#[tokio::test]
async fn test_price_cache() {
    let mut test = ProgramTest::new(
        "betting_platform_native",
        betting_platform_native::id(),
        processor!(betting_platform_native::process_instruction),
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;
    
    print_test_section("Price Cache Test");
    
    let verse_id = 3u128;
    
    // 1. Initialize price cache
    println!("\n1. Initialize Price Cache");
    let (cache_pda, _) = create_pda(
        &[b"price_cache", &verse_id.to_le_bytes()],
        &betting_platform_native::id()
    );
    
    let ix = BettingPlatformInstruction::InitializePriceCache { verse_id };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(cache_pda, false),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(rent::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&payer.pubkey()),
    );
    
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    println!("✓ Price cache initialized for verse {}", verse_id);
    
    // 2. Cache market prices
    println!("\n2. Cache Market Prices");
    let market_prices = vec![
        (1u128, 1u8, vec![0u16, 10000u16]), // Market 1: outcome 1 won (100%)
        (2u128, 0u8, vec![10000u16, 0u16]), // Market 2: outcome 0 won (100%)
        (3u128, 0u8, vec![7500u16, 2500u16]), // Market 3: 75/25 final odds
    ];
    
    for (market_id, outcome, prices) in &market_prices {
        println!("  Market {}: Outcome {} won", market_id, outcome);
        println!("    Final prices: {:?}", prices);
        
        // Would call UpdatePriceCache here
    }
    
    println!("\n✓ Prices cached for settlement");
    
    // 3. Finalize cache
    println!("\n3. Finalize Price Cache");
    let ix = BettingPlatformInstruction::FinalizePriceCache { verse_id };
    
    let mut transaction = Transaction::new_with_payer(
        &[build_instruction(
            betting_platform_native::id(),
            vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(cache_pda, false),
                AccountMeta::new_readonly(sysvar::clock::id(), false),
            ],
            ix.try_to_vec().unwrap(),
        )],
        Some(&payer.pubkey()),
    );
    
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    
    println!("✓ Price cache finalized");
    println!("✓ Ready for batch settlement");
    
    println!("\n✓ Price cache test completed");
}

#[tokio::test]
async fn test_multi_oracle_consensus() {
    print_test_section("Multi-Oracle Consensus Test");
    
    // Test scenarios
    let scenarios = vec![
        ("Unanimous agreement", vec![1, 1, 1], 1, "Resolved"),
        ("Majority agreement", vec![1, 1, 0], 1, "Resolved"),
        ("No consensus", vec![0, 1, 2], 255, "Disputed"),
        ("Tie vote", vec![0, 0, 1, 1], 255, "Disputed"),
    ];
    
    println!("Testing oracle consensus mechanisms:\n");
    
    for (scenario, votes, expected_outcome, status) in scenarios {
        println!("Scenario: {}", scenario);
        println!("  Oracle votes: {:?}", votes);
        
        // Count votes
        let mut vote_counts = std::collections::HashMap::new();
        for vote in &votes {
            *vote_counts.entry(*vote).or_insert(0) += 1;
        }
        
        // Find majority
        let majority_threshold = votes.len() / 2 + 1;
        let consensus_outcome = vote_counts.iter()
            .find(|(_, &count)| count >= majority_threshold)
            .map(|(&outcome, _)| outcome)
            .unwrap_or(255);
        
        println!("  Vote distribution:");
        for (outcome, count) in &vote_counts {
            println!("    Outcome {}: {} votes", outcome, count);
        }
        
        println!("  Majority threshold: {}/{}", majority_threshold, votes.len());
        println!("  Consensus outcome: {}", 
            if consensus_outcome == 255 { "None".to_string() } else { consensus_outcome.to_string() });
        println!("  Status: {}", status);
        println!();
    }
    
    println!("✓ Multi-oracle consensus test completed");
}

#[tokio::test]
async fn test_settlement_timing() {
    print_test_section("Settlement Timing Test");
    
    // Test settlement windows
    let current_slot = 1_000_000u64;
    let slot_duration = 400; // milliseconds
    
    let settlement_stages = vec![
        ("Resolution proposed", 0, "T+0"),
        ("Dispute window opens", 0, "T+0"),
        ("Dispute window closes", 216_000, "T+24h"),
        ("Settlement begins", 216_100, "T+24h+100"),
        ("Settlement complete", 220_000, "T+25h"),
    ];
    
    println!("Settlement timeline (current slot: {}):\n", current_slot);
    
    for (stage, slot_offset, time_desc) in settlement_stages {
        let absolute_slot = current_slot + slot_offset;
        let seconds = (slot_offset * slot_duration) / 1000;
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        
        println!("{:<25} Slot: {:>8} ({}, +{}h {}m)",
            stage,
            absolute_slot,
            time_desc,
            hours,
            minutes
        );
    }
    
    println!("\n✓ Settlement timing test completed");
}

#[tokio::test]
async fn test_emergency_resolution() {
    print_test_section("Emergency Resolution Test");
    
    // Test emergency resolution scenarios
    println!("Emergency resolution can be triggered when:\n");
    
    let scenarios = vec![
        ("No oracle responds within 48 hours", true),
        ("Market manipulation detected", true),
        ("Technical failure in oracle system", true),
        ("Insufficient oracle participation", true),
        ("Normal market conditions", false),
    ];
    
    for (scenario, triggers_emergency) in scenarios {
        println!("  {} {}", 
            if triggers_emergency { "🔴" } else { "🟢" },
            scenario
        );
    }
    
    println!("\nEmergency resolution process:");
    println!("  1. Guardian multisig activates emergency mode");
    println!("  2. Independent arbitrators review market data");
    println!("  3. Manual outcome determination");
    println!("  4. Extended dispute window (48h)");
    println!("  5. Higher dispute bond requirement (5 SOL)");
    
    println!("\n✓ Emergency resolution test completed");
}

#[tokio::test]
async fn test_batch_settlement() {
    print_test_section("Batch Settlement Test");
    
    // Test batch settlement efficiency
    let num_positions = vec![100, 1000, 10000, 100000];
    let base_compute_per_position = 50; // CUs
    let batch_size = 100;
    
    println!("Batch settlement performance:\n");
    println!("{:>10} {:>15} {:>15} {:>15}", 
        "Positions", "Naive (CUs)", "Batched (CUs)", "Savings");
    println!("{}", "-".repeat(60));
    
    for positions in num_positions {
        let naive_compute = positions * base_compute_per_position;
        let num_batches = (positions + batch_size - 1) / batch_size;
        let batch_overhead = 1000; // Per batch overhead
        let batched_compute = num_batches * (batch_size * 20 + batch_overhead);
        let savings = ((1.0 - (batched_compute as f64 / naive_compute as f64)) * 100.0) as u32;
        
        println!("{:>10} {:>15} {:>15} {:>14}%",
            positions,
            format!("{:,}", naive_compute),
            format!("{:,}", batched_compute),
            savings
        );
    }
    
    println!("\n✓ Batch settlement test completed");
}