//! Resolution account structures
//!
//! Account types for market resolution and dispute handling

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::account_validation::DISCRIMINATOR_SIZE;

/// Discriminators for resolution account types
pub mod discriminators {
    pub const RESOLUTION_STATE: [u8; 8] = [89, 201, 45, 167, 23, 78, 156, 234];
    pub const DISPUTE_STATE: [u8; 8] = [156, 234, 89, 45, 201, 167, 23, 78];
    pub const ORACLE_CONFIG: [u8; 8] = [234, 167, 89, 156, 45, 23, 201, 78];
    pub const SETTLEMENT_QUEUE: [u8; 8] = [45, 78, 201, 23, 167, 89, 234, 156];
}

/// Resolution status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ResolutionStatus {
    Pending,
    Proposed,
    Confirmed,
    Disputed,
    Resolved,
    Cancelled,
}

/// Oracle type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OracleType {
    Centralized,      // Single trusted oracle
    Decentralized,    // Multiple oracles with consensus
    UMA,             // UMA protocol integration
    Chainlink,       // Chainlink integration
    Community,       // Community voting
}

/// Resolution state for a market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ResolutionState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// Verse ID
    pub verse_id: u128,
    
    /// Resolution status
    pub status: ResolutionStatus,
    
    /// Proposed outcome
    pub proposed_outcome: Option<u8>,
    
    /// Final outcome
    pub final_outcome: Option<u8>,
    
    /// Oracle that proposed resolution
    pub proposing_oracle: Option<Pubkey>,
    
    /// Proposal timestamp
    pub proposed_at: Option<i64>,
    
    /// Confirmation timestamp
    pub confirmed_at: Option<i64>,
    
    /// Resolution timestamp
    pub resolved_at: Option<i64>,
    
    /// Dispute window end
    pub dispute_window_end: Option<i64>,
    
    /// Settlement completed
    pub settlement_completed: bool,
    
    /// Total payout distributed
    pub total_payout: u64,
    
    /// Number of positions settled
    pub positions_settled: u64,
    
    /// Oracle signatures (for multi-oracle)
    pub oracle_signatures: Vec<OracleSignature>,
}

impl ResolutionState {
    pub const BASE_SIZE: usize = DISCRIMINATOR_SIZE + 16 + 16 + 1 + 1 + 1 + 32 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 4;
    
    pub fn space(max_oracles: usize) -> usize {
        Self::BASE_SIZE + (max_oracles * std::mem::size_of::<OracleSignature>())
    }
    
