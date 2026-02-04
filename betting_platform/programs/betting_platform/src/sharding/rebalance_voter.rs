use anchor_lang::prelude::*;
use std::collections::HashMap;

use crate::sharding::{
    RebalanceProposal, RebalanceExecution, RebalanceError,
    VOTE_THRESHOLD, VOTING_PERIOD_SLOTS
};

pub struct RebalanceVoter {
    pub keeper_stakes: HashMap<Pubkey, u64>,
    pub active_proposals: Vec<RebalanceProposal>,
    pub vote_threshold: f64,
    pub voted_keepers: HashMap<[u8; 32], Vec<Pubkey>>, // Track who voted on each proposal
}

impl RebalanceVoter {
    pub fn new() -> Self {
        Self {
            keeper_stakes: HashMap::new(),
            active_proposals: Vec::new(),
            vote_threshold: VOTE_THRESHOLD,
            voted_keepers: HashMap::new(),
        }
    }

    /// Initialize keeper with stake
    pub fn register_keeper(&mut self, keeper: Pubkey, stake: u64) {
        self.keeper_stakes.insert(keeper, stake);
    }

    /// Submit a new rebalance proposal
    pub fn submit_proposal(
        &mut self,
        proposal: RebalanceProposal,
        current_slot: u64,
    ) -> Result<()> {
        // Validate proposal
        if proposal.markets_to_move.is_empty() {
            return Err(RebalanceError::NoMarketsToMove.into());
        }

        if proposal.estimated_improvement < 0.1 {
            return Err(RebalanceError::InsufficientImprovement.into());
        }

        // Check for duplicate proposals
        for existing in &self.active_proposals {
            if self.proposals_are_similar(&proposal, existing) {
                msg!("Similar proposal already exists");
                return Ok(());
            }
        }

        // Set voting period
        let mut proposal = proposal;
        proposal.voting_ends_slot = current_slot + VOTING_PERIOD_SLOTS;
        proposal.id = self.generate_proposal_id(&proposal);

        self.active_proposals.push(proposal);
        Ok(())
    }

    /// Vote on a proposal
    pub fn vote(
        &mut self,
        keeper: &Pubkey,
        proposal_id: &[u8; 32],
        vote: bool,
    ) -> Result<()> {
        // Check keeper authorization
        let stake = self.keeper_stakes.get(keeper)
            .ok_or(RebalanceError::UnauthorizedKeeper)?;

        // Check if keeper already voted
        if let Some(voters) = self.voted_keepers.get(proposal_id) {
            if voters.contains(keeper) {
                msg!("Keeper already voted on this proposal");
                return Ok(());
            }
        }

        // Find proposal
        let proposal = self.active_proposals.iter_mut()
            .find(|p| p.id == *proposal_id)
            .ok_or(RebalanceError::ProposalNotFound)?;

        // Record vote
        if vote {
            proposal.votes_for += stake;
        } else {
            proposal.votes_against += stake;
        }

        // Track that keeper voted
        self.voted_keepers.entry(*proposal_id)
            .or_insert_with(Vec::new)
            .push(*keeper);

        Ok(())
    }

    /// Execute approved proposals
    pub fn execute_approved_proposals(
        &mut self,
        current_slot: u64,
    ) -> Vec<RebalanceExecution> {
        let mut executions = vec![];
        let total_stake: u64 = self.keeper_stakes.values().sum();

        self.active_proposals.retain(|proposal| {
            if current_slot > proposal.voting_ends_slot {
                let total_votes = proposal.votes_for + proposal.votes_against;
                
                // Need minimum participation (33% of total stake)
                if total_votes < total_stake / 3 {
                    msg!("Insufficient participation for proposal {:?}", proposal.id);
                    return false; // Remove proposal
                }

                let approval_ratio = proposal.votes_for as f64 / total_votes as f64;

                if approval_ratio >= self.vote_threshold {
                    executions.push(RebalanceExecution {
                        proposal_id: proposal.id,
                        moves: proposal.markets_to_move.clone(),
                        execution_slot: current_slot,
                    });
                    
                    // Clean up voted keepers tracking
                    self.voted_keepers.remove(&proposal.id);
                    
                    false // Remove from active
                } else {
                    msg!("Proposal rejected with approval ratio: {}", approval_ratio);
                    self.voted_keepers.remove(&proposal.id);
                    false // Remove rejected proposal
                }
            } else {
                true // Keep active
            }
        });

        executions
    }

    /// Check if two proposals are similar
    fn proposals_are_similar(&self, a: &RebalanceProposal, b: &RebalanceProposal) -> bool {
        // Check if they move the same markets
        let a_markets: Vec<_> = a.markets_to_move.iter().map(|(m, _, _)| m).collect();
        let b_markets: Vec<_> = b.markets_to_move.iter().map(|(m, _, _)| m).collect();

        let overlap = a_markets.iter().filter(|m| b_markets.contains(m)).count();
        
        // If more than 50% overlap, consider similar
        overlap > a_markets.len() / 2 || overlap > b_markets.len() / 2
    }

    /// Generate unique proposal ID
    fn generate_proposal_id(&self, proposal: &RebalanceProposal) -> [u8; 32] {
        use anchor_lang::solana_program::keccak;
        
        let mut data = Vec::new();
        
        // Include key proposal data in ID
        data.extend_from_slice(&proposal.overloaded_shards.len().to_le_bytes());
        data.extend_from_slice(&proposal.markets_to_move.len().to_le_bytes());
        
        // Add first market to move for uniqueness
        if let Some((market, _, _)) = proposal.markets_to_move.first() {
            data.extend_from_slice(&market.to_bytes());
        }
        
        keccak::hash(&data).0
    }

    /// Get current proposal status
    pub fn get_proposal_status(&self, proposal_id: &[u8; 32]) -> Option<ProposalStatus> {
        self.active_proposals.iter()
            .find(|p| p.id == *proposal_id)
            .map(|p| {
                let total_votes = p.votes_for + p.votes_against;
                let approval_ratio = if total_votes > 0 {
                    p.votes_for as f64 / total_votes as f64
                } else {
                    0.0
                };

                ProposalStatus {
                    votes_for: p.votes_for,
                    votes_against: p.votes_against,
                    approval_ratio,
                    voting_ends_slot: p.voting_ends_slot,
                }
            })
    }

    /// Emergency cancel proposal (requires super majority)
    pub fn emergency_cancel_proposal(
        &mut self,
        proposal_id: &[u8; 32],
        keeper: &Pubkey,
    ) -> Result<()> {
        // Require 90% stake to emergency cancel
        let keeper_stake = self.keeper_stakes.get(keeper)
            .ok_or(RebalanceError::UnauthorizedKeeper)?;
        
        let total_stake: u64 = self.keeper_stakes.values().sum();
        
        if (*keeper_stake as f64) < (total_stake as f64 * 0.9) {
            return Err(RebalanceError::InsufficientVoteCount.into());
        }

        // Remove proposal
        self.active_proposals.retain(|p| p.id != *proposal_id);
        self.voted_keepers.remove(proposal_id);
        
        msg!("Emergency cancelled proposal {:?}", proposal_id);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ProposalStatus {
    pub votes_for: u64,
    pub votes_against: u64,
    pub approval_ratio: f64,
    pub voting_ends_slot: u64,
}