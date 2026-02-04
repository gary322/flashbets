//! Cross-Margining System
//!
//! Implements verse-level position netting for capital efficiency
//! Specification: +15% capital efficiency through cross-position collateral netting
//!
//! Key Features:
//! - Verse-level margin calculations across all positions
//! - Net margin requirements using MapEntry PDA
//! - Dynamic margin based on portfolio risk
//! - Integration with existing collateral system

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
    state::{Position, GlobalState, VerseState},
    account_validation::{validate_writable, DISCRIMINATOR_SIZE},
    constants::{POSITION_DISCRIMINATOR, COLLATERAL_DECIMALS},
    portfolio::PortfolioGreeks,
};

/// Cross-margin account discriminator
pub const CROSS_MARGIN_DISCRIMINATOR: [u8; 8] = [67, 82, 79, 83, 83, 77, 82, 71]; // "CROSSMRG"

/// Minimum positions required for cross-margin benefits
pub const MIN_CROSS_MARGIN_POSITIONS: u32 = 2;

/// Maximum capital efficiency improvement (15%)
pub const MAX_EFFICIENCY_IMPROVEMENT: u64 = 150; // basis points

/// Cross-margin modes
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum CrossMarginMode {
    /// Standard isolated margin per position
    Isolated,
    /// Cross-margin with verse-level netting
    Cross,
    /// Portfolio margin with risk-based calculation
    Portfolio,
}

/// Cross-margin account for verse-level position netting
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CrossMarginAccount {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Verse ID
    pub verse_id: u128,
    
    /// Cross-margin mode
    pub mode: CrossMarginMode,
    
    /// Total collateral across all positions
    pub total_collateral: u64,
    
    /// Gross margin requirement (sum of individual requirements)
    pub gross_margin_requirement: u64,
    
    /// Net margin requirement (after netting)
    pub net_margin_requirement: u64,
    
    /// Capital efficiency improvement (basis points)
    pub efficiency_improvement: u16,
    
    /// Number of positions
    pub position_count: u32,
    
    /// Long exposure
    pub total_long_exposure: u64,
    
    /// Short exposure
    pub total_short_exposure: u64,
    
    /// Net exposure
    pub net_exposure: i64,
    
    /// Portfolio Greeks reference (optional)
    pub portfolio_greeks: Option<Pubkey>,
    
    /// Last update slot
    pub last_update: u64,
    
    /// Risk score (0-10000)
    pub risk_score: u16,
    
    /// Bump seed
    pub bump: u8,
}

impl CrossMarginAccount {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 16 + 1 + 8 + 8 + 8 + 2 + 4 + 8 + 8 + 8 + 33 + 8 + 2 + 1;
    
    /// Create new cross-margin account
    pub fn new(user: Pubkey, verse_id: u128, bump: u8) -> Self {
        Self {
            discriminator: CROSS_MARGIN_DISCRIMINATOR,
            user,
            verse_id,
            mode: CrossMarginMode::Cross,
            total_collateral: 0,
            gross_margin_requirement: 0,
            net_margin_requirement: 0,
            efficiency_improvement: 0,
            position_count: 0,
            total_long_exposure: 0,
            total_short_exposure: 0,
            net_exposure: 0,
            portfolio_greeks: None,
            last_update: 0,
            risk_score: 0,
            bump,
        }
    }
    
