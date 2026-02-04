use std::collections::HashMap;
use anchor_lang::prelude::*;
use fixed::types::{U64F64, I64F64};

use crate::amm::pm_amm::{PMAMMState, NewtonRaphsonSolver, MultiOutcomePricing};
use super::core::{QuantumMarket, QuantumState, RefundEntry};
use super::credits::{QuantumCredits, ProposalOutcome, CreditError};

#[derive(Debug, Clone)]
pub enum TradingError {
    MarketNotActive,
    ProposalLocked,
    NoCredits,
    NotCollapsed,
    NoWinner,
    InvalidProposal,
    SolverError,
}

impl From<TradingError> for ProgramError {
    fn from(e: TradingError) -> Self {
        match e {
            TradingError::MarketNotActive => ProgramError::Custom(400),
            TradingError::ProposalLocked => ProgramError::Custom(401),
            TradingError::NoCredits => ProgramError::Custom(402),
            TradingError::NotCollapsed => ProgramError::Custom(403),
            TradingError::NoWinner => ProgramError::Custom(404),
            TradingError::InvalidProposal => ProgramError::Custom(405),
            TradingError::SolverError => ProgramError::Custom(406),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TradeDirection {
    Buy,
    Sell,
}

impl TradeDirection {
    pub fn sign(&self) -> I64F64 {
        match self {
            TradeDirection::Buy => I64F64::from_num(1),
            TradeDirection::Sell => I64F64::from_num(-1),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuantumTradeResult {
    pub proposal_id: u8,
    pub old_probability: U64F64,
    pub new_probability: U64F64,
    pub effective_size: u64,
    pub leverage_applied: u64,
    pub slippage: U64F64,
    pub lvr_cost: U64F64,
    pub credits_remaining: u64,
}

#[derive(Debug, Clone)]
pub struct RefundSummary {
    pub total_refunded: u64,
    pub refund_count: u32,
    pub winner_proposal: u8,
    pub settlement_slot: u64,
}

#[derive(Clone, Debug)]
pub struct ProposalLock {
    pub proposal_id: u8,
    pub locked_until_slot: u64,
    pub reason: LockReason,
}

#[derive(Clone, Debug)]
pub enum LockReason {
    HighVolatility,
    PreCollapse,
    Maintenance,
}

pub struct QuantumTrading {
    pub market: QuantumMarket,
    pub pm_amm: PMAMMState,
    pub credit_ledger: HashMap<Pubkey, QuantumCredits>,
    pub proposal_locks: Vec<ProposalLock>,
}

impl QuantumTrading {
    pub fn place_quantum_trade(
        &mut self,
        user: &Pubkey,
        proposal_id: u8,
        amount: u64,
        leverage: u64,
        direction: TradeDirection,
    ) -> std::result::Result<QuantumTradeResult, TradingError> {
        // Check market state
        if self.market.state != QuantumState::Active {
            return Err(TradingError::MarketNotActive);
        }

        // Check proposal lock
        if self.is_proposal_locked(proposal_id)? {
            return Err(TradingError::ProposalLocked);
        }

        // Get user credits
        let credits = self.credit_ledger
            .get_mut(user)
            .ok_or(TradingError::NoCredits)?;

        // Use credits for this trade
        credits.use_credits(proposal_id, amount, leverage)
            .map_err(|_| TradingError::NoCredits)?;

        // Calculate effective trade size with leverage
        let effective_size = amount.saturating_mul(leverage);

        // Execute trade through PM-AMM
        let solver = NewtonRaphsonSolver::new();
        let order_size = I64F64::from_num(effective_size as i64) * direction.sign();
        
        let price_result = solver.solve_pm_amm_price(
            &self.pm_amm,
            proposal_id,
            order_size,
        ).map_err(|_| TradingError::SolverError)?;

        // Update proposal statistics
        let proposal = self.market.proposals
            .get_mut(proposal_id as usize)
            .ok_or(TradingError::InvalidProposal)?;
            
        proposal.current_probability = price_result.new_price;
        proposal.total_volume = proposal.total_volume.saturating_add(effective_size);
        proposal.unique_traders = proposal.unique_traders.saturating_add(1); // Simplified - should check uniqueness
        proposal.last_trade_slot = Clock::get()
            .map(|c| c.slot)
            .unwrap_or(0);

        // Update all proposal prices to maintain sum = 1
        let pricing = MultiOutcomePricing::new();
        pricing.update_all_prices(
            &mut self.pm_amm,
            proposal_id,
            price_result.new_price,
            &solver,
        ).map_err(|_| TradingError::SolverError)?;

        let credits_remaining = credits.credits_per_proposal
            .saturating_sub(credits.used_credits[proposal_id as usize].amount_used);

        Ok(QuantumTradeResult {
            proposal_id,
            old_probability: price_result.old_price,
            new_probability: price_result.new_price,
            effective_size,
            leverage_applied: leverage,
            slippage: price_result.slippage,
            lvr_cost: price_result.lvr_cost,
            credits_remaining,
        })
    }

    pub fn process_collapse_refunds(&mut self) -> std::result::Result<RefundSummary, TradingError> {
        if self.market.state != QuantumState::Collapsed {
            return Err(TradingError::NotCollapsed);
        }

        let winner = self.market.winner_index
            .ok_or(TradingError::NoWinner)?;

        let mut total_refunded = 0u64;
        let mut refund_count = 0u32;

        // Get proposal outcomes once before the loop
        let proposal_outcomes = self.get_proposal_outcomes()?;
        
        // Process refunds for all users
        for (user, credits) in self.credit_ledger.iter_mut() {
            if !credits.refund_claimed {
                credits.calculate_refunds(winner, &proposal_outcomes)
                    .map_err(|_| TradingError::SolverError)?;

                if credits.refund_amount > 0 {
                    // Queue refund for processing
                    self.market.refund_queue.push(RefundEntry {
                        user: *user,
                        amount: credits.refund_amount,
                        processed: false,
                    });

                    total_refunded = total_refunded.saturating_add(credits.refund_amount);
                    refund_count = refund_count.saturating_add(1);
                }

                credits.refund_claimed = true;
            }
        }

        self.market.state = QuantumState::Settled;

        Ok(RefundSummary {
            total_refunded,
            refund_count,
            winner_proposal: winner,
            settlement_slot: Clock::get()
                .map(|c| c.slot)
                .unwrap_or(0),
        })
    }

    fn is_proposal_locked(&self, proposal_id: u8) -> std::result::Result<bool, TradingError> {
        let current_slot = Clock::get()
            .map(|c| c.slot)
            .unwrap_or(0);

        Ok(self.proposal_locks.iter().any(|lock| {
            lock.proposal_id == proposal_id &&
            lock.locked_until_slot > current_slot
        }))
    }

    fn get_proposal_outcomes(&self) -> std::result::Result<Vec<ProposalOutcome>, TradingError> {
        // In a real implementation, this would fetch actual market outcomes
        // For now, returning placeholder data based on current probabilities
        Ok(self.market.proposals
            .iter()
            .map(|proposal| ProposalOutcome {
                final_price: (proposal.current_probability.to_num::<f64>() * 1000.0) as u64,
                avg_entry_price: 500, // Placeholder - would track actual entry prices
            })
            .collect())
    }
}