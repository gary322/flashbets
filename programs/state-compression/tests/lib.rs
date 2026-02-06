use solana_program_test::{*};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
};
use state_compression::{
    instructions::{
        CompressionInstruction,
    },
    state::{
        CompressionConfig,
        CompressedStateProof,
        DecompressionCache,
        MarketEssentials,
        MarketStatus,
        ProofType,
        MarketUpdate,
    },
    compression::{
        poseidon::{PoseidonHasher, PoseidonMerkleTree},
        proof_builder::{ProofBuilder, BatchProofBuilder},
    },
};
use borsh::BorshDeserialize;

#[test]
fn test_poseidon_hashing() {
    // Test basic Poseidon hashing
    let mut hasher = PoseidonHasher::new();
    hasher.update(b"test data");
    let hash1 = hasher.finalize();
    
    hasher.reset();
    hasher.update(b"test data");
    let hash2 = hasher.finalize();
    
    assert_eq!(hash1, hash2, "Hashing should be deterministic");
    
    // Test different data produces different hash
    hasher.reset();
    hasher.update(b"different data");
    let hash3 = hasher.finalize();
    
    assert_ne!(hash1, hash3, "Different data should produce different hash");
}

#[test]
fn test_merkle_tree() {
    let mut tree = PoseidonMerkleTree::new();
    
    // Add 8 leaves
    for i in 0..8u8 {
        let mut hasher = PoseidonHasher::new();
        hasher.update(&[i]);
        tree.add_leaf(hasher.finalize());
    }
    
    // Build tree
    let root = tree.build().unwrap();
    
    // Verify proofs for each leaf
    for i in 0..8 {
        let proof = tree.get_proof(i).unwrap();
        let verified = PoseidonMerkleTree::verify_proof(
            &tree.leaves[i],
            &proof,
            &root,
            i,
        );
        assert!(verified, "Proof for leaf {} should verify", i);
    }
}

#[test]
fn test_market_essentials_validation() {
    // Valid market
    let valid_market = MarketEssentials {
        market_id: [1u8; 32],
        current_price: 50_000_000, // 50%
        total_volume: 1_000_000,
        outcome_count: 2,
        status: MarketStatus::Active,
        last_update: 1000,
    };
    
    assert!(valid_market.validate().is_ok());
    
    // Invalid market - no outcomes
    let invalid_market = MarketEssentials {
        market_id: [2u8; 32],
        current_price: 50_000_000,
        total_volume: 1_000_000,
        outcome_count: 0, // Invalid
        status: MarketStatus::Active,
        last_update: 1000,
    };
    
    assert!(invalid_market.validate().is_err());
    
    // Invalid market - price > 100%
    let invalid_price = MarketEssentials {
        market_id: [3u8; 32],
        current_price: 150_000_000, // 150% - invalid
        total_volume: 1_000_000,
        outcome_count: 2,
        status: MarketStatus::Active,
        last_update: 1000,
    };
    
    assert!(invalid_price.validate().is_err());
}

#[test]
fn test_proof_builder() {
    let mut builder = ProofBuilder::new(ProofType::Poseidon);
    
    // Add 10 markets
    for i in 0..10 {
        let market = MarketEssentials {
            market_id: [i as u8; 32],
            current_price: 40_000_000 + (i as u64 * 2_000_000), // 40-58%
            total_volume: 100_000 * (i as u64 + 1),
            outcome_count: 2,
            status: MarketStatus::Active,
            last_update: 1000 + i as i64,
        };
        
        builder.add_market(market).unwrap();
    }
    
    // Build proof
    let proof = builder.build().unwrap();
    
    // Verify compression ratio
    let ratio = proof.uncompressed_size as f64 / proof.compressed_size as f64;
    println!("Compression ratio: {:.2}x", ratio);
    assert!(ratio > 1.0, "Should achieve some compression");
    
    // Verify proof data
    assert_eq!(proof.markets.len(), 10);
    assert!(proof.proof_data.len() > 0);
}

#[test]
fn test_batch_proof_builder() {
    let mut batch_builder = BatchProofBuilder::new(5, ProofType::Poseidon);
    
    // Add 12 markets (should create 3 batches: 5, 5, 2)
    for i in 0..12 {
        let market = MarketEssentials {
            market_id: [i as u8; 32],
            current_price: 50_000_000,
            total_volume: 100_000,
            outcome_count: 2,
            status: MarketStatus::Active,
            last_update: 1000,
        };
        
        batch_builder.add_market(market).unwrap();
    }
    
    // Build all batches
    let proofs = batch_builder.build_all().unwrap();
    
    assert_eq!(proofs.len(), 3, "Should create 3 batches");
    assert_eq!(proofs[0].markets.len(), 5);
    assert_eq!(proofs[1].markets.len(), 5);
    assert_eq!(proofs[2].markets.len(), 2);
}

