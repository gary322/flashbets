//! Portfolio Value at Risk (VaR) Calculations
//!
//! Implements native Solana portfolio risk metrics using fixed-point arithmetic

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
    math::fixed_point::U64F64,
    state::Position,
    account_validation::{validate_writable, DISCRIMINATOR_SIZE},
};

/// Confidence levels for VaR calculations
pub const VAR_CONFIDENCE_95: u8 = 95;
pub const VAR_CONFIDENCE_99: u8 = 99;
pub const VAR_CONFIDENCE_999: u16 = 999; // 99.9%

/// Time horizons for VaR (in slots)
pub const VAR_HORIZON_1DAY: u64 = 216_000;     // ~24 hours at 400ms/slot
pub const VAR_HORIZON_1WEEK: u64 = 1_512_000;  // ~7 days
pub const VAR_HORIZON_1MONTH: u64 = 6_480_000; // ~30 days

/// Maximum positions for portfolio VaR
pub const MAX_PORTFOLIO_POSITIONS: usize = 100;

/// Portfolio VaR discriminator
pub const PORTFOLIO_VAR_DISCRIMINATOR: [u8; 8] = [80, 79, 82, 84, 86, 65, 82, 0]; // "PORTVAR"

/// Portfolio VaR account structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PortfolioVaR {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User who owns the portfolio
    pub user: Pubkey,
    
    /// Verse ID this VaR is for
    pub verse_id: u128,
    
    /// Total portfolio value
    pub portfolio_value: u64,
    
    /// VaR at 95% confidence
    pub var_95: u64,
    
    /// VaR at 99% confidence
    pub var_99: u64,
    
    /// VaR at 99.9% confidence
    pub var_999: u64,
    
    /// Conditional VaR (Expected Shortfall) at 99%
    pub cvar_99: u64,
    
    /// Portfolio volatility (annualized)
    pub portfolio_volatility: U64F64,
    
    /// Correlation matrix hash
    pub correlation_hash: [u8; 32],
    
    /// Number of positions
    pub position_count: u32,
    
    /// Last update slot
    pub last_update: u64,
    
    /// Time horizon used (in slots)
    pub time_horizon: u64,
    
    /// Historical returns (last 30 data points)
    pub returns_history: Vec<i64>,
    
    /// Maximum drawdown observed
    pub max_drawdown: U64F64,
    
    /// Sharpe ratio
    pub sharpe_ratio: U64F64,
    
    /// Beta vs market
    pub beta: U64F64,
    
    /// Bump seed
    pub bump: u8,
}

impl PortfolioVaR {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 16 + 8 + 8 + 8 + 8 + 8 + 8 + 32 + 4 + 8 + 8 + (30 * 8) + 8 + 8 + 8 + 1;
    
    /// Create new portfolio VaR
    pub fn new(user: Pubkey, verse_id: u128, bump: u8) -> Self {
        Self {
            discriminator: PORTFOLIO_VAR_DISCRIMINATOR,
            user,
            verse_id,
            portfolio_value: 0,
            var_95: 0,
            var_99: 0,
            var_999: 0,
            cvar_99: 0,
            portfolio_volatility: U64F64::from_num(0),
            correlation_hash: [0; 32],
            position_count: 0,
            last_update: 0,
            time_horizon: VAR_HORIZON_1DAY,
            returns_history: Vec::with_capacity(30),
            max_drawdown: U64F64::from_num(0),
            sharpe_ratio: U64F64::from_num(0),
            beta: U64F64::from_num(0),
            bump,
        }
    }
    
    /// Validate account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != PORTFOLIO_VAR_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Position risk metrics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionRisk {
    /// Position pubkey
    pub position: Pubkey,
    
    /// Market/proposal ID
    pub market_id: [u8; 32],
    
    /// Position value
    pub value: u64,
    
    /// Position weight in portfolio
    pub weight: U64F64,
    
    /// Individual volatility
    pub volatility: U64F64,
    
    /// Individual VaR
    pub var_99: u64,
    
    /// Marginal VaR contribution
    pub marginal_var: u64,
    
    /// Component VaR
    pub component_var: u64,
}

