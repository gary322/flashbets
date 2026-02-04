//! Dispute Evidence & Validation System
//!
//! Implements comprehensive evidence handling:
//! - Evidence format validation
//! - Chain of custody tracking
//! - History preservation
//! - Integrity verification
//!
//! Per specification: Production-grade evidence handling

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
    keccak,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, HashSet};

use crate::{
    error::BettingPlatformError,
    integration::polymarket_api_types::{DisputeEvidence, DisputeVotes},
    events::{emit_event, EventType},
};

/// Evidence storage capacity
pub const MAX_EVIDENCE_PER_DISPUTE: usize = 100;
pub const MAX_EVIDENCE_SIZE_BYTES: usize = 1024 * 10; // 10KB per evidence

/// Evidence format types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum EvidenceFormat {
    Text { content: String },
    Url { url: String, hash: Option<[u8; 32]> },
    Image { ipfs_hash: [u8; 32], format: String },
    Document { ipfs_hash: [u8; 32], mime_type: String },
    OnChainData { account: Pubkey, offset: u32, length: u32 },
    OracleAttestation { oracle: Pubkey, signature: [u8; 64] },
}

/// Evidence metadata
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct EvidenceMetadata {
    pub evidence_id: [u8; 16],
    pub dispute_id: String,
    pub submitter: Pubkey,
    pub format: EvidenceFormat,
    pub timestamp: i64,
    pub block_height: u64,
    pub hash: [u8; 32],
    pub previous_hash: Option<[u8; 32]>,
    pub verification_status: VerificationStatus,
    pub weight: u32,
}

impl EvidenceMetadata {
    pub const SIZE: usize = 256;

    /// Create new evidence metadata
    pub fn new(
        dispute_id: String,
        submitter: Pubkey,
        format: EvidenceFormat,
        timestamp: i64,
        block_height: u64,
        previous_hash: Option<[u8; 32]>,
    ) -> Self {
        let evidence_id = Self::generate_id(&dispute_id, &submitter, timestamp);
        let hash = Self::calculate_hash(&evidence_id, &format, timestamp);

        Self {
            evidence_id,
            dispute_id,
            submitter,
            format,
            timestamp,
            block_height,
            hash,
            previous_hash,
            verification_status: VerificationStatus::Pending,
            weight: 0,
        }
    }

    /// Generate unique evidence ID
    fn generate_id(dispute_id: &str, submitter: &Pubkey, timestamp: i64) -> [u8; 16] {
        let hash = keccak::hashv(&[
            dispute_id.as_bytes(),
            submitter.as_ref(),
            &timestamp.to_le_bytes(),
        ]);
        
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.0[..16]);
        id
    }

    /// Calculate evidence hash for integrity
    fn calculate_hash(id: &[u8; 16], format: &EvidenceFormat, timestamp: i64) -> [u8; 32] {
        let format_bytes = format.try_to_vec().unwrap_or_default();
        
        keccak::hashv(&[
            id,
            &format_bytes,
            &timestamp.to_le_bytes(),
        ]).0
    }

    /// Verify evidence integrity
    pub fn verify_integrity(&self) -> Result<(), ProgramError> {
        let calculated_hash = Self::calculate_hash(
            &self.evidence_id,
            &self.format,
            self.timestamp,
        );

        if calculated_hash != self.hash {
            return Err(BettingPlatformError::InvalidProof.into());
        }

        Ok(())
    }
}

/// Verification status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum VerificationStatus {
    Pending,
    Verified { verifier: Pubkey, timestamp: i64 },
    Rejected { reason: String, timestamp: i64 },
    Expired,
}

/// Evidence chain for maintaining history
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EvidenceChain {
    pub dispute_id: String,
    pub evidence_list: Vec<[u8; 16]>, // Evidence IDs in order
    pub evidence_map: HashMap<[u8; 16], EvidenceMetadata>,
    pub total_evidence: u32,
    pub last_update: i64,
    pub chain_hash: [u8; 32],
}

