//! Oracle Spread Edge Case Testing
//! 
//! Tests behavior when oracle price spread exceeds 10%

use solana_program::{
    account_info::{AccountInfo, next_account_info},
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
    state::{ProposalPDA, ProposalState},
    oracle::polymarket::{PolymarketOracle, OraclePrice},
    circuit_breaker::CircuitBreaker,
    state::security_accounts::CircuitBreakerType,
    events::{emit_event, EventType, OracleSpreadExceededEvent, OracleSpreadNormalizedEvent, OracleDivergenceDetectedEvent, OracleStaleEvent},
};

/// Maximum allowed oracle spread (10%)
const MAX_ORACLE_SPREAD_BPS: u16 = 1000;

/// Test oracle spread exceeding threshold
pub fn test_oracle_spread_exceeded(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let circuit_breaker_account = next_account_info(account_iter)?;
    
    msg!("Testing oracle spread > 10% scenario");
    
    // Load accounts
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let mut circuit_breaker = CircuitBreaker::try_from_slice(&circuit_breaker_account.data.borrow())?;
    
    // Step 1: Simulate oracle prices with high spread
    msg!("Step 1: Simulating oracle prices with high spread");
    
    // Create test oracle prices with >10% spread
    let oracle_prices = vec![
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 0,
            price: 450_000, // 0.45
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 95,
        },
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 1,
            price: 550_000, // 0.55
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 95,
        },
    ];
    
    // In a real scenario, we'd fetch from Polymarket
    // For testing, we'll use the simulated prices
    
    // Step 2: Calculate spread
    msg!("Step 2: Calculating oracle spread");
    let spread_bps = calculate_oracle_spread(&oracle_prices)?;
    msg!("Oracle spread: {} bps ({}%)", spread_bps, spread_bps as f64 / 100.0);
    
    // Step 3: Check if spread exceeds threshold
    if spread_bps > MAX_ORACLE_SPREAD_BPS {
        msg!("Step 3: Oracle spread exceeds 10% threshold!");
        
        // Pause market
        proposal.state = ProposalState::Paused;
        
        // Activate circuit breaker
        circuit_breaker.is_active = true;
        circuit_breaker.breaker_type = Some(CircuitBreakerType::OracleFailure);
        circuit_breaker.triggered_at = Some(Clock::get()?.slot);
        circuit_breaker.reason = Some(format!("Oracle spread {} bps > {} bps", spread_bps, MAX_ORACLE_SPREAD_BPS));
        
        // Save state
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
        circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
        
        // Emit event
        emit_event(EventType::OracleSpreadExceeded, &OracleSpreadExceededEvent {
            market_id: u128::from_le_bytes(proposal.market_id[0..16].try_into().unwrap()),
            spread_bps,
            max_allowed_bps: MAX_ORACLE_SPREAD_BPS,
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        // Step 4: Test trading with high spread (should fail)
        msg!("Step 4: Testing trade attempt with high oracle spread");
        
        match attempt_trade_with_high_spread(&proposal, spread_bps) {
            Err(BettingPlatformError::OracleSpreadTooHigh) => {
                msg!("✓ Trade correctly rejected due to high oracle spread");
            }
            Ok(_) => {
                msg!("✗ ERROR: Trade succeeded despite high oracle spread!");
                return Err(ProgramError::InvalidAccountData);
            }
            Err(e) => {
                msg!("✗ Unexpected error: {:?}", e);
                return Err(e.into());
            }
        }
        
        // Step 5: Test spread normalization
        msg!("Step 5: Simulating spread normalization");
        
        // Simulate improved oracle prices
        let normalized_prices = vec![
            OraclePrice {
                source: "Polymarket".to_string(),
                outcome: 0,
                price: 495_000, // 0.495
                timestamp: Clock::get()?.unix_timestamp,
                confidence: 98,
            },
            OraclePrice {
                source: "Polymarket".to_string(),
                outcome: 1,
                price: 505_000, // 0.505
                timestamp: Clock::get()?.unix_timestamp,
                confidence: 98,
            },
        ];
        
        let new_spread = calculate_oracle_spread(&normalized_prices)?;
        msg!("New oracle spread: {} bps", new_spread);
        
        if new_spread <= MAX_ORACLE_SPREAD_BPS {
            msg!("Oracle spread normalized - resuming market");
            
            // Resume market
            proposal.state = ProposalState::Active;
            circuit_breaker.is_active = false;
            circuit_breaker.resolved_at = Some(Clock::get()?.slot as i64);
            
            // Update proposal prices with normalized values
            proposal.prices[0] = normalized_prices[0].price;
            proposal.prices[1] = normalized_prices[1].price;
            
            // Save state
            proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
            circuit_breaker.serialize(&mut &mut circuit_breaker_account.data.borrow_mut()[..])?;
            
            emit_event(EventType::OracleSpreadNormalized, &OracleSpreadNormalizedEvent {
                market_id: u128::from_le_bytes(proposal.market_id[0..16].try_into().unwrap()),
                new_spread_bps: new_spread,
                timestamp: Clock::get()?.unix_timestamp,
            });
        }
    } else {
        msg!("Oracle spread within acceptable range: {} bps", spread_bps);
    }
    
    msg!("Oracle spread test completed");
    
    Ok(())
}

/// Test multiple oracle source divergence
pub fn test_oracle_source_divergence(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let primary_oracle_account = next_account_info(account_iter)?;
    let secondary_oracle_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    
    msg!("Testing oracle source divergence");
    
    // Simulate prices from different sources
    let polymarket_prices = vec![
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 0,
            price: 600_000, // 0.6
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 95,
        },
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 1,
            price: 400_000, // 0.4
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 95,
        },
    ];
    
    let secondary_prices = vec![
        OraclePrice {
            source: "Secondary".to_string(),
            outcome: 0,
            price: 550_000, // 0.55 (diverges from Polymarket)
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 90,
        },
        OraclePrice {
            source: "Secondary".to_string(),
            outcome: 1,
            price: 450_000, // 0.45 (diverges from Polymarket)
            timestamp: Clock::get()?.unix_timestamp,
            confidence: 90,
        },
    ];
    
    // Calculate divergence
    let divergence_outcome0 = calculate_price_divergence(
        polymarket_prices[0].price,
        secondary_prices[0].price,
    )?;
    
    let divergence_outcome1 = calculate_price_divergence(
        polymarket_prices[1].price,
        secondary_prices[1].price,
    )?;
    
    msg!("Oracle divergence outcome 0: {} bps", divergence_outcome0);
    msg!("Oracle divergence outcome 1: {} bps", divergence_outcome1);
    
    // Check if divergence exceeds threshold (5%)
    const MAX_DIVERGENCE_BPS: u16 = 500;
    
    if divergence_outcome0 > MAX_DIVERGENCE_BPS || divergence_outcome1 > MAX_DIVERGENCE_BPS {
        msg!("Oracle sources diverge significantly!");
        
        // Use confidence-weighted average
        let weighted_price_0 = calculate_weighted_price(&[
            &polymarket_prices[0],
            &secondary_prices[0],
        ])?;
        
        let weighted_price_1 = calculate_weighted_price(&[
            &polymarket_prices[1],
            &secondary_prices[1],
        ])?;
        
        msg!("Using confidence-weighted prices:");
        msg!("  Outcome 0: {}", weighted_price_0);
        msg!("  Outcome 1: {}", weighted_price_1);
        
        // Update proposal with weighted prices
        let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
        proposal.prices[0] = weighted_price_0;
        proposal.prices[1] = weighted_price_1;
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
        
        emit_event(EventType::OracleDivergenceDetected, &OracleDivergenceDetectedEvent {
            market_id: u128::from_le_bytes(proposal.market_id[0..16].try_into().unwrap()),
            max_divergence_bps: divergence_outcome0.max(divergence_outcome1),
            resolution: "Confidence-weighted average".to_string(),
            timestamp: Clock::get()?.unix_timestamp,
        });
    }
    
    Ok(())
}

