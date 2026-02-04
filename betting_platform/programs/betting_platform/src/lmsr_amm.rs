use anchor_lang::prelude::*;
use crate::fixed_math::*;
use crate::errors::ErrorCode;

#[derive(Debug, Clone)]
pub struct LSMRMarket {
    pub b: FixedPoint,  // Liquidity parameter
    pub q: Vec<FixedPoint>,  // Quantity vector for each outcome
    pub alpha: FixedPoint,  // Dynamic liquidity depth
}

impl LSMRMarket {
    pub fn new(b: FixedPoint, num_outcomes: usize) -> Self {
        Self {
            b,
            q: vec![FixedPoint::zero(); num_outcomes],
            alpha: FixedPoint::from_u64(1),
        }
    }

    /// Calculate cost function C(q) = b * log(Σ exp(q_i/b))
    pub fn cost(&self) -> Result<FixedPoint> {
        let mut sum = FixedPoint::zero();

        for q_i in &self.q {
            let exp_term = (*q_i).div(&self.b)?.exp()?;
            sum = sum.add(&exp_term)?;
        }

        self.b.mul(&sum.ln()?)
    }

    /// Calculate price for outcome i: p_i = exp(q_i/b) / Σ exp(q_j/b)
    pub fn price(&self, outcome: usize) -> Result<FixedPoint> {
        require!(outcome < self.q.len(), ErrorCode::InvalidOutcome);

        let mut sum = FixedPoint::zero();
        for q_j in &self.q {
            let exp_term = (*q_j).div(&self.b)?.exp()?;
            sum = sum.add(&exp_term)?;
        }

        let numerator = self.q[outcome].div(&self.b)?.exp()?;
        numerator.div(&sum)
    }

    /// Calculate all prices ensuring they sum to 1
    pub fn all_prices(&self) -> Result<Vec<FixedPoint>> {
        let mut prices = vec![];
        let mut sum = FixedPoint::zero();

        // Calculate all exp terms
        let mut exp_terms = vec![];
        for q_i in &self.q {
            let exp_term = (*q_i).div(&self.b)?.exp()?;
            exp_terms.push(exp_term);
            sum = sum.add(&exp_term)?;
        }

        // Calculate normalized prices
        for exp_term in exp_terms {
            prices.push(exp_term.div(&sum)?);
        }

        // Verify sum to 1
        let total = prices.iter().fold(
            Ok(FixedPoint::zero()),
            |acc: Result<FixedPoint>, p| {
                acc.and_then(|a| a.add(p))
            }
        )?;

        let one = FixedPoint::from_u64(1);
        let epsilon = FixedPoint::from_float(0.000001);
        require!(
            (total.sub(&one)?.abs()?) < epsilon,
            ErrorCode::PriceSumError
        );

        Ok(prices)
    }

    /// Calculate cost of buying shares
    pub fn buy_cost(
        &self,
        outcome: usize,
        shares: FixedPoint,
    ) -> Result<FixedPoint> {
        require!(outcome < self.q.len(), ErrorCode::InvalidOutcome);
        require!(shares > FixedPoint::zero(), ErrorCode::InvalidShares);

        let cost_before = self.cost()?;

        // Create temporary state with updated quantities
        let mut new_q = self.q.clone();
        new_q[outcome] = new_q[outcome].add(&shares)?;

        let temp_market = LSMRMarket {
            b: self.b,
            q: new_q,
            alpha: self.alpha,
        };

        let cost_after = temp_market.cost()?;
        cost_after.sub(&cost_before)
    }
}

#[account]
pub struct LSMRStatePDA {
    pub market_id: u128,
    pub b_parameter: u64,  // Fixed point representation
    pub quantities: Vec<u64>,  // Fixed point quantities
    pub alpha: u64,  // Dynamic liquidity
    pub total_volume: u64,
    pub last_update_slot: u64,
}

#[derive(Accounts)]
pub struct LSMRTrade<'info> {
    #[account(mut)]
    pub lmsr_state: Account<'info, LSMRStatePDA>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[event]
pub struct LSMRPriceUpdateEvent {
    pub market_id: u128,
    pub prices: Vec<u64>,
    pub volume: u64,
    pub outcome: u8,
    pub is_buy: bool,
}

pub fn execute_lmsr_trade(
    ctx: Context<LSMRTrade>,
    outcome: u8,
    amount: u64,
    is_buy: bool,
) -> Result<()> {
    let market_state = &mut ctx.accounts.lmsr_state;

    // Convert to fixed point
    let b = FixedPoint::from_raw(market_state.b_parameter);
    let shares = FixedPoint::from_raw(amount);

    // Build market model
    let mut market = LSMRMarket {
        b,
        q: market_state.quantities.iter()
            .map(|&q| FixedPoint::from_raw(q))
            .collect(),
        alpha: FixedPoint::from_raw(market_state.alpha),
    };

    // Calculate cost
    let cost = if is_buy {
        market.buy_cost(outcome as usize, shares)?
    } else {
        // For selling, reverse the operation
        market.buy_cost(outcome as usize, shares.neg()?)?
    };

    // Update state
    if is_buy {
        market.q[outcome as usize] = market.q[outcome as usize].add(&shares)?;
    } else {
        market.q[outcome as usize] = market.q[outcome as usize].sub(&shares)?;
    }

    // Save updated quantities
    market_state.quantities = market.q.iter()
        .map(|q| q.to_raw())
        .collect();

    // Update volume
    market_state.total_volume = market_state.total_volume
        .checked_add(cost.to_u64_truncate())
        .ok_or(ErrorCode::MathOverflow)?;

    market_state.last_update_slot = Clock::get()?.slot;

    // Emit pricing event
    let prices = market.all_prices()?;
    emit!(LSMRPriceUpdateEvent {
        market_id: market_state.market_id,
        prices: prices.iter().map(|p| p.to_raw()).collect(),
        volume: cost.to_u64_truncate(),
        outcome,
        is_buy,
    });

    Ok(())
}

// Initialize LMSR market
pub fn initialize_lmsr_market(
    ctx: Context<InitializeLSMR>,
    market_id: u128,
    b_parameter: u64,
    num_outcomes: u8,
) -> Result<()> {
    require!(b_parameter >= 100_000_000_000_000_000_000, ErrorCode::InvalidInput); // >= 100 USDC in fixed point
    require!(num_outcomes >= 2 && num_outcomes <= 64, ErrorCode::InvalidInput);
    
    let market_state = &mut ctx.accounts.lmsr_state;
    
    market_state.market_id = market_id;
    market_state.b_parameter = b_parameter;
    market_state.quantities = vec![0; num_outcomes as usize];
    market_state.alpha = 1_000_000_000_000_000_000; // 1.0 in fixed point
    market_state.total_volume = 0;
    market_state.last_update_slot = Clock::get()?.slot;
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(market_id: u128, num_outcomes: u8)]
pub struct InitializeLSMR<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 16 + 8 + (8 * num_outcomes as usize) + 8 + 8 + 8,
        seeds = [b"lmsr", market_id.to_le_bytes().as_ref()],
        bump
    )]
    pub lmsr_state: Account<'info, LSMRStatePDA>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}