/// Calculate portfolio VaR
pub fn process_calculate_portfolio_var(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    confidence_level: u8,
    time_horizon: u64,
) -> ProgramResult {
    msg!("Calculating portfolio VaR");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let portfolio_var_account = next_account_info(account_info_iter)?;
    let verse_account = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_writable(portfolio_var_account)?;
    
    // Derive portfolio VaR PDA
    let (portfolio_var_pda, bump) = derive_portfolio_var_pda(
        program_id,
        user.key,
        &verse_account.key.to_bytes(),
    );
    
    if portfolio_var_account.key != &portfolio_var_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Initialize or load portfolio VaR
    let mut portfolio_var = if portfolio_var_account.data_len() == 0 {
        // Create new account
        let rent = solana_program::rent::Rent::get()?;
        let required_lamports = rent.minimum_balance(PortfolioVaR::LEN);
        
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::create_account(
                user.key,
                portfolio_var_account.key,
                required_lamports,
                PortfolioVaR::LEN as u64,
                program_id,
            ),
            &[
                user.clone(),
                portfolio_var_account.clone(),
            ],
            &[&[
                b"portfolio_var",
                user.key.as_ref(),
                &verse_account.key.to_bytes(),
                &[bump],
            ]],
        )?;
        
        PortfolioVaR::new(*user.key, u128::from_le_bytes(verse_account.key.to_bytes()[0..16].try_into().unwrap()), bump)
    } else {
        PortfolioVaR::try_from_slice(&portfolio_var_account.data.borrow())?
    };
    
    // Collect position data
    let mut positions = Vec::new();
    let mut total_value = 0u64;
    
    // Process remaining accounts as positions
    while let Ok(position_account) = next_account_info(account_info_iter) {
        if let Ok(position) = Position::try_from_slice(&position_account.data.borrow()) {
            // Calculate position value
            let position_value = calculate_position_value(&position)?;
            total_value = total_value.checked_add(position_value)
                .ok_or(BettingPlatformError::MathOverflow)?;
            
            positions.push((position, position_value));
        }
    }
    
    if positions.is_empty() {
        return Err(BettingPlatformError::NoOpenPositions.into());
    }
    
    // Calculate portfolio metrics
    let portfolio_volatility = calculate_portfolio_volatility(&positions)?;
    
    // Calculate VaR at different confidence levels
    let var_95 = calculate_var(total_value, portfolio_volatility, 95, time_horizon)?;
    let var_99 = calculate_var(total_value, portfolio_volatility, 99, time_horizon)?;
    let var_999 = calculate_var(total_value, portfolio_volatility, 999, time_horizon)?;
    
    // Calculate Conditional VaR (Expected Shortfall)
    let cvar_99 = calculate_cvar(total_value, portfolio_volatility, 99, time_horizon)?;
    
    // Update portfolio VaR
    let clock = Clock::get()?;
    portfolio_var.portfolio_value = total_value;
    portfolio_var.var_95 = var_95;
    portfolio_var.var_99 = var_99;
    portfolio_var.var_999 = var_999;
    portfolio_var.cvar_99 = cvar_99;
    portfolio_var.portfolio_volatility = portfolio_volatility;
    portfolio_var.position_count = positions.len() as u32;
    portfolio_var.last_update = clock.slot;
    portfolio_var.time_horizon = time_horizon;
    
    // Update historical returns
    update_returns_history(&mut portfolio_var, total_value)?;
    
    // Calculate additional risk metrics
    portfolio_var.max_drawdown = calculate_max_drawdown(&portfolio_var.returns_history)?;
    portfolio_var.sharpe_ratio = calculate_sharpe_ratio(&portfolio_var.returns_history, portfolio_volatility)?;
    
    // Serialize updated data
    portfolio_var.serialize(&mut &mut portfolio_var_account.data.borrow_mut()[..])?;
    
    msg!("Portfolio VaR calculated: 95%={}, 99%={}, 99.9%={}", var_95, var_99, var_999);
    Ok(())
}