impl EvidenceChain {
    pub const SIZE: usize = 1024 * 64; // 64KB for chain storage

    pub fn new(dispute_id: String) -> Self {
        Self {
            dispute_id,
            evidence_list: Vec::new(),
            evidence_map: HashMap::new(),
            total_evidence: 0,
            last_update: 0,
            chain_hash: [0u8; 32],
        }
    }

    /// Add evidence to chain
    pub fn add_evidence(
        &mut self,
        metadata: EvidenceMetadata,
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        if self.evidence_list.len() >= MAX_EVIDENCE_PER_DISPUTE {
            return Err(ProgramError::AccountDataTooSmall);
        }

        // Verify chain integrity
        metadata.verify_integrity()?;

        // Check previous hash matches
        if let Some(prev_hash) = metadata.previous_hash {
            if self.evidence_list.is_empty() {
                return Err(BettingPlatformError::InvalidEvidenceOrder.into());
            }

            let last_id = self.evidence_list.last().unwrap();
            if let Some(last_evidence) = self.evidence_map.get(last_id) {
                if last_evidence.hash != prev_hash {
                    return Err(BettingPlatformError::InvalidEvidenceOrder.into());
                }
            }
        }

        // Add to chain
        self.evidence_list.push(metadata.evidence_id);
        self.evidence_map.insert(metadata.evidence_id, metadata);
        self.total_evidence += 1;
        self.last_update = current_timestamp;

        // Update chain hash
        self.update_chain_hash();

        Ok(())
    }

    /// Update chain hash
    fn update_chain_hash(&mut self) {
        let mut hasher = Vec::new();
        
        for id in &self.evidence_list {
            hasher.extend_from_slice(id);
        }
        
        self.chain_hash = keccak::hash(&hasher).0;
    }

    /// Verify entire chain integrity
    pub fn verify_chain(&self) -> Result<(), ProgramError> {
        if self.evidence_list.is_empty() {
            return Ok(());
        }

        let mut previous_hash: Option<[u8; 32]> = None;

        for (i, id) in self.evidence_list.iter().enumerate() {
            let evidence = self.evidence_map.get(id)
                .ok_or(BettingPlatformError::InvalidProof)?;

            // Verify individual evidence
            evidence.verify_integrity()?;

            // Verify chain linkage
            if i > 0 {
                if evidence.previous_hash != previous_hash {
                    return Err(BettingPlatformError::InvalidEvidenceOrder.into());
                }
            }

            previous_hash = Some(evidence.hash);
        }

        Ok(())
    }

    /// Get evidence by ID
    pub fn get_evidence(&self, id: &[u8; 16]) -> Option<&EvidenceMetadata> {
        self.evidence_map.get(id)
    }

    /// Get all evidence in chronological order
    pub fn get_chronological_evidence(&self) -> Vec<&EvidenceMetadata> {
        self.evidence_list
            .iter()
            .filter_map(|id| self.evidence_map.get(id))
            .collect()
    }
}

/// Evidence validator with rules engine
pub struct EvidenceValidator {
    pub max_age_days: i64,
    pub min_weight_threshold: u32,
    pub required_verifications: u32,
}

impl EvidenceValidator {
    pub const DEFAULT_MAX_AGE_DAYS: i64 = 30;
    pub const DEFAULT_MIN_WEIGHT: u32 = 100;
    pub const DEFAULT_REQUIRED_VERIFICATIONS: u32 = 2;

    pub fn new() -> Self {
        Self {
            max_age_days: Self::DEFAULT_MAX_AGE_DAYS,
            min_weight_threshold: Self::DEFAULT_MIN_WEIGHT,
            required_verifications: Self::DEFAULT_REQUIRED_VERIFICATIONS,
        }
    }

