use anchor_lang::prelude::*;

declare_id!("Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4");

#[program]
pub mod betting_platform {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, seed: u128) -> Result<()> {
        msg!("Initialize called with seed: {}", seed);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}