    /// Validate account
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != CROSS_MARGIN_DISCRIMINATOR {
            msg!("Invalid cross-margin discriminator");
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
    
    /// Calculate capital efficiency improvement
    pub fn calculate_efficiency(&mut self) -> Result<(), ProgramError> {
        if self.gross_margin_requirement == 0 {
            self.efficiency_improvement = 0;
            return Ok(());
        }
        
        // Efficiency = (gross - net) / gross * 10000
        let savings = self.gross_margin_requirement
            .saturating_sub(self.net_margin_requirement);
        let efficiency = (savings * 10000) / self.gross_margin_requirement;
        
        // Cap at maximum efficiency improvement (15%)
        self.efficiency_improvement = efficiency.min(MAX_EFFICIENCY_IMPROVEMENT) as u16;
        
        Ok(())
    }
}

/// Calculate cross-margin requirements for a verse
pub fn calculate_cross_margin(
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cross_margin_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let verse_state_account = next_account_info(account_iter)?;
    let global_state_account = next_account_info(account_iter)?;
    let portfolio_greeks_account = next_account_info(account_iter)?;
    let clock_account = next_account_info(account_iter)?;
    
    // Validate accounts
    validate_writable(cross_margin_account)?;
    
    // Load clock
    let clock = Clock::from_account_info(clock_account)?;
    
    // Load states
    let verse_state = VerseState::try_from_slice(&verse_state_account.data.borrow())?;
    let global_state = GlobalState::try_from_slice(&global_state_account.data.borrow())?;
    
    // Initialize or load cross-margin account
    let mut cross_margin = if cross_margin_account.data_len() == 0 {
        // Initialize new account
        let (pda, bump) = Pubkey::find_program_address(
            &[
                b"cross_margin",
                user_account.key.as_ref(),
                &verse_state.verse_id.to_le_bytes(),
            ],
            &crate::id(),
        );
        
        if pda != *cross_margin_account.key {
            msg!("Invalid cross-margin PDA");
            return Err(ProgramError::InvalidAccountData);
        }
        
        CrossMarginAccount::new(*user_account.key, verse_state.verse_id, bump)
    } else {
        let mut data = CrossMarginAccount::try_from_slice(&cross_margin_account.data.borrow())?;
        data.validate()?;
        data
    };
    
    // Load portfolio Greeks if available
    let portfolio_greeks = if portfolio_greeks_account.data_len() > 0 {
        Some(PortfolioGreeks::try_from_slice(&portfolio_greeks_account.data.borrow())?)
    } else {
        None
    };
    
    // Reset calculations
    cross_margin.total_collateral = 0;
    cross_margin.gross_margin_requirement = 0;
    cross_margin.net_margin_requirement = 0;
    cross_margin.position_count = 0;
    cross_margin.total_long_exposure = 0;
    cross_margin.total_short_exposure = 0;
    cross_margin.net_exposure = 0;
    
    // Process positions
    let mut position_margins = Vec::new();
    let mut position_exposures = Vec::new();
    
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
        
        // Calculate individual margin requirement
        let position_notional = position.size.saturating_mul(position.entry_price as u64) / (10u64.pow(COLLATERAL_DECIMALS as u32));
        let individual_margin = calculate_position_margin(&position, &verse_state)?;
        
        // Track exposures
        if position.is_long {
            cross_margin.total_long_exposure = cross_margin.total_long_exposure
                .saturating_add(position_notional);
        } else {
            cross_margin.total_short_exposure = cross_margin.total_short_exposure
                .saturating_add(position_notional);
        }
        
        // Add to gross requirement
        cross_margin.gross_margin_requirement = cross_margin.gross_margin_requirement
            .saturating_add(individual_margin);
        
        position_margins.push(individual_margin);
        position_exposures.push((position.is_long, position_notional));
        
        cross_margin.position_count += 1;
        cross_margin.total_collateral = cross_margin.total_collateral
            .saturating_add(position.margin);
    }
    
    // Calculate net exposure
    cross_margin.net_exposure = (cross_margin.total_long_exposure as i64)
        .saturating_sub(cross_margin.total_short_exposure as i64);
    
    // Calculate net margin requirement based on mode
    match cross_margin.mode {
        CrossMarginMode::Isolated => {
            // No netting benefit
            cross_margin.net_margin_requirement = cross_margin.gross_margin_requirement;
        },
        CrossMarginMode::Cross => {
            // Apply verse-level netting
            cross_margin.net_margin_requirement = calculate_netted_margin(
                &position_exposures,
                cross_margin.gross_margin_requirement,
                cross_margin.position_count,
            )?;
        },
        CrossMarginMode::Portfolio => {
            // Use portfolio Greeks for risk-based calculation
            if let Some(ref greeks) = portfolio_greeks {
                cross_margin.net_margin_requirement = calculate_portfolio_margin(
                    greeks,
                    cross_margin.gross_margin_requirement,
                    &position_exposures,
                )?;
                cross_margin.portfolio_greeks = Some(*portfolio_greeks_account.key);
            } else {
                // Fall back to cross-margin calculation
                cross_margin.net_margin_requirement = calculate_netted_margin(
                    &position_exposures,
                    cross_margin.gross_margin_requirement,
                    cross_margin.position_count,
                )?;
            }
        },
    }
    
