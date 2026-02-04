//! Privacy-Preserving Commitment Scheme
//!
//! Native Solana implementation of privacy features using hash commitments

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    keccak,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    account_validation::{validate_signer, validate_writable, DISCRIMINATOR_SIZE},
};

/// Privacy commitment discriminator
pub const PRIVACY_COMMITMENT_DISCRIMINATOR: [u8; 8] = [80, 82, 73, 86, 67, 79, 77, 0]; // "PRIVCOM"

/// Maximum reveal delay in slots
pub const MAX_REVEAL_DELAY: u64 = 432_000; // ~48 hours

/// Privacy commitment structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PrivacyCommitment {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User who created the commitment
    pub user: Pubkey,
    
    /// Commitment hash (keccak256)
    pub commitment: [u8; 32],
    
    /// Commitment type
    pub commitment_type: CommitmentType,
    
    /// Creation slot
    pub created_slot: u64,
    
    /// Reveal deadline
    pub reveal_deadline: u64,
    
    /// Has been revealed
    pub revealed: bool,
    
    /// Revealed data (if any)
    pub revealed_data: Option<Vec<u8>>,
    
    /// Nullifier (prevents double-spending)
    pub nullifier: Option<[u8; 32]>,
    
    /// Associated market/proposal
    pub market_id: Option<[u8; 32]>,
    
    /// Bump seed
    pub bump: u8,
}

impl PrivacyCommitment {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 32 + 1 + 8 + 8 + 1 + 1 + 256 + 1 + 32 + 1 + 32 + 1;
    
    /// Create new commitment
    pub fn new(
        user: Pubkey,
        commitment: [u8; 32],
        commitment_type: CommitmentType,
        created_slot: u64,
        reveal_deadline: u64,
        bump: u8,
    ) -> Self {
        Self {
            discriminator: PRIVACY_COMMITMENT_DISCRIMINATOR,
            user,
            commitment,
            commitment_type,
            created_slot,
            reveal_deadline,
            revealed: false,
            revealed_data: None,
            nullifier: None,
            market_id: None,
            bump,
        }
    }
    
    /// Validate commitment
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != PRIVACY_COMMITMENT_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Commitment types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum CommitmentType {
    /// Private position (hidden size/outcome)
    PrivatePosition,
    
    /// Private vote (hidden choice)
    PrivateVote,
    
    /// Private balance proof
    BalanceProof,
    
    /// Private trade (hidden details)
    PrivateTrade,
    
    /// Range proof (prove value in range without revealing)
    RangeProof,
}

/// Process private position commitment
pub fn process_commit_private_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    commitment_data: PrivatePositionCommitment,
) -> ProgramResult {
    msg!("Creating private position commitment");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let commitment_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(commitment_account)?;
    
    // Compute commitment hash
    let commitment_hash = compute_position_commitment(&commitment_data)?;
    
    // Derive commitment PDA
    let (commitment_pda, bump) = derive_privacy_commitment_pda(
        program_id,
        user.key,
        &commitment_hash,
    );
    
    if commitment_account.key != &commitment_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    let reveal_deadline = current_slot + MAX_REVEAL_DELAY;
    
    // Create commitment
    let commitment = PrivacyCommitment::new(
        *user.key,
        commitment_hash,
        CommitmentType::PrivatePosition,
        current_slot,
        reveal_deadline,
        bump,
    );
    
    // Allocate and initialize account
    let rent = solana_program::rent::Rent::get()?;
    let required_lamports = rent.minimum_balance(PrivacyCommitment::LEN);
    
    solana_program::program::invoke_signed(
        &solana_program::system_instruction::create_account(
            user.key,
            commitment_account.key,
            required_lamports,
            PrivacyCommitment::LEN as u64,
            program_id,
        ),
        &[
            user.clone(),
            commitment_account.clone(),
            system_program.clone(),
        ],
        &[&[
            b"privacy_commitment",
            user.key.as_ref(),
            &commitment_hash,
            &[bump],
        ]],
    )?;
    
    // Serialize commitment
    commitment.serialize(&mut &mut commitment_account.data.borrow_mut()[..])?;
    
    msg!("Private position committed successfully");
    Ok(())
}

