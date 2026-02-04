//! Unit tests for chain event logging

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::instruction::ChainStepType;
    use crate::math::U64F64;
    use solana_program::pubkey::Pubkey;

    #[test]
    fn test_chain_event_creation() {
        let user = Pubkey::new_unique();
        
        let event = ChainEvent {
            chain_id: 12345,
            user,
            step: 0,
            step_type: ChainStepType::Borrow { amount: 1000 },
            step_return_bps: 1500, // 15%
            effective_leverage: 150, // 1.5x
            base_leverage: 100,
            cumulative_multiplier: U64F64::from_fraction(115, 100).unwrap(),
            step_amount: 1000,
            current_balance: 1150,
            timestamp: 1234567890,
            slot: 100,
        };
        
        assert_eq!(event.chain_id, 12345);
        assert_eq!(event.step_return_bps, 1500);
        assert_eq!(event.effective_leverage, 150);
    }

    #[test]
    fn test_step_return_calculation() {
        // Test 10% profit
        let return_bps = calculate_step_return(
            &ChainStepType::Borrow { amount: 1000 },
            1000,
            1100
        );
        assert_eq!(return_bps, 1000); // 10% = 1000 bps
        
        // Test 5% loss
        let return_bps = calculate_step_return(
            &ChainStepType::Long { outcome: 0, leverage: 1 },
            1000,
            950
        );
        assert_eq!(return_bps, -500); // -5% = -500 bps
        
        // Test no change
        let return_bps = calculate_step_return(
            &ChainStepType::Stake { amount: 1000 },
            1000,
            1000
        );
        assert_eq!(return_bps, 0);
        
        // Test edge case: zero initial amount
        let return_bps = calculate_step_return(
            &ChainStepType::Liquidity { amount: 0 },
            0,
            100
        );
        assert_eq!(return_bps, 0);
    }

    #[test]
    fn test_audit_compression() {
        let steps = vec![
            ChainStepSummary {
                step: 0,
                step_type: ChainStepType::Borrow { amount: 1000 },
                r_i: 1500,
                eff_lev: 150,
            },
            ChainStepSummary {
                step: 1,
                step_type: ChainStepType::Stake { amount: 1150 },
                r_i: 1000,
                eff_lev: 165,
            },
        ];
        
        let compressed = ChainAuditAccount::compress_steps(&steps);
        
        // Each step should be 6 bytes (1 + 1 + 2 + 2)
        assert_eq!(compressed.len(), 12);
        
        // Create mock audit account
        let audit = ChainAuditAccount {
            discriminator: ChainAuditAccount::DISCRIMINATOR,
            chain_id: 12345,
            user: Pubkey::new_unique(),
            created_slot: 100,
            num_steps: 2,
            step_data: compressed,
            final_effective_leverage: 165,
            total_return_bps: 2500,
            ipfs_hash: None,
        };
        
        // Test decompression
        let decompressed = audit.decompress_steps();
        assert_eq!(decompressed.len(), 2);
        assert_eq!(decompressed[0].r_i, 1500);
        assert_eq!(decompressed[1].eff_lev, 165);
    }

    #[test]
    fn test_build_audit_trail() {
        let steps = vec![
            ChainStepSummary {
                step: 0,
                step_type: ChainStepType::Borrow { amount: 1000 },
                r_i: 1500, // 15%
                eff_lev: 150,
            },
            ChainStepSummary {
                step: 1,
                step_type: ChainStepType::Liquidity { amount: 1150 },
                r_i: 1000, // 10%
                eff_lev: 165,
            },
            ChainStepSummary {
                step: 2,
                step_type: ChainStepType::Stake { amount: 1265 },
                r_i: 800, // 8%
                eff_lev: 178,
            },
        ];
        
        let audit_trail = build_chain_audit_trail(
            12345,
            steps,
            178,
            true
        );
        
        assert_eq!(audit_trail.chain_id, 12345);
        assert_eq!(audit_trail.final_effective_leverage, 178);
        assert!(audit_trail.success);
        assert_eq!(audit_trail.steps.len(), 3);
        
        // Verify compounded return calculation
        // (1 + 0.15) * (1 + 0.10) * (1 + 0.08) - 1 ≈ 0.3534 = 3534 bps
        assert!(audit_trail.total_return_bps > 3500 && audit_trail.total_return_bps < 3600);
    }

    #[test]
    fn test_compression_efficiency() {
        // Test with maximum steps allowed
        let mut steps = Vec::new();
        for i in 0..10 {
            steps.push(ChainStepSummary {
                step: i,
                step_type: match i % 6 {
                    0 => ChainStepType::Borrow { amount: 1000 },
                    1 => ChainStepType::Lend { amount: 1000 },
                    2 => ChainStepType::Liquidity { amount: 1000 },
                    3 => ChainStepType::Stake { amount: 1000 },
                    4 => ChainStepType::Long { outcome: 0, leverage: 2 },
                    _ => ChainStepType::Short { outcome: 1, leverage: 3 },
                },
                r_i: (i as i64 + 1) * 100,
                eff_lev: 100 + i as u64 * 10,
            });
        }
        
        let compressed = ChainAuditAccount::compress_steps(&steps);
        let uncompressed_size = std::mem::size_of_val(&steps[..]);
        let compressed_size = compressed.len();
        
        println!("Uncompressed: {} bytes, Compressed: {} bytes", 
                 uncompressed_size, compressed_size);
        println!("Compression ratio: {:.2}x", 
                 uncompressed_size as f64 / compressed_size as f64);
        
        // Should achieve significant compression
        assert!(compressed_size < uncompressed_size / 2);
    }

    #[test]
    fn test_extreme_values() {
        // Test with extreme return values
        let steps = vec![
            ChainStepSummary {
                step: 0,
                step_type: ChainStepType::Long { outcome: 255, leverage: 255 },
                r_i: 32000, // 320% gain
                eff_lev: 500, // 5x max leverage
            },
            ChainStepSummary {
                step: 1,
                step_type: ChainStepType::Short { outcome: 0, leverage: 1 },
                r_i: -9000, // 90% loss
                eff_lev: 50, // Reduced to 0.5x
            },
        ];
        
        let compressed = ChainAuditAccount::compress_steps(&steps);
        
        let audit = ChainAuditAccount {
            discriminator: ChainAuditAccount::DISCRIMINATOR,
            chain_id: u128::MAX,
            user: Pubkey::new_unique(),
            created_slot: u64::MAX,
            num_steps: 2,
            step_data: compressed,
            final_effective_leverage: 50,
            total_return_bps: -5800, // Net loss
            ipfs_hash: Some([0xFF; 32]),
        };
        
        let decompressed = audit.decompress_steps();
        
        // Values should be preserved within compression limits
        assert_eq!(decompressed[0].r_i, 32000); // Fits in i16 * 100
        assert_eq!(decompressed[0].eff_lev, 500);
        assert_eq!(decompressed[1].r_i, -9000);
        assert_eq!(decompressed[1].eff_lev, 50);
    }
}