use anchor_lang::prelude::*;
use crate::errors::*;
use crate::events::*;
use crate::state::*;

#[derive(Accounts)]
#[instruction(verse_id: u128, market_id: String)]
pub struct ProcessResolution<'info> {
    #[account(
        init,
        payer = authority,
        space = ResolutionPDA::LEN,
        seeds = [b"resolution", verse_id.to_le_bytes().as_ref(), market_id.as_bytes()],
        bump
    )]
    pub resolution: Account<'info, ResolutionPDA>,
    
    #[account(
        mut,
        seeds = [b"verse", verse_id.to_le_bytes().as_ref()],
        bump
    )]
    pub verse: Account<'info, VersePDA>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn process_resolution(
    ctx: Context<ProcessResolution>,
    verse_id: u128,
    market_id: String,
    resolution_outcome: String,
) -> Result<()> {
    let resolution = &mut ctx.accounts.resolution;
    let verse = &mut ctx.accounts.verse;
    let clock = &ctx.accounts.clock;
    
    // Validate market_id length
    require!(
        market_id.len() <= 64,
        BettingPlatformError::InvalidInput
    );
    
    // Validate resolution outcome length
    require!(
        resolution_outcome.len() <= 32,
        BettingPlatformError::InvalidInput
    );
    
    // Initialize resolution data
    resolution.verse_id = verse_id;
    resolution.market_id = market_id.clone();
    resolution.resolution = resolution_outcome.clone();
    resolution.resolved_at = clock.unix_timestamp;
    resolution.resolver = ctx.accounts.authority.key();
    resolution.is_disputed = false;
    resolution.dispute_deadline = clock.unix_timestamp + 86400; // 24 hour dispute window
    
    // Update verse status to resolved (can be disputed later)
    verse.status = VerseStatus::Resolved;
    
    emit!(ProposalResolvedEvent {
        proposal_id: verse_id,
        winning_outcome: resolution_outcome,
        resolver: ctx.accounts.authority.key(),
        resolution_slot: clock.slot,
    });
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(verse_id: u128, market_id: String)]
pub struct InitiateDispute<'info> {
    #[account(
        mut,
        seeds = [b"resolution", verse_id.to_le_bytes().as_ref(), market_id.as_bytes()],
        bump
    )]
    pub resolution: Account<'info, ResolutionPDA>,
    
    #[account(
        mut,
        seeds = [b"verse", verse_id.to_le_bytes().as_ref()],
        bump
    )]
    pub verse: Account<'info, VersePDA>,
    
    #[account(mut)]
    pub disputer: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

pub fn initiate_dispute(
    ctx: Context<InitiateDispute>,
    verse_id: u128,
    market_id: String,
) -> Result<()> {
    let resolution = &mut ctx.accounts.resolution;
    let verse = &mut ctx.accounts.verse;
    let clock = &ctx.accounts.clock;
    
    // Check if still within dispute window
    require!(
        clock.unix_timestamp <= resolution.dispute_deadline,
        BettingPlatformError::ProposalExpired
    );
    
    // Mark as disputed
    resolution.is_disputed = true;
    verse.status = VerseStatus::Disputed;
    
    emit!(DisputeEvent {
        verse_id,
        market_id: market_id.clone(),
        disputed: true,
        slot: clock.slot,
    });
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(verse_id: u128, market_id: String)]
pub struct ResolveDispute<'info> {
    #[account(
        mut,
        seeds = [b"resolution", verse_id.to_le_bytes().as_ref(), market_id.as_bytes()],
        bump
    )]
    pub resolution: Account<'info, ResolutionPDA>,
    
    #[account(
        mut,
        seeds = [b"verse", verse_id.to_le_bytes().as_ref()],
        bump
    )]
    pub verse: Account<'info, VersePDA>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

pub fn resolve_dispute(
    ctx: Context<ResolveDispute>,
    verse_id: u128,
    market_id: String,
    final_resolution: String,
) -> Result<()> {
    let resolution = &mut ctx.accounts.resolution;
    let verse = &mut ctx.accounts.verse;
    let clock = &ctx.accounts.clock;
    
    // Ensure dispute was initiated
    require!(
        resolution.is_disputed,
        BettingPlatformError::InvalidProposalStatus
    );
    
    // Update resolution
    resolution.resolution = final_resolution.clone();
    resolution.is_disputed = false;
    resolution.resolved_at = clock.unix_timestamp;
    resolution.resolver = ctx.accounts.authority.key();
    
    // Update verse status back to resolved
    verse.status = VerseStatus::Resolved;
    
    emit!(DisputeEvent {
        verse_id,
        market_id,
        disputed: false,
        slot: clock.slot,
    });
    
    emit!(ProposalResolvedEvent {
        proposal_id: verse_id,
        winning_outcome: final_resolution,
        resolver: ctx.accounts.authority.key(),
        resolution_slot: clock.slot,
    });
    
    Ok(())
}

pub fn mirror_dispute(
    ctx: Context<InitiateDispute>,
    market_id: String,
    disputed: bool,
) -> Result<()> {
    let verse = &mut ctx.accounts.verse;
    let clock = &ctx.accounts.clock;
    
    if disputed {
        verse.status = VerseStatus::Disputed;
        
        msg!("Verse {} frozen due to Polymarket dispute", verse.verse_id_as_u128());
    } else {
        // Dispute resolved - unfreeze
        verse.status = VerseStatus::Active;
    }
    
    emit!(DisputeEvent {
        verse_id: verse.verse_id_as_u128(),
        market_id,
        disputed,
        slot: clock.slot,
    });
    
    Ok(())
}