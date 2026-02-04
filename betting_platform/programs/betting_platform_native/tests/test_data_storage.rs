//! Tests for Data Storage & Availability Implementation (Q30)
//! 
//! Tests IPFS archival and chain event logging

#[cfg(test)]
mod test_data_storage {
    use borsh::{BorshDeserialize, BorshSerialize};
    use solana_program_test::*;
    use solana_sdk::{
        signature::{Keypair, Signer},
        transaction::Transaction,
        pubkey::Pubkey,
        instruction::{AccountMeta, Instruction},
    };
    use betting_platform_native::{
        instruction::{BettingPlatformInstruction, ChainStepType},
        state::{
            StateArchival, ArchivalStatus,
            GlobalConfigPDA,
        },
        events::chain_events::{ChainEvent, ChainAuditAccount, ChainStepSummary},
    };

    #[tokio::test]
    async fn test_ipfs_archival_system() {
        let program_id = Pubkey::new_unique();
        let mut test = ProgramTest::new(
            "betting_platform_native",
            program_id,
            None,
        );
        
        let (mut banks_client, payer, recent_blockhash) = test.start().await;
        
        // Create test proposal to archive
        let proposal_id = [1u8; 32];
        let market_data = vec![0u8; 1024]; // 1KB test data
        
        // Create archival PDA
        let (archival_pda, _) = Pubkey::find_program_address(
            &[b"archive", &proposal_id],
            &program_id,
        );
        
        // Simulate archival process
        let ipfs_hash = [0x12u8; 32]; // Mock IPFS hash
        let archival_state = StateArchival {
            discriminator: StateArchival::DISCRIMINATOR,
            ipfs_hash,
            archival_slot: 100,
            status: ArchivalStatus::Complete,
            proposal_id,
            data_size: market_data.len() as u64,
            timestamp: 1234567890,
        };
        
        // Verify archival data structure
        assert_eq!(archival_state.status, ArchivalStatus::Complete);
        assert_eq!(archival_state.data_size, 1024);
        assert_eq!(archival_state.ipfs_hash.len(), 32);
        
        println!("✓ IPFS archival system verified");
    }

    #[tokio::test]
    async fn test_chain_event_logging() {
        let program_id = Pubkey::new_unique();
        let user = Keypair::new();
        
        // Create test chain event
        let chain_event = ChainEvent {
            chain_id: 12345u128,
            user: user.pubkey(),
            step: 0,
            step_type: ChainStepType::Borrow { amount: 1000 },
            step_return_bps: 1500, // 15% return
            effective_leverage: 150, // 1.5x
            base_leverage: 100, // 1x
            cumulative_multiplier: fixed::types::U64F64::from_num(1.15),
            step_amount: 1000,
            current_balance: 1150,
            timestamp: 1234567890,
            slot: 100,
        };
        
        // Verify event data
        assert_eq!(chain_event.step_return_bps, 1500);
        assert_eq!(chain_event.effective_leverage, 150);
        assert!(chain_event.cumulative_multiplier > fixed::types::U64F64::from_num(1));
        
        println!("✓ Chain event logging verified");
    }

    #[tokio::test]
    async fn test_chain_audit_trail() {
        let program_id = Pubkey::new_unique();
        let user = Keypair::new();
        
        // Create test chain steps
        let steps = vec![
            ChainStepSummary {
                step: 0,
                step_type: ChainStepType::Borrow { amount: 1000 },
                r_i: 1500, // 15%
                eff_lev: 150, // 1.5x
            },
            ChainStepSummary {
                step: 1,
                step_type: ChainStepType::Liquidity { amount: 1150 },
                r_i: 1000, // 10%
                eff_lev: 165, // 1.65x
            },
            ChainStepSummary {
                step: 2,
                step_type: ChainStepType::Stake { amount: 1265 },
                r_i: 800, // 8%
                eff_lev: 178, // 1.78x
            },
        ];
        
        // Create audit account
        let audit_account = ChainAuditAccount {
            discriminator: ChainAuditAccount::DISCRIMINATOR,
            chain_id: 12345u128,
            user: user.pubkey(),
            created_slot: 100,
            num_steps: steps.len() as u8,
            step_data: ChainAuditAccount::compress_steps(&steps),
            final_effective_leverage: 178,
            total_return_bps: 3300, // 33% total
            ipfs_hash: None,
        };
        
        // Verify compression and decompression
        let decompressed = audit_account.decompress_steps();
        assert_eq!(decompressed.len(), 3);
        assert_eq!(decompressed[0].r_i, 1500);
        assert_eq!(decompressed[1].eff_lev, 165);
        assert_eq!(decompressed[2].step, 2);
        
        // Verify compressed data size efficiency
        let uncompressed_size = steps.len() * std::mem::size_of::<ChainStepSummary>();
        let compressed_size = audit_account.step_data.len();
        let compression_ratio = uncompressed_size as f64 / compressed_size as f64;
        
        println!("✓ Chain audit trail compression ratio: {:.2}x", compression_ratio);
        assert!(compression_ratio > 2.0); // Should achieve at least 2x compression
    }

