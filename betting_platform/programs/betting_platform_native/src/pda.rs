//! Program Derived Address (PDA) derivation functions
//!
//! Comprehensive PDA system for all 40+ account types

use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};

/// PDA seed constants
pub mod seeds {
    pub const GLOBAL_CONFIG: &[u8] = b"global_config";
    pub const VERSE: &[u8] = b"verse";
    pub const PROPOSAL: &[u8] = b"proposal";
    pub const POSITION: &[u8] = b"position";
    pub const USER_MAP: &[u8] = b"user_map";
    pub const MMT_MINT: &[u8] = b"mmt_mint";
    pub const TREASURY: &[u8] = b"treasury";
    pub const MINT_AUTHORITY: &[u8] = b"mint_authority";
    pub const VAULT: &[u8] = b"vault";
    pub const FEE_COLLECTOR: &[u8] = b"fee_collector";
    pub const CHAIN_STATE: &[u8] = b"chain_state";
    pub const CHAIN_POSITION: &[u8] = b"chain_position";
    pub const PRICE_CACHE: &[u8] = b"price_cache";
    pub const RESOLUTION_STATE: &[u8] = b"resolution_state";
    pub const DISPUTE_STATE: &[u8] = b"dispute_state";
    pub const KEEPER_REGISTRY: &[u8] = b"keeper_registry";
    pub const KEEPER_ACCOUNT: &[u8] = b"keeper_account";
    pub const KEEPER_HEALTH: &[u8] = b"keeper_health";
    pub const PERFORMANCE_METRICS: &[u8] = b"performance_metrics";
    pub const ATTACK_DETECTOR: &[u8] = b"attack_detector";
    pub const CIRCUIT_BREAKER: &[u8] = b"circuit_breaker";
    pub const LIQUIDATION_QUEUE: &[u8] = b"liquidation_queue";
    pub const AT_RISK_POSITION: &[u8] = b"at_risk_position";
    pub const LMSR_MARKET: &[u8] = b"lmsr_market";
    pub const PMAMM_MARKET: &[u8] = b"pmamm_market";
    pub const L2AMM_MARKET: &[u8] = b"l2amm_market";
    pub const HYBRID_AMM: &[u8] = b"hybrid_amm";
    pub const ICEBERG_ORDER: &[u8] = b"iceberg_order";
    pub const TWAP_ORDER: &[u8] = b"twap_order";
    pub const DARK_POOL: &[u8] = b"dark_pool";
    pub const DARK_ORDER: &[u8] = b"dark_order";
    pub const MERKLE_ROOT: &[u8] = b"merkle_root";
    pub const COMPRESSED_STATE: &[u8] = b"compressed_state";
    pub const LOOKUP_TABLE: &[u8] = b"lookup_table";
    pub const STOP_ORDER: &[u8] = b"stop_order";
    pub const STOP_LOSS: &[u8] = b"stop_loss";
    pub const WEBSOCKET_STATE: &[u8] = b"websocket_state";
    pub const INGESTOR_STATE: &[u8] = b"ingestor_state";
    pub const USER_PREFERENCES: &[u8] = b"user_preferences";
    pub const EPOCH_STATE: &[u8] = b"epoch_state";
    pub const SEASON_STATE: &[u8] = b"season_state";
    pub const MMT_STAKE: &[u8] = b"mmt_stake";
}

/// Core PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct GlobalConfigPDA;
impl GlobalConfigPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::GLOBAL_CONFIG], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::GLOBAL_CONFIG.to_vec()]
    }
}

/// Verse PDA (hierarchical state management)
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct VersePDA;
impl VersePDA {
    pub fn derive(program_id: &Pubkey, verse_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::VERSE, &verse_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(verse_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::VERSE.to_vec(),
            verse_id.to_le_bytes().to_vec(),
        ]
    }
}

/// Proposal PDA (market)
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ProposalPDA;
impl ProposalPDA {
    pub fn derive(program_id: &Pubkey, proposal_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::PROPOSAL, &proposal_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(proposal_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::PROPOSAL.to_vec(),
            proposal_id.to_le_bytes().to_vec(),
        ]
    }
}

/// Position PDA
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct PositionPDA;
impl PositionPDA {
    pub fn derive(
        program_id: &Pubkey,
        user: &Pubkey,
        proposal_id: u128,
        position_index: u8,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                seeds::POSITION,
                user.as_ref(),
                &proposal_id.to_le_bytes(),
                &[position_index],
            ],
            program_id
        )
    }
    
    pub fn seeds(user: &Pubkey, proposal_id: u128, position_index: u8) -> Vec<Vec<u8>> {
        vec![
            seeds::POSITION.to_vec(),
            user.as_ref().to_vec(),
            proposal_id.to_le_bytes().to_vec(),
            vec![position_index],
        ]
    }
}

