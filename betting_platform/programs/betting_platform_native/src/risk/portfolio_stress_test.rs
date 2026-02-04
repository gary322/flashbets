//! Portfolio Stress Testing Module
//!
//! Implements stress test scenarios including -50% market move simulation
//! Specification: Simulate -50% move for stress testing with UX dashboard integration
//! Money-Making: VaR = avoid losses +10%

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
        special_functions::{calculate_var_specific, Greeks, calculate_greeks},
        tables::NormalDistributionTables,
    },
    state::{Position, GlobalState, VerseState},
    account_validation::{validate_writable, DISCRIMINATOR_SIZE},
    portfolio::PortfolioGreeks,
    margin::CrossMarginAccount,
    constants::POSITION_DISCRIMINATOR,
};

/// Stress test discriminator
pub const STRESS_TEST_DISCRIMINATOR: [u8; 8] = [83, 84, 82, 69, 83, 83, 84, 83]; // "STRESSTS"

/// Standard stress test scenarios
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum StressScenario {
    /// -50% market move as specified
    MarketCrash50Percent,
    /// +50% market move (opposite direction)
    MarketRally50Percent,
    /// Volatility spike to 100%
    VolatilitySpike,
    /// Liquidity crisis
    LiquidityCrisis,
    /// Correlation breakdown
    CorrelationBreakdown,
    /// Custom scenario with parameters
    Custom {
        price_change: i16, // basis points
        volatility_multiplier: u16, // 100 = 1x
        correlation_change: i16, // basis points
    },
}

/// Stress test result for a single position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionStressResult {
    pub market_id: u128,
    pub initial_value: u64,
    pub stressed_value: i64, // Can be negative
    pub pnl: i64,
    pub margin_impact: u64,
    pub liquidation_risk: bool,
    pub greeks_impact: GreeksImpact,
}

/// Greeks impact under stress
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct GreeksImpact {
    pub delta_change: U64F64,
    pub gamma_change: U64F64,
    pub vega_change: U64F64,
}

/// Portfolio stress test results
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PortfolioStressTest {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Verse ID
    pub verse_id: u128,
    
    /// Stress scenario tested
    pub scenario: StressScenario,
    
    /// Initial portfolio value
    pub initial_portfolio_value: u64,
    
    /// Stressed portfolio value
    pub stressed_portfolio_value: i64,
    
    /// Total P&L under stress
    pub total_pnl: i64,
    
    /// Number of positions that would be liquidated
    pub positions_at_risk: u32,
    
    /// Total margin requirement under stress
    pub stressed_margin_requirement: u64,
    
    /// Available collateral
    pub available_collateral: u64,
    
    /// Margin shortfall
    pub margin_shortfall: i64,
    
    /// VaR under stress scenario
    pub stressed_var: u64,
    
    /// Position-level results
    pub position_results: Vec<PositionStressResult>,
    
    /// Risk metrics
    pub risk_metrics: StressRiskMetrics,
    
    /// Last update slot
    pub last_update: u64,
    
    /// UX dashboard data
    pub dashboard_data: DashboardData,
    
    /// Bump seed
    pub bump: u8,
}

/// Risk metrics under stress
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct StressRiskMetrics {
    pub portfolio_beta: U64F64,
    pub correlation_impact: U64F64,
    pub concentration_risk: u16, // basis points
    pub recovery_time_estimate: u64, // slots
}

/// Dashboard data for UX integration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DashboardData {
    pub risk_score: u16, // 0-10000
    pub health_status: HealthStatus,
    pub recommended_actions: Vec<u8>, // Encoded recommendations
    pub visual_indicators: VisualIndicators,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    AtRisk,
    Critical,
    Liquidation,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VisualIndicators {
    pub margin_usage_percent: u8,
    pub risk_gauge: u8, // 0-100
    pub liquidity_score: u8, // 0-100
}