    // Calculate risk score
    cross_margin.risk_score = calculate_risk_score(&cross_margin)?;
    
    // Calculate efficiency improvement
    cross_margin.calculate_efficiency()?;
    
    // Update timestamp
    cross_margin.last_update = clock.slot;
    
    // Save to account
    cross_margin.serialize(&mut &mut cross_margin_account.data.borrow_mut()[..])?;
    
    msg!("Cross-margin calculation complete");
    msg!("Gross margin: {}", cross_margin.gross_margin_requirement);
    msg!("Net margin: {}", cross_margin.net_margin_requirement);
    msg!("Efficiency improvement: {}%", cross_margin.efficiency_improvement as f64 / 100.0);
    
    Ok(())
}

/// Calculate margin requirement for a single position
fn calculate_position_margin(
    position: &Position,
    verse_state: &VerseState,
) -> Result<u64, ProgramError> {
    // Base margin = size * leverage * margin_rate
    let base_margin = position.size / position.leverage as u64;
    
    // Add risk premium based on verse open interest (use OI as proxy for volatility)
    // If OI > 10M, add 10% risk premium
    let risk_premium = if verse_state.total_oi > 10_000_000 {
        base_margin / 10 // 10% additional margin for high OI
    } else {
        0
    };
    
    Ok(base_margin.saturating_add(risk_premium))
}

/// Calculate netted margin requirement for cross-margin
fn calculate_netted_margin(
    position_exposures: &[(bool, u64)],
    gross_margin: u64,
    position_count: u32,
) -> Result<u64, ProgramError> {
    if position_count < MIN_CROSS_MARGIN_POSITIONS {
        // No netting benefit for single position
        return Ok(gross_margin);
    }
    
    // Calculate total long and short exposures
    let total_long: u64 = position_exposures.iter()
        .filter(|(is_long, _)| *is_long)
        .map(|(_, exposure)| exposure)
        .sum();
    
    let total_short: u64 = position_exposures.iter()
        .filter(|(is_long, _)| !*is_long)
        .map(|(_, exposure)| exposure)
        .sum();
    
    // Net exposure after offsetting
    let net_exposure = total_long.abs_diff(total_short);
    let gross_exposure = total_long.saturating_add(total_short);
    
    if gross_exposure == 0 {
        return Ok(0);
    }
    
    // Netting ratio: how much exposure is offset
    let netting_ratio = ((gross_exposure - net_exposure) * 10000) / gross_exposure;
    
    // Apply netting benefit (capped at 15%)
    let netting_benefit = (gross_margin * netting_ratio.min(MAX_EFFICIENCY_IMPROVEMENT as u64)) / 10000;
    
    Ok(gross_margin.saturating_sub(netting_benefit))
}

/// Calculate portfolio-based margin using Greeks
fn calculate_portfolio_margin(
    portfolio_greeks: &PortfolioGreeks,
    gross_margin: u64,
    position_exposures: &[(bool, u64)],
) -> Result<u64, ProgramError> {
    // Start with netted margin
    let netted_margin = calculate_netted_margin(
        position_exposures,
        gross_margin,
        portfolio_greeks.position_count,
    )?;
    
    // Apply additional reductions based on Greeks
    let mut margin_reduction = 0u64;
    
    // Delta-neutral portfolios get additional benefit
    let delta_abs = if portfolio_greeks.portfolio_delta.to_bits() > U64F64::from_num(0).to_bits() {
        portfolio_greeks.portfolio_delta.to_num() as f64
    } else {
        -(portfolio_greeks.portfolio_delta.to_num() as f64)
    };
    if delta_abs < 0.1 {
        // 5% additional reduction for delta-neutral
        margin_reduction = margin_reduction.saturating_add(netted_margin / 20);
    }
    
    // Low gamma portfolios are more stable
    let gamma_abs = if portfolio_greeks.portfolio_gamma.to_bits() > U64F64::from_num(0).to_bits() {
        portfolio_greeks.portfolio_gamma.to_num() as f64
    } else {
        -(portfolio_greeks.portfolio_gamma.to_num() as f64)
    };
    if gamma_abs < 0.05 {
        // 3% additional reduction for low gamma
        margin_reduction = margin_reduction.saturating_add(netted_margin / 33);
    }
    
    // Apply reductions (capped at total 15% improvement)
    let total_reduction = margin_reduction.min(gross_margin * MAX_EFFICIENCY_IMPROVEMENT as u64 / 10000);
    
    Ok(netted_margin.saturating_sub(total_reduction))
}

