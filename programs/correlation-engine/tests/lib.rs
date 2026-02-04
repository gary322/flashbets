use solana_program::{
    pubkey::Pubkey,
};
use solana_program_test::{*};
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
};
use correlation_engine::{
    instruction::{initialize_engine, initialize_verse_tracking},
};

#[tokio::test]
async fn test_initialize_engine() {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "correlation_engine",
        program_id,
        processor!(correlation_engine::process),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Derive engine PDA
    let (engine_pda, _bump) = Pubkey::find_program_address(
        &[b"correlation_engine"],
        &program_id,
    );

    // Create initialize instruction
    let init_ix = initialize_engine(
        &program_id,
        &payer.pubkey(),
        &engine_pda,
    );

    // Send transaction
    let mut transaction = Transaction::new_with_payer(
        &[init_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_initialize_verse_tracking() {
    let program_id = Pubkey::new_unique();
    let program_test = ProgramTest::new(
        "correlation_engine",
        program_id,
        processor!(correlation_engine::process),
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // First initialize engine
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

    // Now initialize verse tracking
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

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok());
}

#[test]
fn test_pearson_correlation_calculation() {
    use correlation_engine::math::calculate_pearson_correlation;

    // Test perfect positive correlation
    let prices_1 = vec![100_000_000, 200_000_000, 300_000_000, 400_000_000, 500_000_000, 600_000_000, 700_000_000];
    let prices_2 = prices_1.clone();

    let correlation = calculate_pearson_correlation(&prices_1, &prices_2).unwrap();
    // Should be +1.0, represented as 2*ONE (2_000_000) in our [-1,1] mapping
    assert!(correlation > 1_900_000); // Allow small rounding error

    // Test perfect negative correlation
    let prices_3: Vec<u64> = prices_1.iter()
        .map(|&p| 800_000_000 - p) // Inverse
        .collect();

    let correlation = calculate_pearson_correlation(&prices_1, &prices_3).unwrap();
    // Should be -1.0, represented as 0 in our [-1,1] mapping
    assert!(correlation < 100_000); // Close to 0
}

#[test]
fn test_enhanced_tail_loss() {
    use correlation_engine::state::tail_loss::VerseTailLoss;

    // Test with no correlation
    let tail_loss = VerseTailLoss::calculate_enhanced_tail_loss(4, 0).unwrap();
    // Base: 1 - 1/4 = 0.75
    // Enhanced with 0 correlation: same as base = 0.75
    assert_eq!(tail_loss, 750_000); // 0.75 in fixed point

    // Test with high correlation (0.8)
    let tail_loss = VerseTailLoss::calculate_enhanced_tail_loss(4, 800_000).unwrap();
    // Enhanced: 1 - (1/4) * (1 - 0.8) = 1 - 0.25 * 0.2 = 1 - 0.05 = 0.95
    assert_eq!(tail_loss, 950_000); // 0.95 in fixed point
}

#[test]
fn test_coverage_calculation() {
    use correlation_engine::state::tail_loss::{VerseTailLoss, CoverageCalculator, TailLossParameters};

    let tail_loss = VerseTailLoss {
        is_initialized: true,
        verse_id: [0u8; 16],
        parameters: TailLossParameters {
            base_tail_loss: 750_000,      // 0.75
            correlation_factor: 0,
            enhanced_tail_loss: 750_000,  // 0.75
            last_updated: 0,
        },
        coverage_impact: 1_000_000,
        outcome_count: 4,
        bump: 0,
    };

    let vault_balance = 10_000_000_000; // $10,000 in fixed point
    let open_interest = 5_000_000_000;  // $5,000 in fixed point

    let coverage = CoverageCalculator::calculate_coverage(
        vault_balance,
        open_interest,
        &tail_loss,
    ).unwrap();

    // Coverage = 10,000 / (0.75 * 5,000) = 10,000 / 3,750 = 2.67
    // In fixed point: 2,666,666 (approximately)
    assert!(coverage > 2_600_000 && coverage < 2_700_000);
}