#[test]
fn test_compression_config_validation() {
    let authority = Pubkey::new_unique();
    let config = CompressionConfig::default(authority);
    
    // Default config should be valid
    assert!(config.validate().is_ok());
    
    // Test invalid compression ratio
    let mut invalid_config = config.clone();
    invalid_config.compression_ratio = 0;
    assert!(invalid_config.validate().is_err());
    
    invalid_config.compression_ratio = 101;
    assert!(invalid_config.validate().is_err());
    
    // Test invalid batch size
    let mut invalid_batch = config.clone();
    invalid_batch.batch_size = 0;
    assert!(invalid_batch.validate().is_err());
    
    invalid_batch.batch_size = 1001;
    assert!(invalid_batch.validate().is_err());
}

#[test]
fn test_decompression_cache_stats() {
    let authority = Pubkey::new_unique();
    let mut cache = DecompressionCache::default(authority);
    
    // Simulate cache operations
    cache.total_hits = 800;
    cache.total_misses = 200;
    cache.update_hit_rate();
    
    // Hit rate should be 80%
    assert_eq!(cache.hit_rate, 800_000); // 0.8 with 6 decimals
    
    // Test cache cleanup threshold
    let current_time = 10000;
    cache.last_cleanup = 5000;
    
    assert!(cache.needs_cleanup(current_time));
    
    cache.last_cleanup = 9500;
    assert!(!cache.needs_cleanup(current_time));
}

#[test]
fn test_market_update_operations() {
    let mut market = MarketEssentials {
        market_id: [1u8; 32],
        current_price: 50_000_000,
        total_volume: 1_000_000,
        outcome_count: 2,
        status: MarketStatus::Active,
        last_update: 1000,
    };
    
    // Test price update
    MarketUpdate::Price(60_000_000).apply(&mut market).unwrap();
    assert_eq!(market.current_price, 60_000_000);
    
    // Test volume update
    MarketUpdate::Volume(500_000).apply(&mut market).unwrap();
    assert_eq!(market.total_volume, 1_500_000);
    
    // Test status update
    MarketUpdate::Status(MarketStatus::Settled).apply(&mut market).unwrap();
    assert_eq!(market.status, MarketStatus::Settled);
}

#[tokio::test]
async fn test_initialize_config_instruction() {
    let program_id = state_compression::id();
    let mut program_test = ProgramTest::new(
        "state_compression",
        program_id,
        processor!(state_compression::processor::process_instruction),
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    
    // Create config account
    let config_account = Keypair::new();
    
    // Build initialize instruction
    let init_ix = {
        let accounts = vec![
            solana_program::instruction::AccountMeta::new(payer.pubkey(), true),
            solana_program::instruction::AccountMeta::new(config_account.pubkey(), true),
            solana_program::instruction::AccountMeta::new_readonly(solana_program::system_program::id(), false),
            solana_program::instruction::AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
        ];
        
        let data = CompressionInstruction::InitializeConfig {
            compression_ratio: 10,
            batch_size: 100,
            proof_verification_cu: 2000,
        }.pack();
        
        solana_program::instruction::Instruction {
            program_id,
            accounts,
            data,
        }
    };
    
    let mut transaction = Transaction::new_with_payer(
        &[init_ix],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer, &config_account], recent_blockhash);
    
    banks_client.process_transaction(transaction).await.unwrap();
    
    // Verify config was initialized
    let config_data = banks_client
        .get_account(config_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    
    let config = CompressionConfig::try_from_slice(&config_data.data).unwrap();
    assert!(config.is_initialized);
    assert_eq!(config.authority, payer.pubkey());
    assert_eq!(config.compression_ratio, 10);
    assert_eq!(config.batch_size, 100);
    assert_eq!(config.proof_verification_cu, 2000);
}

#[test]
fn test_compressed_proof_validation() {
    let authority = Pubkey::new_unique();
    let proof_hash = [1u8; 32];
    let state_root = [2u8; 32];
    
    // Valid proof
    let valid_proof = CompressedStateProof::new(
        authority,
        proof_hash,
        state_root,
        1000,
        10, // 10 markets
        5200, // 520 bytes * 10
        520, // Compressed to 520 bytes (10x compression)
        ProofType::Poseidon,
        1000,
        vec![0u8; 100],
    ).unwrap();
    
    assert!(valid_proof.validate().is_ok());
    assert_eq!(valid_proof.get_compression_ratio(), 10.0);
    
    // Invalid proof - no compression
    let invalid_proof = CompressedStateProof::new(
        authority,
        proof_hash,
        state_root,
        1000,
        10,
        5200,
        5200, // Same size - no compression
        ProofType::Poseidon,
        1000,
        vec![0u8; 100],
    );
    
    assert!(invalid_proof.is_err());
}

#[test]
fn test_proof_contains_market() {
    let mut proof = CompressedStateProof::new(
        Pubkey::new_unique(),
        [0u8; 32],
        [0u8; 32],
        1000,
        10,
        5200,
        520,
        ProofType::Poseidon,
        1000,
        vec![0u8; 100],
    ).unwrap();
    
    // Add sample market IDs
    for i in 0..10 {
        proof.sample_market_ids[i] = [i as u8; 32];
    }
    
    // Test contains
    assert!(proof.contains_market_sample(&[5u8; 32]));
    assert!(!proof.contains_market_sample(&[15u8; 32]));
}
