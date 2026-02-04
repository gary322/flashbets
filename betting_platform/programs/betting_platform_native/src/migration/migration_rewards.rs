//! Migration Rewards System with +30bp Bonus
//! 
//! Implements enhanced rewards for early migrators from Polymarket
//! as specified in sections 45-50 of the specification.

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    program::{invoke, invoke_signed},
    system_instruction,
};

use borsh::{BorshDeserialize, BorshSerialize};
use spl_token::instruction as token_instruction;

use crate::{
    constants::BASIS_POINTS_DIVISOR,
    error::BettingPlatformError,
    state::accounts::{Position, GlobalConfigPDA, discriminators},
    events::{EventType, Event},
    define_event,
    pda::seeds::MMT_MINT,
};

/// Migration bonus in basis points (+30bp as per spec)
pub const MIGRATION_BONUS_BPS: u64 = 30;

/// Early bird bonus period (first 7 days)
pub const EARLY_BIRD_SLOTS: u64 = 1_512_000; // ~7 days at 400ms/slot

/// Extra early bird bonus (+10bp for first week)
pub const EARLY_BIRD_BONUS_BPS: u64 = 10;

/// Migration rewards tracker
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationRewards {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User who migrated
    pub user: Pubkey,
    
    /// Total positions migrated
    pub positions_migrated: u32,
    
    /// Total notional migrated
    pub total_notional_migrated: u64,
    
    /// Base MMT rewards earned
    pub base_mmt_rewards: u64,
    
    /// Migration bonus earned (+30bp)
    pub migration_bonus: u64,
    
    /// Early bird bonus (if applicable)
    pub early_bird_bonus: u64,
    
    /// Total MMT rewards (base + bonuses)
    pub total_mmt_rewards: u64,
    
    /// Migration timestamp
    pub migration_timestamp: i64,
    
    /// Is early bird (migrated in first week)
    pub is_early_bird: bool,
    
    /// Rewards claimed
    pub rewards_claimed: bool,
    
    /// Claim timestamp
    pub claim_timestamp: Option<i64>,
}

impl MigrationRewards {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        4 + // positions_migrated
        8 + // total_notional_migrated
        8 + // base_mmt_rewards
        8 + // migration_bonus
        8 + // early_bird_bonus
        8 + // total_mmt_rewards
        8 + // migration_timestamp
        1 + // is_early_bird
        1 + // rewards_claimed
        9; // claim_timestamp (Option<i64>)

    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_STATS, // Reuse existing discriminator
            user,
            positions_migrated: 0,
            total_notional_migrated: 0,
            base_mmt_rewards: 0,
            migration_bonus: 0,
            early_bird_bonus: 0,
            total_mmt_rewards: 0,
            migration_timestamp: Clock::get().unwrap_or_default().unix_timestamp,
            is_early_bird: false,
            rewards_claimed: false,
            claim_timestamp: None,
        }
    }

    /// Calculate rewards for a migrated position
    pub fn calculate_position_rewards(
        &mut self,
        position_notional: u64,
        migration_start_slot: u64,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        // Base reward: 0.1% of notional (10bp)
        let base_reward = position_notional
            .checked_mul(10)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(BASIS_POINTS_DIVISOR)
            .ok_or(BettingPlatformError::MathOverflow)?;

        // Migration bonus: +30bp
        let migration_bonus = position_notional
            .checked_mul(MIGRATION_BONUS_BPS)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_div(BASIS_POINTS_DIVISOR)
            .ok_or(BettingPlatformError::MathOverflow)?;

        // Early bird bonus: +10bp if within first week
        let early_bird_bonus = if current_slot <= migration_start_slot + EARLY_BIRD_SLOTS {
            self.is_early_bird = true;
            position_notional
                .checked_mul(EARLY_BIRD_BONUS_BPS)
                .ok_or(BettingPlatformError::MathOverflow)?
                .checked_div(BASIS_POINTS_DIVISOR)
                .ok_or(BettingPlatformError::MathOverflow)?
        } else {
            0
        };

        // Total rewards = base + migration bonus + early bird
        let total = base_reward
            .checked_add(migration_bonus)
            .ok_or(BettingPlatformError::MathOverflow)?
            .checked_add(early_bird_bonus)
            .ok_or(BettingPlatformError::MathOverflow)?;

        // Update tracker
        self.base_mmt_rewards = self.base_mmt_rewards
            .checked_add(base_reward)
            .ok_or(BettingPlatformError::MathOverflow)?;
            
        self.migration_bonus = self.migration_bonus
            .checked_add(migration_bonus)
            .ok_or(BettingPlatformError::MathOverflow)?;
            
        self.early_bird_bonus = self.early_bird_bonus
            .checked_add(early_bird_bonus)
            .ok_or(BettingPlatformError::MathOverflow)?;
            
        self.total_mmt_rewards = self.total_mmt_rewards
            .checked_add(total)
            .ok_or(BettingPlatformError::MathOverflow)?;

        Ok(total)
    }
}