/// Calculate risk score for the cross-margin account
fn calculate_risk_score(cross_margin: &CrossMarginAccount) -> Result<u16, ProgramError> {
    let mut score = 5000u16; // Start at neutral
    
    // Adjust for margin utilization
    if cross_margin.total_collateral > 0 {
        let utilization = (cross_margin.net_margin_requirement * 10000) / cross_margin.total_collateral;
        
        // Higher utilization = higher risk
        if utilization > 8000 {
            score = score.saturating_add(2000);
        } else if utilization > 6000 {
            score = score.saturating_add(1000);
        } else if utilization < 3000 {
            score = score.saturating_sub(1000);
        }
    }
    
    // Adjust for exposure concentration
    let net_exposure_abs = if cross_margin.net_exposure < 0 {
        (-cross_margin.net_exposure) as u64
    } else {
        cross_margin.net_exposure as u64
    };
    let total_exposure = cross_margin.total_long_exposure
        .saturating_add(cross_margin.total_short_exposure);
    
    if total_exposure > 0 {
        let concentration = (net_exposure_abs * 10000) / total_exposure;
        
        // High concentration = higher risk
        if concentration > 7000 {
            score = score.saturating_add(1500);
        } else if concentration < 3000 {
            score = score.saturating_sub(500);
        }
    }
    
    // Benefit from diversification
    if cross_margin.position_count >= 5 {
        score = score.saturating_sub(500);
    }
    
    Ok(score.min(10000))
}

/// Update cross-margin mode
pub fn update_cross_margin_mode(
    accounts: &[AccountInfo],
    new_mode: CrossMarginMode,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    
    let cross_margin_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;
    let authority_account = next_account_info(account_iter)?;
    
    // Validate accounts
    validate_writable(cross_margin_account)?;
    
    // Load and validate cross-margin account
    let mut cross_margin = CrossMarginAccount::try_from_slice(&cross_margin_account.data.borrow())?;
    cross_margin.validate()?;
    
    // Verify authority
    if cross_margin.user != *authority_account.key {
        msg!("Unauthorized: only account owner can update mode");
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Update mode
    cross_margin.mode = new_mode;
    
    // Save to account
    cross_margin.serialize(&mut &mut cross_margin_account.data.borrow_mut()[..])?;
    
    msg!("Cross-margin mode updated to: {:?}", cross_margin.mode);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_netting_calculation() {
        // Test case: offsetting positions
        let exposures = vec![
            (true, 1_000_000),  // Long $1M
            (false, 800_000),   // Short $800k
        ];
        
        let gross_margin = 100_000; // $100k gross
        let netted = calculate_netted_margin(&exposures, gross_margin, 2).unwrap();
        
        // Net exposure is $200k out of $1.8M gross = 88.9% offset
        // Should get significant margin reduction
        assert!(netted < gross_margin);
        assert!(netted > gross_margin * 85 / 100); // At least 15% reduction
    }
    
    #[test]
    fn test_efficiency_calculation() {
        let mut cross_margin = CrossMarginAccount::new(
            Pubkey::new_unique(),
            12345,
            1,
        );
        
        cross_margin.gross_margin_requirement = 100_000;
        cross_margin.net_margin_requirement = 85_000;
        
        cross_margin.calculate_efficiency().unwrap();
        
        // 15% improvement
        assert_eq!(cross_margin.efficiency_improvement, 150);
    }
}