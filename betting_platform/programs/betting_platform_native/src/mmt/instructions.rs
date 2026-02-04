//! MMT Token Instructions
//! 
//! All instruction definitions for the MMT token system
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::mmt::state::DistributionType;

/// MMT instruction types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum MMTInstruction {
    /// Initialize the MMT token system
    /// 
    /// Accounts expected:
    /// 0. `[writable]` MMT config account (PDA)
    /// 1. `[writable]` MMT mint account (PDA)
    /// 2. `[writable]` Season emission account (PDA)
    /// 3. `[writable]` Treasury account (PDA)
    /// 4. `[writable]` Treasury token account (PDA)
    /// 5. `[writable]` Reserved vault account (PDA)
    /// 6. `[writable]` Reserved vault token account (PDA)
    /// 7. `[signer]` Authority
    /// 8. `[]` System program
    /// 9. `[]` Token program
    /// 10. `[]` Rent sysvar
    /// 11. `[]` Clock sysvar
    InitializeMMT,

    /// Lock the reserved vault permanently
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Reserved vault account (PDA)
    /// 1. `[writable]` Reserved vault token account
    /// 2. `[signer]` Authority
    /// 3. `[]` System program
    /// 4. `[]` Token program
    LockReservedVault,

    /// Initialize the staking pool
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Staking pool account (PDA)
    /// 1. `[writable]` Stake vault token account (PDA)
    /// 2. `[]` MMT mint
    /// 3. `[signer]` Authority
    /// 4. `[]` System program
    /// 5. `[]` Token program
    /// 6. `[]` Rent sysvar
    InitializeStakingPool,

    /// Stake MMT tokens
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Stake account (PDA)
    /// 1. `[writable]` Staking pool account
    /// 2. `[writable]` User token account (source)
    /// 3. `[writable]` Stake vault token account (destination)
    /// 4. `[]` MMT mint
    /// 5. `[signer]` Staker
    /// 6. `[]` System program
    /// 7. `[]` Token program
    /// 8. `[]` Clock sysvar
    /// 9. `[]` Rent sysvar
    StakeMMT {
        amount: u64,
        lock_period_slots: Option<u64>,
    },

    /// Unstake MMT tokens
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Stake account (PDA)
    /// 1. `[writable]` Staking pool account
    /// 2. `[writable]` User token account (destination)
    /// 3. `[writable]` Stake vault token account (source)
    /// 4. `[]` Staking pool PDA (vault authority)
    /// 5. `[signer]` Staker
    /// 6. `[]` Token program
    /// 7. `[]` Clock sysvar
    UnstakeMMT {
        amount: u64,
    },

    /// Distribute trading fees to stakers
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Staking pool account
    /// 1. `[writable]` Fee collection token account (source)
    /// 2. `[writable]` Stake vault token account (destination)
    /// 3. `[signer]` Authority
    /// 4. `[]` Token program
    /// 5. `[]` Clock sysvar
    DistributeTradingFees {
        total_fees: u64,
    },

    /// Initialize a maker account
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Maker account (PDA)
    /// 1. `[signer]` Maker
    /// 2. `[]` System program
    /// 3. `[]` Rent sysvar
    InitializeMakerAccount,

    /// Record a maker trade and calculate rewards
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Maker account (PDA)
    /// 1. `[writable]` Season emission account
    /// 2. `[]` Early trader registry (optional)
    /// 3. `[signer]` Maker
    /// 4. `[]` Clock sysvar
    RecordMakerTrade {
        notional: u64,
        spread_improvement_bp: u16,
    },

    /// Claim accumulated maker rewards
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Maker account (PDA)
    /// 1. `[]` Treasury account
    /// 2. `[writable]` Treasury token account (source)
    /// 3. `[writable]` Maker token account (destination)
    /// 4. `[signer]` Maker
    /// 5. `[]` Token program
    ClaimMakerRewards,

    /// Distribute MMT tokens from treasury
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Season emission account
    /// 1. `[]` MMT config account
    /// 2. `[writable]` Distribution record account (PDA)
    /// 3. `[]` Treasury account
    /// 4. `[writable]` Treasury token account (source)
    /// 5. `[writable]` Recipient token account (destination)
    /// 6. `[signer]` Authority
    /// 7. `[]` System program
    /// 8. `[]` Token program
    /// 9. `[]` Clock sysvar
    /// 10. `[]` Rent sysvar
    DistributeEmission {
        distribution_type: DistributionType,
        amount: u64,
        distribution_id: u64,
    },

    /// Transition to the next season
    /// 
    /// Accounts expected:
    /// 0. `[writable]` MMT config account
    /// 1. `[]` Current season emission account
    /// 2. `[writable]` Next season emission account (PDA)
    /// 3. `[signer]` Authority
    /// 4. `[]` System program
    /// 5. `[]` Clock sysvar
    /// 6. `[]` Rent sysvar
    TransitionSeason,

    /// Initialize early trader registry for a season
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Early trader registry account (PDA)
    /// 1. `[]` Season emission account
    /// 2. `[signer]` Authority
    /// 3. `[]` System program
    /// 4. `[]` Rent sysvar
    InitializeEarlyTraderRegistry {
        season: u8,
    },

    /// Register a trader as an early trader
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Early trader registry account
    /// 1. `[writable]` Maker account (may be uninitialized)
    /// 2. `[signer]` Trader
    /// 3. `[]` System program
    /// 4. `[]` Clock sysvar
    /// 5. `[]` Rent sysvar
    RegisterEarlyTrader {
        season: u8,
    },

    /// Update treasury balance
    /// 
    /// Accounts expected:
    /// 0. `[writable]` Treasury account
    /// 1. `[]` Treasury token account
    /// 2. `[signer]` Authority
    UpdateTreasuryBalance,
}

