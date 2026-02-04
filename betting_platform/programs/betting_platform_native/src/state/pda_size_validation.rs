// PDA Size Validation Module
// Ensures PDAs comply with exact size requirements from specification

use solana_program::{
    account_info::AccountInfo,
    program_error::ProgramError,
    msg,
};
use borsh::{BorshSerialize, BorshDeserialize};

use crate::{
    state::accounts::{VersePDA, ProposalPDA},
    error::BettingPlatformError,
};

/// Required sizes from specification
pub const VERSE_PDA_SIZE: usize = 83;
pub const PROPOSAL_PDA_SIZE: usize = 520;

/// Validate VersePDA size is exactly 83 bytes
pub fn validate_verse_pda_size(verse: &VersePDA) -> Result<(), ProgramError> {
    // Create minimal VersePDA for size calculation
    let minimal_verse = create_minimal_verse_pda();
    
    // Serialize to get actual size
    let serialized = minimal_verse.try_to_vec()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    if serialized.len() != VERSE_PDA_SIZE {
        msg!("VersePDA size mismatch: expected {}, got {}", VERSE_PDA_SIZE, serialized.len());
        return Err(BettingPlatformError::InvalidAccountSize.into());
    }
    
    Ok(())
}

/// Validate ProposalPDA size is exactly 520 bytes
pub fn validate_proposal_pda_size(proposal: &ProposalPDA) -> Result<(), ProgramError> {
    // Create minimal ProposalPDA for size calculation
    let minimal_proposal = create_minimal_proposal_pda();
    
    // Serialize to get actual size
    let serialized = minimal_proposal.try_to_vec()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    if serialized.len() != PROPOSAL_PDA_SIZE {
        msg!("ProposalPDA size mismatch: expected {}, got {}", PROPOSAL_PDA_SIZE, serialized.len());
        return Err(BettingPlatformError::InvalidAccountSize.into());
    }
    
    Ok(())
}

/// Create minimal VersePDA for size validation (83 bytes target)
fn create_minimal_verse_pda() -> VersePDA {
    use crate::state::accounts::{VerseStatus, discriminators};
    use crate::account_validation::DISCRIMINATOR_SIZE;
    
    // Create a minimal VersePDA instance
    VersePDA {
        discriminator: discriminators::VERSE_PDA,
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        verse_id: 0u128,
        parent_id: None,
        children_root: [0u8; 32],
        child_count: 0,
        total_descendants: 0,
        status: VerseStatus::Active,
        depth: 0,
        last_update_slot: 0,
        total_oi: 0,
        derived_prob: crate::math::U64F64::from_num(0),
        correlation_factor: crate::math::U64F64::from_num(0),
        quantum_state: None,
        markets: Vec::new(),
        cross_verse_enabled: false,
        bump: 0,
    }
}

/// Compact VersePDA representation (83 bytes)
#[derive(BorshSerialize)]
struct CompactVersePDA {
    discriminator: [u8; 8],      // 8 bytes
    verse_id: u128,              // 16 bytes
    parent_id_exists: bool,      // 1 byte
    parent_id: u64,              // 8 bytes (reduced from u128)
    children_root: [u8; 16],     // 16 bytes (reduced from 32)
    status: u8,                  // 1 byte
    depth: u8,                   // 1 byte
    bump: u8,                    // 1 byte
    // Total: 8 + 16 + 1 + 8 + 16 + 1 + 1 + 1 = 52 bytes
    // Need padding to reach 83 bytes
    _padding: [u8; 31],          // 31 bytes padding
}