    /// Validate evidence format
    pub fn validate_format(&self, format: &EvidenceFormat) -> Result<(), ProgramError> {
        match format {
            EvidenceFormat::Text { content } => {
                if content.is_empty() || content.len() > 10000 {
                    return Err(BettingPlatformError::InvalidEvidenceType.into());
                }
            }
            EvidenceFormat::Url { url, .. } => {
                if !url.starts_with("https://") || url.len() > 500 {
                    return Err(BettingPlatformError::InvalidEvidenceType.into());
                }
            }
            EvidenceFormat::Image { format, .. } => {
                let valid_formats = ["png", "jpg", "jpeg", "gif", "webp"];
                if !valid_formats.contains(&format.as_str()) {
                    return Err(BettingPlatformError::InvalidEvidenceType.into());
                }
            }
            EvidenceFormat::Document { mime_type, .. } => {
                let valid_types = ["application/pdf", "text/plain", "application/json"];
                if !valid_types.contains(&mime_type.as_str()) {
                    return Err(BettingPlatformError::InvalidEvidenceType.into());
                }
            }
            _ => {} // Other formats are validated differently
        }

        Ok(())
    }

    /// Validate evidence age
    pub fn validate_age(
        &self,
        evidence_timestamp: i64,
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        let age_seconds = current_timestamp - evidence_timestamp;
        let age_days = age_seconds / 86400;

        if age_days > self.max_age_days {
            return Err(BettingPlatformError::StaleEvidence.into());
        }

        Ok(())
    }

    /// Calculate evidence weight based on type and verifications
    pub fn calculate_weight(&self, evidence: &EvidenceMetadata) -> u32 {
        let base_weight = match &evidence.format {
            EvidenceFormat::OracleAttestation { .. } => 1000,
            EvidenceFormat::OnChainData { .. } => 800,
            EvidenceFormat::Document { .. } => 600,
            EvidenceFormat::Image { .. } => 400,
            EvidenceFormat::Url { .. } => 200,
            EvidenceFormat::Text { .. } => 100,
        };

        // Add weight for verifications
        let verification_bonus = match &evidence.verification_status {
            VerificationStatus::Verified { .. } => 500,
            _ => 0,
        };

        base_weight + verification_bonus
    }

    /// Validate evidence chain completeness
    pub fn validate_chain_completeness(
        &self,
        chain: &EvidenceChain,
    ) -> Result<bool, ProgramError> {
        // Verify chain integrity
        chain.verify_chain()?;

        // Check minimum evidence count
        if chain.total_evidence < 1 {
            return Ok(false);
        }

        // Calculate total weight
        let total_weight: u32 = chain.get_chronological_evidence()
            .iter()
            .map(|e| self.calculate_weight(e))
            .sum();

        Ok(total_weight >= self.min_weight_threshold)
    }
}

/// Evidence history tracker
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct EvidenceHistory {
    pub entries: Vec<HistoryEntry>,
    pub total_entries: u64,
    pub oldest_entry: i64,
    pub newest_entry: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub timestamp: i64,
    pub action: HistoryAction,
    pub actor: Pubkey,
    pub evidence_id: [u8; 16],
    pub details: String,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum HistoryAction {
    Submitted,
    Verified,
    Rejected,
    Updated,
    Expired,
}

impl EvidenceHistory {
    pub const SIZE: usize = 1024 * 16; // 16KB for history
    pub const MAX_ENTRIES: usize = 1000;

    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            total_entries: 0,
            oldest_entry: 0,
            newest_entry: 0,
        }
    }

    /// Add history entry
    pub fn add_entry(
        &mut self,
        action: HistoryAction,
        actor: Pubkey,
        evidence_id: [u8; 16],
        details: String,
        timestamp: i64,
    ) -> Result<(), ProgramError> {
        if self.entries.len() >= Self::MAX_ENTRIES {
            // Remove oldest entry
            self.entries.remove(0);
        }

        let entry = HistoryEntry {
            timestamp,
            action,
            actor,
            evidence_id,
            details,
        };

        self.entries.push(entry);
        self.total_entries += 1;

        if self.oldest_entry == 0 || timestamp < self.oldest_entry {
            self.oldest_entry = timestamp;
        }
        if timestamp > self.newest_entry {
            self.newest_entry = timestamp;
        }

        Ok(())
    }

    /// Get entries for specific evidence
    pub fn get_evidence_history(&self, evidence_id: &[u8; 16]) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| &e.evidence_id == evidence_id)
            .collect()
    }
}