/// Process position migration with enhanced rewards
pub fn process_migrate_position_enhanced(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    position_id: [u8; 32],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let old_position = next_account_info(account_info_iter)?;
    let new_position = next_account_info(account_info_iter)?;
    let migration_rewards_account = next_account_info(account_info_iter)?;
    let migration_state = next_account_info(account_info_iter)?;
    let mmt_mint = next_account_info(account_info_iter)?;
    let user_mmt_account = next_account_info(account_info_iter)?;
    let mmt_treasury = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Verify user signed
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load old position
    let position = Position::try_from_slice(&old_position.data.borrow())?;
    
    // Verify ownership
    if position.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Verify position is open
    if position.is_closed {
        return Err(BettingPlatformError::PositionAlreadyClosed.into());
    }
    
    // Load migration state to get start slot
    let migration_data = &migration_state.data.borrow();
    let migration_start_slot = u64::from_le_bytes(
        migration_data[16..24].try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?
    );
    
    let clock = Clock::get()?;
    
    // Load or create migration rewards tracker
    let mut rewards = if migration_rewards_account.data_is_empty() {
        MigrationRewards::new(*user.key)
    } else {
        MigrationRewards::try_from_slice(&migration_rewards_account.data.borrow())?
    };
    
    // Calculate rewards for this position
    let position_rewards = rewards.calculate_position_rewards(
        position.notional,
        migration_start_slot,
        clock.slot,
    )?;
    
    // Update migration stats
    rewards.positions_migrated += 1;
    rewards.total_notional_migrated = rewards.total_notional_migrated
        .checked_add(position.notional)
        .ok_or(BettingPlatformError::MathOverflow)?;
    
    // Mint MMT rewards to user
    let mint_ix = token_instruction::mint_to(
        &spl_token::id(),
        mmt_mint.key,
        user_mmt_account.key,
        mmt_treasury.key,
        &[],
        position_rewards,
    )?;
    
    invoke_signed(
        &mint_ix,
        &[
            mmt_mint.clone(),
            user_mmt_account.clone(),
            mmt_treasury.clone(),
            token_program.clone(),
        ],
        &[&[b"mmt_treasury", &[255]]], // Treasury bump seed
    )?;
    
    // Save rewards tracker
    rewards.serialize(&mut &mut migration_rewards_account.data.borrow_mut()[..])?;
    
    // Copy position data to new account
    let position_data = old_position.data.borrow();
    new_position.data.borrow_mut().copy_from_slice(&position_data);
    
    // Close old position account
    let old_position_lamports = old_position.lamports();
    **old_position.lamports.borrow_mut() = 0;
    **user.lamports.borrow_mut() += old_position_lamports;
    old_position.data.borrow_mut().fill(0);
    
    msg!(
        "Position migrated with rewards: base={}, migration_bonus={}, early_bird={}, total={}",
        rewards.base_mmt_rewards,
        rewards.migration_bonus,
        rewards.early_bird_bonus,
        position_rewards
    );
    
    // Emit event
    let event = PositionMigratedEnhanced {
        user: *user.key,
        position_id,
        notional: position.notional,
        base_reward: rewards.base_mmt_rewards,
        migration_bonus: rewards.migration_bonus,
        early_bird_bonus: rewards.early_bird_bonus,
        total_mmt_reward: position_rewards,
        is_early_bird: rewards.is_early_bird,
        timestamp: clock.unix_timestamp,
    };
    event.emit();
    
    Ok(())
}

