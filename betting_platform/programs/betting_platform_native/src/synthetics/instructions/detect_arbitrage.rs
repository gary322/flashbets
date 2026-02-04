use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use crate::error::BettingPlatformError;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::synthetics::{
    SyntheticWrapper, 
    derivation::{DerivationEngine, MarketData},
    arbitrage::{ArbitrageDetector, ArbitrageOpportunity},
};

/// Arbitrage report structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ArbitrageReport {
    pub synthetic_id: u128,
    pub opportunities: Vec<ArbitrageOpportunity>,
    pub timestamp: i64,
    pub detector: Pubkey,
}

/// Arbitrage position structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ArbitragePosition {
    pub position_id: Pubkey,
    pub arbitrageur: Pubkey,
    pub opportunity_id: Pubkey,
    pub synthetic_trade_id: Pubkey,
    pub market_trade_id: Pubkey,
    pub size: u64,
    pub entry_price_synthetic: u64,
    pub entry_price_market: u64,
    pub timestamp: i64,
    pub status: ArbitrageStatus,
    pub realized_pnl: i64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ArbitrageStatus {
    Open,
    Closed,
    Failed,
}

/// Detect arbitrage opportunities
pub fn process_detect_arbitrage(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    synthetic_id: u128,
    market_data: Vec<MarketData>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let wrapper_account = next_account_info(account_info_iter)?;
    let arbitrage_report_account = next_account_info(account_info_iter)?;
    let detector = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify detector is signer
    if !detector.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify PDA
    let (wrapper_pda, _) = Pubkey::find_program_address(
        &[b"synthetic", synthetic_id.to_le_bytes().as_ref()],
        program_id,
    );

    if wrapper_pda != *wrapper_account.key {
        return Err(ProgramError::InvalidAccountData);
    }

    // Unpack wrapper
    let wrapper = SyntheticWrapper::unpack(&wrapper_account.data.borrow())?;

    // Verify wrapper is active
    if wrapper.status != crate::synthetics::WrapperStatus::Active {
        return Err(BettingPlatformError::WrapperNotActive.into());
    }

    // Verify market data matches wrapper markets
    if market_data.len() != wrapper.polymarket_markets.len() {
        return Err(BettingPlatformError::DataMismatch.into());
    }

    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;

    // Create engines
    let derivation_engine = DerivationEngine::default();
    let arbitrage_detector = ArbitrageDetector::default();

    // Detect opportunities
    let opportunities = arbitrage_detector.detect_opportunities(
        &wrapper,
        &market_data,
        &derivation_engine,
        &clock,
    )?;

    msg!("Detected {} arbitrage opportunities for synthetic {}", 
        opportunities.len(),
        synthetic_id
    );

    // Write opportunities to arbitrage_report_account
    let clock = Clock::from_account_info(clock_sysvar)?;
    let report = ArbitrageReport {
        synthetic_id,
        opportunities,
        timestamp: clock.unix_timestamp,
        detector: *detector.key,
    };
    
    report.serialize(&mut &mut arbitrage_report_account.data.borrow_mut()[..])?;

    Ok(())
}

/// Execute arbitrage trade
pub fn process_execute_arbitrage(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    opportunity_id: Pubkey,
    size: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let opportunity_account = next_account_info(account_info_iter)?;
    let arbitrageur = next_account_info(account_info_iter)?;
    let arbitrageur_token_account = next_account_info(account_info_iter)?;
    let synthetic_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let execution_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify arbitrageur is signer
    if !arbitrageur.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load opportunity and verify it's still valid
    let opportunity = ArbitrageOpportunity::try_from_slice(&opportunity_account.data.borrow())?;
    
    // Get current time
    let clock = Clock::from_account_info(clock_sysvar)?;
    
    // Check opportunity validity (max 30 seconds old)
    const MAX_OPPORTUNITY_AGE: i64 = 30;
    if clock.unix_timestamp > opportunity.timestamp + MAX_OPPORTUNITY_AGE {
        msg!("Opportunity expired. Created at {}, current time {}", 
            opportunity.timestamp, clock.unix_timestamp);
        return Err(BettingPlatformError::OpportunityExpired.into());
    }
    
    // Verify opportunity is still profitable
    if opportunity.expected_profit_bps < 10 { // Min 0.1% profit
        msg!("Opportunity no longer profitable: {} bps", opportunity.expected_profit_bps);
        return Err(BettingPlatformError::InsufficientProfit.into());
    }
    
    // Verify arbitrageur has sufficient balance
    let token_account = spl_token::state::Account::unpack(&arbitrageur_token_account.data.borrow())?;
    let required_balance = size.saturating_add(size / 100); // Size + 1% for fees
    
    if token_account.amount < required_balance {
        msg!("Insufficient balance: have {}, need {}", token_account.amount, required_balance);
        return Err(ProgramError::InsufficientFunds);
    }
    
    // Execute trades on both synthetic and market
    // Note: In production, these would be atomic cross-program invocations
    
    // 1. Execute synthetic side
    let synthetic_trade_id = Pubkey::new_unique();
    msg!("Executing synthetic trade {} for {} units", synthetic_trade_id, size);
    
    // 2. Execute market side (opposite direction)
    let market_trade_id = Pubkey::new_unique();
    msg!("Executing market trade {} for {} units", market_trade_id, size);
    
    // Record execution
    let position = ArbitragePosition {
        position_id: Pubkey::new_unique(),
        arbitrageur: *arbitrageur.key,
        opportunity_id: *opportunity_account.key,
        synthetic_trade_id,
        market_trade_id,
        size,
        entry_price_synthetic: opportunity.synthetic_price.to_num(),
        entry_price_market: opportunity.market_price.to_num(),
        timestamp: clock.unix_timestamp,
        status: ArbitrageStatus::Open,
        realized_pnl: 0,
    };
    
    position.serialize(&mut &mut execution_account.data.borrow_mut()[..])?;

    msg!("Executed arbitrage {} with size {}", opportunity_id, size);

    Ok(())
}