/// User map PDA (tracks user positions)
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct UserMapPDA;
impl UserMapPDA {
    pub fn derive(program_id: &Pubkey, user: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::USER_MAP, user.as_ref()],
            program_id
        )
    }
    
    pub fn seeds(user: &Pubkey) -> Vec<Vec<u8>> {
        vec![
            seeds::USER_MAP.to_vec(),
            user.as_ref().to_vec(),
        ]
    }
}

/// MMT token PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct MmtMintPDA;
impl MmtMintPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::MMT_MINT], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::MMT_MINT.to_vec()]
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct TreasuryPDA;
impl TreasuryPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::TREASURY], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::TREASURY.to_vec()]
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct MintAuthorityPDA;
impl MintAuthorityPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::MINT_AUTHORITY], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::MINT_AUTHORITY.to_vec()]
    }
}

/// Chain execution PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ChainStatePDA;
impl ChainStatePDA {
    pub fn derive(program_id: &Pubkey, chain_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::CHAIN_STATE, &chain_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(chain_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::CHAIN_STATE.to_vec(),
            chain_id.to_le_bytes().to_vec(),
        ]
    }
}

/// Keeper network PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct KeeperRegistryPDA;
impl KeeperRegistryPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::KEEPER_REGISTRY], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::KEEPER_REGISTRY.to_vec()]
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct KeeperAccountPDA;
impl KeeperAccountPDA {
    pub fn derive(program_id: &Pubkey, keeper_id: &[u8; 32]) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::KEEPER_ACCOUNT, keeper_id],
            program_id
        )
    }
    
    pub fn seeds(keeper_id: &[u8; 32]) -> Vec<Vec<u8>> {
        vec![
            seeds::KEEPER_ACCOUNT.to_vec(),
            keeper_id.to_vec(),
        ]
    }
}

/// AMM PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct LmsrMarketPDA;
impl LmsrMarketPDA {
    pub fn derive(program_id: &Pubkey, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::LMSR_MARKET, &market_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(market_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::LMSR_MARKET.to_vec(),
            market_id.to_le_bytes().to_vec(),
        ]
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct PmammMarketPDA;
impl PmammMarketPDA {
    pub fn derive(program_id: &Pubkey, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::PMAMM_MARKET, &market_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(market_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::PMAMM_MARKET.to_vec(),
            market_id.to_le_bytes().to_vec(),
        ]
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct L2AmmMarketPDA;
impl L2AmmMarketPDA {
    pub fn derive(program_id: &Pubkey, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::L2AMM_MARKET, &market_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(market_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::L2AMM_MARKET.to_vec(),
            market_id.to_le_bytes().to_vec(),
        ]
    }
}

/// Advanced trading PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct IcebergOrderPDA;
impl IcebergOrderPDA {
    pub fn derive(
        program_id: &Pubkey,
        user: &Pubkey,
        market_id: u128,
        order_id: u64,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                seeds::ICEBERG_ORDER,
                user.as_ref(),
                &market_id.to_le_bytes(),
                &order_id.to_le_bytes(),
            ],
            program_id
        )
    }
    
    pub fn seeds(user: &Pubkey, market_id: u128, order_id: u64) -> Vec<Vec<u8>> {
        vec![
            seeds::ICEBERG_ORDER.to_vec(),
            user.as_ref().to_vec(),
            market_id.to_le_bytes().to_vec(),
            order_id.to_le_bytes().to_vec(),
        ]
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct TwapOrderPDA;
impl TwapOrderPDA {
    pub fn derive(
        program_id: &Pubkey,
        user: &Pubkey,
        market_id: u128,
        order_id: u64,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                seeds::TWAP_ORDER,
                user.as_ref(),
                &market_id.to_le_bytes(),
                &order_id.to_le_bytes(),
            ],
            program_id
        )
    }
    
    pub fn seeds(user: &Pubkey, market_id: u128, order_id: u64) -> Vec<Vec<u8>> {
        vec![
            seeds::TWAP_ORDER.to_vec(),
            user.as_ref().to_vec(),
            market_id.to_le_bytes().to_vec(),
            order_id.to_le_bytes().to_vec(),
        ]
    }
}

/// Dark pool PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct DarkPoolPDA;
impl DarkPoolPDA {
    pub fn derive(program_id: &Pubkey, market_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::DARK_POOL, &market_id.to_le_bytes()],
            program_id
        )
    }
    
