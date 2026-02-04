//! Program Derived Address (PDA) utilities
//! 
//! Centralized module for generating all PDAs used in the betting platform.
//! This ensures consistency across the codebase and makes it easier to audit PDA usage.

use solana_sdk::pubkey::Pubkey;

/// PDA seeds for different account types
pub mod seeds {
    pub const MARKET: &[u8] = b"market";
    pub const POSITION: &[u8] = b"position";
    pub const DEMO_ACCOUNT: &[u8] = b"demo_account";
    pub const VERSE: &[u8] = b"verse";
    pub const GLOBAL_CONFIG: &[u8] = b"global_config";
    pub const QUANTUM_POSITION: &[u8] = b"quantum_position";
    pub const LIQUIDITY_POOL: &[u8] = b"liquidity_pool";
    pub const STAKING_ACCOUNT: &[u8] = b"staking_account";
    pub const ORDER_BOOK: &[u8] = b"order_book";
    pub const ORACLE_FEED: &[u8] = b"oracle_feed";
    pub const ESCROW: &[u8] = b"escrow";
    pub const FEE_COLLECTOR: &[u8] = b"fee_collector";
}

/// PDA generator for all account types
pub struct PdaGenerator {
    program_id: Pubkey,
}

impl PdaGenerator {
    /// Create a new PDA generator for the given program
    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }
    
    /// Get the market account PDA
    pub fn get_market_pda(&self, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::MARKET, &market_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the position account PDA for a user and market
    pub fn get_position_pda(&self, owner: &Pubkey, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::POSITION, owner.as_ref(), &market_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the demo account PDA for a user
    pub fn get_demo_account_pda(&self, owner: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::DEMO_ACCOUNT, owner.as_ref()],
            &self.program_id,
        )
    }
    
    /// Get the verse account PDA
    pub fn get_verse_pda(&self, verse_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::VERSE, &verse_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the global config PDA
    pub fn get_global_config_pda(&self) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::GLOBAL_CONFIG, &0u128.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the quantum position PDA
    pub fn get_quantum_position_pda(&self, owner: &Pubkey, position_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::QUANTUM_POSITION, owner.as_ref(), &position_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the liquidity pool PDA for a market
    pub fn get_liquidity_pool_pda(&self, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::LIQUIDITY_POOL, &market_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the staking account PDA for a user
    pub fn get_staking_account_pda(&self, owner: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::STAKING_ACCOUNT, owner.as_ref()],
            &self.program_id,
        )
    }
    
    /// Get the order book PDA for a market
    pub fn get_order_book_pda(&self, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::ORDER_BOOK, &market_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the oracle feed PDA for a market
    pub fn get_oracle_feed_pda(&self, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::ORACLE_FEED, &market_id.to_le_bytes()],
            &self.program_id,
        )
    }
    
    /// Get the escrow PDA for a market and user
    pub fn get_escrow_pda(&self, market_id: u128, owner: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::ESCROW, &market_id.to_le_bytes(), owner.as_ref()],
            &self.program_id,
        )
    }
    
    /// Get the fee collector PDA
    pub fn get_fee_collector_pda(&self) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::FEE_COLLECTOR],
            &self.program_id,
        )
    }
}

/// Convenience functions for quick PDA generation
pub mod helpers {
    use super::*;
    
    /// Get market PDA without bump
    pub fn market_pda(program_id: &Pubkey, market_id: u128) -> Pubkey {
        PdaGenerator::new(*program_id).get_market_pda(market_id).0
    }
    
    /// Get position PDA without bump
    pub fn position_pda(program_id: &Pubkey, owner: &Pubkey, market_id: u128) -> Pubkey {
        PdaGenerator::new(*program_id).get_position_pda(owner, market_id).0
    }
    
    /// Get demo account PDA without bump
    pub fn demo_account_pda(program_id: &Pubkey, owner: &Pubkey) -> Pubkey {
        PdaGenerator::new(*program_id).get_demo_account_pda(owner).0
    }
    
    /// Get verse PDA without bump
    pub fn verse_pda(program_id: &Pubkey, verse_id: u128) -> Pubkey {
        PdaGenerator::new(*program_id).get_verse_pda(verse_id).0
    }
    
    /// Get global config PDA without bump
    pub fn global_config_pda(program_id: &Pubkey) -> Pubkey {
        PdaGenerator::new(*program_id).get_global_config_pda().0
    }
    
    /// Get quantum position PDA without bump
    pub fn quantum_position_pda(program_id: &Pubkey, owner: &Pubkey, position_id: u128) -> Pubkey {
        PdaGenerator::new(*program_id).get_quantum_position_pda(owner, position_id).0
    }
    
    /// Get liquidity pool PDA without bump
    pub fn liquidity_pool_pda(program_id: &Pubkey, market_id: u128) -> Pubkey {
        PdaGenerator::new(*program_id).get_liquidity_pool_pda(market_id).0
    }
    
    /// Get staking account PDA without bump
    pub fn staking_account_pda(program_id: &Pubkey, owner: &Pubkey) -> Pubkey {
        PdaGenerator::new(*program_id).get_staking_account_pda(owner).0
    }
}

// Standalone functions for direct usage
pub fn get_market_pda(program_id: &Pubkey, market_id: u128) -> Pubkey {
    Pubkey::find_program_address(
        &[seeds::MARKET, &market_id.to_le_bytes()],
        program_id,
    ).0
}

pub fn get_position_pda(program_id: &Pubkey, owner: &Pubkey, market_id: u128) -> Pubkey {
    Pubkey::find_program_address(
        &[seeds::POSITION, owner.as_ref(), &market_id.to_le_bytes()],
        program_id,
    ).0
}

pub fn get_demo_account_pda(program_id: &Pubkey, owner: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[seeds::DEMO_ACCOUNT, owner.as_ref()],
        program_id,
    ).0
}

pub fn get_verse_pda(program_id: &Pubkey, verse_id: u128) -> Pubkey {
    Pubkey::find_program_address(
        &[seeds::VERSE, &verse_id.to_le_bytes()],
        program_id,
    ).0
}

pub fn get_global_config_pda(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[seeds::GLOBAL_CONFIG],
        program_id,
    ).0
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pda_generation() {
        let program_id = Pubkey::new_unique();
        let generator = PdaGenerator::new(program_id);
        
        // Test market PDA
        let market_id = 12345u128;
        let (pda1, bump1) = generator.get_market_pda(market_id);
        let (pda2, bump2) = generator.get_market_pda(market_id);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
        
        // Different market IDs should produce different PDAs
        let (pda3, _) = generator.get_market_pda(market_id + 1);
        assert_ne!(pda1, pda3);
    }
    
    #[test]
    fn test_position_pda() {
        let program_id = Pubkey::new_unique();
        let generator = PdaGenerator::new(program_id);
        let owner = Pubkey::new_unique();
        let market_id = 12345u128;
        
        let (pda1, _) = generator.get_position_pda(&owner, market_id);
        let (pda2, _) = generator.get_position_pda(&owner, market_id);
        assert_eq!(pda1, pda2);
        
        // Different owners should produce different PDAs
        let owner2 = Pubkey::new_unique();
        let (pda3, _) = generator.get_position_pda(&owner2, market_id);
        assert_ne!(pda1, pda3);
    }
    
    #[test]
    fn test_helper_functions() {
        let program_id = Pubkey::new_unique();
        let market_id = 12345u128;
        
        let pda1 = helpers::market_pda(&program_id, market_id);
        let generator = PdaGenerator::new(program_id);
        let (pda2, _) = generator.get_market_pda(market_id);
        
        assert_eq!(pda1, pda2);
    }
}