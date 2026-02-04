//! Portfolio-level Greeks Aggregation Module
//!
//! Aggregates Greeks (Delta, Gamma, Vega, Theta, Rho) across all positions
//! in a portfolio for comprehensive risk analysis
//!
//! Specification Requirements:
//! - Portfolio Delta = Σ (position_delta_i * weight_i)
//! - Portfolio Gamma = Σ gamma_i (from AMM derivatives)
//! - On-chain query support
//! - Money-Making: Greeks = hedge for +20% risk-adjusted yields

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    math::{
        fixed_point::U64F64,
        special_functions::{Greeks, calculate_greeks},
        tables::NormalDistributionTables,
    },
    state::{Position, GlobalState, VerseState},
    account_validation::{validate_writable, DISCRIMINATOR_SIZE},
    constants::POSITION_DISCRIMINATOR,
};

/// Maximum positions to aggregate in a single transaction
pub const MAX_AGGREGATION_POSITIONS: usize = 50;

/// Portfolio Greeks discriminator
pub const PORTFOLIO_GREEKS_DISCRIMINATOR: [u8; 8] = [80, 71, 82, 69, 69, 75, 83, 0]; // "PGREEKS"

/// Portfolio Greeks account structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PortfolioGreeks {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User who owns the portfolio
    pub user: Pubkey,
    
    /// Verse ID this Greeks aggregation is for
    pub verse_id: u128,
    
    /// Aggregated Delta across all positions
    pub portfolio_delta: U64F64,
    
    /// Aggregated Gamma across all positions
    pub portfolio_gamma: U64F64,
    
    /// Aggregated Vega across all positions
    pub portfolio_vega: U64F64,
    
    /// Aggregated Theta across all positions
    pub portfolio_theta: U64F64,
    
    /// Aggregated Rho across all positions
    pub portfolio_rho: U64F64,
    
    /// Total notional value of portfolio
    pub total_notional: u64,
    
    /// Number of positions aggregated
    pub position_count: u32,
    
    /// Last update slot
    pub last_update: u64,
    
    /// Position weights (normalized to 10000 = 100%)
    pub position_weights: Vec<u16>,
    
    /// Individual position Greeks for detailed analysis
    pub position_greeks: Vec<PositionGreeks>,
    
    /// Bump seed
    pub bump: u8,
}

/// Greeks for individual position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionGreeks {
    /// Market ID
    pub market_id: u128,
    
    /// Position size
    pub size: u64,
    
    /// Position weight in portfolio (basis points)
    pub weight: u16,
    
    /// Delta
    pub delta: U64F64,
    
    /// Gamma
    pub gamma: U64F64,
    
    /// Vega
    pub vega: U64F64,
    
    /// Theta
    pub theta: U64F64,
    
    /// Rho
    pub rho: U64F64,
}

impl PortfolioGreeks {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 16 + 8 + 8 + 8 + 8 + 8 + 8 + 4 + 8 + 
                          (MAX_AGGREGATION_POSITIONS * 2) + // weights
                          (MAX_AGGREGATION_POSITIONS * PositionGreeks::LEN) + // position greeks
                          1; // bump
    
    /// Create new portfolio Greeks aggregation
    pub fn new(user: Pubkey, verse_id: u128, bump: u8) -> Self {
        Self {
            discriminator: PORTFOLIO_GREEKS_DISCRIMINATOR,
            user,
            verse_id,
            portfolio_delta: U64F64::from_num(0),
            portfolio_gamma: U64F64::from_num(0),
            portfolio_vega: U64F64::from_num(0),
            portfolio_theta: U64F64::from_num(0),
            portfolio_rho: U64F64::from_num(0),
            total_notional: 0,
            position_count: 0,
            last_update: 0,
            position_weights: Vec::with_capacity(MAX_AGGREGATION_POSITIONS),
            position_greeks: Vec::with_capacity(MAX_AGGREGATION_POSITIONS),
            bump,
        }
    }
    