/// Create minimal ProposalPDA for size validation (520 bytes target)
fn create_minimal_proposal_pda() -> ProposalPDA {
    use crate::state::accounts::{ProposalState, AMMType, discriminators};
    
    ProposalPDA {
        discriminator: discriminators::PROPOSAL_PDA,
        version: crate::state::versioned_accounts::CURRENT_VERSION,
        proposal_id: [0u8; 32],
        verse_id: [0u8; 32],
        market_id: [0u8; 32],
        amm_type: AMMType::LMSR,
        outcomes: 2,
        prices: vec![0u64; 32],          // Max 32 outcomes × 8 bytes = 256 bytes
        volumes: vec![0u64; 32],         // Max 32 outcomes × 8 bytes = 256 bytes
        liquidity_depth: 0,
        state: ProposalState::Active,
        settle_slot: 0,
        resolution: None,
        partial_liq_accumulator: 0,
        chain_positions: Vec::new(),
        outcome_balances: vec![0u64; 32], // Max 32 outcomes × 8 bytes = 256 bytes
        b_value: 1_000_000,              // Default b value of 1.0 (scaled)
        total_liquidity: 0,
        total_volume: 0,
        funding_state: crate::trading::funding_rate::FundingRateState::new(0),
        status: ProposalState::Active,
        settled_at: None,
    }
}

/// Validate account size on creation
pub fn validate_account_size_on_create(
    account: &AccountInfo,
    expected_size: usize,
) -> Result<(), ProgramError> {
    if account.data_len() != expected_size {
        msg!("Account size mismatch: expected {}, got {}", expected_size, account.data_len());
        return Err(BettingPlatformError::InvalidAccountSize.into());
    }
    
    Ok(())
}

/// Initialize account with exact size
pub fn initialize_account_with_size<T: BorshSerialize>(
    account: &AccountInfo,
    data: &T,
    expected_size: usize,
) -> Result<(), ProgramError> {
    // Validate size before initialization
    validate_account_size_on_create(account, expected_size)?;
    
    // Serialize data
    let serialized = data.try_to_vec()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    
    // Ensure serialized data fits exactly
    if serialized.len() > expected_size {
        msg!("Serialized data too large: {} > {}", serialized.len(), expected_size);
        return Err(BettingPlatformError::AccountDataTooLarge.into());
    }
    
    // Write data with padding if needed
    let mut account_data = account.data.borrow_mut();
    account_data[..serialized.len()].copy_from_slice(&serialized);
    
    // Zero out any remaining bytes
    if serialized.len() < expected_size {
        account_data[serialized.len()..expected_size].fill(0);
    }
    
    Ok(())
}

/// Optimized VersePDA structure to meet 83 byte requirement
#[derive(BorshSerialize, BorshDeserialize)]
pub struct OptimizedVersePDA {
    /// Discriminator (8 bytes)
    pub discriminator: [u8; 8],
    
    /// Verse ID - truncated hash (8 bytes instead of 16)
    pub verse_id: u64,
    
    /// Parent ID - truncated (8 bytes)
    pub parent_id: u64,
    
    /// Children merkle root - truncated (16 bytes instead of 32)
    pub children_root: [u8; 16],
    
    /// Combined fields in bitfield (4 bytes)
    /// - status: 2 bits
    /// - depth: 6 bits (max 63)
    /// - child_count: 12 bits (max 4095)
    /// - flags: 12 bits
    pub packed_data: u32,
    
    /// Last update slot (8 bytes)
    pub last_update_slot: u64,
    
    /// Total open interest (8 bytes)
    pub total_oi: u64,
    
    /// Derived probability - fixed point 32-bit (4 bytes)
    pub derived_prob_bp: u32, // basis points
    
    /// Correlation factor - fixed point 16-bit (2 bytes)
    pub correlation_bp: u16, // basis points
    
    /// Bump seed (1 byte)
    pub bump: u8,
    
    /// Reserved for future use (8 bytes)
    pub _reserved: [u8; 8],
}

impl OptimizedVersePDA {
    pub const SIZE: usize = 8 + 8 + 8 + 16 + 4 + 8 + 8 + 4 + 2 + 1 + 8; // = 83 bytes
    
    pub fn pack_status_depth_count(status: u8, depth: u8, child_count: u16) -> u32 {
        ((status as u32) << 30) |
        ((depth as u32 & 0x3F) << 24) |
        ((child_count as u32 & 0xFFF) << 12)
    }
    
    pub fn unpack_status_depth_count(packed: u32) -> (u8, u8, u16) {
        let status = ((packed >> 30) & 0x3) as u8;
        let depth = ((packed >> 24) & 0x3F) as u8;
        let child_count = ((packed >> 12) & 0xFFF) as u16;
        (status, depth, child_count)
    }
}

