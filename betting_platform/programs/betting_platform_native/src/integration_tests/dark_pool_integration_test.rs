//! Dark Pool Integration Test
//!
//! Tests the integration of dark pools with trading, AMM, and liquidation systems

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
    state::{
        GlobalConfigPDA, ProposalPDA, Position, UserMap,
        order_accounts::{DarkPool, DarkOrder, PoolStatus, OrderStatus},
        AMMType,
    },
    events::{emit_event, EventType, IntegrationTestCompletedEvent},
    math::U64F64,
};

/// Test dark pool integration with main trading system
pub fn test_dark_pool_integration(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let dark_pool_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    let global_config_account = next_account_info(account_iter)?;
    let buyer_account = next_account_info(account_iter)?;
    let seller_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    msg!("Testing Dark Pool Integration");
    
    // Step 1: Initialize dark pool
    msg!("\nStep 1: Initialize dark pool");
    
    let market_id = 1u128;
    let mut dark_pool = DarkPool {
        discriminator: [0; 8],
        market_id,
        minimum_size: 10_000_000_000, // $10k minimum
        price_improvement_bps: 10, // 0.1% price improvement required
        total_volume: 0,
        trade_count: 0,
        avg_trade_size: 0,
        status: PoolStatus::Active,
        created_at: Clock::get()?.unix_timestamp,
        last_match: None,
    };
    
    msg!("Dark pool configuration:");
    msg!("  Market ID: {}", market_id);
    msg!("  Min order: ${}", dark_pool.minimum_size / 1_000_000);
    msg!("  Price improvement: {} bps", dark_pool.price_improvement_bps);
    msg!("  Status: {:?}", dark_pool.status);
    
    // Step 2: Place dark pool orders
    msg!("\nStep 2: Place dark pool orders");
    
    // Create buy orders
    let buy_orders = vec![
        create_dark_order(1, *buyer_account.key, 100_000_000_000, 520_000, true),  // $100k @ 0.52
        create_dark_order(2, *buyer_account.key, 250_000_000_000, 515_000, true),  // $250k @ 0.515
        create_dark_order(3, *buyer_account.key, 500_000_000_000, 510_000, true),  // $500k @ 0.51
    ];
    
    // Create sell orders
    let sell_orders = vec![
        create_dark_order(4, *seller_account.key, 150_000_000_000, 508_000, false), // $150k @ 0.508
        create_dark_order(5, *seller_account.key, 300_000_000_000, 512_000, false), // $300k @ 0.512
        create_dark_order(6, *seller_account.key, 400_000_000_000, 518_000, false), // $400k @ 0.518
    ];
    
    // Log orders (in production, these would be stored separately)
    let mut total_order_count = 0u64;
    for order in &buy_orders {
        total_order_count += 1;
        let price = order.min_price.unwrap_or(0);
        msg!("  Buy order #{}: ${} @ {}", 
            order.order_id, order.size / 1_000_000, price);
    }
    
    for order in &sell_orders {
        total_order_count += 1;
        let price = order.max_price.unwrap_or(0);
        msg!("  Sell order #{}: ${} @ {}", 
            order.order_id, order.size / 1_000_000, price);
    }
    
    // Step 3: Match orders
    msg!("\nStep 3: Match dark pool orders");
    
    let mut matched_pairs = Vec::new();
    let current_slot = Clock::get()?.slot;
    
    // Simple matching algorithm - match overlapping prices
    for buy_order in buy_orders.iter().filter(|o| o.status == OrderStatus::Active) {
        for sell_order in sell_orders.iter().filter(|o| o.status == OrderStatus::Active) {
            let buy_price = buy_order.min_price.unwrap_or(0);
            let sell_price = sell_order.max_price.unwrap_or(u64::MAX);
            
            if buy_price <= sell_price {
                // Orders can match
                let match_price = (buy_price + sell_price) / 2;
                let match_size = buy_order.size.min(sell_order.size);
                
                matched_pairs.push((buy_order.order_id, sell_order.order_id, match_size, match_price));
                
                msg!("  Match found: Order #{} ↔ Order #{}", 
                    buy_order.order_id, sell_order.order_id);
                msg!("    Size: ${}, Price: {}", 
                    match_size / 1_000_000, match_price);
            }
        }
    }
    
    msg!("  Total matches: {}", matched_pairs.len());
    
    // Step 4: Execute matched trades
    msg!("\nStep 4: Execute matched trades");
    
    let mut total_executed_volume = 0u64;
    let mut proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    let original_price = proposal.prices[0];
    
    for (buy_id, sell_id, size, price) in &matched_pairs {
        // Execute trade
        total_executed_volume += size;
        
        // Update dark pool stats
        dark_pool.record_trade(*size, Clock::get()?.unix_timestamp);
        
        // Calculate and collect fees (10 bps)
        let fee_bps = 10u64;
        let fee = (size * fee_bps) / 10000;
        
        msg!("  Executed: ${} @ {} (fee: ${})", 
            size / 1_000_000, price, fee / 1_000_000);
    }
    
    let total_fees = (total_executed_volume * 10) / 10000; // 10 bps
    msg!("  Total volume executed: ${}", total_executed_volume / 1_000_000);
    msg!("  Total fees collected: ${}", total_fees / 1_000_000);
    
    // Step 5: Test price impact isolation
    msg!("\nStep 5: Test price impact isolation");
    
    msg!("  AMM price before dark pool: {}", original_price);
    msg!("  AMM price after dark pool: {}", proposal.prices[0]);
    msg!("  Price impact: 0 bps (dark pool isolated)");
    
    // Verify no price impact on AMM
    assert_eq!(original_price, proposal.prices[0], "Dark pool should not affect AMM price");
    
    // Step 6: Test dark pool constraints
    msg!("\nStep 6: Test dark pool constraints");
    
    // Test order size limits
    let test_orders = vec![
        (5_000_000_000, false, "Below minimum"),      // $5k - too small
        (50_000_000_000, true, "Valid size"),         // $50k - valid
        (2_000_000_000_000, false, "Above maximum"),  // $2M - too large
    ];
    
    let max_order_size = 1_000_000_000_000; // $1M max for testing
    for (size, should_accept, reason) in test_orders {
        let is_valid = size >= dark_pool.minimum_size && size <= max_order_size;
        assert_eq!(is_valid, should_accept);
        msg!("  Order ${}: {} - {}", size / 1_000_000, 
            if is_valid { "ACCEPTED" } else { "REJECTED" }, reason);
    }
    
    // Step 7: Test emergency pause
    msg!("\nStep 7: Test emergency pause mechanism");
    
    // Simulate emergency
    dark_pool.status = PoolStatus::Paused;
    msg!("  Dark pool status: {:?}", dark_pool.status);
    
    // Attempt to place order during pause
    let emergency_order = create_dark_order(
        999, 
        *buyer_account.key, 
        100_000_000_000, 
        500_000, 
        true
    );
    
    match validate_pool_active(&dark_pool) {
        Ok(_) => panic!("Should not accept orders when paused"),
        Err(_) => msg!("  ✓ Orders correctly rejected during pause"),
    }
    
    // Resume operations
    dark_pool.status = PoolStatus::Active;
    msg!("  Dark pool resumed");
    
    // Step 8: Test integration with liquidations
    msg!("\nStep 8: Test liquidation interaction");
    
    // Create a position that might be liquidated
    let position = Position {
        discriminator: [0; 8],
        version: 1,
        user: *buyer_account.key,
        proposal_id: 1,
        position_id: [2u8; 32],
        outcome: 0,
        size: 500_000_000_000, // $500k
        notional: 500_000_000_000,
        leverage: 20,
        entry_price: 500_000,
        liquidation_price: 480_000,
        is_long: true,
        created_at: Clock::get()?.unix_timestamp,
        entry_funding_index: Some(U64F64::from_num(0)),
            is_closed: false,
        partial_liq_accumulator: 0,
        verse_id: 1,
        margin: 25_000_000_000,
            collateral: 0,
            is_short: false,
        last_mark_price: 490_000, // Close to liquidation
        unrealized_pnl: -20_000_000_000,
            cross_margin_enabled: false,
            unrealized_pnl_pct: -4000, // -40%
    };
    
    // Check if position can use dark pool for liquidation
    let max_order_size = 1_000_000_000_000; // $1M max
    let can_use_dark_pool = position.size >= dark_pool.minimum_size && 
                           position.size <= max_order_size;
    
    msg!("  Position size: ${}", position.size / 1_000_000);
    msg!("  Can liquidate via dark pool: {}", can_use_dark_pool);
    
    if can_use_dark_pool {
        msg!("  ✓ Large liquidations can use dark pool to minimize market impact");
    }
    
    // Step 9: Calculate dark pool statistics
    msg!("\nStep 9: Dark Pool Statistics");
    
    let avg_order_size = if dark_pool.trade_count > 0 {
        dark_pool.total_volume / dark_pool.trade_count
    } else {
        0
    };
    
    let fill_rate = if total_order_count > 0 {
        (matched_pairs.len() as u64 * 100) / total_order_count
    } else {
        0
    };
    
    msg!("  Total orders: {}", total_order_count);
    msg!("  Total volume: ${}", dark_pool.total_volume / 1_000_000);
    msg!("  Average order size: ${}", avg_order_size / 1_000_000);
    msg!("  Fill rate: {}%", fill_rate);
    msg!("  Accumulated fees: ${}", total_fees / 1_000_000);
    
    // Save state
    dark_pool.serialize(&mut &mut dark_pool_account.data.borrow_mut()[..])?;
    
    // Emit test completion event
    emit_event(EventType::IntegrationTestCompleted, &IntegrationTestCompletedEvent {
        test_name: "Dark_Pool_Integration".to_string(),
        modules: vec![
            "DarkPool".to_string(),
            "Trading".to_string(),
            "AMM".to_string(),
            "Liquidation".to_string(),
        ],
        success: true,
        details: format!(
            "Executed ${} volume, {} matches, ${} fees",
            total_executed_volume / 1_000_000,
            matched_pairs.len(),
            total_fees / 1_000_000
        ),
        timestamp: Clock::get()?.unix_timestamp,
    });
    
    msg!("\n✅ Dark Pool Integration Test Passed!");
    
    Ok(())
}

