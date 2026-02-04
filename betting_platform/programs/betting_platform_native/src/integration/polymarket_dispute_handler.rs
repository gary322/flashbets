//! Polymarket Dispute Handler
//!
//! Implements direct Polymarket dispute API integration:
//! - Dispute polling and detection
//! - State synchronization
//! - Timeline event tracking
//! - Evidence validation
//!
//! Per specification: Mirror Polymarket dispute outcomes exactly

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::HashMap;

use crate::{
    error::BettingPlatformError,
    integration::polymarket_api_types::{
        DisputeInfo as PolymarketDisputeInfo, DisputeStatus, DisputeEvidence, DisputeVotes,
    },
    events::{emit_event, EventType, DisputeDetected},
};

/// Dispute synchronization state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct DisputeSyncState {
    pub last_poll_timestamp: i64,
    pub active_disputes: HashMap<String, TrackedDispute>,
    pub resolved_disputes: HashMap<String, ResolvedDispute>,
    pub total_disputes_tracked: u64,
    pub total_positions_affected: u64,
    pub total_reversals: u32,
}

impl DisputeSyncState {
    pub const SIZE: usize = 1024 * 32; // 32KB for dispute tracking

    pub fn new() -> Self {
        Self {
            last_poll_timestamp: 0,
            active_disputes: HashMap::new(),
            resolved_disputes: HashMap::new(),
            total_disputes_tracked: 0,
            total_positions_affected: 0,
            total_reversals: 0,
        }
    }
}

/// Local dispute info that can be serialized
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct DisputeInfo {
    pub dispute_id: String,
    pub market_id: String,
    pub disputer: String,
    pub proposed_outcome: String,
    pub original_outcome: String,
    pub dispute_bond: u64, // Stored as lamports
    pub status: String, // Status as string for serialization
    pub created_at: i64,
    pub deadline: i64,
    pub evidence_count: u32,
    pub has_votes: bool,
}

impl DisputeInfo {
    pub fn from_polymarket(pm_dispute: &PolymarketDisputeInfo) -> Self {
        Self {
            dispute_id: pm_dispute.dispute_id.clone(),
            market_id: pm_dispute.market_id.clone(),
            disputer: pm_dispute.disputer.clone(),
            proposed_outcome: pm_dispute.proposed_outcome.clone(),
            original_outcome: pm_dispute.original_outcome.clone(),
            dispute_bond: (pm_dispute.dispute_bond * 1_000_000_000.0) as u64, // Convert to lamports
            status: format!("{:?}", pm_dispute.status),
            created_at: pm_dispute.created_at,
            deadline: pm_dispute.deadline,
            evidence_count: pm_dispute.evidence.len() as u32,
            has_votes: pm_dispute.votes.is_some(),
        }
    }
}

/// Tracked dispute with local state
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct TrackedDispute {
    pub dispute_info: DisputeInfo,
    pub local_market_id: [u8; 16],
    pub positions_frozen: Vec<Pubkey>,
    pub original_outcome: String,
    pub tracking_started: i64,
    pub last_update: i64,
    pub update_count: u32,
}

/// Resolved dispute record
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ResolvedDispute {
    pub dispute_id: String,
    pub market_id: [u8; 16],
    pub original_outcome: String,
    pub final_outcome: String,
    pub resolution_timestamp: i64,
    pub positions_reversed: u32,
    pub total_volume_affected: u64,
}

/// Dispute handler for Polymarket integration
pub struct PolymarketDisputeHandler {
    pub sync_state: DisputeSyncState,
    pub poll_interval_seconds: i64,
}

impl PolymarketDisputeHandler {
    pub const POLL_INTERVAL: i64 = 300; // 5 minutes
    pub const MAX_EVIDENCE_AGE_DAYS: i64 = 30;

    pub fn new() -> Self {
        Self {
            sync_state: DisputeSyncState::new(),
            poll_interval_seconds: Self::POLL_INTERVAL,
        }
    }

    /// Process dispute update from Polymarket
    pub fn process_dispute_update(
        &mut self,
        dispute_info: DisputeInfo,
        current_timestamp: i64,
    ) -> Result<DisputeAction, ProgramError> {
        let dispute_id = dispute_info.dispute_id.clone();

        // Check if this is a new dispute
        if !self.sync_state.active_disputes.contains_key(&dispute_id) {
            return self.handle_new_dispute(dispute_info, current_timestamp);
        }

        // Update existing dispute
        self.handle_dispute_update(dispute_info, current_timestamp)
    }