/// Calculate VaR using parametric method
fn calculate_var(
    portfolio_value: u64,
    volatility: U64F64,
    confidence_level: u16,
    time_horizon: u64,
) -> Result<u64, ProgramError> {
    // Get z-score for confidence level
    let z_score = get_z_score(confidence_level)?;
    
    // Adjust volatility for time horizon
    // sqrt(time_horizon / annual_slots)
    let annual_slots = 315_360_000u64; // ~365 days
    let time_factor = U64F64::from_num(time_horizon) / U64F64::from_num(annual_slots);
    let sqrt_time = time_factor.sqrt();
    
    // VaR = portfolio_value * volatility * z_score * sqrt(time)
    let var_ratio = volatility * z_score * sqrt_time?;
    let var = (U64F64::from_num(portfolio_value) * var_ratio).to_num();
    
    Ok(var)
}

/// Calculate Conditional VaR (Expected Shortfall)
fn calculate_cvar(
    portfolio_value: u64,
    volatility: U64F64,
    confidence_level: u16,
    time_horizon: u64,
) -> Result<u64, ProgramError> {
    // For normal distribution, CVaR = portfolio_value * volatility * phi(z) / (1 - confidence)
    let z_score = get_z_score(confidence_level)?;
    
    // Get PDF value at z-score
    let pdf_value = get_normal_pdf(z_score)?;
    
    // Calculate tail probability
    let tail_prob = U64F64::from_num((100 - confidence_level) as u64) / U64F64::from_num(100);
    
    // Adjust for time horizon
    let annual_slots = 315_360_000u64;
    let time_factor = U64F64::from_num(time_horizon) / U64F64::from_num(annual_slots);
    let sqrt_time = time_factor.sqrt();
    
    // CVaR = portfolio_value * volatility * sqrt(time) * phi(z) / tail_prob
    let cvar_ratio = volatility * sqrt_time? * pdf_value / tail_prob;
    let cvar = (U64F64::from_num(portfolio_value) * cvar_ratio).to_num();
    
    Ok(cvar)
}

/// Get z-score for confidence level
fn get_z_score(confidence_level: u16) -> Result<U64F64, ProgramError> {
    let z_score = match confidence_level {
        95 => U64F64::from_fraction(1645, 1000)?,    // 95% confidence
        99 => U64F64::from_fraction(2326, 1000)?,    // 99% confidence
        999 => U64F64::from_fraction(3090, 1000)?,   // 99.9% confidence
        _ => return Err(BettingPlatformError::InvalidInput.into()),
    };
    Ok(z_score)
}

/// Get normal PDF value
fn get_normal_pdf(z: U64F64) -> Result<U64F64, ProgramError> {
    // phi(z) = exp(-z^2/2) / sqrt(2*pi)
    let z_squared = z * z;
    let exponent = z_squared / U64F64::from_num(2);
    
    // Approximate exp(-x) for small x
    let exp_neg = U64F64::from_num(1) - exponent + (exponent * exponent) / U64F64::from_num(2);
    
    // sqrt(2*pi) ≈ 2.507
    let sqrt_2pi = U64F64::from_fraction(2507, 1000)?;
    
    Ok(exp_neg / sqrt_2pi)
}

/// Calculate portfolio volatility
fn calculate_portfolio_volatility(
    positions: &[(Position, u64)],
) -> Result<U64F64, ProgramError> {
    // Simplified: assume equal correlations and use average volatility
    // In production, would use full correlation matrix
    
    let mut weighted_vol = U64F64::from_num(0);
    let total_value: u64 = positions.iter().map(|(_, v)| v).sum();
    
    for (position, value) in positions {
        let weight = U64F64::from_num(*value) / U64F64::from_num(total_value);
        // Assume 30% annualized volatility for all positions (simplified)
        let position_vol = U64F64::from_fraction(30, 100)?;
        weighted_vol = weighted_vol + (weight * position_vol);
    }
    
    // Adjust for diversification (simplified)
    let diversification_factor = U64F64::from_fraction(4, 5)?;
    Ok(weighted_vol * diversification_factor)
}

/// Calculate position value
fn calculate_position_value(position: &Position) -> Result<u64, ProgramError> {
    // Simplified: use notional value
    // In production, would mark-to-market using current prices
    Ok(position.notional)
}

/// Update returns history
fn update_returns_history(
    portfolio_var: &mut PortfolioVaR,
    new_value: u64,
) -> Result<(), ProgramError> {
    if portfolio_var.portfolio_value > 0 {
        let return_bps = ((new_value as i128 - portfolio_var.portfolio_value as i128) * 10000 
            / portfolio_var.portfolio_value as i128) as i64;
        
        portfolio_var.returns_history.push(return_bps);
        
        // Keep only last 30 data points
        if portfolio_var.returns_history.len() > 30 {
            portfolio_var.returns_history.remove(0);
        }
    }
    
    Ok(())
}

