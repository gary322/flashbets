use solana_program::pubkey::Pubkey;
use solana_program_test::{*};
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
};
use correlation_engine::{
    instruction::{
        initialize_engine, 
        initialize_verse_tracking,
    },
    state::{
        CorrelationEngine,
        VerseTracking,
        CorrelationMatrix,
        VerseTailLoss,
    },
};
use borsh::BorshDeserialize;

#[tokio::test]
async fn test_basic_initialization() {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "correlation_engine",
        program_id,
        processor!(correlation_engine::process),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Step 1: Initialize the correlation engine
    let (engine_pda, _) = Pubkey::find_program_address(
        &[b"correlation_engine"],
        &program_id,
    );

    let init_engine_ix = initialize_engine(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
    );

    let mut transaction = Transaction::new_with_payer(
        &[init_engine_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Verify engine was initialized
    let engine_account = banks_client.get_account(engine_pda).await.unwrap().unwrap();
    println!("Engine account data length: {}", engine_account.data.len());
    println!("Expected CorrelationEngine size: {}", CorrelationEngine::LEN);
    
    // Try to deserialize just the data we need
    let engine_data = &engine_account.data[..CorrelationEngine::LEN];
    let engine = CorrelationEngine::try_from_slice(engine_data).unwrap();
    assert!(engine.is_initialized);
    assert_eq!(engine.authority, payer.pubkey());

    // Step 2: Initialize verse tracking
    let verse_id = [1u8; 16];
    let (verse_tracking_pda, _) = Pubkey::find_program_address(
        &[b"verse_tracking", &verse_id],
        &program_id,
    );
    let (correlation_matrix_pda, _) = Pubkey::find_program_address(
        &[b"correlation_matrix", &verse_id],
        &program_id,
    );
    let (tail_loss_pda, _) = Pubkey::find_program_address(
        &[b"tail_loss", &verse_id],
        &program_id,
    );

    let init_verse_ix = initialize_verse_tracking(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
    );

    let mut transaction = Transaction::new_with_payer(
        &[init_verse_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Verify verse tracking was initialized
    let tracking_account = banks_client.get_account(verse_tracking_pda).await.unwrap().unwrap();
    println!("Tracking account data length: {}", tracking_account.data.len());
    // For VerseTracking, we need to handle dynamic size
    let tracking = VerseTracking::try_from_slice(&tracking_account.data).unwrap();
    assert!(tracking.is_initialized);
    assert_eq!(tracking.verse_id, verse_id);

    // Verify correlation matrix was initialized
    let matrix_account = banks_client.get_account(correlation_matrix_pda).await.unwrap().unwrap();
    println!("Matrix account data length: {}", matrix_account.data.len());
    let matrix = CorrelationMatrix::try_from_slice(&matrix_account.data).unwrap();
    assert!(matrix.is_initialized);
    assert_eq!(matrix.verse_id, verse_id);
    assert_eq!(matrix.correlations.len(), 0); // No correlations yet

    // Verify tail loss was initialized
    let tail_loss_account = banks_client.get_account(tail_loss_pda).await.unwrap().unwrap();
    println!("Tail loss account data length: {}", tail_loss_account.data.len());
    let tail_loss_data = &tail_loss_account.data[..VerseTailLoss::LEN];
    let tail_loss = VerseTailLoss::try_from_slice(tail_loss_data).unwrap();
    assert!(tail_loss.is_initialized);
    assert_eq!(tail_loss.verse_id, verse_id);

    println!("Basic initialization test passed!");
}

#[test]
fn test_correlation_calculations() {
    use correlation_engine::math::correlation::calculate_pearson_correlation;

    // Test perfect positive correlation
    let prices_1 = vec![100_000, 200_000, 300_000, 400_000, 500_000, 600_000, 700_000];
    let prices_2 = prices_1.clone();

    let correlation = calculate_pearson_correlation(&prices_1, &prices_2).unwrap();
    // Should be +1.0, represented as 2*ONE (2_000_000) in our [-1,1] mapping
    assert!(correlation > 1_900_000);

    // Test perfect negative correlation
    let prices_3: Vec<u64> = prices_1.iter()
        .map(|&p| 800_000 - p)
        .collect();

    let correlation = calculate_pearson_correlation(&prices_1, &prices_3).unwrap();
    // Should be -1.0, represented as 0 in our [-1,1] mapping
    assert!(correlation < 100_000);

    // Test no correlation
    let prices_4 = vec![500_000, 900_000, 300_000, 1_100_000, 400_000, 800_000, 600_000];
    let correlation = calculate_pearson_correlation(&prices_1, &prices_4).unwrap();
    // Should be around 0, represented as 1_000_000 (ONE) in our mapping
    assert!(correlation > 500_000 && correlation < 1_500_000);

    println!("Correlation calculations test passed!");
}

#[test]
fn test_tail_loss_calculations() {
    use correlation_engine::state::tail_loss::VerseTailLoss;

    // Test with no correlation
    let tail_loss = VerseTailLoss::calculate_enhanced_tail_loss(4, 0).unwrap();
    // Base: 1 - 1/4 = 0.75
    assert_eq!(tail_loss, 750_000);

    // Test with high correlation (0.8)
    let tail_loss = VerseTailLoss::calculate_enhanced_tail_loss(4, 800_000).unwrap();
    // Enhanced: 1 - (1/4) * (1 - 0.8) = 0.95
    assert_eq!(tail_loss, 950_000);

    // Test with different outcome counts
    let tail_loss = VerseTailLoss::calculate_enhanced_tail_loss(10, 500_000).unwrap();
    // Enhanced: 1 - (1/10) * (1 - 0.5) = 0.95
    assert_eq!(tail_loss, 950_000);

    println!("Tail loss calculations test passed!");
}

#[test]
fn test_correlation_clustering() {
    use correlation_engine::state::correlation_matrix::{CorrelationMatrix, CorrelationEntry};
    use correlation_engine::analysis::{identify_correlation_clusters, analyze_cluster_risk};

    let matrix = CorrelationMatrix {
        is_initialized: true,
        verse_id: [0u8; 16],
        correlations: vec![
            // Cluster 1: markets 0, 1, 2 (high correlation)
            CorrelationEntry {
                market_i: 0,
                market_j: 1,
                correlation: 900_000,  // 0.9
                last_updated: 0,
                sample_size: 7,
            },
            CorrelationEntry {
                market_i: 1,
                market_j: 2,
                correlation: 850_000,  // 0.85
                last_updated: 0,
                sample_size: 7,
            },
            CorrelationEntry {
                market_i: 0,
                market_j: 2,
                correlation: 800_000,  // 0.8
                last_updated: 0,
                sample_size: 7,
            },
            // Cluster 2: markets 3, 4 (high correlation)
            CorrelationEntry {
                market_i: 3,
                market_j: 4,
                correlation: 750_000,  // 0.75
                last_updated: 0,
                sample_size: 7,
            },
            // Low correlation between clusters
            CorrelationEntry {
                market_i: 0,
                market_j: 3,
                correlation: 200_000,  // 0.2
                last_updated: 0,
                sample_size: 7,
            },
        ],
        average_correlation: 0,
        last_calculated: 0,
        calculation_version: 1,
        market_count: 5,
        bump: 0,
    };

    let results = identify_correlation_clusters(&matrix, 700_000, 5).unwrap();
    
    assert_eq!(results.num_clusters, 2);
    assert_eq!(results.clusters[0].size, 3);  // Cluster with markets 0, 1, 2
    assert_eq!(results.clusters[1].size, 2);  // Cluster with markets 3, 4

    // Test risk analysis
    let risk = analyze_cluster_risk(&results, 5);
    assert_eq!(risk.largest_cluster_size, 3);
    assert!(risk.concentration_ratio > 0); // Should have some concentration

    println!("Correlation clustering test passed!");
}