use anchor_lang::prelude::*;
use std::collections::HashMap;

use crate::sharding::shard_manager::RebalanceProposal;
use crate::sharding::types::*;
use crate::sharding::errors::ShardingError;

pub struct RebalanceVoter {
    pub keeper_stakes: HashMap<Pubkey, u64>,
    pub active_proposals: Vec<RebalanceProposal>,
    pub vote_threshold: f64, // 66.7% majority
}

impl RebalanceVoter {
    pub fn new() -> Self {
        Self {
            keeper_stakes: HashMap::new(),
            active_proposals: Vec::new(),
            vote_threshold: 0.667,
        }
    }

    pub fn submit_proposal(
        &mut self,
        mut proposal: RebalanceProposal,
        current_slot: u64,
    ) -> Result<()> {
        // Validate proposal
        if proposal.markets_to_move.is_empty() {
            return Err(ShardingError::NoMarketsToMove.into());
        }

        if proposal.estimated_improvement < 0.1 {
            return Err(ShardingError::InsufficientImprovement.into());
        }

        // Set voting period
        proposal.voting_ends_slot = current_slot + 100; // ~40 seconds
        proposal.id = self.generate_proposal_id(&proposal);

        self.active_proposals.push(proposal);
        Ok(())
    }

    pub fn vote(
        &mut self,
        keeper: &Pubkey,
        proposal_id: &[u8; 32],
        vote: bool,
    ) -> Result<()> {
        let stake = self.keeper_stakes.get(keeper)
            .ok_or(ShardingError::UnauthorizedKeeper)?;

        let proposal = self.active_proposals.iter_mut()
            .find(|p| p.id == *proposal_id)
            .ok_or(ShardingError::ProposalNotFound)?;

        if vote {
            proposal.votes_for += stake;
        } else {
            proposal.votes_against += stake;
        }

        Ok(())
    }

    pub fn execute_approved_proposals(
        &mut self,
        current_slot: u64,
    ) -> Vec<RebalanceExecution> {
        let mut executions = vec![];

        self.active_proposals.retain(|proposal| {
            if current_slot > proposal.voting_ends_slot {
                let total_votes = proposal.votes_for + proposal.votes_against;
                if total_votes == 0 {
                    return false; // Remove proposal with no votes
                }

                let approval_ratio = proposal.votes_for as f64 / total_votes as f64;

                if approval_ratio >= self.vote_threshold {
                    executions.push(RebalanceExecution {
                        proposal_id: proposal.id,
                        moves: proposal.markets_to_move.clone(),
                        execution_slot: current_slot,
                    });
                    false // Remove from active
                } else {
                    false // Remove rejected proposal
                }
            } else {
                true // Keep active
            }
        });

        executions
    }

    pub fn add_keeper_stake(&mut self, keeper: Pubkey, stake: u64) {
        self.keeper_stakes.insert(keeper, stake);
    }

    pub fn remove_keeper(&mut self, keeper: &Pubkey) {
        self.keeper_stakes.remove(keeper);
    }

    pub fn update_keeper_stake(&mut self, keeper: &Pubkey, new_stake: u64) {
        if let Some(stake) = self.keeper_stakes.get_mut(keeper) {
            *stake = new_stake;
        }
    }

    fn generate_proposal_id(&self, proposal: &RebalanceProposal) -> [u8; 32] {
        use anchor_lang::solana_program::keccak;
        
        let mut data = Vec::new();
        
        // Include key proposal data in the ID generation
        for (shard_id, _) in &proposal.overloaded_shards {
            data.extend_from_slice(&shard_id.to_le_bytes());
        }
        
        for (market, from, to) in &proposal.markets_to_move {
            data.extend_from_slice(&market.to_bytes());
            data.push(*from);
            data.push(*to);
        }
        
        data.extend_from_slice(&proposal.voting_ends_slot.to_le_bytes());
        
        let hash = keccak::hash(&data);
        hash.0
    }

    pub fn get_total_stake(&self) -> u64 {
        self.keeper_stakes.values().sum()
    }

    pub fn is_keeper_authorized(&self, keeper: &Pubkey) -> bool {
        self.keeper_stakes.contains_key(keeper)
    }

    pub fn get_active_proposal_count(&self) -> usize {
        self.active_proposals.len()
    }

    pub fn get_proposal_status(&self, proposal_id: &[u8; 32]) -> Option<(u64, u64, u64)> {
        self.active_proposals.iter()
            .find(|p| p.id == *proposal_id)
            .map(|p| (p.votes_for, p.votes_against, p.voting_ends_slot))
    }
}