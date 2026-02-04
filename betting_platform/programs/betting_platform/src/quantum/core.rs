use anchor_lang::prelude::*;
use fixed::types::U64F64;

pub const MAX_QUANTUM_PROPOSALS: u8 = 10;
pub const COLLAPSE_BUFFER_SLOTS: u64 = 100; // Grace period before collapse

#[derive(Clone, Debug)]
pub struct RefundEntry {
    pub user: Pubkey,
    pub amount: u64,
    pub processed: bool,
}

#[derive(Clone, Debug)]
pub struct QuantumMarket {
    pub market_id: [u8; 32],
    pub proposals: Vec<QuantumProposal>,
    pub total_deposits: u64,
    pub settle_slot: u64,
    pub collapse_rule: CollapseRule,
    pub state: QuantumState,
    pub winner_index: Option<u8>,
    pub refund_queue: Vec<RefundEntry>,
}

#[derive(Clone, Debug)]
pub struct QuantumProposal {
    pub proposal_id: u8,
    pub description: [u8; 64],
    pub current_probability: U64F64,
    pub total_volume: u64,
    pub unique_traders: u32,
    pub last_trade_slot: u64,
}

#[derive(Clone, Debug)]
pub enum CollapseRule {
    MaxProbability,     // Highest probability wins
    MaxVolume,         // Most traded volume wins
    MaxTraders,        // Most unique traders wins
    WeightedComposite, // Weighted combination
}

#[derive(Clone, Debug, PartialEq)]
pub enum QuantumState {
    Active,
    PreCollapse,    // Buffer period before collapse
    Collapsing,     // During collapse execution
    Collapsed,      // Post-collapse, refunds pending
    Settled,        // All refunds processed
}

impl QuantumMarket {
    pub fn new(
        market_id: [u8; 32],
        proposals: Vec<String>,
        settle_slot: u64,
        collapse_rule: CollapseRule,
    ) -> std::result::Result<Self, ProgramError> {
        if proposals.len() < 2 || proposals.len() > MAX_QUANTUM_PROPOSALS as usize {
            return Err(ProgramError::InvalidArgument);
        }

        let proposal_count = proposals.len();
        let quantum_proposals: Vec<QuantumProposal> = proposals
            .into_iter()
            .enumerate()
            .map(|(i, desc)| {
                let mut desc_bytes = [0u8; 64];
                let bytes = desc.as_bytes();
                desc_bytes[..bytes.len().min(64)].copy_from_slice(&bytes[..bytes.len().min(64)]);

                QuantumProposal {
                    proposal_id: i as u8,
                    description: desc_bytes,
                    current_probability: U64F64::from_num(1.0) / U64F64::from_num(proposal_count),
                    total_volume: 0,
                    unique_traders: 0,
                    last_trade_slot: 0,
                }
            })
            .collect();

        Ok(Self {
            market_id,
            proposals: quantum_proposals,
            total_deposits: 0,
            settle_slot,
            collapse_rule,
            state: QuantumState::Active,
            winner_index: None,
            refund_queue: Vec::new(),
        })
    }

    pub fn check_collapse_trigger(&mut self, current_slot: u64) -> std::result::Result<bool, ProgramError> {
        match self.state {
            QuantumState::Active => {
                if current_slot >= self.settle_slot.saturating_sub(COLLAPSE_BUFFER_SLOTS) {
                    self.state = QuantumState::PreCollapse;
                    msg!("Quantum market entering pre-collapse state");
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            QuantumState::PreCollapse => {
                if current_slot >= self.settle_slot {
                    self.state = QuantumState::Collapsing;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    pub fn execute_collapse(&mut self) -> std::result::Result<(), ProgramError> {
        if self.state != QuantumState::Collapsing {
            return Err(ProgramError::InvalidAccountData);
        }

        // Determine winner based on collapse rule
        let winner_index = match self.collapse_rule {
            CollapseRule::MaxProbability => {
                self.proposals
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, p)| p.current_probability.to_bits())
                    .map(|(i, _)| i as u8)
                    .ok_or(ProgramError::InvalidAccountData)?
            }
            CollapseRule::MaxVolume => {
                self.proposals
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, p)| p.total_volume)
                    .map(|(i, _)| i as u8)
                    .ok_or(ProgramError::InvalidAccountData)?
            }
            CollapseRule::MaxTraders => {
                self.proposals
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, p)| p.unique_traders)
                    .map(|(i, _)| i as u8)
                    .ok_or(ProgramError::InvalidAccountData)?
            }
            CollapseRule::WeightedComposite => {
                self.calculate_weighted_winner()?
            }
        };

        self.winner_index = Some(winner_index);
        self.state = QuantumState::Collapsed;

        msg!("Quantum market collapsed. Winner: Proposal {}", winner_index);

        Ok(())
    }

    fn calculate_weighted_winner(&self) -> std::result::Result<u8, ProgramError> {
        // Weight: 50% probability, 30% volume, 20% traders
        let mut max_score = U64F64::from_num(0);
        let mut winner = 0u8;

        for (i, proposal) in self.proposals.iter().enumerate() {
            let prob_score = proposal.current_probability * U64F64::from_num(0.5);
            let vol_score = U64F64::from_num(proposal.total_volume)
                / U64F64::from_num(self.total_deposits.max(1))
                * U64F64::from_num(0.3);
            let trader_score = U64F64::from_num(proposal.unique_traders)
                / U64F64::from_num(1000) // Normalize to 1000 traders
                * U64F64::from_num(0.2);

            let total_score = prob_score + vol_score + trader_score;

            if total_score > max_score {
                max_score = total_score;
                winner = i as u8;
            }
        }

        Ok(winner)
    }
}