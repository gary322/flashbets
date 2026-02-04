//! Comprehensive Credit System Test
//!
//! Verifies all credit system requirements from specification

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        credits::{
            credits_manager::{UserCredits, CreditsManager, derive_user_credits_pda},
            credit_locking::{CreditLockingManager, CreditLock},
            refund_processor::{RefundProcessor, RefundType},
        },
        state::{
            VersePDA, ProposalPDA, Position, UserMap, 
            VerseStatus, ProposalState,
        },
        error::BettingPlatformError,
    };
    use solana_program::{
        pubkey::Pubkey,
        clock::Clock,
        program_error::ProgramError,
    };

    #[test]
    fn test_credits_equal_deposit() {
        println!("=== Test 1: Credits = Deposit (1:1 conversion) ===");
        
        let user = Pubkey::new_unique();
        let verse_id = 123;
        let deposit_amounts = vec![100, 1_000, 10_000, 100_000, 1_000_000];
        
        for deposit in deposit_amounts {
            let credits = UserCredits::new(user, verse_id, deposit, 255);
            
            println!("Deposit: {}, Credits: {}", deposit, credits.available_credits);
            assert_eq!(credits.total_deposit, deposit, "Total deposit mismatch");
            assert_eq!(credits.available_credits, deposit, "Credits != Deposit");
            assert_eq!(credits.locked_credits, 0, "Should have no locked credits initially");
            
            // Test conversion
            let mut verse = VersePDA::new(verse_id, None, 5);
            verse.status = VerseStatus::Active;
            
            let conversion = CreditsManager::deposit_to_credits(&user, &verse, deposit).unwrap();
            assert_eq!(conversion.credits, deposit, "Conversion should be 1:1");
            assert_eq!(conversion.conversion_rate, 1, "Rate should be 1");
        }
        
        println!("✅ Credits = Deposit verification passed\n");
    }

    #[test]
    fn test_credit_locking_per_position() {
        println!("=== Test 2: Credit Locking Per Position ===");
        
        let user = Pubkey::new_unique();
        let verse_id = 456;
        let mut user_credits = UserCredits::new(user, verse_id, 10_000, 255);
        
        println!("Initial: {} total, {} available", user_credits.total_deposit, user_credits.available_credits);
        
        // Lock credits for position 1
        let lock_amount_1 = 2_000;
        user_credits.lock_credits(lock_amount_1).unwrap();
        assert_eq!(user_credits.available_credits, 8_000);
        assert_eq!(user_credits.locked_credits, 2_000);
        assert_eq!(user_credits.active_positions, 1);
        println!("After position 1: {} available, {} locked", user_credits.available_credits, user_credits.locked_credits);
        
        // Lock credits for position 2
        let lock_amount_2 = 3_000;
        user_credits.lock_credits(lock_amount_2).unwrap();
        assert_eq!(user_credits.available_credits, 5_000);
        assert_eq!(user_credits.locked_credits, 5_000);
        assert_eq!(user_credits.active_positions, 2);
        println!("After position 2: {} available, {} locked", user_credits.available_credits, user_credits.locked_credits);
        
        // Try to lock more than available (should fail)
        let result = user_credits.lock_credits(6_000);
        assert!(result.is_err(), "Should not be able to lock more than available");
        println!("Correctly rejected locking 6000 (only 5000 available)");
        
        // Release credits from position 1
        user_credits.release_credits(lock_amount_1).unwrap();
        assert_eq!(user_credits.available_credits, 7_000);
        assert_eq!(user_credits.locked_credits, 3_000);
        assert_eq!(user_credits.active_positions, 1);
        println!("After releasing position 1: {} available, {} locked", user_credits.available_credits, user_credits.locked_credits);
        
        // Verify total always equals available + locked
        assert_eq!(user_credits.total_deposit, user_credits.available_credits + user_credits.locked_credits);
        
        println!("✅ Credit locking per position works correctly\n");
    }

    #[test]
    fn test_conflicting_positions_same_credits() {
        println!("=== Test 3: Conflicting Positions with Same Credits ===");
        
        let user = Pubkey::new_unique();
        let proposal_id = 789;
        let verse_id = 111;
        
        // Create user credits
        let user_credits = UserCredits::new(user, verse_id, 10_000, 255);
        
        // Create position 1 on outcome A
        let position1 = Position::new(
            user,
            proposal_id,
            verse_id,
            0, // outcome A
            5_000, // size
            5, // leverage
            50_000, // entry price
            true, // long
            0,
        );
        
        // Create position 2 on outcome B (same proposal)
        let position2 = Position::new(
            user,
            proposal_id,
            verse_id,
            1, // outcome B (conflicting)
            3_000, // size
            5, // leverage
            50_000,
            true,
            0,
        );
        
        println!("Position 1: Outcome {}, Margin {}", position1.outcome, position1.margin);
        println!("Position 2: Outcome {}, Margin {}", position2.outcome, position2.margin);
        
        // Test conflict resolution
        let existing_positions = vec![position1.clone()];
        let resolution = CreditLockingManager::handle_conflicting_positions(
            &user_credits,
            &position2,
            &existing_positions,
        ).unwrap();
        
        assert!(resolution.has_conflicts, "Should detect conflict");
        assert_eq!(resolution.conflicts.len(), 1, "Should have 1 conflict");
        assert!(resolution.conflicts[0].is_opposite, "Should be opposite outcome");
        assert!(resolution.can_proceed, "Should allow conflicting positions");
        
        println!("Conflict detected: {} existing positions", resolution.conflicts.len());
        println!("Total locked in proposal: {}", resolution.total_locked_in_proposal);
        println!("Available for proposal: {}", resolution.available_for_proposal);
        println!("Can proceed: {}", resolution.can_proceed);
        
        // Test quantum superposition - both positions share same credit pool
        let total_margin_needed = position1.margin + position2.margin;
        assert!(total_margin_needed <= user_credits.total_deposit, "Both positions should fit within total deposit");
        
        println!("✅ Conflicting positions allowed with same credits (quantum superposition)\n");
    }

    #[test]
    fn test_instant_refund_at_settle_slot() {
        println!("=== Test 4: Instant Refunds at settle_slot ===");
        
        let user = Pubkey::new_unique();
        let verse_id = 222;
        let proposal_id = 333;
        
        // Create user credits
        let mut user_credits = UserCredits::new(user, verse_id, 10_000, 255);
        
        // Lock some credits
        user_credits.lock_credits(4_000).unwrap();
        assert_eq!(user_credits.available_credits, 6_000);
        
        // Create verse and proposal
        let verse = VersePDA::new(verse_id, None, 1);
        let mut proposal = ProposalPDA::new([0; 32], [0; 32], 2);
        proposal.settle_slot = 1000;
        proposal.state = ProposalState::Active;
        
        // Test refund before settle_slot (should fail)
        let current_slot = 999;
        let refund_amount = CreditsManager::calculate_refund(
            &user_credits,
            &verse,
            current_slot,
            proposal.settle_slot,
        ).unwrap();
        assert_eq!(refund_amount, 0, "Should not refund before settle_slot");
        println!("Correctly rejected refund at slot {} (settle_slot: {})", current_slot, proposal.settle_slot);
        
        // Test refund at settle_slot (should work)
        let current_slot = 1000;
        let refund_amount = CreditsManager::calculate_refund(
            &user_credits,
            &verse,
            current_slot,
            proposal.settle_slot,
        ).unwrap();
        assert_eq!(refund_amount, 6_000, "Should refund all available credits");
        println!("Refund allowed at settle_slot: {} credits", refund_amount);
        
        // Test refund after settle_slot (should work)
        let current_slot = 1100;
        let refund_amount = CreditsManager::calculate_refund(
            &user_credits,
            &verse,
            current_slot,
            proposal.settle_slot,
        ).unwrap();
        assert_eq!(refund_amount, 6_000, "Should refund all available credits");
        println!("Refund allowed after settle_slot: {} credits", refund_amount);
        
        // Process the refund
        user_credits.mark_refund_eligible();
        let processed = user_credits.process_refund().unwrap();
        assert_eq!(processed, 6_000, "Should process full refund");
        assert_eq!(user_credits.available_credits, 0, "Available should be zero after refund");
        assert_eq!(user_credits.total_deposit, 4_000, "Only locked credits remain");
        
        println!("✅ Instant refunds at settle_slot verified (no claiming needed)\n");
    }

    #[test]
    fn test_credit_flows_end_to_end() {
        println!("=== Test 5: Complete Credit Flow (Deposit → Lock → Use → Refund) ===");
        
        let user = Pubkey::new_unique();
        let verse_id = 444;
        let deposit = 50_000;
        
        // Step 1: Deposit creates credits
        println!("\n1. DEPOSIT:");
        let mut user_credits = UserCredits::new(user, verse_id, deposit, 255);
        println!("   Deposited: {}", deposit);
        println!("   Credits available: {}", user_credits.available_credits);
        assert_eq!(user_credits.available_credits, deposit);
        
        // Step 2: Lock credits for multiple positions
        println!("\n2. LOCK FOR POSITIONS:");
        let positions = vec![
            (1, 10_000, 10), // proposal_id, size, leverage
            (2, 20_000, 5),
            (3, 15_000, 3),
        ];
        
        let mut total_locked = 0;
        for (i, (prop_id, size, leverage)) in positions.iter().enumerate() {
            let margin = size / leverage;
            user_credits.lock_credits(margin).unwrap();
            total_locked += margin;
            println!("   Position {}: Locked {} credits (size={}, leverage={}x)", 
                i+1, margin, size, leverage);
        }
        
        assert_eq!(user_credits.locked_credits, total_locked);
        assert_eq!(user_credits.active_positions, 3);
        println!("   Total locked: {}, Available: {}", user_credits.locked_credits, user_credits.available_credits);
        
        // Step 3: Use credits (positions are active)
        println!("\n3. CREDITS IN USE:");
        println!("   Active positions: {}", user_credits.active_positions);
        println!("   Credits locked: {}", user_credits.locked_credits);
        println!("   Credits available: {}", user_credits.available_credits);
        
        // Step 4: Close positions and release credits
        println!("\n4. RELEASE CREDITS:");
        // Close position 1
        user_credits.release_credits(1_000).unwrap();
        println!("   Closed position 1: Released 1000 credits");
        
        // Close position 2
        user_credits.release_credits(4_000).unwrap();
        println!("   Closed position 2: Released 4000 credits");
        
        assert_eq!(user_credits.active_positions, 1);
        assert_eq!(user_credits.locked_credits, 5_000);
        assert_eq!(user_credits.available_credits, 45_000);
        
        // Step 5: Process refund
        println!("\n5. REFUND:");
        // Close last position
        user_credits.release_credits(5_000).unwrap();
        assert_eq!(user_credits.active_positions, 0);
        
        // Mark eligible and process refund
        user_credits.mark_refund_eligible();
        let refund_amount = user_credits.process_refund().unwrap();
        println!("   Refunded: {} credits", refund_amount);
        assert_eq!(refund_amount, deposit); // Should get back full deposit
        assert_eq!(user_credits.available_credits, 0);
        assert_eq!(user_credits.total_deposit, 0);
        
        println!("\n✅ Complete credit flow verified successfully!");
    }

    #[test]
    fn test_margin_calculation_with_volatility() {
        println!("\n=== Test 6: Margin Calculation with Volatility Buffer ===");
        
        let mut proposal = ProposalPDA::new([0; 32], [0; 32], 2);
        
        // Binary market (no buffer)
        let margin = CreditLockingManager::calculate_required_margin(10_000, 10, &proposal).unwrap();
        assert_eq!(margin, 1_000); // 10,000 / 10 = 1,000
        println!("Binary market: Size=10,000, Leverage=10x, Margin={}", margin);
        
        // Multi-outcome market (10% buffer)
        proposal.outcomes = 5;
        let margin = CreditLockingManager::calculate_required_margin(10_000, 10, &proposal).unwrap();
        assert_eq!(margin, 1_100); // 1,000 + 10% = 1,100
        println!("Multi-outcome market: Size=10,000, Leverage=10x, Margin={} (includes 10% buffer)", margin);
        
        println!("✅ Margin calculation with volatility buffer verified\n");
    }

    #[test]
    fn test_user_map_position_tracking() {
        println!("=== Test 7: UserMap Position Tracking ===");
        
        let user = Pubkey::new_unique();
        let mut user_map = UserMap::new(user);
        
        // Add positions
        let proposal_ids = vec![100, 200, 300];
        for &id in &proposal_ids {
            user_map.add_position(id).unwrap();
        }
        
        assert_eq!(user_map.position_count, 3);
        assert_eq!(user_map.position_ids.len(), 3);
        println!("Added 3 positions: {:?}", user_map.position_ids);
        
        // Remove a position
        user_map.remove_position(200).unwrap();
        assert_eq!(user_map.position_count, 2);
        assert!(!user_map.position_ids.contains(&200));
        println!("After removing position 200: {:?}", user_map.position_ids);
        
        // Test position limit
        for i in 4..34 {
            user_map.add_position(i as u128).unwrap();
        }
        assert_eq!(user_map.position_count, 32);
        
        // Try to add 33rd position (should fail)
        let result = user_map.add_position(999);
        assert!(result.is_err(), "Should not allow more than 32 positions");
        println!("Correctly enforced 32 position limit");
        
        println!("✅ UserMap position tracking verified\n");
    }
}