    /// Handle new dispute detection
    fn handle_new_dispute(
        &mut self,
        dispute_info: DisputeInfo,
        current_timestamp: i64,
    ) -> Result<DisputeAction, ProgramError> {
        msg!("New dispute detected: {}", dispute_info.dispute_id);

        // Convert market ID
        let mut local_market_id = [0u8; 16];
        let market_bytes = dispute_info.market_id.as_bytes();
        let copy_len = market_bytes.len().min(16);
        local_market_id[..copy_len].copy_from_slice(&market_bytes[..copy_len]);

        let tracked = TrackedDispute {
            dispute_info: dispute_info.clone(),
            local_market_id,
            positions_frozen: Vec::new(),
            original_outcome: dispute_info.original_outcome.clone(),
            tracking_started: current_timestamp,
            last_update: current_timestamp,
            update_count: 1,
        };

        self.sync_state.active_disputes.insert(dispute_info.dispute_id.clone(), tracked);
        self.sync_state.total_disputes_tracked += 1;

        // Emit event
        let market_id_bytes = local_market_id;
        
        emit_event(EventType::DisputeDetected, &DisputeDetected {
            market_id: market_id_bytes,
            dispute_id: dispute_info.dispute_id,
            proposed_outcome: dispute_info.proposed_outcome,
        });

        Ok(DisputeAction::FreezePositions { market_id: local_market_id })
    }

    /// Handle update to existing dispute
    fn handle_dispute_update(
        &mut self,
        dispute_info: DisputeInfo,
        current_timestamp: i64,
    ) -> Result<DisputeAction, ProgramError> {
        let dispute_id = dispute_info.dispute_id.clone();
        
        if let Some(tracked) = self.sync_state.active_disputes.get_mut(&dispute_id) {
            let old_status = tracked.dispute_info.status.clone();
            
            // Update tracking
            tracked.dispute_info = dispute_info.clone();
            tracked.last_update = current_timestamp;
            tracked.update_count += 1;

            // Check for status change
            if old_status != dispute_info.status {
                msg!("Dispute {} status changed: {:?} -> {:?}", 
                    dispute_id, old_status, dispute_info.status);

                match dispute_info.status.as_str() {
                    "Resolved" => {
                        return self.handle_dispute_resolution(dispute_id, current_timestamp);
                    }
                    "Rejected" => {
                        return self.handle_dispute_rejection(dispute_id, current_timestamp);
                    }
                    _ => {}
                }
            }

            // Check for new evidence
            if dispute_info.evidence_count > tracked.dispute_info.evidence_count {
                msg!("New evidence submitted for dispute {}", dispute_id);
                return Ok(DisputeAction::ProcessEvidence { 
                    dispute_id,
                    evidence_count: dispute_info.evidence_count,
                });
            }
        }

        Ok(DisputeAction::NoAction)
    }

    /// Handle dispute resolution
    fn handle_dispute_resolution(
        &mut self,
        dispute_id: String,
        current_timestamp: i64,
    ) -> Result<DisputeAction, ProgramError> {
        if let Some(tracked) = self.sync_state.active_disputes.remove(&dispute_id) {
            let final_outcome = tracked.dispute_info.proposed_outcome.clone();
            let original_outcome = tracked.original_outcome.clone();
            let local_market_id = tracked.local_market_id;
            let positions_frozen = tracked.positions_frozen.clone();
            let requires_reversal = original_outcome != final_outcome;

            let resolved = ResolvedDispute {
                dispute_id: dispute_id.clone(),
                market_id: local_market_id,
                original_outcome: original_outcome.clone(),
                final_outcome: final_outcome.clone(),
                resolution_timestamp: current_timestamp,
                positions_reversed: if requires_reversal { 
                    positions_frozen.len() as u32 
                } else { 
                    0 
                },
                total_volume_affected: 0, // Would be calculated from positions
            };

            self.sync_state.resolved_disputes.insert(dispute_id.clone(), resolved);

            if requires_reversal {
                self.sync_state.total_reversals += 1;
                msg!("Dispute resolved with reversal: {} -> {}", 
                    original_outcome, final_outcome);

                return Ok(DisputeAction::ReversePositions {
                    market_id: local_market_id,
                    original_outcome,
                    new_outcome: final_outcome,
                    affected_positions: tracked.positions_frozen,
                });
            } else {
                msg!("Dispute resolved, original outcome upheld");
                return Ok(DisputeAction::UnfreezePositions {
                    market_id: tracked.local_market_id,
                });
            }
        }

        Ok(DisputeAction::NoAction)
    }

