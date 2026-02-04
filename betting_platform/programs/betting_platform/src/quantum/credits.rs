use anchor_lang::prelude::*;
use fixed::types::U64F64;

pub const FIXED_POINT_SCALE: u64 = 1_000_000_000;

#[derive(Debug, Clone)]
pub enum CreditError {
    InvalidProposal,
    InsufficientCredits,
    MathOverflow,
    InvalidOutcome,
}

impl From<CreditError> for ProgramError {
    fn from(e: CreditError) -> Self {
        match e {
            CreditError::InvalidProposal => ProgramError::Custom(300),
            CreditError::InsufficientCredits => ProgramError::Custom(301),
            CreditError::MathOverflow => ProgramError::Custom(302),
            CreditError::InvalidOutcome => ProgramError::Custom(303),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProposalOutcome {
    pub final_price: u64,
    pub avg_entry_price: u64,
}

#[derive(Clone, Debug)]
pub struct QuantumCredits {
    pub user: Pubkey,
    pub market_id: [u8; 32],
    pub initial_deposit: u64,
    pub credits_per_proposal: u64,
    pub used_credits: Vec<UsedCredit>,
    pub refund_amount: u64,
    pub refund_claimed: bool,
}

#[derive(Clone, Debug)]
pub struct UsedCredit {
    pub proposal_id: u8,
    pub amount_used: u64,
    pub leverage_applied: u64,
    pub pnl: i64,
    pub position_closed: bool,
}

impl QuantumCredits {
    pub fn deposit_and_allocate(
        user: Pubkey,
        market_id: [u8; 32],
        deposit_amount: u64,
        proposal_count: u8,
    ) -> std::result::Result<Self, ProgramError> {
        if deposit_amount == 0 || proposal_count == 0 {
            return Err(ProgramError::InvalidArgument);
        }

        // Each proposal gets full deposit amount as credits
        let credits_per_proposal = deposit_amount;

        Ok(Self {
            user,
            market_id,
            initial_deposit: deposit_amount,
            credits_per_proposal,
            used_credits: vec![UsedCredit {
                proposal_id: 0,
                amount_used: 0,
                leverage_applied: 0,
                pnl: 0,
                position_closed: false,
            }; proposal_count as usize],
            refund_amount: 0,
            refund_claimed: false,
        })
    }

    pub fn use_credits(
        &mut self,
        proposal_id: u8,
        amount: u64,
        leverage: u64,
    ) -> std::result::Result<(), CreditError> {
        let credit = self.used_credits
            .get_mut(proposal_id as usize)
            .ok_or(CreditError::InvalidProposal)?;

        let available = self.credits_per_proposal.saturating_sub(credit.amount_used);
        if amount > available {
            return Err(CreditError::InsufficientCredits);
        }

        credit.amount_used = credit.amount_used.saturating_add(amount);
        credit.leverage_applied = leverage;

        Ok(())
    }

    pub fn calculate_refunds(
        &mut self,
        winner_proposal: u8,
        proposal_outcomes: &[ProposalOutcome],
    ) -> std::result::Result<(), CreditError> {
        let mut total_refund = 0u64;

        // Calculate PnLs first
        let mut pnls = Vec::new();
        for (i, credit) in self.used_credits.iter().enumerate() {
            if i as u8 != winner_proposal {
                // Refund unused credits from losing proposals
                let unused = self.credits_per_proposal.saturating_sub(credit.amount_used);
                total_refund = total_refund.saturating_add(unused);

                // Close any open positions at 0 value
                if !credit.position_closed {
                    pnls.push(-(credit.amount_used as i64));
                } else {
                    pnls.push(credit.pnl);
                }
            } else {
                // Calculate PnL for winning proposal
                let outcome = proposal_outcomes.get(i)
                    .ok_or(CreditError::InvalidOutcome)?;
                pnls.push(self.calculate_position_pnl(credit, outcome)?);
            }
        }
        
        // Now update the credits
        for (i, credit) in self.used_credits.iter_mut().enumerate() {
            credit.pnl = pnls[i];
            if i as u8 != winner_proposal && !credit.position_closed {
                credit.position_closed = true;
            }
        }

        self.refund_amount = total_refund;
        Ok(())
    }

    fn calculate_position_pnl(
        &self,
        credit: &UsedCredit,
        outcome: &ProposalOutcome,
    ) -> std::result::Result<i64, CreditError> {
        let price_diff = outcome.final_price.saturating_sub(outcome.avg_entry_price) as i64;
        let base_pnl = price_diff
            .saturating_mul(credit.amount_used as i64)
            .saturating_div(FIXED_POINT_SCALE as i64);

        let leveraged_pnl = base_pnl.saturating_mul(credit.leverage_applied as i64);

        Ok(leveraged_pnl)
    }
}