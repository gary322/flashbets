use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use crate::state::*;

// Re-export the Initialize struct type alias
pub type GlobalConfig = GlobalConfigPDA;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 8 + 16 + 16 + 8 + 1 + 8 + 8,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeGenesis<'info> {
    #[account(mut)]
    pub global_config: Account<'info, GlobalConfig>,
    
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}