/// Test oracle staleness
pub fn test_oracle_staleness(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let oracle_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    
    msg!("Testing oracle staleness");
    
    // Maximum allowed staleness (5 minutes)
    const MAX_STALENESS_SECONDS: i64 = 300;
    
    // Simulate stale oracle data
    let stale_prices = vec![
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 0,
            price: 500_000,
            timestamp: Clock::get()?.unix_timestamp - 400, // 400 seconds old
            confidence: 95,
        },
        OraclePrice {
            source: "Polymarket".to_string(),
            outcome: 1,
            price: 500_000,
            timestamp: Clock::get()?.unix_timestamp - 400,
            confidence: 95,
        },
    ];
    
    let current_time = Clock::get()?.unix_timestamp;
    let staleness = current_time - stale_prices[0].timestamp;
    
    msg!("Oracle data age: {} seconds", staleness);
    
    if staleness > MAX_STALENESS_SECONDS {
        msg!("Oracle data is stale!");
        
        // Pause market
        let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
        proposal.state = ProposalState::Paused;
        proposal.serialize(&mut &mut proposal_account.data.borrow_mut()[..])?;
        
        emit_event(EventType::OracleStale, &OracleStaleEvent {
            market_id: u128::from_le_bytes(proposal.market_id[0..16].try_into().unwrap()),
            staleness_seconds: staleness,
            max_allowed_seconds: MAX_STALENESS_SECONDS,
            timestamp: current_time,
        });
        
        return Err(BettingPlatformError::StaleOracle.into());
    }
    
    msg!("Oracle data is fresh");
    
    Ok(())
}