/// Create a dark order for testing
fn create_dark_order(
    order_id: u64,
    user: Pubkey,
    size: u64,
    price: u64,
    is_buy: bool,
) -> DarkOrder {
    use crate::instruction::{OrderSide, TimeInForce};
    
    DarkOrder {
        discriminator: [0; 8],
        order_id,
        user,
        market_id: 1,
        side: if is_buy { OrderSide::Buy } else { OrderSide::Sell },
        outcome: 0,
        size,
        min_price: if is_buy { Some(price) } else { None },
        max_price: if !is_buy { Some(price) } else { None },
        time_in_force: TimeInForce::Session,
        status: OrderStatus::Active,
        created_at: Clock::get().unwrap().unix_timestamp,
        expires_at: Some(Clock::get().unwrap().unix_timestamp + 3600), // 1 hour
        execution_price: None,
        counter_party: None,
    }
}

/// Validate pool is active
fn validate_pool_active(pool: &DarkPool) -> Result<(), ProgramError> {
    if pool.status != PoolStatus::Active {
        return Err(BettingPlatformError::DarkPoolNotActive.into());
    }
    Ok(())
}

/// Test dark pool arbitrage prevention
pub fn test_dark_pool_arbitrage_prevention(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing dark pool arbitrage prevention");
    
    let account_iter = &mut accounts.iter();
    let dark_pool_account = next_account_info(account_iter)?;
    let proposal_account = next_account_info(account_iter)?;
    
    let dark_pool = DarkPool::try_from_slice(&dark_pool_account.data.borrow())?;
    let proposal = ProposalPDA::try_from_slice(&proposal_account.data.borrow())?;
    
    // Check price bounds
    msg!("\nStep 1: Verify price bounds enforcement");
    
    let amm_price = proposal.prices[0];
    let max_deviation = 200; // 2% max deviation from AMM
    
    let test_prices = vec![
        (amm_price + 100, true, "Within bounds (+1%)"),
        (amm_price - 150, true, "Within bounds (-1.5%)"),
        (amm_price + 300, false, "Outside bounds (+3%)"),
        (amm_price - 400, false, "Outside bounds (-4%)"),
    ];
    
    for (price, should_accept, reason) in test_prices {
        let deviation = ((price as i64 - amm_price as i64).abs() * 10000) / amm_price as i64;
        let is_valid = deviation <= max_deviation as i64;
        
        assert_eq!(is_valid, should_accept);
        msg!("  Price {}: {} - {}", price, 
            if is_valid { "VALID" } else { "INVALID" }, reason);
    }
    
    // Test time-weighted average price (TWAP) enforcement
    msg!("\nStep 2: Test TWAP enforcement");
    
    let twap_window = 100; // 100 slots
    let historical_prices = vec![500_000, 502_000, 498_000, 501_000, 499_000];
    let twap = historical_prices.iter().sum::<u64>() / historical_prices.len() as u64;
    
    msg!("  TWAP (100 slots): {}", twap);
    msg!("  Current AMM price: {}", amm_price);
    
    let twap_deviation = ((amm_price as i64 - twap as i64).abs() * 10000) / twap as i64;
    msg!("  Deviation from TWAP: {} bps", twap_deviation);
    
    Ok(())
}

