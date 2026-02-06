use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use correlation_engine::{
    instruction::{
        initialize_engine, 
        initialize_verse_tracking,
        calculate_correlations,
        update_price_history,
        update_tail_loss,
    },
    state::{
        CorrelationMatrix,
        VerseTailLoss,
        SLOTS_PER_DAY,
    },
};
use borsh::BorshDeserialize;

async fn process_transaction(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
) {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = {
        let payer = &context.payer;
        Transaction::new_signed_with_payer(
            instructions,
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        )
    };
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_end_to_end_correlation_system() {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "correlation_engine",
        program_id,
        processor!(correlation_engine::process),
    );

    let mut context = program_test.start_with_context().await;

    // Step 1: Initialize the correlation engine
    let (engine_pda, _) = Pubkey::find_program_address(
        &[b"correlation_engine"],
        &program_id,
    );

    let init_engine_ix = initialize_engine(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
    );

    process_transaction(&mut context, &[init_engine_ix]).await;

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
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
    );

    process_transaction(&mut context, &[init_verse_ix]).await;

    // Step 3: Update price history for 7 days
    // Markets 0 and 1 will be highly correlated
    // Markets 2 and 3 will be negatively correlated
    let market_ids: Vec<[u8; 16]> = (0u8..4u8)
        .map(|i| {
            let mut id = [0u8; 16];
            id[0] = i;
            id
        })
        .collect();

    let market_price_history_pdas: Vec<Pubkey> = market_ids
        .iter()
        .map(|market_id| Pubkey::find_program_address(&[b"price_history", market_id], &program_id).0)
        .collect();

    let mut target_slot = context.banks_client.get_root_slot().await.unwrap();
    for day in 0..7u64 {
        if day > 0 {
            target_slot = target_slot.saturating_add(SLOTS_PER_DAY);
            context.warp_to_slot(target_slot).unwrap();
        }

        let base_price = 500_000 + (day * 50_000); // Base price increases each day
        let prices = [
            base_price,                   // Market 0: follows base
            base_price + 10_000,          // Market 1: highly correlated with 0
            600_000 - (day * 20_000),     // Market 2: decreases over time
            1_000_000 - base_price,       // Market 3: negatively correlated with 0
        ];

        let instructions: Vec<Instruction> = prices
            .into_iter()
            .enumerate()
            .map(|(idx, price)| {
                update_price_history(
                    &program_id,
                    &context.payer.pubkey(),
                    &market_price_history_pdas[idx],
                    market_ids[idx],
                    price,
                    1_000_000 * (day + 1),
                )
            })
            .collect();

        process_transaction(&mut context, &instructions).await;
    }

    // Step 4: Calculate correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
        &market_price_history_pdas,
    );

    process_transaction(&mut context, &[calc_corr_ix]).await;

    // Step 5: Verify correlation matrix
    let correlation_account = context
        .banks_client
        .get_account(correlation_matrix_pda)
        .await
        .unwrap()
        .unwrap();
    let mut correlation_data: &[u8] = &correlation_account.data;
    let correlation_matrix = CorrelationMatrix::deserialize(&mut correlation_data).unwrap();

    // Should have 6 correlations (C(4,2) = 6)
    assert_eq!(correlation_matrix.correlations.len(), 6);
    assert_eq!(correlation_matrix.market_count, 4);

    // Find correlation between markets 0 and 1 (should be high positive)
    let corr_0_1 = correlation_matrix.get_correlation(0, 1).unwrap();
    assert!(corr_0_1 > 1_800_000); // Should be close to +1

    // Find correlation between markets 0 and 3 (should be negative)
    let corr_0_3 = correlation_matrix.get_correlation(0, 3).unwrap();
    assert!(corr_0_3 < 200_000); // Should be close to -1

    // Step 6: Update tail loss with correlation
    let update_tail_loss_ix = update_tail_loss(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
        4,
    );

    process_transaction(&mut context, &[update_tail_loss_ix]).await;

    // Step 7: Verify tail loss calculation
    let tail_loss_account = context
        .banks_client
        .get_account(tail_loss_pda)
        .await
        .unwrap()
        .unwrap();
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

    let mut context = program_test.start_with_context().await;

    // Initialize engine
    let (engine_pda, _) = Pubkey::find_program_address(
        &[b"correlation_engine"],
        &program_id,
    );

    let init_engine_ix = initialize_engine(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
    );

    process_transaction(&mut context, &[init_engine_ix]).await;

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
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
    );

    process_transaction(&mut context, &[init_verse_ix]).await;

    let market_ids: Vec<[u8; 16]> = (0u8..3u8)
        .map(|i| {
            let mut id = [0u8; 16];
            id[0] = i;
            id
        })
        .collect();

    let market_price_history_pdas: Vec<Pubkey> = market_ids
        .iter()
        .map(|market_id| Pubkey::find_program_address(&[b"price_history", market_id], &program_id).0)
        .collect();

    let mut target_slot = context.banks_client.get_root_slot().await.unwrap();

    // Week 1: Low correlation (independent movements)
    for day in 0..7u64 {
        if day > 0 {
            target_slot = target_slot.saturating_add(SLOTS_PER_DAY);
            context.warp_to_slot(target_slot).unwrap();
        }

        let prices = vec![
            500_000 + (day % 3) as u64 * 10_000,       // Market 0: small variations
            600_000 + ((day + 1) % 3) as u64 * 10_000, // Market 1: different pattern
            400_000 + ((day + 2) % 3) as u64 * 10_000, // Market 2: another pattern
        ];

        let instructions: Vec<Instruction> = prices
            .into_iter()
            .enumerate()
            .map(|(idx, price)| {
                update_price_history(
                    &program_id,
                    &context.payer.pubkey(),
                    &market_price_history_pdas[idx],
                    market_ids[idx],
                    price,
                    1_000_000,
                )
            })
            .collect();

        process_transaction(&mut context, &instructions).await;
    }

    // Calculate week 1 correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
        &market_price_history_pdas,
    );

    process_transaction(&mut context, &[calc_corr_ix]).await;

    let correlation_account = context
        .banks_client
        .get_account(correlation_matrix_pda)
        .await
        .unwrap()
        .unwrap();
    let mut week1_data: &[u8] = &correlation_account.data;
    let week1_matrix = CorrelationMatrix::deserialize(&mut week1_data).unwrap();
    let week1_avg_corr = week1_matrix.average_correlation;

    // Week 2: Markets become correlated (e.g., market contagion)
    for day in 0..7u64 {
        target_slot = target_slot.saturating_add(SLOTS_PER_DAY);
        context.warp_to_slot(target_slot).unwrap();

        let common_trend = 500_000 + (day as u64 * 20_000); // Common upward trend
        
        let prices = vec![
            common_trend,
            common_trend + 5_000,  // Small deviation
            common_trend - 5_000,  // Small deviation
        ];

        let instructions: Vec<Instruction> = prices
            .into_iter()
            .enumerate()
            .map(|(idx, price)| {
                update_price_history(
                    &program_id,
                    &context.payer.pubkey(),
                    &market_price_history_pdas[idx],
                    market_ids[idx],
                    price,
                    2_000_000,
                )
            })
            .collect();

        process_transaction(&mut context, &instructions).await;
    }

    // Calculate week 2 correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
        &market_price_history_pdas,
    );

    process_transaction(&mut context, &[calc_corr_ix]).await;

    let correlation_account = context
        .banks_client
        .get_account(correlation_matrix_pda)
        .await
        .unwrap()
        .unwrap();
    let mut week2_data: &[u8] = &correlation_account.data;
    let week2_matrix = CorrelationMatrix::deserialize(&mut week2_data).unwrap();
    let week2_avg_corr = week2_matrix.average_correlation;

    // Verify correlation increased in week 2
    assert!(week2_avg_corr > week1_avg_corr);

    // Update tail loss and verify it increased
    let update_tail_loss_ix = update_tail_loss(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
        3,
    );

    process_transaction(&mut context, &[update_tail_loss_ix]).await;

    let tail_loss_account = context
        .banks_client
        .get_account(tail_loss_pda)
        .await
        .unwrap()
        .unwrap();
    let tail_loss = VerseTailLoss::try_from_slice(&tail_loss_account.data).unwrap();

    // Enhanced tail loss should be significantly higher than base
    assert_eq!(tail_loss.parameters.base_tail_loss, 666_667);
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

    let mut context = program_test.start_with_context().await;

    // Initialize engine
    let (engine_pda, _) = Pubkey::find_program_address(
        &[b"correlation_engine"],
        &program_id,
    );

    let init_engine_ix = initialize_engine(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
    );

    process_transaction(&mut context, &[init_engine_ix]).await;

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
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        &tail_loss_pda,
        verse_id,
    );

    process_transaction(&mut context, &[init_verse_ix]).await;

    let market_ids: Vec<[u8; 16]> = (0u8..5u8)
        .map(|i| {
            let mut id = [0u8; 16];
            id[0] = i;
            id
        })
        .collect();

    let market_price_history_pdas: Vec<Pubkey> = market_ids
        .iter()
        .map(|market_id| Pubkey::find_program_address(&[b"price_history", market_id], &program_id).0)
        .collect();

    let mut target_slot = context.banks_client.get_root_slot().await.unwrap();

    // Create price patterns that form clusters
    // Cluster 1: Markets 0, 1, 2 (crypto markets moving together)
    // Cluster 2: Markets 3, 4 (traditional markets moving together)
    // Deterministic pattern chosen to keep crypto-vs-trad |corr| below clustering threshold.
    let trad_pattern: [u64; 7] = [600_989, 698_693, 610_250, 510_612, 567_873, 634_027, 627_383];
    for day in 0..7u64 {
        if day > 0 {
            target_slot = target_slot.saturating_add(SLOTS_PER_DAY);
            context.warp_to_slot(target_slot).unwrap();
        }

        let crypto_trend = 500_000 + (day as u64 * 30_000);
        // Non-monotonic series keeps crypto vs trad cross-correlation low (two distinct clusters).
        let trad_trend = trad_pattern[day as usize];
        
        let prices = vec![
            crypto_trend,           // Market 0 (BTC)
            crypto_trend + 2_000,   // Market 1 (ETH)
            crypto_trend - 2_000,   // Market 2 (SOL)
            trad_trend,             // Market 3 (SPY)
            trad_trend + 1_000,     // Market 4 (QQQ)
        ];

        let instructions: Vec<Instruction> = prices
            .into_iter()
            .enumerate()
            .map(|(idx, price)| {
                update_price_history(
                    &program_id,
                    &context.payer.pubkey(),
                    &market_price_history_pdas[idx],
                    market_ids[idx],
                    price,
                    1_000_000,
                )
            })
            .collect();

        process_transaction(&mut context, &instructions).await;
    }

    // Calculate correlations
    let calc_corr_ix = calculate_correlations(
        &program_id,
        &context.payer.pubkey(),
        &engine_pda,
        &verse_tracking_pda,
        &correlation_matrix_pda,
        verse_id,
        &market_price_history_pdas,
    );

    process_transaction(&mut context, &[calc_corr_ix]).await;

    // Verify correlation patterns
    let correlation_account = context
        .banks_client
        .get_account(correlation_matrix_pda)
        .await
        .unwrap()
        .unwrap();
    let mut matrix_data: &[u8] = &correlation_account.data;
    let matrix = CorrelationMatrix::deserialize(&mut matrix_data).unwrap();

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
