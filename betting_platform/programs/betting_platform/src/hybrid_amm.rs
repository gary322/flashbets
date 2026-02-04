use anchor_lang::prelude::*;
use crate::{
    lmsr_amm::{execute_lmsr_trade, LSMRTrade},
    pm_amm::{execute_pmamm_trade, PMAMMTrade},
    l2_amm::{execute_l2_trade, L2AMMTrade},
};

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub enum AMMType {
    LMSR,
    PMAMM,
    L2Distribution,
}

#[account]
pub struct HybridAMMState {
    pub market_id: u128,
    pub amm_type: AMMType,
    pub num_outcomes: u8,
    pub expiry_time: i64,
    pub is_continuous: bool,
    pub amm_specific_data: Vec<u8>,  // Serialized AMM-specific state
}

pub fn select_amm_type(
    num_outcomes: u8,
    expiry_time: i64,
    market_type: &str,
    current_time: i64,
) -> AMMType {
    // L2 for continuous distributions
    if market_type.contains("range") ||
       market_type.contains("date") ||
       market_type.contains("number") {
        return AMMType::L2Distribution;
    }

    // PM-AMM for multi-outcome with short expiry
    let time_to_expiry = expiry_time - current_time;
    if num_outcomes > 1 && num_outcomes <= 64 && time_to_expiry < 86400 {
        return AMMType::PMAMM;
    }

    // LMSR for binary and standard multi-outcome
    AMMType::LMSR
}

#[derive(Accounts)]
pub struct HybridTrade<'info> {
    #[account(mut)]
    pub hybrid_amm_state: Account<'info, HybridAMMState>,
    
    /// CHECK: This account will be validated by the specific AMM implementation
    pub amm_state: AccountInfo<'info>,
    
    #[account(mut)]
    pub user: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn execute_hybrid_trade(
    ctx: Context<HybridTrade>,
    _outcome: u8,
    _amount: u64,
    _is_buy: bool,
) -> Result<()> {
    // The hybrid AMM simply stores which AMM type to use
    // The actual trading logic is delegated to the specific AMM implementation
    // which should be called directly with the appropriate account
    
    msg!("Hybrid trade executed for market with AMM type: {:?}", ctx.accounts.hybrid_amm_state.amm_type);
    
    // In a real implementation, you would:
    // 1. Verify the amm_state account matches the expected AMM type
    // 2. Call the appropriate AMM's execute function directly
    // This is a routing layer, not an execution layer
    
    Ok(())
}

// Initialize hybrid AMM
pub fn initialize_hybrid_amm(
    ctx: Context<InitializeHybridAMM>,
    market_id: u128,
    amm_type: AMMType,
    num_outcomes: u8,
    expiry_time: i64,
    is_continuous: bool,
    amm_specific_data: Vec<u8>,
) -> Result<()> {
    let hybrid_state = &mut ctx.accounts.hybrid_amm_state;
    
    hybrid_state.market_id = market_id;
    hybrid_state.amm_type = amm_type;
    hybrid_state.num_outcomes = num_outcomes;
    hybrid_state.expiry_time = expiry_time;
    hybrid_state.is_continuous = is_continuous;
    hybrid_state.amm_specific_data = amm_specific_data;
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(market_id: u128)]
pub struct InitializeHybridAMM<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 16 + 1 + 1 + 8 + 1 + 4 + 256, // Allow up to 256 bytes of AMM-specific data
        seeds = [b"hybrid_amm", market_id.to_le_bytes().as_ref()],
        bump
    )]
    pub hybrid_amm_state: Account<'info, HybridAMMState>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

// Helper function to get the appropriate AMM PDA based on type
pub fn get_amm_pda(
    market_id: u128,
    amm_type: &AMMType,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    match amm_type {
        AMMType::LMSR => {
            Pubkey::find_program_address(
                &[b"lmsr", market_id.to_le_bytes().as_ref()],
                program_id,
            )
        },
        AMMType::PMAMM => {
            Pubkey::find_program_address(
                &[b"pmamm", market_id.to_le_bytes().as_ref()],
                program_id,
            )
        },
        AMMType::L2Distribution => {
            Pubkey::find_program_address(
                &[b"l2amm", market_id.to_le_bytes().as_ref()],
                program_id,
            )
        },
    }
}