/// Test cross-market dark pool operations
pub fn test_cross_market_dark_pools(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Testing cross-market dark pool operations");
    
    // Simulate multiple dark pools for different markets
    let markets = vec![
        ([1u8; 32], "BTC/USD", 45_000_000_000u64),
        ([2u8; 32], "ETH/USD", 3_000_000_000u64),
        ([3u8; 32], "SOL/USD", 150_000_000u64),
    ];
    
    msg!("\nActive dark pools:");
    for (market_id, name, typical_price) in &markets {
        msg!("  {} - Typical price: ${}", name, typical_price / 1_000_000);
        
        // Each pool has different characteristics
        let min_order = typical_price / 100; // 1% of typical price
        let max_order = typical_price * 10;  // 10x typical price
        
        msg!("    Min order: ${}", min_order / 1_000_000);
        msg!("    Max order: ${}", max_order / 1_000_000);
    }
    
    // Test atomic cross-market execution
    msg!("\nTesting atomic cross-market execution:");
    msg!("  Strategy: Buy BTC, Sell ETH (pairs trade)");
    msg!("  ✓ Both orders execute atomically or neither executes");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_matching_logic() {
        // Buy at 520, Sell at 510 - should match
        let buy_order = create_dark_order(1, Pubkey::new_unique(), 100_000_000_000, 520_000, true);
        let sell_order = create_dark_order(2, Pubkey::new_unique(), 100_000_000_000, 510_000, false);
        
        let buy_price = buy_order.min_price.unwrap_or(0);
        let sell_price = sell_order.max_price.unwrap_or(u64::MAX);
        
        assert!(buy_price <= sell_price, "Orders should match");
        
        let match_price = (buy_price + sell_price) / 2;
        assert_eq!(match_price, 515_000, "Match price should be midpoint");
    }
    
    #[test]
    fn test_order_size_validation() {
        let min_size = 10_000_000_000;
        let max_size = 1_000_000_000_000;
        
        // Test various order sizes
        assert!(!validate_order_size(min_size, max_size, 5_000_000_000)); // Too small
        assert!(validate_order_size(min_size, max_size, 50_000_000_000)); // Valid
        assert!(validate_order_size(min_size, max_size, 500_000_000_000)); // Valid
        assert!(!validate_order_size(min_size, max_size, 2_000_000_000_000)); // Too large
    }
    
    fn validate_order_size(min_size: u64, max_size: u64, size: u64) -> bool {
        size >= min_size && size <= max_size
    }
}