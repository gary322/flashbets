//! Test to verify all operations use PDAs correctly

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use crate::pda::{PdaGenerator, helpers};
    use solana_sdk::pubkey::Pubkey;
    use crate::transaction_signing::TransactionBuilder;
    
    #[test]
    fn test_all_pdas_generated_correctly() {
        let program_id = Pubkey::new_unique();
        let generator = PdaGenerator::new(program_id);
        let owner = Pubkey::new_unique();
        let market_id = 12345u128;
        
        // Test all PDA types
        let (market_pda, _) = generator.get_market_pda(market_id);
        let (position_pda, _) = generator.get_position_pda(&owner, market_id);
        let (demo_account_pda, _) = generator.get_demo_account_pda(&owner);
        let (verse_pda, _) = generator.get_verse_pda(1);
        let (global_config_pda, _) = generator.get_global_config_pda();
        let (quantum_position_pda, _) = generator.get_quantum_position_pda(&owner, 1);
        let (liquidity_pool_pda, _) = generator.get_liquidity_pool_pda(market_id);
        let (staking_account_pda, _) = generator.get_staking_account_pda(&owner);
        let (order_book_pda, _) = generator.get_order_book_pda(market_id);
        let (oracle_feed_pda, _) = generator.get_oracle_feed_pda(market_id);
        let (escrow_pda, _) = generator.get_escrow_pda(market_id, &owner);
        let (fee_collector_pda, _) = generator.get_fee_collector_pda();
        
        // Verify all PDAs are unique
        let pdas = vec![
            market_pda,
            position_pda,
            demo_account_pda,
            verse_pda,
            global_config_pda,
            quantum_position_pda,
            liquidity_pool_pda,
            staking_account_pda,
            order_book_pda,
            oracle_feed_pda,
            escrow_pda,
            fee_collector_pda,
        ];
        
        for i in 0..pdas.len() {
            for j in (i + 1)..pdas.len() {
                assert_ne!(pdas[i], pdas[j], "PDAs at indices {} and {} are not unique", i, j);
            }
        }
    }
    
    #[test]
    fn test_transaction_builder_uses_pdas() {
        let program_id = Pubkey::new_unique();
        let admin = Pubkey::new_unique();
        let market_id = 12345u128;
        
        // Build create market transaction
        let tx = TransactionBuilder::build_create_market_tx(
            &program_id,
            market_id,
            &admin,
            "Test Market",
            &vec!["Yes".to_string(), "No".to_string()],
            chrono::Utc::now().timestamp() + 86400,
            crate::types::MarketType::Binary,
            250,
        ).unwrap();
        
        // Verify the transaction uses the correct PDA
        let expected_market_pda = helpers::market_pda(&program_id, market_id);
        let create_ix = tx.message.instructions.first().expect("create market instruction");
        let market_account_idx = *create_ix.accounts.first().expect("market account index") as usize;
        assert_eq!(tx.message.account_keys[market_account_idx], expected_market_pda);
        
        // Test place trade transaction
        let trader = Pubkey::new_unique();
        let tx = TransactionBuilder::build_place_trade_tx(
            &program_id,
            &trader,
            market_id,
            0,
            1000,
            5,
        ).unwrap();
        
        // Verify it uses the correct PDAs
        let expected_demo_pda = helpers::demo_account_pda(&program_id, &trader);
        let expected_market_pda = helpers::market_pda(&program_id, market_id);
        let expected_position_pda = helpers::position_pda(&program_id, &trader, market_id);
        
        let account_keys = &tx.message.account_keys;
        assert!(account_keys.contains(&expected_demo_pda));
        assert!(account_keys.contains(&expected_market_pda));
        assert!(account_keys.contains(&expected_position_pda));
    }
    
    #[test]
    fn test_pda_consistency() {
        let program_id = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let market_id = 12345u128;
        
        // Test that helper functions produce same results as generator
        let generator = PdaGenerator::new(program_id);
        
        let (gen_market_pda, _) = generator.get_market_pda(market_id);
        let helper_market_pda = helpers::market_pda(&program_id, market_id);
        assert_eq!(gen_market_pda, helper_market_pda);
        
        let (gen_position_pda, _) = generator.get_position_pda(&owner, market_id);
        let helper_position_pda = helpers::position_pda(&program_id, &owner, market_id);
        assert_eq!(gen_position_pda, helper_position_pda);
        
        let (gen_demo_pda, _) = generator.get_demo_account_pda(&owner);
        let helper_demo_pda = helpers::demo_account_pda(&program_id, &owner);
        assert_eq!(gen_demo_pda, helper_demo_pda);
    }
}