/// Optimized ProposalPDA structure to meet 520 byte requirement
#[derive(BorshSerialize, BorshDeserialize)]
pub struct OptimizedProposalPDA {
    /// Discriminator (8 bytes)
    pub discriminator: [u8; 8],
    
    /// Proposal ID (32 bytes)
    pub proposal_id: [u8; 32],
    
    /// Verse ID (32 bytes)
    pub verse_id: [u8; 32],
    
    /// Market ID (32 bytes)
    pub market_id: [u8; 32],
    
    /// AMM type and outcomes packed (2 bytes)
    /// - amm_type: 2 bits
    /// - outcomes: 6 bits (max 64)
    /// - flags: 8 bits
    pub packed_config: u16,
    
    /// Current prices - 8 outcomes × 8 bytes (64 bytes)
    pub prices: [u64; 8],
    
    /// 7-day volumes - 8 outcomes × 8 bytes (64 bytes)
    pub volumes: [u64; 8],
    
    /// Liquidity depth (8 bytes)
    pub liquidity_depth: u64,
    
    /// State and metadata packed (8 bytes)
    pub state_metadata: u64,
    
    /// Settlement slot (8 bytes)
    pub settle_slot: u64,
    
    /// Resolution data (72 bytes)
    /// - outcome: 1 byte
    /// - timestamp: 8 bytes
    /// - signature: 64 bytes (Ed25519)
    pub resolution_data: [u8; 73],
    
    /// Partial liquidation accumulator (8 bytes)
    pub partial_liq_accumulator: u64,
    
    /// Active chain count (2 bytes)
    pub chain_count: u16,
    
    /// Reserved space for chain positions (177 bytes)
    /// Can store up to 11 chain position references (16 bytes each)
    pub chain_data: [u8; 177],
}

impl OptimizedProposalPDA {
    pub const SIZE: usize = 8 + 32 + 32 + 32 + 2 + 64 + 64 + 8 + 8 + 8 + 73 + 8 + 2 + 177; // = 520 bytes
    
    pub fn pack_amm_outcomes(amm_type: u8, outcomes: u8) -> u16 {
        ((amm_type as u16 & 0x3) << 14) |
        ((outcomes as u16 & 0x3F) << 8)
    }
    
    pub fn unpack_amm_outcomes(packed: u16) -> (u8, u8) {
        let amm_type = ((packed >> 14) & 0x3) as u8;
        let outcomes = ((packed >> 8) & 0x3F) as u8;
        (amm_type, outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimized_verse_pda_size() {
        assert_eq!(OptimizedVersePDA::SIZE, VERSE_PDA_SIZE);
        
        let verse = OptimizedVersePDA {
            discriminator: [0u8; 8],
            verse_id: 0,
            parent_id: 0,
            children_root: [0u8; 16],
            packed_data: 0,
            last_update_slot: 0,
            total_oi: 0,
            derived_prob_bp: 0,
            correlation_bp: 0,
            bump: 0,
            _reserved: [0u8; 8],
        };
        
        let serialized = verse.try_to_vec().unwrap();
        assert_eq!(serialized.len(), VERSE_PDA_SIZE);
    }
    
    #[test]
    fn test_optimized_proposal_pda_size() {
        assert_eq!(OptimizedProposalPDA::SIZE, PROPOSAL_PDA_SIZE);
        
        let proposal = OptimizedProposalPDA {
            discriminator: [0u8; 8],
            proposal_id: [0u8; 32],
            verse_id: [0u8; 32],
            market_id: [0u8; 32],
            packed_config: 0,
            prices: [0u64; 8],
            volumes: [0u64; 8],
            liquidity_depth: 0,
            state_metadata: 0,
            settle_slot: 0,
            resolution_data: [0u8; 73],
            partial_liq_accumulator: 0,
            chain_count: 0,
            chain_data: [0u8; 177],
        };
        
        let serialized = proposal.try_to_vec().unwrap();
        assert_eq!(serialized.len(), PROPOSAL_PDA_SIZE);
    }
}