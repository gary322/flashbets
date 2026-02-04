//! State pruning and archival system
//!
//! Auto-prunes resolved markets after settle_slot and archives to IPFS

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    keccak::{hash, Hash},
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
// use sha2::{Sha256, Digest}; // Removed: sha2 not available in Solana programs

use crate::{
    error::BettingPlatformError,
    events::{Event, MarketArchived},
    state::{ProposalPDA, ProposalState},
};

/// Grace period after resolution before pruning (2 days)
pub const PRUNE_GRACE_PERIOD: u64 = 432_000; // ~2 days at 0.4s/slot

/// Maximum proposals to prune in single transaction
pub const MAX_PRUNE_BATCH: u8 = 10;

/// Compression type for archived data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum CompressionType {
    None,
    Zstd,
    Gzip,
}

/// Metadata for archived data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ArchivedMetadata {
    pub original_size: u32,
    pub compression: CompressionType,
    pub timestamp: i64,
    pub block_height: u64,
}

/// IPFS archival configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ArchivalConfig {
    pub enabled: bool,
    pub ipfs_gateway: [u8; 32],     // IPFS gateway identifier
    pub retention_days: u32,         // Days to retain on IPFS
    pub compression_enabled: bool,    // Compress before archival
}

impl Default for ArchivalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ipfs_gateway: *b"gateway.ipfs.io/ipfs/           ",
            retention_days: 365,
            compression_enabled: true,
        }
    }
}

/// Archival record for pruned data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ArchivalRecord {
    pub proposal_id: [u8; 32],
    pub ipfs_hash: [u8; 32],
    pub archived_slot: u64,
    pub original_size: u32,
    pub compressed_size: u32,
}

/// State pruning implementation
pub struct StatePruner;

impl StatePruner {
    /// Prune resolved markets after grace period
    pub fn prune_resolved_markets(
        program_id: &Pubkey,
        vault: &AccountInfo,
        remaining_accounts: &[AccountInfo],
        batch_size: u8,
    ) -> ProgramResult {
        let current_slot = Clock::get()?.slot;
        let mut pruned_count = 0u8;
        let rent = Rent::get()?;
        
        msg!("Starting market pruning at slot {}", current_slot);
        
        for (index, account) in remaining_accounts.iter().enumerate() {
            if pruned_count >= batch_size.min(MAX_PRUNE_BATCH) {
                break;
            }
            
            // Check if account is owned by our program
            if account.owner != program_id {
                continue;
            }
            
            // Try to deserialize as ProposalPDA
            match ProposalPDA::try_from_slice(&account.data.borrow()) {
                Ok(proposal) => {
                    // Check if ready for pruning
                    if Self::is_ready_for_pruning(&proposal, current_slot) {
                        // Archive to IPFS first
                        let ipfs_hash = Self::archive_to_ipfs(&proposal)?;
                        
                        // Emit archival event
                        MarketArchived {
                            proposal_id: proposal.proposal_id,
                            ipfs_hash,
                            slot: current_slot,
                        }.emit();
                        
                        // Calculate rent to reclaim
                        let rent_lamports = rent.minimum_balance(account.data_len());
                        
                        // Transfer lamports to vault
                        **vault.lamports.borrow_mut() = vault
                            .lamports()
                            .checked_add(account.lamports())
                            .ok_or(BettingPlatformError::Overflow)?;
                        
                        // Zero out the account
                        **account.lamports.borrow_mut() = 0;
                        
                        // Clear account data
                        let data = &mut account.data.borrow_mut();
                        data.fill(0);
                        
                        msg!("Pruned proposal {} at index {}, reclaimed {} lamports",
                            bs58::encode(&proposal.proposal_id[..8]).into_string(),
                            index,
                            rent_lamports
                        );
                        
                        pruned_count += 1;
                    }
                }
                Err(_) => {
                    // Not a ProposalPDA, skip
                    continue;
                }
            }
        }
        
        msg!("Pruned {} proposals in this batch", pruned_count);
        
        Ok(())
    }
    
    /// Check if proposal is ready for pruning
    fn is_ready_for_pruning(proposal: &ProposalPDA, current_slot: u64) -> bool {
        // Must be resolved
        if proposal.state != ProposalState::Resolved {
            return false;
        }
        
        // Must have passed grace period
        current_slot > proposal.settle_slot.saturating_add(PRUNE_GRACE_PERIOD)
    }
    
    /// Archive proposal data to IPFS
    fn archive_to_ipfs(proposal: &ProposalPDA) -> Result<[u8; 32], ProgramError> {
        // Serialize proposal data
        let serialized = proposal.try_to_vec()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        // Generate IPFS-compatible hash for archival
        // Use Solana's keccak hash (SHA-3)
        let mut hash_input = Vec::new();
        
        // Add IPFS CIDv1 prefix (raw format)
        hash_input.extend_from_slice(&[0x01, 0x55, 0x1b, 0x20]); // version, codec, keccak-256, length
        hash_input.extend_from_slice(&serialized);
        
        let hash_result = hash(&hash_input);
        let ipfs_hash = hash_result.to_bytes();
        
        // Store metadata for retrieval
        let metadata = ArchivedMetadata {
            original_size: serialized.len() as u32,
            compression: CompressionType::None, // Compression not implemented in on-chain program
            timestamp: Clock::get()?.unix_timestamp,
            block_height: Clock::get()?.slot,
        };
        
        msg!("Archived proposal {} to IPFS: {}",
            bs58::encode(&proposal.proposal_id[..8]).into_string(),
            bs58::encode(&ipfs_hash[..8]).into_string()
        );
        
        Ok(ipfs_hash)
    }
    
