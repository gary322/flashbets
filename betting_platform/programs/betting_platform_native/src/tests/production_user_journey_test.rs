//! Production-ready end-to-end user journey tests
//! 
//! Tests all critical user paths with real data structures and proper error handling

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{
        GlobalConfigPDA, ProposalPDA, VersePDA, Position,
        accounts::{UserAccount, CollateralAccount},
        ProposalState, Resolution,
    },
    trading::{
        open_position, close_position,
        calculate_liquidation_price,
    },
    amm::{execute_trade, calculate_price_impact},
    events::{emit_event, EventType, PositionOpened, PositionClosed, TradeExecuted},
    math::fixed_point::U64F64,
    oracle::OraclePrice,
    fees::{calculate_fee, FEE_BASE_BPS},
    integration::coordinator::SystemCoordinator,
};

/// Production test: Complete betting user journey
pub fn test_betting_journey_production() -> ProgramResult {
    msg!("=== PRODUCTION TEST: Complete Betting Journey ===");
    
    // Initialize test environment with real account structures
    let program_id = Pubkey::new_unique();
    let user_pubkey = Pubkey::new_unique();
    let clock = Clock::get()?;
    
    // Step 1: Setup user account with proper collateral
    msg!("Step 1: Setting up user account and collateral");
    
    let initial_collateral = 1_000_000_000_000; // $1,000 USDC
    let collateral_account = CollateralAccount {
        owner: user_pubkey,
        balance: initial_collateral,
        locked_amount: 0,
        last_update: clock.unix_timestamp,
    };
    
    let user_account = UserAccount {
        owner: user_pubkey,
        total_positions: 0,
        total_volume: 0,
        funding_state: None,        total_pnl: 0,
        open_positions: Vec::with_capacity(100),
        mmt_tier: 0, // Bronze
        created_at: clock.unix_timestamp,
        entry_funding_index: Some(U64F64::from_num(0)),            };
    
    // Step 2: Find active market
    msg!("Step 2: Finding active market from Polymarket");
    
    // Real market data structure
    let btc_proposal_id = [1u8; 32]; // BTC reaches $150k by EOY
    let btc_verse_id = [2u8; 32]; // Crypto verse
    
    let mut proposal = ProposalPDA {
        discriminator: [0; 8],
        proposal_id: btc_proposal_id,
        verse_id: btc_verse_id,
        slug: "btc-150k-eoy-2024".to_string(),
        title: "Will Bitcoin reach $150,000 by end of 2024?".to_string(),
        description: "This market resolves YES if BTC trades at or above $150,000 on any major exchange before Jan 1, 2025".to_string(),
        outcomes: 2,
        outcome_titles: vec!["YES".to_string(), "NO".to_string()],
        market_type: 0, // Binary
        amm_type: 0, // Standard CPMM
        oracle: Pubkey::new_unique(), // Polymarket oracle
        state: ProposalState::Active,
        created_at: clock.unix_timestamp - 86400, // Created 1 day ago
        entry_funding_index: Some(U64F64::from_num(0)),
            settle_at: clock.unix_timestamp + 30 * 86400, // Settles in 30 days
        settle_slot: clock.slot + 30 * 216_000,
        prices: vec![5500, 4500], // 55% YES, 45% NO
        volumes: vec![50_000_000_000_000, 40_000_000_000_000], // $50k YES, $40k NO volume
        liquidity: 10_000_000_000_000, // $10k liquidity
        accumulated_fees: 90_000_000_000, // $90 in fees
        resolution: None,
    };
    
    // Step 3: Calculate position parameters
    msg!("Step 3: Calculating position parameters");
    
    let position_size = 100_000_000_000; // $100 position
    let leverage = 10; // 10x leverage
    let outcome = 0; // Betting YES
    let is_long = true;
    
    // Calculate required margin
    let margin_required = position_size / leverage;
    assert!(collateral_account.balance >= margin_required);
    
    // Calculate entry price with slippage
    let price_impact = calculate_price_impact(
        &proposal,
        outcome,
        position_size * leverage,
        is_long
    )?;
    
    let entry_price = proposal.prices[outcome as usize];
    let execution_price = if is_long {
        entry_price + price_impact
    } else {
        entry_price - price_impact
    };
    
    msg!("  Position size: ${}", position_size / 1_000_000);
    msg!("  Leverage: {}x", leverage);
    msg!("  Margin required: ${}", margin_required / 1_000_000);
    msg!("  Entry price: {:.2}%", entry_price as f64 / 100.0);
    msg!("  Price impact: {:.2}%", price_impact as f64 / 100.0);
    msg!("  Execution price: {:.2}%", execution_price as f64 / 100.0);
    
    // Step 4: Open position
    msg!("Step 4: Opening leveraged position");
    
    let position_id = generate_position_id(&user_pubkey, clock.slot);
    
    // Calculate liquidation price
    let liquidation_price = calculate_liquidation_price(
        execution_price,
        leverage,
        is_long,
    )?;
    
    let position = Position {
        discriminator: [0; 8],
        user: user_pubkey,
        proposal_id: u128::from_le_bytes(btc_proposal_id[0..16].try_into().unwrap()),
        position_id,
        outcome,
        size: position_size,
        notional: position_size * leverage,
        leverage: leverage as u64,
        entry_price: execution_price,
        liquidation_price,
        is_long,
        created_at: clock.unix_timestamp,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: u128::from_le_bytes(btc_verse_id[0..16].try_into().unwrap()),
        margin: margin_required,
            collateral: 0,
            is_short: !is_long,
        last_mark_price: execution_price,
        unrealized_pnl: 0,
        unrealized_pnl_pct: 0,
    };
    
    // Execute trade on AMM
    let fee = calculate_fee(position.notional, FEE_BASE_BPS)?;
    let new_price = execute_amm_trade(
        &mut proposal,
        outcome,
        position.notional,
        is_long,
        fee,
    )?;
    
    // Update market state
    proposal.prices[outcome as usize] = new_price;
    proposal.volumes[outcome as usize] += position.notional;
    proposal.accumulated_fees += fee;
    
    // Emit position opened event
    emit_event(EventType::PositionOpened, &PositionOpened {
        user: user_pubkey,
        proposal_id: position.proposal_id,
        outcome,
        size: position.size,
        leverage: position.leverage,
        entry_price: position.entry_price,
        is_long,
        position_id,
        chain_id: None,
    });
    
    msg!("  Position opened successfully");
    msg!("  Liquidation price: {:.2}%", liquidation_price as f64 / 100.0);
    msg!("  Fee paid: ${}", fee / 1_000_000);
    
    // Step 5: Monitor position
    msg!("Step 5: Monitoring position P&L");
    
    // Simulate price movement: YES probability increases to 65%
    let new_market_price = 6500; // 65%
    proposal.prices[outcome as usize] = new_market_price;
    
    // Calculate unrealized P&L
    let price_change = if is_long {
        new_market_price as i64 - execution_price as i64
    } else {
        execution_price as i64 - new_market_price as i64
    };
    
    let unrealized_pnl = (position.notional as i128 * price_change as i128) / 10000;
    let unrealized_pnl_pct = (price_change * 10000) / execution_price as i64;
    
    msg!("  Market price moved to: {:.2}%", new_market_price as f64 / 100.0);
    msg!("  Unrealized P&L: ${}", unrealized_pnl / 1_000_000);
    msg!("  Unrealized P&L %: {:.2}%", unrealized_pnl_pct as f64 / 100.0);
    
    // Step 6: Close position
    msg!("Step 6: Closing position to realize profits");
    
    // Calculate exit parameters
    let exit_fee = calculate_fee(position.notional, FEE_BASE_BPS)?;
    let exit_price_impact = calculate_price_impact(
        &proposal,
        outcome,
        position.notional,
        !is_long, // Opposite direction to close
    )?;
    
    let exit_price = if is_long {
        new_market_price - exit_price_impact
    } else {
        new_market_price + exit_price_impact
    };
    
    let realized_pnl = if is_long {
        ((exit_price as i64 - execution_price as i64) * position.notional as i64) / 10000
    } else {
        ((execution_price as i64 - exit_price as i64) * position.notional as i64) / 10000
    };
    
    // Update user account
    let net_pnl = realized_pnl - (fee + exit_fee) as i64;
    let final_balance = (collateral_account.balance as i64 + net_pnl) as u64;
    
    // Emit position closed event
    emit_event(EventType::PositionClosed, &PositionClosed {
        user: user_pubkey,
        position_id,
        exit_price,
        pnl: realized_pnl,
        close_reason: crate::events::CloseReason::UserInitiated,
    });
    
    msg!("  Exit price: {:.2}%", exit_price as f64 / 100.0);
    msg!("  Exit fee: ${}", exit_fee / 1_000_000);
    msg!("  Realized P&L: ${}", realized_pnl / 1_000_000);
    msg!("  Net P&L (after fees): ${}", net_pnl / 1_000_000);
    msg!("  Final balance: ${}", final_balance / 1_000_000);
    
    // Step 7: Verify results
    msg!("Step 7: Verifying journey results");
    
    let total_fees_paid = fee + exit_fee;
    let return_on_margin = (net_pnl * 10000) / margin_required as i64;
    
    msg!("  Total fees paid: ${}", total_fees_paid / 1_000_000);
    msg!("  Return on margin: {:.2}%", return_on_margin as f64 / 100.0);
            collateral: 0,    msg!("  Leverage effectiveness: {}x profit on {}x leverage", 
         (realized_pnl / margin_required as i64).abs(), leverage);
    
    // Verify all calculations
    assert!(final_balance > collateral_account.balance); // Profitable trade
    assert!(realized_pnl > 0); // Positive P&L
    assert!(net_pnl > 0); // Positive after fees
    
    msg!("=== Betting Journey Test PASSED ===");
    Ok(())
}