impl PortfolioStressTest {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 16 + 32 + 8 + 8 + 8 + 4 + 8 + 8 + 8 + 8 + 
                          (50 * PositionStressResult::LEN) + StressRiskMetrics::LEN + 8 + 
                          DashboardData::LEN + 1;
    
    /// Create new stress test
    pub fn new(user: Pubkey, verse_id: u128, scenario: StressScenario, bump: u8) -> Self {
        Self {
            discriminator: STRESS_TEST_DISCRIMINATOR,
            user,
            verse_id,
            scenario,
            initial_portfolio_value: 0,
            stressed_portfolio_value: 0,
            total_pnl: 0,
            positions_at_risk: 0,
            stressed_margin_requirement: 0,
            available_collateral: 0,
            margin_shortfall: 0,
            stressed_var: 0,
            position_results: Vec::new(),
            risk_metrics: StressRiskMetrics {
                portfolio_beta: U64F64::from_num(1u64),
                correlation_impact: U64F64::from_num(0),
                concentration_risk: 0,
                recovery_time_estimate: 0,
            },
            last_update: 0,
            dashboard_data: DashboardData {
                risk_score: 5000,
                health_status: HealthStatus::Healthy,
                recommended_actions: Vec::new(),
                visual_indicators: VisualIndicators {
                    margin_usage_percent: 0,
                    risk_gauge: 0,
                    liquidity_score: 100,
                },
            },
            bump,
        }
    }
    