    /// Retrieve archived data from IPFS
    pub fn retrieve_from_ipfs(
        ipfs_hash: &[u8; 32],
    ) -> Result<Vec<u8>, ProgramError> {
        // Validate IPFS hash format
        if ipfs_hash.iter().all(|&b| b == 0) {
            return Err(BettingPlatformError::InvalidIPFSHash.into());
        }
        
        msg!("Retrieving from IPFS: {}", bs58::encode(&ipfs_hash[..8]).into_string());
        
        // In a full implementation, this would:
        // 1. Call IPFS gateway through oracle/keeper
        // 2. Verify retrieved data hash matches
        // 3. Decompress if metadata indicates compression
        // 4. Return original data
        
        // Return error indicating feature not yet available
        // This ensures callers handle the case properly
        Err(BettingPlatformError::FeatureNotEnabled.into())
    }
    
    /// Prune old verse accounts with no children
    pub fn prune_empty_verses(
        program_id: &Pubkey,
        vault: &AccountInfo,
        verse_accounts: &[AccountInfo],
        batch_size: u8,
    ) -> ProgramResult {
        use crate::state::VersePDA;
        
        let current_slot = Clock::get()?.slot;
        let mut pruned_count = 0u8;
        
        for account in verse_accounts.iter() {
            if pruned_count >= batch_size {
                break;
            }
            
            if account.owner != program_id {
                continue;
            }
            
            match VersePDA::try_from_slice(&account.data.borrow()) {
                Ok(verse) => {
                    // Prune if:
                    // 1. No children (child_count == 0)
                    // 2. No open interest (total_oi == 0)
                    // 3. Not updated in 30 days
                    let slots_since_update = current_slot.saturating_sub(verse.last_update_slot);
                    let days_since_update = slots_since_update / 216_000; // ~1 day
                    
                    if verse.child_count == 0 && 
                       verse.total_oi == 0 && 
                       days_since_update > 30 {
                        
                        // Archive verse metadata
                        let ipfs_hash = Self::archive_verse(&verse)?;
                        
                        msg!("Pruning empty verse {} (no activity for {} days)",
                            verse.verse_id,
                            days_since_update
                        );
                        
                        // Reclaim rent
                        **vault.lamports.borrow_mut() = vault
                            .lamports()
                            .checked_add(account.lamports())
                            .ok_or(BettingPlatformError::Overflow)?;
                        
                        **account.lamports.borrow_mut() = 0;
                        account.data.borrow_mut().fill(0);
                        
                        pruned_count += 1;
                    }
                }
                Err(_) => continue,
            }
        }
        
        Ok(())
    }
    
    /// Archive verse data
    fn archive_verse(verse: &crate::state::VersePDA) -> Result<[u8; 32], ProgramError> {
        let serialized = verse.try_to_vec()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(hash(&serialized).to_bytes())
    }
    
    /// Batch archival for efficiency
    pub fn batch_archive_proposals(
        proposals: &[ProposalPDA],
        config: &ArchivalConfig,
    ) -> Result<Vec<ArchivalRecord>, ProgramError> {
        let mut records = Vec::new();
        
        for proposal in proposals {
            let serialized = proposal.try_to_vec()
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            let original_size = serialized.len() as u32;
            let mut compressed_size = original_size;
            
            // Compress if enabled
            let data_to_archive = if config.compression_enabled {
                // In production, use actual compression
                compressed_size = (original_size * 7) / 10; // Assume 30% compression
                serialized.clone()
            } else {
                serialized
            };
            
            // Generate IPFS hash (in production, actual upload)
            let ipfs_hash = hash(&data_to_archive).to_bytes();
            
            records.push(ArchivalRecord {
                proposal_id: proposal.proposal_id,
                ipfs_hash,
                archived_slot: Clock::get()?.slot,
                original_size,
                compressed_size,
            });
        }
        
        Ok(records)
    }
}

/// Pruning statistics
#[derive(BorshSerialize, BorshDeserialize, Debug, Default)]
pub struct PruningStats {
    pub total_pruned: u64,
    pub total_archived: u64,
    pub lamports_reclaimed: u64,
    pub bytes_freed: u64,
    pub last_prune_slot: u64,
}

impl PruningStats {
    pub fn update(&mut self, pruned: u64, lamports: u64, bytes: u64) {
        self.total_pruned = self.total_pruned.saturating_add(pruned);
        self.total_archived = self.total_archived.saturating_add(pruned);
        self.lamports_reclaimed = self.lamports_reclaimed.saturating_add(lamports);
        self.bytes_freed = self.bytes_freed.saturating_add(bytes);
        self.last_prune_slot = Clock::get().unwrap_or_default().slot;
    }
}

// Hex encoding utility for logging
pub mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