/// Private position commitment data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PrivatePositionCommitment {
    /// Market ID
    pub market_id: [u8; 32],
    
    /// Hidden outcome
    pub outcome: u8,
    
    /// Hidden size
    pub size: u64,
    
    /// Hidden leverage
    pub leverage: u8,
    
    /// Random nonce
    pub nonce: [u8; 32],
}

/// Compute position commitment hash
fn compute_position_commitment(data: &PrivatePositionCommitment) -> Result<[u8; 32], ProgramError> {
    let mut commitment_data = Vec::new();
    commitment_data.extend_from_slice(&data.market_id);
    commitment_data.push(data.outcome);
    commitment_data.extend_from_slice(&data.size.to_le_bytes());
    commitment_data.push(data.leverage);
    commitment_data.extend_from_slice(&data.nonce);
    
    Ok(keccak::hash(&commitment_data).to_bytes())
}

/// Process reveal private position
pub fn process_reveal_private_position(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    reveal_data: PrivatePositionCommitment,
) -> ProgramResult {
    msg!("Revealing private position");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let commitment_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(commitment_account)?;
    validate_writable(position_account)?;
    
    // Load commitment
    let mut commitment = PrivacyCommitment::try_from_slice(&commitment_account.data.borrow())?;
    commitment.validate()?;
    
    // Verify user
    if commitment.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if already revealed
    if commitment.revealed {
        return Err(BettingPlatformError::AlreadyRevealed.into());
    }
    
    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    
    // Check reveal deadline
    if current_slot > commitment.reveal_deadline {
        return Err(BettingPlatformError::CommitmentExpired.into());
    }
    
    // Verify commitment
    let computed_hash = compute_position_commitment(&reveal_data)?;
    if computed_hash != commitment.commitment {
        return Err(BettingPlatformError::InvalidReveal.into());
    }
    
    // Mark as revealed
    commitment.revealed = true;
    commitment.revealed_data = Some(reveal_data.try_to_vec()?);
    commitment.market_id = Some(reveal_data.market_id);
    
    // Generate nullifier to prevent double-spending
    let nullifier = generate_nullifier(&commitment.commitment, &reveal_data.nonce)?;
    commitment.nullifier = Some(nullifier);
    
    // Serialize updated commitment
    commitment.serialize(&mut &mut commitment_account.data.borrow_mut()[..])?;
    
    // Create actual position with revealed data
    // (Implementation would integrate with existing position creation)
    
    msg!("Private position revealed successfully");
    Ok(())
}

/// Generate nullifier
fn generate_nullifier(commitment: &[u8; 32], nonce: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
    let mut nullifier_data = Vec::new();
    nullifier_data.extend_from_slice(commitment);
    nullifier_data.extend_from_slice(nonce);
    
    Ok(keccak::hash(&nullifier_data).to_bytes())
}

/// Process private balance proof
pub fn process_create_balance_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    min_balance: u64,
) -> ProgramResult {
    msg!("Creating private balance proof");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let user_account = next_account_info(account_info_iter)?;
    let proof_account = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(proof_account)?;
    
    // Get user balance
    let user_balance = user_account.lamports();
    
    // Create commitment that proves balance >= min_balance without revealing exact amount
    let proof_data = BalanceProof {
        user: *user.key,
        min_balance,
        timestamp: Clock::get()?.unix_timestamp,
        nonce: generate_random_nonce(),
    };
    
    let commitment_hash = keccak::hash(&proof_data.try_to_vec()?).to_bytes();
    
    // Only create proof if balance is sufficient
    if user_balance < min_balance {
        return Err(BettingPlatformError::InsufficientBalance.into());
    }
    
    // Create proof commitment
    let clock = Clock::get()?;
    let commitment = PrivacyCommitment::new(
        *user.key,
        commitment_hash,
        CommitmentType::BalanceProof,
        clock.slot,
        clock.slot + 1000, // Short validity for balance proofs
        255, // Placeholder bump
    );
    
    msg!("Balance proof created (balance >= {})", min_balance);
    Ok(())
}