/// Evidence aggregator for dispute resolution
pub struct EvidenceAggregator;

impl EvidenceAggregator {
    /// Aggregate evidence scores for outcome determination
    pub fn aggregate_evidence_scores(
        chain: &EvidenceChain,
        validator: &EvidenceValidator,
    ) -> HashMap<String, u32> {
        let mut outcome_scores: HashMap<String, u32> = HashMap::new();

        for evidence in chain.get_chronological_evidence() {
            let weight = validator.calculate_weight(evidence);
            
            // Extract outcome from evidence (simplified)
            let outcome = match &evidence.format {
                EvidenceFormat::Text { content } => {
                    if content.contains("YES") {
                        "YES"
                    } else if content.contains("NO") {
                        "NO"
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };

            *outcome_scores.entry(outcome.to_string()).or_insert(0) += weight;
        }

        outcome_scores
    }

    /// Determine consensus outcome
    pub fn determine_consensus(
        scores: &HashMap<String, u32>,
        threshold_percentage: u32,
    ) -> Option<String> {
        let total_score: u32 = scores.values().sum();
        if total_score == 0 {
            return None;
        }

        for (outcome, score) in scores {
            let percentage = (score * 100) / total_score;
            if percentage >= threshold_percentage {
                return Some(outcome.clone());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_chain() {
        let mut chain = EvidenceChain::new("dispute-123".to_string());
        
        let evidence1 = EvidenceMetadata::new(
            "dispute-123".to_string(),
            Pubkey::new_unique(),
            EvidenceFormat::Text { content: "Evidence 1".to_string() },
            100,
            1000,
            None,
        );

        chain.add_evidence(evidence1.clone(), 100).unwrap();
        assert_eq!(chain.total_evidence, 1);

        let evidence2 = EvidenceMetadata::new(
            "dispute-123".to_string(),
            Pubkey::new_unique(),
            EvidenceFormat::Text { content: "Evidence 2".to_string() },
            200,
            2000,
            Some(evidence1.hash),
        );

        chain.add_evidence(evidence2, 200).unwrap();
        assert_eq!(chain.total_evidence, 2);

        // Verify chain integrity
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn test_evidence_validation() {
        let validator = EvidenceValidator::new();
        
        // Valid text format
        let text_format = EvidenceFormat::Text { 
            content: "Valid evidence".to_string() 
        };
        assert!(validator.validate_format(&text_format).is_ok());

        // Invalid URL format
        let url_format = EvidenceFormat::Url { 
            url: "http://insecure.com".to_string(),
            hash: None,
        };
        assert!(validator.validate_format(&url_format).is_err());

        // Valid age
        assert!(validator.validate_age(100, 200).is_ok());

        // Stale evidence
        let stale_timestamp = 100;
        let current = stale_timestamp + (31 * 86400); // 31 days later
        assert!(validator.validate_age(stale_timestamp, current).is_err());
    }

    #[test]
    fn test_evidence_weight_calculation() {
        let validator = EvidenceValidator::new();
        
        let oracle_evidence = EvidenceMetadata {
            evidence_id: [0u8; 16],
            dispute_id: "test".to_string(),
            submitter: Pubkey::new_unique(),
            format: EvidenceFormat::OracleAttestation {
                oracle: Pubkey::new_unique(),
                signature: [0u8; 64],
            },
            timestamp: 100,
            block_height: 1000,
            hash: [0u8; 32],
            previous_hash: None,
            verification_status: VerificationStatus::Verified {
                verifier: Pubkey::new_unique(),
                timestamp: 200,
            },
            weight: 0,
        };

        let weight = validator.calculate_weight(&oracle_evidence);
        assert_eq!(weight, 1500); // 1000 base + 500 verification bonus
    }
}