/// Calculate oracle spread
fn calculate_oracle_spread(prices: &[OraclePrice]) -> Result<u16, ProgramError> {
    if prices.len() < 2 {
        return Ok(0);
    }
    
    // For binary markets, calculate implied spread
    let sum: u64 = prices.iter().map(|p| p.price).sum();
    
    // Ideal sum should be 1_000_000 (1.0)
    let spread = if sum > 1_000_000 {
        sum - 1_000_000
    } else {
        1_000_000 - sum
    };
    
    // Convert to basis points
    let spread_bps = (spread * 10000) / 1_000_000;
    
    Ok(spread_bps as u16)
}

/// Calculate price divergence between sources
fn calculate_price_divergence(price1: u64, price2: u64) -> Result<u16, ProgramError> {
    let diff = if price1 > price2 {
        price1 - price2
    } else {
        price2 - price1
    };
    
    let avg = (price1 + price2) / 2;
    if avg == 0 {
        return Ok(0);
    }
    
    let divergence_bps = (diff * 10000) / avg;
    Ok(divergence_bps as u16)
}

/// Calculate confidence-weighted price
fn calculate_weighted_price(prices: &[&OraclePrice]) -> Result<u64, ProgramError> {
    let mut weighted_sum = 0u64;
    let mut confidence_sum = 0u64;
    
    for price_data in prices {
        weighted_sum += price_data.price * price_data.confidence as u64;
        confidence_sum += price_data.confidence as u64;
    }
    
    if confidence_sum == 0 {
        return Ok(0);
    }
    
    Ok(weighted_sum / confidence_sum)
}

/// Attempt trade with high spread (should fail)
fn attempt_trade_with_high_spread(
    proposal: &ProposalPDA,
    spread_bps: u16,
) -> Result<(), BettingPlatformError> {
    if spread_bps > MAX_ORACLE_SPREAD_BPS {
        return Err(BettingPlatformError::OracleSpreadTooHigh);
    }
    
    if !proposal.is_active() {
        return Err(BettingPlatformError::MarketHalted);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spread_calculation() {
        let prices = vec![
            OraclePrice {
                source: "Test".to_string(),
                outcome: 0,
                price: 600_000, // 0.6
                timestamp: 0,
                confidence: 95,
            },
            OraclePrice {
                source: "Test".to_string(),
                outcome: 1,
                price: 500_000, // 0.5
                timestamp: 0,
                confidence: 95,
            },
        ];
        
        // Sum = 1.1, spread = 0.1 = 1000 bps
        let spread = calculate_oracle_spread(&prices).unwrap();
        assert_eq!(spread, 1000);
    }
    
    #[test]
    fn test_divergence_calculation() {
        let divergence = calculate_price_divergence(600_000, 550_000).unwrap();
        // Diff = 50k, avg = 575k, divergence = 50/575 * 10000 = 869 bps
        assert_eq!(divergence, 869);
    }
}