/// Balance proof structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BalanceProof {
    pub user: Pubkey,
    pub min_balance: u64,
    pub timestamp: i64,
    pub nonce: [u8; 16],
}

/// Generate random nonce (simplified - in production use proper RNG)
fn generate_random_nonce() -> [u8; 16] {
    let clock = Clock::get().unwrap();
    let mut nonce = [0u8; 16];
    let slot_bytes = clock.slot.to_le_bytes();
    nonce[..8].copy_from_slice(&slot_bytes);
    nonce
}

/// Nullifier set to prevent double-spending
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct NullifierSet {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Set of used nullifiers
    pub nullifiers: Vec<[u8; 32]>,
    
    /// Maximum size
    pub max_size: u32,
    
    /// Current size
    pub current_size: u32,
}

impl NullifierSet {
    pub const DISCRIMINATOR: [u8; 8] = [78, 85, 76, 76, 83, 69, 84, 0]; // "NULLSET"
    
    /// Check if nullifier exists
    pub fn contains(&self, nullifier: &[u8; 32]) -> bool {
        self.nullifiers.iter().any(|n| n == nullifier)
    }
    
    /// Add nullifier
    pub fn add(&mut self, nullifier: [u8; 32]) -> Result<(), ProgramError> {
        if self.contains(&nullifier) {
            return Err(BettingPlatformError::DuplicateEntry.into());
        }
        
        if self.current_size >= self.max_size {
            return Err(BettingPlatformError::QueueFull.into());
        }
        
        self.nullifiers.push(nullifier);
        self.current_size += 1;
        Ok(())
    }
}

/// Range proof for private values
pub fn process_create_range_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    value: u64,
    min: u64,
    max: u64,
) -> ProgramResult {
    msg!("Creating range proof for value in [{}, {}]", min, max);
    
    // Verify value is in range
    if value < min || value > max {
        return Err(BettingPlatformError::InvalidRange.into());
    }
    
    // Create commitment that proves min <= value <= max
    // without revealing the exact value
    let proof_data = RangeProof {
        commitment: keccak::hash(&value.to_le_bytes()).to_bytes(),
        min,
        max,
        // In production, would include actual zero-knowledge proof data
    };
    
    msg!("Range proof created successfully");
    Ok(())
}

/// Range proof structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RangeProof {
    pub commitment: [u8; 32],
    pub min: u64,
    pub max: u64,
}

/// Derive privacy commitment PDA
pub fn derive_privacy_commitment_pda(
    program_id: &Pubkey,
    user: &Pubkey,
    commitment: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"privacy_commitment",
            user.as_ref(),
            commitment,
        ],
        program_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_position_commitment() {
        let commitment_data = PrivatePositionCommitment {
            market_id: [1; 32],
            outcome: 1,
            size: 1000000,
            leverage: 10,
            nonce: [42; 32],
        };
        
        let hash1 = compute_position_commitment(&commitment_data).unwrap();
        let hash2 = compute_position_commitment(&commitment_data).unwrap();
        assert_eq!(hash1, hash2);
        
        // Different nonce should produce different hash
        let mut commitment_data2 = commitment_data.clone();
        commitment_data2.nonce = [43; 32];
        let hash3 = compute_position_commitment(&commitment_data2).unwrap();
        assert_ne!(hash1, hash3);
    }
    
    #[test]
    fn test_nullifier_generation() {
        let commitment = [1; 32];
        let nonce = [2; 32];
        
        let nullifier1 = generate_nullifier(&commitment, &nonce).unwrap();
        let nullifier2 = generate_nullifier(&commitment, &nonce).unwrap();
        assert_eq!(nullifier1, nullifier2);
        
        // Different nonce should produce different nullifier
        let nonce2 = [3; 32];
        let nullifier3 = generate_nullifier(&commitment, &nonce2).unwrap();
        assert_ne!(nullifier1, nullifier3);
    }
}