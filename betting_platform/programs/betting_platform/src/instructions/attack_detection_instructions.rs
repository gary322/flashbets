use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::state::*;
use crate::attack_detection::*;
use crate::errors::*;
use crate::fixed_types::U64F64;

#[derive(Accounts)]
pub struct InitializeAttackDetector<'info> {
    #[account(
        init,
        payer = authority,
        space = AttackDetector::LEN,
        seeds = [b"attack_detector", verse.key().as_ref()],
        bump
    )]
    pub attack_detector: Account<'info, AttackDetector>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ProcessTrade<'info> {
    #[account(
        mut,
        seeds = [b"attack_detector", verse.key().as_ref()],
        bump
    )]
    pub attack_detector: Account<'info, AttackDetector>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(
        constraint = vault.mint == verse.usdc_mint @ crate::errors::ErrorCode::InvalidInput
    )]
    pub vault: Account<'info, TokenAccount>,
    
    pub trader: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UpdateVolumeBaseline<'info> {
    #[account(
        mut,
        seeds = [b"attack_detector", verse.key().as_ref()],
        bump
    )]
    pub attack_detector: Account<'info, AttackDetector>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(
        constraint = authority.key() == verse.authority @ crate::errors::ErrorCode::Unauthorized
    )]
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct ResetDetector<'info> {
    #[account(
        mut,
        seeds = [b"attack_detector", verse.key().as_ref()],
        bump
    )]
    pub attack_detector: Account<'info, AttackDetector>,
    
    pub verse: Account<'info, Verse>,
    
    #[account(
        constraint = authority.key() == verse.authority @ crate::errors::ErrorCode::Unauthorized
    )]
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

// Instruction implementations
pub fn initialize_attack_detector(ctx: Context<InitializeAttackDetector>) -> Result<()> {
    let detector = &mut ctx.accounts.attack_detector;
    let clock = &ctx.accounts.clock;
    
    detector.init(clock)?;
    
    msg!("Attack detector initialized for verse: {}", ctx.accounts.verse.key());
    
    Ok(())
}

pub fn process_trade(
    ctx: Context<ProcessTrade>,
    market_id: [u8; 32],
    size: u64,
    price: u64, // Fixed point representation
    leverage: u64,
    is_buy: bool,
) -> Result<()> {
    let detector = &mut ctx.accounts.attack_detector;
    let clock = &ctx.accounts.clock;
    let vault_size = ctx.accounts.vault.amount;
    
    // Create trade snapshot
    let trade = TradeSnapshot {
        trader: ctx.accounts.trader.key(),
        market_id,
        size,
        price: U64F64::from_num(price) / U64F64::from_num(1_000_000),
        leverage,
        slot: clock.slot,
        is_buy,
    };
    
    // Process trade for attack detection
    let alerts = detector.process_trade(&trade, vault_size, clock)?;
    
    // Log any security alerts
    for alert in &alerts {
        msg!("SECURITY ALERT: {:?}", alert.alert_type);
        msg!("Severity: {:?}", alert.severity);
        msg!("Message: {}", alert.message);
        msg!("Action: {:?}", alert.action);
        
        // Emit event
        emit!(SecurityAlertEvent {
            alert_type: alert.alert_type,
            severity: alert.severity,
            trader: ctx.accounts.trader.key(),
            market_id,
            slot: clock.slot,
            action: alert.action,
        });
    }
    
    // Check if we should halt trading based on alerts
    if alerts.iter().any(|a| a.action == SecurityAction::HaltTrading) {
        return Err(crate::errors::ErrorCode::AttackDetected.into());
    }
    
    Ok(())
}

pub fn update_volume_baseline(
    ctx: Context<UpdateVolumeBaseline>,
    new_avg_volume: u64,
    new_std_dev: u64,
) -> Result<()> {
    let detector = &mut ctx.accounts.attack_detector;
    
    detector.volume_detector.avg_volume_7d = new_avg_volume;
    detector.volume_detector.volume_std_dev = 
        U64F64::from_num(new_std_dev) / U64F64::from_num(1_000_000);
    
    msg!("Volume baseline updated");
    msg!("New average: {}", new_avg_volume);
    msg!("New std dev: {}", new_std_dev);
    
    Ok(())
}

pub fn reset_detector(ctx: Context<ResetDetector>) -> Result<()> {
    let detector = &mut ctx.accounts.attack_detector;
    let clock = &ctx.accounts.clock;
    
    // Reset risk level and clear patterns
    detector.risk_level = 0;
    detector.detected_patterns.clear();
    detector.recent_trades.clear();
    
    // Reset detectors
    detector.price_tracker.violation_count = 0;
    detector.price_tracker.price_changes.clear();
    detector.flash_loan_detector.detected_attempts = 0;
    detector.wash_trade_detector.wash_trades_detected = 0;
    detector.wash_trade_detector.trader_activity.clear();
    
    detector.last_update_slot = clock.slot;
    
    msg!("Attack detector reset");
    
    Ok(())
}

// Events
#[event]
pub struct SecurityAlertEvent {
    pub alert_type: AlertType,
    pub severity: AttackSeverity,
    pub trader: Pubkey,
    pub market_id: [u8; 32],
    pub slot: u64,
    pub action: SecurityAction,
}