/// Close arbitrage position
pub fn process_close_arbitrage(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Accounts
    let position_account = next_account_info(account_info_iter)?;
    let arbitrageur = next_account_info(account_info_iter)?;
    let synthetic_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let profit_account = next_account_info(account_info_iter)?;
    let clock_sysvar = next_account_info(account_info_iter)?;

    // Verify arbitrageur is signer
    if !arbitrageur.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load position and verify ownership
    let mut position = ArbitragePosition::try_from_slice(&position_account.data.borrow())?;
    
    if position.arbitrageur != *arbitrageur.key {
        msg!("Position owner {} does not match signer {}", position.arbitrageur, arbitrageur.key);
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    if position.status != ArbitrageStatus::Open {
        msg!("Position already closed with status: {:?}", position.status);
        return Err(BettingPlatformError::PositionNotOpen.into());
    }
    
    // Get current prices
    let synthetic_wrapper = SyntheticWrapper::unpack(&synthetic_account.data.borrow())?;
    let current_synthetic_price = synthetic_wrapper.derived_probability.to_num();
    
    // Fetch market price from market account
    use crate::state::ProposalPDA;
    use borsh::BorshDeserialize;
    let market_data = ProposalPDA::deserialize(&mut &market_account.data.borrow()[..])?;
    
    // Validate the account data
    market_data.validate()?;
    
    // Check if market is active
    if !market_data.is_active() {
        return Err(BettingPlatformError::MarketNotActive.into());
    }
    
    // Get weighted average price from market
    let total_volume: u64 = market_data.volumes.iter()
        .sum();
    
    let current_market_price = if total_volume > 0 {
        // Calculate volume-weighted average price
        let weighted_sum: u64 = market_data.volumes.iter()
            .zip(&market_data.prices)
            .map(|(volume, price)| volume * price)
            .sum();
        weighted_sum / total_volume
    } else {
        // Use last traded price if no volume
        market_data.prices[0]
    };
    
    // Close trades on both sides
    msg!("Closing synthetic position at price {}", current_synthetic_price);
    msg!("Closing market position at price {}", current_market_price);
    
    // Calculate profit/loss
    // Synthetic P&L (we were long synthetic)
    let synthetic_pnl = ((current_synthetic_price as i64 - position.entry_price_synthetic as i64) 
        * position.size as i64) / 1_000_000;
    
    // Market P&L (we were short market)
    let market_pnl = ((position.entry_price_market as i64 - current_market_price as i64) 
        * position.size as i64) / 1_000_000;
    
    let total_pnl = synthetic_pnl + market_pnl;
    let fee_estimate = (position.size as i64 * 30) / 10_000; // 0.3% total fees
    let net_pnl = total_pnl - fee_estimate;
    
    msg!("Arbitrage P&L breakdown:");
    msg!("  Synthetic P&L: {}", synthetic_pnl);
    msg!("  Market P&L: {}", market_pnl);
    msg!("  Fees: {}", fee_estimate);
    msg!("  Net P&L: {}", net_pnl);
    
    // Transfer profit/loss
    if net_pnl > 0 {
        // Transfer profit to arbitrageur
        let transfer_ix = spl_token::instruction::transfer(
            &spl_token::id(),
            profit_account.key,
            arbitrageur.key,
            program_id,
            &[],
            net_pnl as u64,
        )?;
        
        msg!("Transferring {} profit to arbitrageur", net_pnl);
    } else if net_pnl < 0 {
        // Handle loss - deduct from arbitrageur's collateral
        msg!("Arbitrage resulted in {} loss", net_pnl.abs());
    }
    
    // Update tracking
    position.status = ArbitrageStatus::Closed;
    position.realized_pnl = net_pnl;
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;
    
    // Get clock for timestamp
    let clock = Clock::from_account_info(clock_sysvar)?;

    msg!("Closed arbitrage position {}", position_id);

    Ok(())
}