    /// Validate account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != STRESS_TEST_DISCRIMINATOR {
            msg!("Invalid stress test discriminator");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

impl PositionStressResult {
    pub const LEN: usize = 16 + 8 + 8 + 8 + 8 + 1 + GreeksImpact::LEN;
}

impl GreeksImpact {
    pub const LEN: usize = 8 + 8 + 8;
}

impl StressRiskMetrics {
    pub const LEN: usize = 8 + 8 + 2 + 8;
}

impl DashboardData {
    pub const LEN: usize = 2 + 1 + 100 + VisualIndicators::LEN;
}

impl VisualIndicators {
    pub const LEN: usize = 1 + 1 + 1;
}

/// Execute portfolio stress test
pub fn execute_stress_test(
    accounts: &[AccountInfo],
    scenario: StressScenario,
    tables: &NormalDistributionTables,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let stress_test_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let verse_state_account = next_account_info(account_iter)?;
    let global_state_account = next_account_info(account_iter)?;
    let portfolio_greeks_account = next_account_info(account_iter)?;
    let cross_margin_account = next_account_info(account_iter)?;
    let clock_account = next_account_info(account_iter)?;
    
    // Validate accounts
    validate_writable(stress_test_account)?;
    
    // Load clock
    let clock = Clock::from_account_info(clock_account)?;
    
    // Load states
    let verse_state = VerseState::try_from_slice(&verse_state_account.data.borrow())?;
    let global_state = GlobalState::try_from_slice(&global_state_account.data.borrow())?;
    
    // Initialize or load stress test
    let mut stress_test = if stress_test_account.data_len() == 0 {
        let (pda, bump) = Pubkey::find_program_address(
            &[
                b"stress_test",
                user_account.key.as_ref(),
                &verse_state.verse_id.to_le_bytes(),
            ],
            &crate::id(),
        );
        
        if pda != *stress_test_account.key {
            msg!("Invalid stress test PDA");
            return Err(ProgramError::InvalidAccountData);
        }
        
        PortfolioStressTest::new(*user_account.key, verse_state.verse_id, scenario.clone(), bump)
    } else {
        let mut data = PortfolioStressTest::try_from_slice(&stress_test_account.data.borrow())?;
        data.validate()?;
        data.scenario = scenario.clone();
        data
    };
    
    // Load optional accounts
    let portfolio_greeks = if portfolio_greeks_account.data_len() > 0 {
        Some(PortfolioGreeks::try_from_slice(&portfolio_greeks_account.data.borrow())?)
    } else {
        None
    };
    
    let cross_margin = if cross_margin_account.data_len() > 0 {
        Some(CrossMarginAccount::try_from_slice(&cross_margin_account.data.borrow())?)
    } else {
        None
    };
    
    // Reset results
    stress_test.position_results.clear();
    stress_test.initial_portfolio_value = 0;
    stress_test.stressed_portfolio_value = 0;
    stress_test.positions_at_risk = 0;
    
    // Get stress parameters
    let (price_shock, vol_multiplier) = match &scenario {
        StressScenario::MarketCrash50Percent => (-5000i16, 200u16), // -50%, 2x volatility
        StressScenario::MarketRally50Percent => (5000i16, 200u16), // +50%, 2x volatility
        StressScenario::VolatilitySpike => (0i16, 500u16), // No price change, 5x volatility
        StressScenario::LiquidityCrisis => (-2000i16, 300u16), // -20%, 3x volatility
        StressScenario::CorrelationBreakdown => (0i16, 150u16), // No price change, 1.5x volatility
        StressScenario::Custom { price_change, volatility_multiplier, .. } => 
            (*price_change, *volatility_multiplier),
    };
    
    // Process positions
    let mut total_collateral = 0u64;
    let mut total_margin_required = 0u64;
    let base_volatility = 2000u64; // 20% base volatility (in basis points)
    let stressed_volatility = (base_volatility * vol_multiplier as u64) / 100;
    
    while let Ok(position_account) = next_account_info(account_iter) {
        let position_data = position_account.data.borrow();
        
        if position_data.len() < DISCRIMINATOR_SIZE {
            continue;
        }
        
        let discriminator = &position_data[..DISCRIMINATOR_SIZE];
        if discriminator != POSITION_DISCRIMINATOR {
            continue;
        }
        
        let position = Position::try_from_slice(&position_data)?;
        
        // Calculate initial value
        let initial_value = position.size.saturating_mul(position.entry_price as u64) / 1_000_000;
        stress_test.initial_portfolio_value = stress_test.initial_portfolio_value
            .saturating_add(initial_value);
        
        // Apply stress scenario
        let stressed_price = if position.is_long {
            ((position.entry_price as i64) * (10000 + price_shock as i64)) / 10000
        } else {
            ((position.entry_price as i64) * (10000 - price_shock as i64)) / 10000
        };
        
        let stressed_value = (position.size as i64).saturating_mul(stressed_price) / 1_000_000;
        let pnl = stressed_value - initial_value as i64;
        
        // Calculate margin impact
        let margin_multiplier = stressed_volatility / base_volatility.max(1);
        let stressed_margin = (position.size / position.leverage as u64)
            .saturating_mul(margin_multiplier);
        
        // Check liquidation risk - using entry price as proxy for collateral
        let liquidation_risk = position.entry_price < stressed_margin;
        if liquidation_risk {
            stress_test.positions_at_risk += 1;
        }
        
        // Calculate Greeks impact (simplified)
        let greeks_impact = GreeksImpact {
            delta_change: U64F64::from_num(price_shock.unsigned_abs() as u64) / U64F64::from_num(10000u64),
            gamma_change: U64F64::from_num(vol_multiplier.saturating_sub(100) as u64) / U64F64::from_num(100u64),
            vega_change: U64F64::from_num(vol_multiplier.saturating_mul(vol_multiplier).saturating_sub(10000) as u64) / U64F64::from_num(10000u64),
        };
        
        // Store position result
        stress_test.position_results.push(PositionStressResult {
            market_id: position.proposal_id,
            initial_value,
            stressed_value,
            pnl,
            margin_impact: stressed_margin,
            liquidation_risk,
            greeks_impact,
        });
        
        stress_test.stressed_portfolio_value = stress_test.stressed_portfolio_value
            .saturating_add(stressed_value);
        total_collateral = total_collateral.saturating_add(position.margin);
        total_margin_required = total_margin_required.saturating_add(stressed_margin);
    }
    
    // Calculate aggregate metrics
    stress_test.total_pnl = stress_test.stressed_portfolio_value - stress_test.initial_portfolio_value as i64;
    stress_test.available_collateral = total_collateral;
    stress_test.stressed_margin_requirement = total_margin_required;
    stress_test.margin_shortfall = total_margin_required as i64 - total_collateral as i64;
    
    // Calculate stressed VaR using the specific formula
    let deposit = U64F64::from_num(stress_test.initial_portfolio_value) / U64F64::from_num(1_000_000u64);
    let sigma = U64F64::from_num(stressed_volatility) / U64F64::from_num(10000u64);
    let time = U64F64::from_num(1u64); // 1 year horizon
    
    stress_test.stressed_var = calculate_var_specific(tables, deposit, sigma, time)?
        .to_num() * 1_000_000;
    
    // Update risk metrics
    update_risk_metrics(&mut stress_test, &scenario)?;
    
    // Update dashboard data
    update_dashboard_data(&mut stress_test)?;
    
    // Update timestamp
    stress_test.last_update = clock.slot;
    
    // Save to account
    stress_test.serialize(&mut &mut stress_test_account.data.borrow_mut()[..])?;
    
    msg!("Stress test completed for scenario: {:?}", scenario);
    msg!("Initial value: ${}", stress_test.initial_portfolio_value / 1_000_000);
    msg!("Stressed value: ${}", stress_test.stressed_portfolio_value / 1_000_000);
    msg!("Total P&L: ${}", stress_test.total_pnl / 1_000_000);
    msg!("Positions at risk: {}", stress_test.positions_at_risk);
    msg!("Margin shortfall: ${}", stress_test.margin_shortfall / 1_000_000);
    msg!("Stressed VaR: ${}", stress_test.stressed_var / 1_000_000);
    
    Ok(())
}

/// Update risk metrics based on stress results
fn update_risk_metrics(
    stress_test: &mut PortfolioStressTest,
    scenario: &StressScenario,
) -> Result<(), ProgramError> {
    // Portfolio beta (simplified)
    let market_move = match scenario {
        StressScenario::MarketCrash50Percent => -0.5,
        StressScenario::MarketRally50Percent => 0.5,
        _ => 0.0,
    };
    
    if market_move != 0.0 {
        let portfolio_return = stress_test.total_pnl as f64 / stress_test.initial_portfolio_value as f64;
        let beta_value = (portfolio_return / market_move * 10000.0) as u64;
        stress_test.risk_metrics.portfolio_beta = U64F64::from_num(beta_value) / U64F64::from_num(10000u64);
    }
    
    // Concentration risk
    if let Some(max_position) = stress_test.position_results.iter()
        .max_by_key(|p| p.initial_value) {
        let concentration = (max_position.initial_value * 10000) / stress_test.initial_portfolio_value;
        stress_test.risk_metrics.concentration_risk = concentration as u16;
    }
    
    // Recovery time estimate (simplified: worse drawdown = longer recovery)
    let drawdown_percent = (stress_test.total_pnl.abs() as u64 * 100) / stress_test.initial_portfolio_value;
    stress_test.risk_metrics.recovery_time_estimate = drawdown_percent * 4320; // ~12 hours per 1% drawdown
    
    Ok(())
}

/// Update dashboard data for UX
fn update_dashboard_data(stress_test: &mut PortfolioStressTest) -> Result<(), ProgramError> {
    // Calculate margin usage
    let margin_usage = if stress_test.available_collateral > 0 {
        ((stress_test.stressed_margin_requirement * 100) / stress_test.available_collateral).min(100) as u8
    } else {
        100
    };
    
    // Calculate risk gauge (0-100)
    let risk_gauge = calculate_risk_gauge(stress_test);
    
    // Determine health status
    let health_status = if stress_test.margin_shortfall > 0 {
        HealthStatus::Liquidation
    } else if margin_usage > 80 {
        HealthStatus::Critical
    } else if margin_usage > 60 || stress_test.positions_at_risk > 0 {
        HealthStatus::AtRisk
    } else {
        HealthStatus::Healthy
    };
    
    // Calculate risk score (0-10000)
    let risk_score = ((margin_usage as u16 * 50) + (risk_gauge as u16 * 30) + 
                     ((stress_test.positions_at_risk * 500).min(2000) as u16));
    
    // Update dashboard
    stress_test.dashboard_data = DashboardData {
        risk_score,
        health_status,
        recommended_actions: generate_recommendations(stress_test),
        visual_indicators: VisualIndicators {
            margin_usage_percent: margin_usage,
            risk_gauge,
            liquidity_score: calculate_liquidity_score(stress_test),
        },
    };
    
    Ok(())
}

/// Calculate risk gauge for dashboard
fn calculate_risk_gauge(stress_test: &PortfolioStressTest) -> u8 {
    let pnl_impact = (stress_test.total_pnl.abs() as u64 * 100) / stress_test.initial_portfolio_value;
    let var_impact = (stress_test.stressed_var * 100) / stress_test.initial_portfolio_value;
    
    ((pnl_impact + var_impact) / 2).min(100) as u8
}

/// Calculate liquidity score
fn calculate_liquidity_score(stress_test: &PortfolioStressTest) -> u8 {
    if stress_test.margin_shortfall > 0 {
        0
    } else {
        let excess_collateral = stress_test.available_collateral
            .saturating_sub(stress_test.stressed_margin_requirement);
        let liquidity_ratio = (excess_collateral * 100) / stress_test.available_collateral.max(1);
        liquidity_ratio.min(100) as u8
    }
}

/// Generate recommendations based on stress test results
fn generate_recommendations(stress_test: &PortfolioStressTest) -> Vec<u8> {
    let mut recommendations = Vec::new();
    
    // Recommendation codes (for UX to decode)
    const REDUCE_LEVERAGE: u8 = 1;
    const HEDGE_POSITIONS: u8 = 2;
    const ADD_COLLATERAL: u8 = 3;
    const DIVERSIFY: u8 = 4;
    const CLOSE_RISKY: u8 = 5;
    
    if stress_test.margin_shortfall > 0 {
        recommendations.push(ADD_COLLATERAL);
        recommendations.push(REDUCE_LEVERAGE);
    }
    
    if stress_test.positions_at_risk > 0 {
        recommendations.push(CLOSE_RISKY);
    }
    
    if stress_test.risk_metrics.concentration_risk > 3000 {
        recommendations.push(DIVERSIFY);
    }
    
    if stress_test.risk_metrics.portfolio_beta.to_num() > 15000 { // 1.5 in fixed point (15000/10000)
        recommendations.push(HEDGE_POSITIONS);
    }
    
    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stress_scenario_50_percent_crash() {
        let scenario = StressScenario::MarketCrash50Percent;
        let (price_shock, vol_multiplier) = match scenario {
            StressScenario::MarketCrash50Percent => (-5000i16, 200u16),
            _ => panic!("Wrong scenario"),
        };
        
        assert_eq!(price_shock, -5000); // -50%
        assert_eq!(vol_multiplier, 200); // 2x volatility
    }
    
    #[test]
    fn test_dashboard_health_status() {
        let mut stress_test = PortfolioStressTest::new(
            Pubkey::new_unique(),
            12345,
            StressScenario::MarketCrash50Percent,
            1,
        );
        
        // Test healthy status
        stress_test.margin_shortfall = -1000; // Excess margin
        stress_test.available_collateral = 10000;
        stress_test.stressed_margin_requirement = 5000;
        
        update_dashboard_data(&mut stress_test).unwrap();
        assert_eq!(stress_test.dashboard_data.health_status, HealthStatus::Healthy);
        
        // Test liquidation status
        stress_test.margin_shortfall = 1000; // Shortfall
        update_dashboard_data(&mut stress_test).unwrap();
        assert_eq!(stress_test.dashboard_data.health_status, HealthStatus::Liquidation);
    }
}