    pub fn new(market_id: u128, verse_id: u128) -> Self {
        Self {
            discriminator: discriminators::RESOLUTION_STATE,
            market_id,
            verse_id,
            status: ResolutionStatus::Pending,
            proposed_outcome: None,
            final_outcome: None,
            proposing_oracle: None,
            proposed_at: None,
            confirmed_at: None,
            resolved_at: None,
            dispute_window_end: None,
            settlement_completed: false,
            total_payout: 0,
            positions_settled: 0,
            oracle_signatures: Vec::new(),
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::RESOLUTION_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Oracle signature for resolution
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct OracleSignature {
    /// Oracle pubkey
    pub oracle: Pubkey,
    
    /// Signed outcome
    pub outcome: u8,
    
    /// Signature timestamp
    pub timestamp: i64,
    
    /// Signature data
    pub signature: [u8; 64],
}

/// Dispute state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct DisputeState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// Disputer
    pub disputer: Pubkey,
    
    /// Dispute bond amount
    pub bond_amount: u64,
    
    /// Disputed outcome
    pub disputed_outcome: u8,
    
    /// Proposed alternative outcome
    pub proposed_outcome: u8,
    
    /// Dispute reason
    pub reason: DisputeReason,
    
    /// Evidence CID (IPFS)
    pub evidence_cid: Option<[u8; 32]>,
    
    /// Dispute timestamp
    pub disputed_at: i64,
    
    /// Dispute status
    pub status: DisputeStatus,
    
    /// Resolution timestamp
    pub resolved_at: Option<i64>,
    
    /// Resolution outcome
    pub resolution: Option<DisputeResolution>,
    
    /// Arbitrators assigned
    pub arbitrators: Vec<Pubkey>,
    
    /// Arbitration votes
    pub votes: Vec<ArbitrationVote>,
}

impl DisputeState {
    pub const BASE_SIZE: usize = DISCRIMINATOR_SIZE + 16 + 32 + 8 + 1 + 1 + 1 + 32 + 8 + 1 + 8 + 1 + 4 + 4;
    
    pub fn space(max_arbitrators: usize) -> usize {
        Self::BASE_SIZE + 
        (max_arbitrators * 32) + // arbitrators
        (max_arbitrators * std::mem::size_of::<ArbitrationVote>()) // votes
    }
    
    pub fn new(
        market_id: u128,
        disputer: Pubkey,
        bond_amount: u64,
        disputed_outcome: u8,
        proposed_outcome: u8,
        reason: DisputeReason,
        timestamp: i64,
    ) -> Self {
        Self {
            discriminator: discriminators::DISPUTE_STATE,
            market_id,
            disputer,
            bond_amount,
            disputed_outcome,
            proposed_outcome,
            reason,
            evidence_cid: None,
            disputed_at: timestamp,
            status: DisputeStatus::Active,
            resolved_at: None,
            resolution: None,
            arbitrators: Vec::new(),
            votes: Vec::new(),
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::DISPUTE_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Dispute reason
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum DisputeReason {
    IncorrectOutcome,
    AmbiguousRules,
    TechnicalError,
    MarketManipulation,
    OracleFailure,
    Other,
}

/// Dispute status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum DisputeStatus {
    Active,
    UnderReview,
    Resolved,
    Rejected,
    Expired,
}

/// Dispute resolution
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum DisputeResolution {
    Upheld,      // Original outcome maintained
    Overturned,  // Dispute successful, outcome changed
    Invalid,     // Market declared invalid
}

/// Arbitration vote
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ArbitrationVote {
    /// Arbitrator
    pub arbitrator: Pubkey,
    
    /// Vote
    pub vote: DisputeResolution,
    
    /// Reasoning (optional)
    pub reasoning_cid: Option<[u8; 32]>,
    
    /// Vote timestamp
    pub timestamp: i64,
}

/// Oracle configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct OracleConfig {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Oracle type
    pub oracle_type: OracleType,
    
    /// Primary oracle
    pub primary_oracle: Pubkey,
    
    /// Secondary oracles (for consensus)
    pub secondary_oracles: Vec<Pubkey>,
    
    /// Required confirmations
    pub required_confirmations: u8,
    
    /// Dispute window (seconds)
    pub dispute_window: u64,
    
    /// Dispute bond required
    pub dispute_bond: u64,
    
    /// Oracle fee
    pub oracle_fee: u64,
    
    /// Active
    pub is_active: bool,
    
    /// Total resolutions
    pub total_resolutions: u64,
    
    /// Successful resolutions
    pub successful_resolutions: u64,
    
    /// Failed/disputed resolutions
    pub failed_resolutions: u64,
}

impl OracleConfig {
    pub const BASE_SIZE: usize = DISCRIMINATOR_SIZE + 1 + 32 + 4 + 1 + 8 + 8 + 8 + 1 + 8 + 8 + 8;
    
    pub fn space(max_secondary_oracles: usize) -> usize {
        Self::BASE_SIZE + (max_secondary_oracles * 32)
    }
    
    pub fn new(
        oracle_type: OracleType,
        primary_oracle: Pubkey,
        dispute_window: u64,
        dispute_bond: u64,
    ) -> Self {
        Self {
            discriminator: discriminators::ORACLE_CONFIG,
            oracle_type,
            primary_oracle,
            secondary_oracles: Vec::new(),
            required_confirmations: 1,
            dispute_window,
            dispute_bond,
            oracle_fee: 1_000_000, // 0.001 SOL default
            is_active: true,
            total_resolutions: 0,
            successful_resolutions: 0,
            failed_resolutions: 0,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::ORACLE_CONFIG {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Settlement queue for batch processing
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct SettlementQueue {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// Queue items
    pub items: Vec<SettlementItem>,
    
    /// Head index
    pub head: u32,
    
    /// Tail index
    pub tail: u32,
    
    /// Max size
    pub max_size: u32,
    
    /// Total items processed
    pub total_processed: u64,
    
    /// Settlement started
    pub settlement_started: bool,
    
    /// Settlement completed
    pub settlement_completed: bool,
}

impl SettlementQueue {
    pub const BASE_SIZE: usize = DISCRIMINATOR_SIZE + 16 + 4 + 4 + 4 + 4 + 8 + 1 + 1;
    
    pub fn space(max_size: u32) -> usize {
        Self::BASE_SIZE + (max_size as usize * std::mem::size_of::<SettlementItem>())
    }
    
    pub fn new(market_id: u128, max_size: u32) -> Self {
        Self {
            discriminator: discriminators::SETTLEMENT_QUEUE,
            market_id,
            items: Vec::with_capacity(max_size as usize),
            head: 0,
            tail: 0,
            max_size,
            total_processed: 0,
            settlement_started: false,
            settlement_completed: false,
        }
    }
    
    pub fn add_item(&mut self, item: SettlementItem) -> Result<(), ProgramError> {
        if self.items.len() >= self.max_size as usize {
            return Err(ProgramError::AccountDataTooSmall);
        }
        self.items.push(item);
        self.tail = (self.tail + 1) % self.max_size;
        Ok(())
    }
    
    pub fn get_next(&mut self) -> Option<SettlementItem> {
        if self.head == self.tail && self.items.is_empty() {
            return None;
        }
        
        if let Some(item) = self.items.get(self.head as usize).cloned() {
            self.head = (self.head + 1) % self.max_size;
            self.total_processed += 1;
            Some(item)
        } else {
            None
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::SETTLEMENT_QUEUE {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Settlement item
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct SettlementItem {
    /// Position holder
    pub position_holder: Pubkey,
    
    /// Position account
    pub position_account: Pubkey,
    
    /// Outcome held
    pub outcome: u8,
    
    /// Shares held
    pub shares: u64,
    
    /// Entry price
    pub entry_price: u64,
    
    /// Payout amount
    pub payout: u64,
}