impl MMTInstruction {
    /// Unpack instruction data
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let instruction = Self::try_from_slice(input)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
        Ok(instruction)
    }

    /// Pack instruction data
    pub fn pack(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

/// Create InitializeMMT instruction
pub fn initialize_mmt(
    program_id: &Pubkey,
    mmt_config: &Pubkey,
    mmt_mint: &Pubkey,
    season_emission: &Pubkey,
    treasury: &Pubkey,
    treasury_token: &Pubkey,
    reserved_vault: &Pubkey,
    reserved_vault_token: &Pubkey,
    authority: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*mmt_config, false),
            AccountMeta::new(*mmt_mint, false),
            AccountMeta::new(*season_emission, false),
            AccountMeta::new(*treasury, false),
            AccountMeta::new(*treasury_token, false),
            AccountMeta::new(*reserved_vault, false),
            AccountMeta::new(*reserved_vault_token, false),
            AccountMeta::new(*authority, true),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::clock::id(), false),
        ],
        data: MMTInstruction::InitializeMMT.pack(),
    }
}

/// Create StakeMMT instruction
pub fn stake_mmt(
    program_id: &Pubkey,
    stake_account: &Pubkey,
    staking_pool: &Pubkey,
    user_token: &Pubkey,
    stake_vault: &Pubkey,
    mmt_mint: &Pubkey,
    staker: &Pubkey,
    amount: u64,
    lock_period_slots: Option<u64>,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*stake_account, false),
            AccountMeta::new(*staking_pool, false),
            AccountMeta::new(*user_token, false),
            AccountMeta::new(*stake_vault, false),
            AccountMeta::new_readonly(*mmt_mint, false),
            AccountMeta::new(*staker, true),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::clock::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
        ],
        data: MMTInstruction::StakeMMT { amount, lock_period_slots }.pack(),
    }
}

/// Create RecordMakerTrade instruction
pub fn record_maker_trade(
    program_id: &Pubkey,
    maker_account: &Pubkey,
    season_emission: &Pubkey,
    early_trader_registry: &Pubkey,
    maker: &Pubkey,
    notional: u64,
    spread_improvement_bp: u16,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*maker_account, false),
            AccountMeta::new(*season_emission, false),
            AccountMeta::new_readonly(*early_trader_registry, false),
            AccountMeta::new_readonly(*maker, true),
            AccountMeta::new_readonly(solana_program::sysvar::clock::id(), false),
        ],
        data: MMTInstruction::RecordMakerTrade { notional, spread_improvement_bp }.pack(),
    }
}

/// Create ClaimMakerRewards instruction
pub fn claim_maker_rewards(
    program_id: &Pubkey,
    maker_account: &Pubkey,
    treasury: &Pubkey,
    treasury_token: &Pubkey,
    maker_token: &Pubkey,
    maker: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*maker_account, false),
            AccountMeta::new_readonly(*treasury, false),
            AccountMeta::new(*treasury_token, false),
            AccountMeta::new(*maker_token, false),
            AccountMeta::new_readonly(*maker, true),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: MMTInstruction::ClaimMakerRewards.pack(),
    }
}

/// Create RegisterEarlyTrader instruction
pub fn register_early_trader(
    program_id: &Pubkey,
    registry: &Pubkey,
    maker_account: &Pubkey,
    trader: &Pubkey,
    season: u8,
) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*registry, false),
            AccountMeta::new(*maker_account, false),
            AccountMeta::new(*trader, true),
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::clock::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
        ],
        data: MMTInstruction::RegisterEarlyTrader { season }.pack(),
    }
}