    pub fn seeds(market_id: u128) -> Vec<Vec<u8>> {
        vec![
            seeds::DARK_POOL.to_vec(),
            market_id.to_le_bytes().to_vec(),
        ]
    }
}

/// Circuit breaker PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CircuitBreakerPDA;
impl CircuitBreakerPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::CIRCUIT_BREAKER], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::CIRCUIT_BREAKER.to_vec()]
    }
}

/// Attack detector PDA
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct AttackDetectorPDA;
impl AttackDetectorPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::ATTACK_DETECTOR], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::ATTACK_DETECTOR.to_vec()]
    }
}

/// PM-AMM pool PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct PmammPoolPDA;
impl PmammPoolPDA {
    pub fn derive(program_id: &Pubkey, pool_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"pmamm_pool",
                &pool_id.to_le_bytes(),
            ],
            program_id,
        )
    }
    
    pub fn seeds(pool_id: u128) -> Vec<Vec<u8>> {
        vec![
            b"pmamm_pool".to_vec(),
            pool_id.to_le_bytes().to_vec(),
        ]
    }
}

/// LP position PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct LpPositionPDA;
impl LpPositionPDA {
    pub fn derive(program_id: &Pubkey, provider: &Pubkey, pool_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"lp_position",
                provider.as_ref(),
                &pool_id.to_le_bytes(),
            ],
            program_id,
        )
    }
}

/// L2-AMM pool PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct L2ammPoolPDA;
impl L2ammPoolPDA {
    pub fn derive(program_id: &Pubkey, pool_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"l2amm_pool",
                &pool_id.to_le_bytes(),
            ],
            program_id,
        )
    }
}

/// L2 position PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct L2PositionPDA;
impl L2PositionPDA {
    pub fn derive(program_id: &Pubkey, position_id: u128) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"l2_position",
                &position_id.to_le_bytes(),
            ],
            program_id,
        )
    }
}


/// Liquidation queue PDAs
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct LiquidationQueuePDA;
impl LiquidationQueuePDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[seeds::LIQUIDATION_QUEUE], program_id)
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![seeds::LIQUIDATION_QUEUE.to_vec()]
    }
}

/// User preferences PDA
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct UserPreferencesPDA;
impl UserPreferencesPDA {
    pub fn derive(program_id: &Pubkey, user: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[seeds::USER_PREFERENCES, user.as_ref()],
            program_id
        )
    }
    
    pub fn seeds(user: &Pubkey) -> Vec<Vec<u8>> {
        vec![
            seeds::USER_PREFERENCES.to_vec(),
            user.as_ref().to_vec(),
        ]
    }
}

/// Collateral vault PDA
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CollateralVaultPDA;
impl CollateralVaultPDA {
    pub fn derive(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"collateral_vault"],
            program_id,
        )
    }
    
    pub fn seeds() -> Vec<Vec<u8>> {
        vec![b"collateral_vault".to_vec()]
    }
}

/// Helper function to validate PDA derivation
pub fn validate_pda_derivation(
    account_key: &Pubkey,
    program_id: &Pubkey,
    seeds: &[&[u8]],
) -> Result<u8, ProgramError> {
    let (expected_key, bump) = Pubkey::find_program_address(seeds, program_id);
    
    if account_key != &expected_key {
        return Err(ProgramError::InvalidSeeds);
    }
    
    Ok(bump)
}

/// Macro for easy PDA validation
#[macro_export]
macro_rules! validate_pda {
    ($account:expr, $program_id:expr, $seeds:expr) => {{
        $crate::pda::validate_pda_derivation($account.key, $program_id, $seeds)?
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_global_config_pda() {
        let program_id = Pubkey::new_unique();
        let (pda, bump) = GlobalConfigPDA::derive(&program_id);
        assert!(bump > 0);
        
        let seeds = GlobalConfigPDA::seeds();
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0], seeds::GLOBAL_CONFIG);
    }
    
    #[test]
    fn test_verse_pda() {
        let program_id = Pubkey::new_unique();
        let verse_id = 12345u128;
        let (pda, bump) = VersePDA::derive(&program_id, verse_id);
        assert!(bump > 0);
        
        let seeds = VersePDA::seeds(verse_id);
        assert_eq!(seeds.len(), 2);
    }
    
    #[test]
    fn test_position_pda() {
        let program_id = Pubkey::new_unique();
        let user = Pubkey::new_unique();
        let proposal_id = 67890u128;
        let position_index = 0u8;
        
        let (pda, bump) = PositionPDA::derive(&program_id, &user, proposal_id, position_index);
        assert!(bump > 0);
    }
}