/// Calculate maximum drawdown
fn calculate_max_drawdown(returns: &[i64]) -> Result<U64F64, ProgramError> {
    if returns.is_empty() {
        return Ok(U64F64::from_num(0));
    }
    
    let mut peak = 0i64;
    let mut max_dd = 0i64;
    let mut cumulative = 0i64;
    
    for &ret in returns {
        cumulative += ret;
        if cumulative > peak {
            peak = cumulative;
        }
        let drawdown = peak - cumulative;
        if drawdown > max_dd {
            max_dd = drawdown;
        }
    }
    
    // Convert basis points to percentage
    Ok(U64F64::from_num(max_dd.abs() as u64) / U64F64::from_num(10000))
}

/// Calculate Sharpe ratio
fn calculate_sharpe_ratio(
    returns: &[i64],
    volatility: U64F64,
) -> Result<U64F64, ProgramError> {
    if returns.is_empty() {
        return Ok(U64F64::from_num(0));
    }
    
    // Calculate average return
    let sum: i64 = returns.iter().sum();
    let avg_return = U64F64::from_num(sum.abs() as u64) / U64F64::from_num(returns.len() as u64);
    
    // Convert from basis points to annual return
    let annual_return = avg_return * U64F64::from_num(365) / U64F64::from_num(10000);
    
    // Sharpe = (return - risk_free_rate) / volatility
    // Assume 2% risk-free rate
    let risk_free = U64F64::from_fraction(2, 100)?;
    let excess_return = annual_return - risk_free;
    
    if volatility > U64F64::from_num(0) {
        Ok(excess_return / volatility)
    } else {
        Ok(U64F64::from_num(0))
    }
}

/// Calculate marginal VaR for a position
pub fn calculate_marginal_var(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_pubkey: &Pubkey,
) -> ProgramResult {
    msg!("Calculating marginal VaR for position");
    
    // Implementation would calculate how VaR changes
    // if position size is increased by a small amount
    
    Ok(())
}

/// Stress test portfolio
pub fn process_stress_test_portfolio(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    scenario: StressScenario,
) -> ProgramResult {
    msg!("Running portfolio stress test");
    
    // Implementation would apply various stress scenarios
    // and calculate impact on portfolio value
    
    Ok(())
}

/// Stress test scenarios
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum StressScenario {
    /// Market crash (-20% all positions)
    MarketCrash,
    
    /// Volatility spike (2x volatility)
    VolatilitySpike,
    
    /// Correlation breakdown
    CorrelationBreakdown,
    
    /// Liquidity crisis
    LiquidityCrisis,
    
    /// Custom scenario
    Custom {
        price_shock: i16,      // basis points
        vol_multiplier: u16,   // 100 = 1x, 200 = 2x
        correlation: i16,      // -100 to 100
    },
}

/// Derive portfolio VaR PDA
pub fn derive_portfolio_var_pda(
    program_id: &Pubkey,
    user: &Pubkey,
    verse_id: &[u8],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"portfolio_var",
            user.as_ref(),
            verse_id,
        ],
        program_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_var_calculation() {
        let portfolio_value = 1_000_000;
        let volatility = U64F64::from_fraction(1, 5).unwrap(); // 20% annual vol
        let time_horizon = VAR_HORIZON_1DAY;
        
        let var_99 = calculate_var(portfolio_value, volatility, 99, time_horizon).unwrap();
        
        // Daily VaR should be approximately 2.326 * 20% * sqrt(1/365) * 1M
        // ≈ 2.326 * 0.2 * 0.0523 * 1M ≈ 24,300
        assert!(var_99 > 20_000 && var_99 < 30_000);
    }
    
    #[test]
    fn test_sharpe_ratio() {
        let returns = vec![100, -50, 150, -25, 75]; // basis points
        let volatility = U64F64::from_fraction(15, 100).unwrap();
        
        let sharpe = calculate_sharpe_ratio(&returns, volatility).unwrap();
        
        // Should be positive with these returns
        assert!(sharpe > U64F64::from_num(0));
    }
}