    /// Handle dispute rejection
    fn handle_dispute_rejection(
        &mut self,
        dispute_id: String,
        current_timestamp: i64,
    ) -> Result<DisputeAction, ProgramError> {
        if let Some(tracked) = self.sync_state.active_disputes.remove(&dispute_id) {
            msg!("Dispute {} rejected, unfreezing positions", dispute_id);

            // Record as resolved with original outcome
            let resolved = ResolvedDispute {
                dispute_id: dispute_id.clone(),
                market_id: tracked.local_market_id,
                original_outcome: tracked.original_outcome.clone(),
                final_outcome: tracked.original_outcome, // Same as original
                resolution_timestamp: current_timestamp,
                positions_reversed: 0,
                total_volume_affected: 0,
            };

            self.sync_state.resolved_disputes.insert(dispute_id, resolved);

            return Ok(DisputeAction::UnfreezePositions {
                market_id: tracked.local_market_id,
            });
        }

        Ok(DisputeAction::NoAction)
    }

    /// Poll for dispute updates
    pub fn should_poll(&self, current_timestamp: i64) -> bool {
        current_timestamp >= self.sync_state.last_poll_timestamp + self.poll_interval_seconds
    }

    /// Update poll timestamp
    pub fn update_poll_time(&mut self, timestamp: i64) {
        self.sync_state.last_poll_timestamp = timestamp;
    }

    /// Get dispute statistics
    pub fn get_stats(&self) -> DisputeStats {
        let active_count = self.sync_state.active_disputes.len() as u32;
        let resolved_count = self.sync_state.resolved_disputes.len() as u32;
        
        DisputeStats {
            active_disputes: active_count,
            resolved_disputes: resolved_count,
            total_tracked: self.sync_state.total_disputes_tracked,
            total_reversals: self.sync_state.total_reversals,
            positions_affected: self.sync_state.total_positions_affected,
        }
    }

    /// Validate dispute evidence
    pub fn validate_evidence(
        &self,
        evidence: &DisputeEvidence,
        current_timestamp: i64,
    ) -> Result<(), ProgramError> {
        // Check evidence age
        let age_days = (current_timestamp - evidence.timestamp) / 86400;
        if age_days > Self::MAX_EVIDENCE_AGE_DAYS {
            return Err(BettingPlatformError::StaleEvidence.into());
        }

        // Validate evidence type
        match evidence.evidence_type.as_str() {
            "url" | "text" | "image" | "document" => Ok(()),
            _ => Err(BettingPlatformError::InvalidEvidenceType.into()),
        }
    }

    /// Calculate dispute impact score
    pub fn calculate_dispute_impact(&self, dispute_id: &str) -> f64 {
        if let Some(tracked) = self.sync_state.active_disputes.get(dispute_id) {
            let age_factor = tracked.update_count as f64 / 10.0;
            let position_factor = tracked.positions_frozen.len() as f64 / 100.0;
            let vote_factor = if tracked.dispute_info.has_votes {
                0.7 // Assume favorable vote factor if votes exist
            } else {
                0.5
            };

            (age_factor + position_factor + vote_factor) / 3.0
        } else {
            0.0
        }
    }
}

/// Actions to take based on dispute updates
#[derive(Debug, Clone)]
pub enum DisputeAction {
    NoAction,
    FreezePositions { 
        market_id: [u8; 16] 
    },
    UnfreezePositions { 
        market_id: [u8; 16] 
    },
    ReversePositions {
        market_id: [u8; 16],
        original_outcome: String,
        new_outcome: String,
        affected_positions: Vec<Pubkey>,
    },
    ProcessEvidence {
        dispute_id: String,
        evidence_count: u32,
    },
}

/// Dispute statistics
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct DisputeStats {
    pub active_disputes: u32,
    pub resolved_disputes: u32,
    pub total_tracked: u64,
    pub total_reversals: u32,
    pub positions_affected: u64,
}

/// Evidence validator
pub struct EvidenceValidator;

impl EvidenceValidator {
    /// Validate evidence chain of custody
    pub fn validate_chain_of_custody(
        evidence: &[DisputeEvidence],
    ) -> Result<(), ProgramError> {
        if evidence.is_empty() {
            return Ok(());
        }

        // Check chronological order
        let mut prev_timestamp = evidence[0].timestamp;
        for e in evidence.iter().skip(1) {
            if e.timestamp < prev_timestamp {
                return Err(BettingPlatformError::InvalidEvidenceOrder.into());
            }
            prev_timestamp = e.timestamp;
        }

        // Check for duplicate IDs
        let mut seen_ids = std::collections::HashSet::new();
        for e in evidence {
            if !seen_ids.insert(&e.id) {
                return Err(BettingPlatformError::DuplicateEvidence.into());
            }
        }

        Ok(())
    }