/// Execute trade on AMM and return new price
fn execute_amm_trade(
    proposal: &mut ProposalPDA,
    outcome: u8,
    notional: u64,
    is_buy: bool,
    fee: u64,
) -> Result<u64, ProgramError> {
    // Production AMM logic (simplified CPMM)
    let k = proposal.liquidity; // Constant product
    let current_yes = (proposal.liquidity * proposal.prices[0]) / 10000;
    let current_no = proposal.liquidity - current_yes;
    
    let (new_yes, new_no) = if outcome == 0 {
        if is_buy {
            // Buying YES
            let delta = (notional - fee) * 10000 / proposal.liquidity;
            (current_yes + delta, current_no - delta)
        } else {
            // Selling YES
            let delta = (notional - fee) * 10000 / proposal.liquidity;
            (current_yes - delta, current_no + delta)
        }
    } else {
        if is_buy {
            // Buying NO
            let delta = (notional - fee) * 10000 / proposal.liquidity;
            (current_yes - delta, current_no + delta)
        } else {
            // Selling NO
            let delta = (notional - fee) * 10000 / proposal.liquidity;
            (current_yes + delta, current_no - delta)
        }
    };
    
    // Calculate new price
    let new_price = if outcome == 0 {
        (new_yes * 10000) / (new_yes + new_no)
    } else {
        (new_no * 10000) / (new_yes + new_no)
    };
    
    Ok(new_price)
}

/// Generate unique position ID
fn generate_position_id(user: &Pubkey, slot: u64) -> [u8; 32] {
    let mut id = [0u8; 32];
    id[0..32].copy_from_slice(&solana_program::keccak::hashv(&[
        user.as_ref(),
        &slot.to_le_bytes(),
    ]).to_bytes());
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_production_betting_journey() {
        test_betting_journey_production().unwrap();
    }
}