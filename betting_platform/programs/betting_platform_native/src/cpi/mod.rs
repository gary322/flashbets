//! Cross-Program Invocation (CPI) integration layer
//!
//! This module provides safe wrappers for interacting with other Solana programs

pub mod spl_token;
// pub mod spl_token_2022; // Temporarily disabled due to stack size issues
pub mod system_program;
pub mod associated_token;
pub mod depth_tracker;
pub mod marinade;
pub mod position_nft;
pub mod jupiter;
pub mod raydium;

pub use depth_tracker::CPIDepthTracker;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

/// Common CPI helper functions
pub mod helpers {
    use super::*;
    
    /// Create a new account with rent-exempt balance
    pub fn create_account<'a>(
        payer: &AccountInfo<'a>,
        new_account: &AccountInfo<'a>,
        lamports: u64,
        space: u64,
        owner: &Pubkey,
        system_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        system_program::create_account(
            payer,
            new_account,
            lamports,
            space,
            owner,
            system_program,
        )
    }
    
    /// Transfer lamports between accounts
    pub fn transfer<'a>(
        from: &AccountInfo<'a>,
        to: &AccountInfo<'a>,
        lamports: u64,
        system_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        system_program::transfer(from, to, lamports, system_program)
    }
    
    /// Create and initialize a token mint
    pub fn create_mint<'a>(
        payer: &AccountInfo<'a>,
        mint: &AccountInfo<'a>,
        mint_authority: &Pubkey,
        freeze_authority: Option<&Pubkey>,
        decimals: u8,
        token_program: &AccountInfo<'a>,
        rent_sysvar: &AccountInfo<'a>,
        system_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        let mut depth_tracker = CPIDepthTracker::new();
        spl_token::create_mint(
            payer,
            mint,
            mint_authority,
            freeze_authority,
            decimals,
            token_program,
            rent_sysvar,
            system_program,
            &mut depth_tracker,
        )
    }
    
    /// Create a token account
    pub fn create_token_account<'a>(
        payer: &AccountInfo<'a>,
        token_account: &AccountInfo<'a>,
        mint: &AccountInfo<'a>,
        owner: &AccountInfo<'a>,
        token_program: &AccountInfo<'a>,
        rent_sysvar: &AccountInfo<'a>,
        system_program: &AccountInfo<'a>,
    ) -> ProgramResult {
        spl_token::create_token_account(
            payer,
            token_account,
            mint,
            owner,
            token_program,
            rent_sysvar,
            system_program,
        )
    }
    
    /// Transfer SPL tokens
    pub fn transfer_tokens<'a>(
        source: &AccountInfo<'a>,
        destination: &AccountInfo<'a>,
        authority: &AccountInfo<'a>,
        amount: u64,
        token_program: &AccountInfo<'a>,
        signer_seeds: &[&[&[u8]]],
    ) -> ProgramResult {
        spl_token::transfer(
            source,
            destination,
            authority,
            amount,
            token_program,
            signer_seeds,
        )
    }
}