    /// Calculate evidence strength score
    pub fn calculate_evidence_strength(evidence: &[DisputeEvidence]) -> f64 {
        if evidence.is_empty() {
            return 0.0;
        }

        let mut score = 0.0;
        
        for e in evidence {
            // Type weighting
            let type_weight = match e.evidence_type.as_str() {
                "document" => 1.0,
                "image" => 0.8,
                "url" => 0.6,
                "text" => 0.4,
                _ => 0.2,
            };

            // URL presence bonus
            let url_bonus = if e.url.is_some() { 0.2 } else { 0.0 };

            score += type_weight + url_bonus;
        }

        // Normalize by evidence count
        (score / evidence.len() as f64).min(1.0)
    }
}

/// Money-making calculations for disputes
impl PolymarketDisputeHandler {
    /// Calculate arbitrage opportunity during dispute
    pub fn calculate_dispute_arbitrage(&self, dispute_id: &str) -> f64 {
        let impact = self.calculate_dispute_impact(dispute_id);
        
        // Higher impact = higher potential arbitrage
        // Base 20% opportunity * impact factor
        0.20 * impact
    }

    /// Estimate resolution probability
    pub fn estimate_resolution_probability(&self, dispute_id: &str) -> f64 {
        if let Some(tracked) = self.sync_state.active_disputes.get(dispute_id) {
            // Factors: votes, evidence, time elapsed
            let vote_prob = if tracked.dispute_info.has_votes {
                0.65 // Assume moderate positive vote probability if votes exist
            } else {
                0.5
            };

            // Use evidence count as a proxy for evidence strength
            let evidence_prob = (tracked.dispute_info.evidence_count as f64 / 10.0).min(1.0);

            let time_factor = (tracked.update_count as f64 / 20.0).min(1.0);

            (vote_prob * 0.4 + evidence_prob * 0.4 + time_factor * 0.2)
        } else {
            0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispute_detection() {
        let mut handler = PolymarketDisputeHandler::new();
        
        let dispute = DisputeInfo {
            dispute_id: "dispute-123".to_string(),
            market_id: "market-456".to_string(),
            disputer: "user-789".to_string(),
            proposed_outcome: "No".to_string(),
            original_outcome: "Yes".to_string(),
            dispute_bond: 1000000000, // 1 SOL in lamports
            status: "Active".to_string(),
            created_at: 100,
            deadline: 1000,
            evidence_count: 0,
            has_votes: false,
        };

        let action = handler.process_dispute_update(dispute, 200).unwrap();
        
        match action {
            DisputeAction::FreezePositions { market_id } => {
                assert_eq!(handler.sync_state.active_disputes.len(), 1);
                assert_eq!(handler.sync_state.total_disputes_tracked, 1);
            }
            _ => panic!("Expected FreezePositions action"),
        }
    }

    #[test]
    fn test_dispute_resolution() {
        let mut handler = PolymarketDisputeHandler::new();
        
        // Add dispute
        let mut dispute = DisputeInfo {
            dispute_id: "dispute-123".to_string(),
            market_id: "market-456".to_string(),
            disputer: "user-789".to_string(),
            proposed_outcome: "No".to_string(),
            original_outcome: "Yes".to_string(),
            dispute_bond: 1000000000, // 1 SOL in lamports
            status: "Active".to_string(),
            created_at: 100,
            deadline: 1000,
            evidence_count: 0,
            has_votes: false,
        };

        handler.process_dispute_update(dispute.clone(), 200).unwrap();
        
        // Resolve dispute
        dispute.status = "Resolved".to_string();
        let action = handler.process_dispute_update(dispute, 300).unwrap();
        
        match action {
            DisputeAction::ReversePositions { original_outcome, new_outcome, .. } => {
                assert_eq!(original_outcome, "Yes");
                assert_eq!(new_outcome, "No");
                assert_eq!(handler.sync_state.resolved_disputes.len(), 1);
                assert_eq!(handler.sync_state.total_reversals, 1);
            }
            _ => panic!("Expected ReversePositions action"),
        }
    }

    #[test]
    fn test_evidence_validation() {
        let evidence = vec![
            DisputeEvidence {
                id: "ev1".to_string(),
                submitter: "user1".to_string(),
                evidence_type: "document".to_string(),
                content: "Evidence 1".to_string(),
                url: Some("https://example.com/1".to_string()),
                timestamp: 100,
            },
            DisputeEvidence {
                id: "ev2".to_string(),
                submitter: "user2".to_string(),
                evidence_type: "image".to_string(),
                content: "Evidence 2".to_string(),
                url: None,
                timestamp: 200,
            },
        ];

        assert!(EvidenceValidator::validate_chain_of_custody(&evidence).is_ok());
        
        let strength = EvidenceValidator::calculate_evidence_strength(&evidence);
        assert!(strength > 0.5); // Document + image should score well
    }
}