/// Get migration rewards summary
pub fn get_migration_rewards_summary(
    rewards: &MigrationRewards,
) -> String {
    let mut summary = String::from("🎁 Migration Rewards Summary\n");
    summary.push_str("─────────────────────────\n");
    
    summary.push_str(&format!(
        "Positions Migrated: {}\n",
        rewards.positions_migrated
    ));
    
    summary.push_str(&format!(
        "Total Notional: ${}\n",
        rewards.total_notional_migrated / 1_000_000
    ));
    
    summary.push_str(&format!(
        "\nBase Rewards: {} MMT\n",
        rewards.base_mmt_rewards / 1_000_000_000
    ));
    
    summary.push_str(&format!(
        "Migration Bonus (+30bp): {} MMT\n",
        rewards.migration_bonus / 1_000_000_000
    ));
    
    if rewards.is_early_bird {
        summary.push_str(&format!(
            "Early Bird Bonus (+10bp): {} MMT 🐦\n",
            rewards.early_bird_bonus / 1_000_000_000
        ));
    }
    
    summary.push_str(&format!(
        "\nTotal MMT Rewards: {} MMT\n",
        rewards.total_mmt_rewards / 1_000_000_000
    ));
    
    if rewards.rewards_claimed {
        summary.push_str(&format!(
            "\n✅ Rewards claimed on {}",
            rewards.claim_timestamp.unwrap_or(0)
        ));
    } else {
        summary.push_str("\n⏳ Rewards pending claim");
    }
    
    summary
}

// Event definitions
define_event!(PositionMigratedEnhanced, EventType::PositionMigrated, {
    user: Pubkey,
    position_id: [u8; 32],
    notional: u64,
    base_reward: u64,
    migration_bonus: u64,
    early_bird_bonus: u64,
    total_mmt_reward: u64,
    is_early_bird: bool,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_rewards_calculation() {
        let mut rewards = MigrationRewards::new(Pubkey::new_unique());
        
        // Test with $100,000 notional
        let notional = 100_000_000_000; // $100k with 6 decimals
        let migration_start = 1000;
        let current_slot = 1500; // Within early bird period
        
        let reward = rewards.calculate_position_rewards(
            notional,
            migration_start,
            current_slot,
        ).unwrap();
        
        // Base: 100k * 0.001 = $100
        // Migration bonus: 100k * 0.003 = $300
        // Early bird: 100k * 0.001 = $100
        // Total: $500
        assert_eq!(reward, 500_000_000); // $500 with 6 decimals
        assert!(rewards.is_early_bird);
        assert_eq!(rewards.migration_bonus, 300_000_000); // $300
    }

    #[test]
    fn test_late_migration_no_early_bird() {
        let mut rewards = MigrationRewards::new(Pubkey::new_unique());
        
        let notional = 100_000_000_000;
        let migration_start = 1000;
        let current_slot = migration_start + EARLY_BIRD_SLOTS + 1; // After early bird
        
        let reward = rewards.calculate_position_rewards(
            notional,
            migration_start,
            current_slot,
        ).unwrap();
        
        // No early bird bonus
        assert_eq!(reward, 400_000_000); // $400 (base + migration bonus only)
        assert!(!rewards.is_early_bird);
        assert_eq!(rewards.early_bird_bonus, 0);
    }
}