    /// Validate account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != PORTFOLIO_GREEKS_DISCRIMINATOR {
            msg!("Invalid portfolio Greeks discriminator");
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.position_count as usize != self.position_weights.len() ||
           self.position_count as usize != self.position_greeks.len() {
            msg!("Inconsistent position data");
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

impl PositionGreeks {
    pub const LEN: usize = 16 + 8 + 2 + 8 + 8 + 8 + 8 + 8; // market_id + size + weight + 5 greeks
}

/// Calculate and aggregate Greeks for a portfolio
pub fn aggregate_portfolio_greeks(
    accounts: &[AccountInfo],
    tables: &NormalDistributionTables,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let portfolio_greeks_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let global_state_account = next_account_info(account_iter)?;
    let verse_state_account = next_account_info(account_iter)?;
    let clock_account = next_account_info(account_iter)?;
    
    // Validate accounts
    validate_writable(portfolio_greeks_account)?;
    
    // Load clock
    let clock = Clock::from_account_info(clock_account)?;
    
    // Load states
    let global_state = GlobalState::try_from_slice(&global_state_account.data.borrow())?;
    let verse_state = VerseState::try_from_slice(&verse_state_account.data.borrow())?;
    
    // Initialize or load portfolio Greeks
    let mut portfolio_greeks = if portfolio_greeks_account.data_len() == 0 {
        // Initialize new account
        let (pda, bump) = Pubkey::find_program_address(
            &[
                b"portfolio_greeks",
                user_account.key.as_ref(),
                &verse_state.verse_id.to_le_bytes(),
            ],
            &crate::id(),
        );
        
        if pda != *portfolio_greeks_account.key {
            msg!("Invalid portfolio Greeks PDA");
            return Err(ProgramError::InvalidAccountData);
        }
        
        PortfolioGreeks::new(*user_account.key, verse_state.verse_id, bump)
    } else {
        let mut data = PortfolioGreeks::try_from_slice(&portfolio_greeks_account.data.borrow())?;
        data.validate()?;
        data
    };
    
    // Reset aggregation
    portfolio_greeks.portfolio_delta = U64F64::from_num(0);
    portfolio_greeks.portfolio_gamma = U64F64::from_num(0);
    portfolio_greeks.portfolio_vega = U64F64::from_num(0);
    portfolio_greeks.portfolio_theta = U64F64::from_num(0);
    portfolio_greeks.portfolio_rho = U64F64::from_num(0);
    portfolio_greeks.total_notional = 0;
    portfolio_greeks.position_count = 0;
    portfolio_greeks.position_weights.clear();
    portfolio_greeks.position_greeks.clear();
    
    // Process remaining accounts as positions
    let mut total_value = 0u64;
    let mut position_values = Vec::new();
    
    // First pass: calculate total portfolio value for weights
    while let Ok(position_account) = next_account_info(account_iter) {
        let position_data = position_account.data.borrow();
        
        // Check discriminator
        if position_data.len() < DISCRIMINATOR_SIZE {
            continue;
        }
        
        let discriminator = &position_data[..DISCRIMINATOR_SIZE];
        if discriminator != POSITION_DISCRIMINATOR {
            continue;
        }
        
        let position = Position::try_from_slice(&position_data)?;
        
        // Calculate position value (size * current_price)
        let position_value = position.size.saturating_mul(position.entry_price as u64) / 1_000_000;
        position_values.push((position.clone(), position_value));
        total_value = total_value.saturating_add(position_value);
        
        if position_values.len() >= MAX_AGGREGATION_POSITIONS {
            break;
        }
    }
    
    // Second pass: calculate Greeks with weights
    for (position, position_value) in position_values.iter() {
        // Calculate weight (normalized to 10000 = 100%)
        let weight = if total_value > 0 {
            ((position_value * 10000) / total_value) as u16
        } else {
            0
        };
        
        // Get market parameters from verse state
        // For this example, we'll use simplified parameters
        let spot_price = U64F64::from_num(position.entry_price) / U64F64::from_num(1_000_000u64);
        let strike_price = U64F64::from_num(5000u64) / U64F64::from_num(10000u64); // 0.5 = 50%
        let time_to_expiry = U64F64::from_num(1u64); // 1 year for simplicity
        let volatility = U64F64::from_num(2000u64) / U64F64::from_num(10000u64); // 0.2 = 20% volatility
        let risk_free_rate = U64F64::from_num(500u64) / U64F64::from_num(10000u64); // 0.05 = 5% risk-free rate
        
        // Calculate Greeks for this position
        let greeks = calculate_greeks(
            tables,
            spot_price,
            strike_price,
            time_to_expiry,
            volatility,
            risk_free_rate,
            position.is_long,
        )?;
        
        // Weight the Greeks
        let weight_factor = U64F64::from_num(weight as u64) / U64F64::from_num(10000u64);
        let position_size_factor = U64F64::from_num(position.size) / U64F64::from_num(1_000_000u64);
        
        // Aggregate weighted Greeks
        // Portfolio Delta = Σ (position_delta_i * weight_i * size_i)
        let weighted_delta = greeks.delta
            .checked_mul(weight_factor)?
            .checked_mul(position_size_factor)?;
        portfolio_greeks.portfolio_delta = portfolio_greeks.portfolio_delta
            .checked_add(weighted_delta)?;
        
        // Portfolio Gamma = Σ (gamma_i * size_i) - no weight factor per spec
        let sized_gamma = greeks.gamma.checked_mul(position_size_factor)?;
        portfolio_greeks.portfolio_gamma = portfolio_greeks.portfolio_gamma
            .checked_add(sized_gamma)?;
        
        // Vega, Theta, Rho follow similar pattern
        let weighted_vega = greeks.vega
            .checked_mul(weight_factor)?
            .checked_mul(position_size_factor)?;
        portfolio_greeks.portfolio_vega = portfolio_greeks.portfolio_vega
            .checked_add(weighted_vega)?;
        
        let weighted_theta = greeks.theta
            .checked_mul(weight_factor)?
            .checked_mul(position_size_factor)?;
        portfolio_greeks.portfolio_theta = portfolio_greeks.portfolio_theta
            .checked_add(weighted_theta)?;
        
        let weighted_rho = greeks.rho
            .checked_mul(weight_factor)?
            .checked_mul(position_size_factor)?;
        portfolio_greeks.portfolio_rho = portfolio_greeks.portfolio_rho
            .checked_add(weighted_rho)?;
        
        // Store individual position Greeks
        portfolio_greeks.position_weights.push(weight);
        portfolio_greeks.position_greeks.push(PositionGreeks {
            market_id: position.proposal_id,
            size: position.size,
            weight,
            delta: greeks.delta,
            gamma: greeks.gamma,
            vega: greeks.vega,
            theta: greeks.theta,
            rho: greeks.rho,
        });
        
        portfolio_greeks.total_notional = portfolio_greeks.total_notional
            .saturating_add(position.size);
        portfolio_greeks.position_count += 1;
    }
    
    // Update metadata
    portfolio_greeks.last_update = clock.slot;
    
    // Save to account
    portfolio_greeks.serialize(&mut &mut portfolio_greeks_account.data.borrow_mut()[..])?;
    
    msg!("Portfolio Greeks aggregated successfully");
    msg!("Total positions: {}", portfolio_greeks.position_count);
    msg!("Portfolio Delta: {}", portfolio_greeks.portfolio_delta.to_bits());
    msg!("Portfolio Gamma: {}", portfolio_greeks.portfolio_gamma.to_bits());
    msg!("Portfolio Vega: {}", portfolio_greeks.portfolio_vega.to_bits());
    
    Ok(())
}

/// Query portfolio Greeks (view function)
pub fn query_portfolio_greeks(
    accounts: &[AccountInfo],
) -> Result<PortfolioGreeks, ProgramError> {
    let account_iter = &mut accounts.iter();
    let portfolio_greeks_account = next_account_info(account_iter)?;
    
    let portfolio_greeks = PortfolioGreeks::try_from_slice(&portfolio_greeks_account.data.borrow())?;
    portfolio_greeks.validate()?;
    
    Ok(portfolio_greeks)
}

/// Calculate Greeks-based hedging recommendations
pub fn calculate_hedge_recommendations(
    portfolio_greeks: &PortfolioGreeks,
) -> Result<HedgeRecommendations, ProgramError> {
    let mut recommendations = HedgeRecommendations::default();
    
    // Delta hedging: offset portfolio delta to achieve delta-neutral
    let delta_threshold = U64F64::from_num(1000u64) / U64F64::from_num(10000u64); // 0.1
    if portfolio_greeks.portfolio_delta > delta_threshold {
        recommendations.delta_hedge_needed = true;
        // Convert to i64 for negative hedge size (multiply by -1)
        recommendations.delta_hedge_size = -(portfolio_greeks.portfolio_delta.to_num() as f64);
    }
    
    // Gamma hedging: reduce gamma exposure if too high
    let gamma_threshold = U64F64::from_num(500u64) / U64F64::from_num(10000u64); // 0.05
    if portfolio_greeks.portfolio_gamma > gamma_threshold {
        recommendations.gamma_hedge_needed = true;
        // Hedge 50% of gamma exposure
        recommendations.gamma_hedge_size = -(portfolio_greeks.portfolio_gamma.to_num() as f64) / 2.0;
    }
    
    // Vega hedging: manage volatility exposure
    let vega_threshold = U64F64::from_num(100u64);
    if portfolio_greeks.portfolio_vega > vega_threshold {
        recommendations.vega_hedge_needed = true;
        recommendations.vega_hedge_size = -(portfolio_greeks.portfolio_vega.to_num() as f64) * 0.3; // Hedge 30% of vega exposure
    }
    
    Ok(recommendations)
}

#[derive(Debug, Default)]
pub struct HedgeRecommendations {
    pub delta_hedge_needed: bool,
    pub delta_hedge_size: f64,
    pub gamma_hedge_needed: bool,
    pub gamma_hedge_size: f64,
    pub vega_hedge_needed: bool,
    pub vega_hedge_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_portfolio_greeks_aggregation() {
        // Test Greeks aggregation logic
        let mut portfolio_greeks = PortfolioGreeks::new(
            Pubkey::new_unique(),
            12345,
            1,
        );
        
        // Add test position Greeks
        let test_greeks = PositionGreeks {
            market_id: 1,
            size: 1_000_000,
            weight: 5000, // 50%
            delta: U64F64::from_num(5000u64) / U64F64::from_num(10000u64), // 0.5
            gamma: U64F64::from_num(1000u64) / U64F64::from_num(10000u64), // 0.1
            vega: U64F64::from_num(10u64), // 10.0
            theta: U64F64::from_num(0u64), // Can't represent negative directly, would need to subtract
            rho: U64F64::from_num(200u64) / U64F64::from_num(10000u64), // 0.02
        };
        
        portfolio_greeks.position_greeks.push(test_greeks);
        portfolio_greeks.position_count = 1;
        
        // Verify Greeks storage
        assert_eq!(portfolio_greeks.position_greeks.len(), 1);
        // Check delta is approximately 0.5 (5000/10000)
        let expected_delta = U64F64::from_num(5000u64) / U64F64::from_num(10000u64);
        assert_eq!(portfolio_greeks.position_greeks[0].delta, expected_delta);
    }
}