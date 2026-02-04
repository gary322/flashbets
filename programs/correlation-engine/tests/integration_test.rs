use solana_program::{
    pubkey::Pubkey,
    instruction::{AccountMeta, Instruction},
    clock::Clock,
    system_program,
    sysvar::{clock, rent},
};
use solana_program_test::{*};
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
};
use correlation_engine::{
    instruction::{
        initialize_engine, 
        initialize_verse_tracking,
        CorrelationInstruction,
    },
    state::{
        CorrelationEngine,
        VerseTracking,
        CorrelationMatrix,
        VerseTailLoss,
    },
};
use borsh::{BorshDeserialize, BorshSerialize};

#[tokio::test]
async fn test_end_to_end_correlation_system() {
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

    // Step 2: Initialize verse tracking with 4 markets
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

    // Step 3: Update price history for 7 days
    // Markets 0 and 1 will be highly correlated
    // Markets 2 and 3 will be negatively correlated
    for day in 0..7 {
        let base_price = 500_000 + (day as u64 * 50_000); // Base price increases each day
        
        let prices = vec![
            base_price,                      // Market 0: follows base
            base_price + 10_000,            // Market 1: highly correlated with 0
            600_000 - (day as u64 * 20_000), // Market 2: decreases over time
            1_000_000 - base_price,         // Market 3: negatively correlated with 0
        ];

        // Create UpdatePriceHistory instruction manually
        let update_price_ix = {
            let accounts = vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(engine_pda, false),
                AccountMeta::new(verse_tracking_pda, false),
                AccountMeta::new_readonly(clock::id(), false),
            ];
            
            let data = CorrelationInstruction::UpdatePriceHistory {
                market_id: verse_id, // Using verse_id as market_id for simplicity
                price: prices[0], // Using first price for now
                volume: 1_000_000 * (day + 1),
            }.pack();
            
            Instruction {
                program_id,
                accounts,
                data,
            }
        };

        let mut transaction = Transaction::new_with_payer(
            &[update_price_ix],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);
        banks_client.process_transaction(transaction).await.unwrap();
    }

    // Step 4: Calculate correlations
    let calc_corr_ix = {
        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(engine_pda, false),
            AccountMeta::new_readonly(verse_tracking_pda, false),
            AccountMeta::new(correlation_matrix_pda, false),
            AccountMeta::new_readonly(clock::id(), false),
        ];
        
        let data = CorrelationInstruction::CalculateCorrelations { verse_id }.pack();
        
        Instruction {
            program_id,
            accounts,
            data,
        }
    };

    let mut transaction = Transaction::new_with_payer(
        &[calc_corr_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 5: Verify correlation matrix
    let correlation_account = banks_client.get_account(correlation_matrix_pda).await.unwrap().unwrap();
    let correlation_matrix = CorrelationMatrix::try_from_slice(&correlation_account.data).unwrap();

    // Should have 6 correlations (C(4,2) = 6)
    assert_eq!(correlation_matrix.correlations.len(), 6);
    assert_eq!(correlation_matrix.market_count, 4);

    // Find correlation between markets 0 and 1 (should be high positive)
    let corr_0_1 = correlation_matrix.correlations.iter()
        .find(|e| (e.market_i == 0 && e.market_j == 1))
        .unwrap();
    
    // Convert from [-1,1] representation where ONE = 0
    // Values > ONE are positive correlations
    assert!(corr_0_1.correlation > 1_800_000); // Should be close to +1

    // Find correlation between markets 0 and 3 (should be negative)
    let corr_0_3 = correlation_matrix.correlations.iter()
        .find(|e| (e.market_i == 0 && e.market_j == 3))
        .unwrap();
    
    // Values < ONE are negative correlations
    assert!(corr_0_3.correlation < 200_000); // Should be close to -1

    // Step 6: Update tail loss with correlation
    let update_tail_loss_ix = {
        let accounts = vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(engine_pda, false),
            AccountMeta::new_readonly(correlation_matrix_pda, false),
            AccountMeta::new(tail_loss_pda, false),
            AccountMeta::new_readonly(clock::id(), false),
        ];
        
        let data = CorrelationInstruction::UpdateTailLoss { 
            verse_id,
            outcome_count: 4, // 4 markets
        }.pack();
        
        Instruction {
            program_id,
            accounts,
            data,
        }
    };

    let mut transaction = Transaction::new_with_payer(
        &[update_tail_loss_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 7: Verify tail loss calculation
    let tail_loss_account = banks_client.get_account(tail_loss_pda).await.unwrap().unwrap();
    let tail_loss = VerseTailLoss::try_from_slice(&tail_loss_account.data).unwrap();

    // Base tail loss for 4 outcomes = 1 - 1/4 = 0.75
    assert_eq!(tail_loss.parameters.base_tail_loss, 750_000);

    // With correlation, enhanced tail loss should be higher
    assert!(tail_loss.parameters.enhanced_tail_loss > tail_loss.parameters.base_tail_loss);
    
    // Coverage impact should be > 1.0 (indicating reduced coverage due to correlation)
    assert!(tail_loss.coverage_impact > 1_000_000);

    println!("Correlation system test completed successfully!");
    println!("Average correlation: {}", correlation_matrix.average_correlation);
    println!("Base tail loss: {}", tail_loss.parameters.base_tail_loss);
    println!("Enhanced tail loss: {}", tail_loss.parameters.enhanced_tail_loss);
    println!("Coverage impact: {}", tail_loss.coverage_impact);
}

#[tokio::test]
async fn test_correlation_with_market_dynamics() {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "correlation_engine",
        program_id,
        processor!(correlation_engine::process),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Initialize engine
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

    // Test with 3 markets that become more correlated over time
    let verse_id = [2u8; 16];
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

    // Week 1: Low correlation (independent movements)
    for day in 0..7 {
        let prices = vec![
            500_000 + (day % 3) as u64 * 10_000,       // Market 0: small variations
            600_000 + ((day + 1) % 3) as u64 * 10_000, // Market 1: different pattern
            400_000 + ((day + 2) % 3) as u64 * 10_000, // Market 2: another pattern
        ];

        let update_price_ix = update_price_history(
            &program_id,
            &payer.pubkey(),
            &engine_pda,
            &verse_tracking_pda,
            verse_id,
            prices,
            1_000_000,
        );

        let mut transaction = Transaction::new_with_payer(
            &[update_price_ix],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);
        banks_client.process_transaction(transaction).await.unwrap();
    }

    // Calculate week 1 correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
    );

    let mut transaction = Transaction::new_with_payer(
        &[calc_corr_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let correlation_account = banks_client.get_account(correlation_matrix_pda).await.unwrap().unwrap();
    let week1_matrix = CorrelationMatrix::try_from_slice(&correlation_account.data).unwrap();
    let week1_avg_corr = week1_matrix.average_correlation;

    // Week 2: Markets become correlated (e.g., market contagion)
    for day in 0..7 {
        let common_trend = 500_000 + (day as u64 * 20_000); // Common upward trend
        
        let prices = vec![
            common_trend,
            common_trend + 5_000,  // Small deviation
            common_trend - 5_000,  // Small deviation
        ];

        let update_price_ix = update_price_history(
            &program_id,
            &payer.pubkey(),
            &engine_pda,
            &verse_tracking_pda,
            verse_id,
            prices,
            2_000_000, // Higher volume
        );

        let mut transaction = Transaction::new_with_payer(
            &[update_price_ix],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);
        banks_client.process_transaction(transaction).await.unwrap();
    }

    // Calculate week 2 correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
    );

    let mut transaction = Transaction::new_with_payer(
        &[calc_corr_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let correlation_account = banks_client.get_account(correlation_matrix_pda).await.unwrap().unwrap();
    let week2_matrix = CorrelationMatrix::try_from_slice(&correlation_account.data).unwrap();
    let week2_avg_corr = week2_matrix.average_correlation;

    // Verify correlation increased in week 2
    assert!(week2_avg_corr > week1_avg_corr);

    // Update tail loss and verify it increased
    let update_tail_loss_ix = update_tail_loss(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
    );

    let mut transaction = Transaction::new_with_payer(
        &[update_tail_loss_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let tail_loss_account = banks_client.get_account(tail_loss_pda).await.unwrap().unwrap();
    let tail_loss = VerseTailLoss::try_from_slice(&tail_loss_account.data).unwrap();

    // Enhanced tail loss should be significantly higher than base
    let base = 1.0 - 1.0 / 3.0; // 0.667 for 3 outcomes
    assert_eq!(tail_loss.parameters.base_tail_loss, 666_666);
    assert!(tail_loss.parameters.enhanced_tail_loss > 800_000); // Should be > 0.8 with high correlation

    println!("Market dynamics test completed!");
    println!("Week 1 avg correlation: {}", week1_avg_corr);
    println!("Week 2 avg correlation: {}", week2_avg_corr);
    println!("Enhanced tail loss: {}", tail_loss.parameters.enhanced_tail_loss);
}

#[tokio::test]
async fn test_correlation_clustering_integration() {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "correlation_engine",
        program_id,
        processor!(correlation_engine::process),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Initialize engine
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

    // Set up verse with 5 markets that form 2 clusters
    let verse_id = [3u8; 16];
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

    // Create price patterns that form clusters
    // Cluster 1: Markets 0, 1, 2 (crypto markets moving together)
    // Cluster 2: Markets 3, 4 (traditional markets moving together)
    for day in 0..7 {
        let crypto_trend = 500_000 + (day as u64 * 30_000);
        let trad_trend = 600_000 - (day as u64 * 10_000);
        
        let prices = vec![
            crypto_trend,           // Market 0 (BTC)
            crypto_trend + 2_000,   // Market 1 (ETH)
            crypto_trend - 2_000,   // Market 2 (SOL)
            trad_trend,             // Market 3 (SPY)
            trad_trend + 1_000,     // Market 4 (QQQ)
        ];

        let update_price_ix = update_price_history(
            &program_id,
            &payer.pubkey(),
            &engine_pda,
            &verse_tracking_pda,
            verse_id,
            prices,
            1_000_000,
        );

        let mut transaction = Transaction::new_with_payer(
            &[update_price_ix],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);
        banks_client.process_transaction(transaction).await.unwrap();
    }

    // Calculate correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
    );

    let mut transaction = Transaction::new_with_payer(
        &[calc_corr_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Verify correlation patterns
    let correlation_account = banks_client.get_account(correlation_matrix_pda).await.unwrap().unwrap();
    let matrix = CorrelationMatrix::try_from_slice(&correlation_account.data).unwrap();

    // Use the clustering analysis from our library
    use correlation_engine::analysis::{
        identify_correlation_clusters,
        analyze_cluster_risk,
    };

    let clustering_results = identify_correlation_clusters(&matrix, 700_000, 5).unwrap();
    
    // Should identify 2 clusters
    assert!(clustering_results.num_clusters >= 2);
    
    // Analyze risk concentration
    let risk_analysis = analyze_cluster_risk(&clustering_results, 5);
    
    println!("Clustering test completed!");
    println!("Number of clusters found: {}", clustering_results.num_clusters);
    println!("Largest cluster size: {}", risk_analysis.largest_cluster_size);
    println!("Risk concentration: {}%", risk_analysis.concentration_ratio * 100 / 1_000_000);
    println!("Risk level: {:?}", risk_analysis.risk_level);
}