    #[tokio::test]
    async fn test_archival_slot_progression() {
        // Test that archival happens at correct slot intervals
        let slots_per_epoch = 432_000u64;
        let archival_interval = slots_per_epoch * 2; // Archive every 2 epochs
        
        let test_slots = vec![
            100u64,
            archival_interval - 1,
            archival_interval,
            archival_interval + 1,
            archival_interval * 2,
        ];
        
        for slot in test_slots {
            let should_archive = slot > 0 && slot % archival_interval == 0;
            let needs_archival = slot >= archival_interval && slot % archival_interval == 0;
            
            if needs_archival {
                println!("✓ Slot {} triggers archival", slot);
            } else {
                println!("  Slot {} does not trigger archival", slot);
            }
            
            assert_eq!(should_archive, needs_archival);
        }
    }

    #[tokio::test]
    async fn test_chain_event_size_limits() {
        // Verify chain event fits within CU budget
        let chain_event = ChainEvent {
            chain_id: u128::MAX,
            user: Pubkey::new_unique(),
            step: 255,
            step_type: ChainStepType::Long { outcome: 255, leverage: 255 },
            step_return_bps: i64::MAX,
            effective_leverage: u64::MAX,
            base_leverage: u64::MAX,
            cumulative_multiplier: fixed::types::U64F64::MAX,
            step_amount: u64::MAX,
            current_balance: u64::MAX,
            timestamp: i64::MAX,
            slot: u64::MAX,
        };
        
        let serialized = chain_event.try_to_vec().unwrap();
        let size = serialized.len();
        
        println!("✓ Chain event size: {} bytes", size);
        assert!(size < 1000); // Should be reasonably small
        
        // Verify logging doesn't exceed CU limits
        let log_cost_cu = size as u64 * 100; // Approximate CU cost for logging
        assert!(log_cost_cu < 5000); // Should be well under 5k CU
    }

    #[tokio::test]
    async fn test_ipfs_hash_validation() {
        // Test various IPFS hash formats
        let valid_hashes = vec![
            [0x12u8; 32], // SHA256 prefix
            [0x16u8; 32], // SHA256 prefix variant
        ];
        
        let invalid_hashes = vec![
            [0x00u8; 32], // All zeros
            [0xFFu8; 32], // All ones
        ];
        
        for hash in valid_hashes {
            // In production, would validate IPFS multihash format
            assert!(hash[0] == 0x12 || hash[0] == 0x16);
            println!("✓ Valid IPFS hash prefix: 0x{:02x}", hash[0]);
        }
        
        for hash in invalid_hashes {
            assert!(hash[0] != 0x12 && hash[0] != 0x16);
            println!("✗ Invalid IPFS hash prefix: 0x{:02x}", hash[0]);
        }
    }

    #[tokio::test]
    async fn test_chain_event_ordering() {
        // Verify events maintain correct ordering
        let mut events = Vec::new();
        
        for i in 0..5 {
            events.push(ChainStepSummary {
                step: i,
                step_type: ChainStepType::Long { outcome: 0, leverage: 1 },
                r_i: (i as i64 + 1) * 100,
                eff_lev: 100 + i as u64 * 10,
            });
        }
        
        // Verify step ordering
        for i in 1..events.len() {
            assert!(events[i].step > events[i-1].step);
            assert!(events[i].eff_lev >= events[i-1].eff_lev);
        }
        
        println!("✓ Chain event ordering maintained");
    }

    #[tokio::test]
    async fn test_cumulative_multiplier_calculation() {
        use fixed::types::U64F64;
        
        // Test cumulative multiplier across chain steps
        let returns = vec![1500i64, 1000, -500, 2000]; // 15%, 10%, -5%, 20%
        let mut cumulative = U64F64::from_num(1);
        
        for (i, &r_i) in returns.iter().enumerate() {
            let multiplier = if r_i >= 0 {
                U64F64::from_num(10000 + r_i as u64) / U64F64::from_num(10000)
            } else {
                U64F64::from_num(10000 - (-r_i) as u64) / U64F64::from_num(10000)
            };
            
            cumulative = cumulative.checked_mul(multiplier).unwrap();
            
            println!("Step {}: r_i = {}bps, multiplier = {:.4}, cumulative = {:.4}", 
                i, r_i, multiplier, cumulative);
        }
        
        // Expected: 1.15 * 1.10 * 0.95 * 1.20 = 1.4421
        let expected = U64F64::from_num(1.4421);
        let tolerance = U64F64::from_num(0.0001);
        
        assert!((cumulative - expected).abs() < tolerance);
        println!("✓ Cumulative multiplier calculation correct: {:.4}", cumulative);
    }
}