//! Automated Migration Wizard
//! 
//! Simplifies the migration process for users coming from Polymarket
//! with step-by-step guidance and automated position transfers.

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::{Position, discriminators},
    migration::{MigrationRewards, MIGRATION_BONUS_BPS, EARLY_BIRD_BONUS_BPS},
    events::{EventType, Event},
    define_event,
};

/// Migration wizard state for tracking user progress
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MigrationWizardState {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User going through migration
    pub user: Pubkey,
    
    /// Current step in wizard
    pub current_step: MigrationStep,
    
    /// Total positions to migrate
    pub total_positions: u32,
    
    /// Positions migrated so far
    pub positions_migrated: u32,
    
    /// Positions discovered
    pub positions_discovered: Vec<PositionInfo>,
    
    /// Estimated total rewards
    pub estimated_total_rewards: u64,
    
    /// Actual rewards earned
    pub actual_rewards_earned: u64,
    
    /// Wizard started timestamp
    pub started_at: i64,
    
    /// Wizard completed timestamp
    pub completed_at: Option<i64>,
    
    /// Is completed
    pub is_completed: bool,
    
    /// Error count (for retry logic)
    pub error_count: u8,
    
    /// Last error message
    pub last_error: Option<String>,
}

impl MigrationWizardState {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        1 + // current_step
        4 + // total_positions
        4 + // positions_migrated
        4 + (64 * 32) + // positions_discovered (max 32 positions)
        8 + // estimated_total_rewards
        8 + // actual_rewards_earned
        8 + // started_at
        9 + // completed_at
        1 + // is_completed
        1 + // error_count
        1 + 64; // last_error

    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_STATS, // Reuse existing discriminator
            user,
            current_step: MigrationStep::Welcome,
            total_positions: 0,
            positions_migrated: 0,
            positions_discovered: Vec::new(),
            estimated_total_rewards: 0,
            actual_rewards_earned: 0,
            started_at: Clock::get().unwrap_or_default().unix_timestamp,
            completed_at: None,
            is_completed: false,
            error_count: 0,
            last_error: None,
        }
    }

    /// Advance to next step
    pub fn advance_step(&mut self) -> Result<MigrationStep, ProgramError> {
        self.current_step = match self.current_step {
            MigrationStep::Welcome => MigrationStep::ConnectWallet,
            MigrationStep::ConnectWallet => MigrationStep::ScanPositions,
            MigrationStep::ScanPositions => MigrationStep::ReviewRewards,
            MigrationStep::ReviewRewards => MigrationStep::ConfirmMigration,
            MigrationStep::ConfirmMigration => MigrationStep::MigratingPositions,
            MigrationStep::MigratingPositions => {
                if self.positions_migrated < self.total_positions {
                    MigrationStep::MigratingPositions
                } else {
                    MigrationStep::ClaimRewards
                }
            }
            MigrationStep::ClaimRewards => MigrationStep::Completed,
            MigrationStep::Completed => return Err(BettingPlatformError::MigrationCompleted.into()),
        };

        Ok(self.current_step)
    }

    /// Calculate estimated rewards
    pub fn calculate_estimated_rewards(&mut self, is_early_bird: bool) -> Result<(), ProgramError> {
        let mut total_rewards = 0u64;

        for position in &self.positions_discovered {
            // Base: 10bp
            let base = position.notional / 1000;
            // Migration bonus: 30bp
            let migration = (position.notional * MIGRATION_BONUS_BPS) / 10000;
            // Early bird: 10bp if applicable
            let early = if is_early_bird {
                (position.notional * EARLY_BIRD_BONUS_BPS) / 10000
            } else {
                0
            };

            total_rewards = total_rewards
                .checked_add(base + migration + early)
                .ok_or(BettingPlatformError::MathOverflow)?;
        }

        self.estimated_total_rewards = total_rewards;
        Ok(())
    }

    /// Get progress percentage
    pub fn get_progress_percentage(&self) -> u8 {
        match self.current_step {
            MigrationStep::Welcome => 0,
            MigrationStep::ConnectWallet => 14,
            MigrationStep::ScanPositions => 28,
            MigrationStep::ReviewRewards => 42,
            MigrationStep::ConfirmMigration => 57,
            MigrationStep::MigratingPositions => {
                if self.total_positions > 0 {
                    57 + (28 * self.positions_migrated / self.total_positions) as u8
                } else {
                    71
                }
            }
            MigrationStep::ClaimRewards => 85,
            MigrationStep::Completed => 100,
        }
    }
}

/// Migration steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStep {
    /// Welcome screen explaining benefits
    Welcome,
    /// Connect Polymarket wallet
    ConnectWallet,
    /// Scan for positions
    ScanPositions,
    /// Review rewards preview
    ReviewRewards,
    /// Confirm migration
    ConfirmMigration,
    /// Migrating positions (in progress)
    MigratingPositions,
    /// Claim MMT rewards
    ClaimRewards,
    /// Migration completed
    Completed,
}

impl MigrationStep {
    pub fn to_string(&self) -> &'static str {
        match self {
            MigrationStep::Welcome => "Welcome to Migration",
            MigrationStep::ConnectWallet => "Connect Your Wallet",
            MigrationStep::ScanPositions => "Scanning Positions",
            MigrationStep::ReviewRewards => "Review Your Rewards",
            MigrationStep::ConfirmMigration => "Confirm Migration",
            MigrationStep::MigratingPositions => "Migrating Positions",
            MigrationStep::ClaimRewards => "Claim Your Rewards",
            MigrationStep::Completed => "Migration Complete!",
        }
    }

    pub fn get_description(&self) -> &'static str {
        match self {
            MigrationStep::Welcome => "Migrate for Fixes + Bonus! Get +30bp MMT rewards for switching from Polymarket!",
            MigrationStep::ConnectWallet => "Connect the wallet containing your Polymarket positions",
            MigrationStep::ScanPositions => "Scanning blockchain for your active positions...",
            MigrationStep::ReviewRewards => "Review your positions and estimated MMT rewards",
            MigrationStep::ConfirmMigration => "Confirm to start the automated migration process",
            MigrationStep::MigratingPositions => "Migrating your positions one by one...",
            MigrationStep::ClaimRewards => "Claim your MMT rewards to your wallet",
            MigrationStep::Completed => "Welcome to the new platform! Your migration is complete.",
        }
    }
}

/// Position info for wizard
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionInfo {
    /// Position ID
    pub position_id: [u8; 32],
    /// Market name
    pub market_name: String,
    /// Position size
    pub size: u64,
    /// Notional value
    pub notional: u64,
    /// Is migrated
    pub is_migrated: bool,
}

/// Start migration wizard
pub fn process_start_migration_wizard(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let wizard_state = next_account_info(account_info_iter)?;
    
    // Verify user signed
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Initialize wizard state
    let state = MigrationWizardState::new(*user.key);
    state.serialize(&mut &mut wizard_state.data.borrow_mut()[..])?;
    
    msg!("Migration wizard started for user {}", user.key);
    
    // Emit event
    let event = MigrationWizardStarted {
        user: *user.key,
        timestamp: state.started_at,
    };
    event.emit();
    
    Ok(())
}

/// Advance wizard step
pub fn process_advance_wizard_step(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let wizard_state = next_account_info(account_info_iter)?;
    
    // Verify user signed
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load wizard state
    let mut state = MigrationWizardState::try_from_slice(&wizard_state.data.borrow())?;
    
    // Verify ownership
    if state.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Advance step
    let new_step = state.advance_step()?;
    
    msg!(
        "Migration wizard advanced to step: {} ({}%)",
        new_step.to_string(),
        state.get_progress_percentage()
    );
    
    // Save state
    state.serialize(&mut &mut wizard_state.data.borrow_mut()[..])?;
    
    Ok(())
}

/// Complete migration wizard
pub fn process_complete_migration_wizard(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let wizard_state = next_account_info(account_info_iter)?;
    let rewards_account = next_account_info(account_info_iter)?;
    
    // Load wizard state
    let mut state = MigrationWizardState::try_from_slice(&wizard_state.data.borrow())?;
    
    // Verify ownership
    if state.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Verify all positions migrated
    if state.positions_migrated < state.total_positions {
        return Err(BettingPlatformError::MigrationInProgress.into());
    }
    
    // Load rewards to get actual amount
    let rewards = MigrationRewards::try_from_slice(&rewards_account.data.borrow())?;
    
    // Complete wizard
    state.is_completed = true;
    state.completed_at = Some(Clock::get()?.unix_timestamp);
    state.actual_rewards_earned = rewards.total_mmt_rewards;
    state.current_step = MigrationStep::Completed;
    
    // Save state
    state.serialize(&mut &mut wizard_state.data.borrow_mut()[..])?;
    
    msg!(
        "Migration wizard completed! {} positions migrated, {} MMT earned",
        state.positions_migrated,
        state.actual_rewards_earned
    );
    
    // Emit event
    let event = MigrationWizardCompleted {
        user: *user.key,
        positions_migrated: state.positions_migrated,
        total_rewards: state.actual_rewards_earned,
        duration_seconds: state.completed_at.unwrap_or(0) - state.started_at,
        timestamp: state.completed_at.unwrap_or(0),
    };
    event.emit();
    
    Ok(())
}

/// Format wizard display
pub fn format_wizard_display(state: &MigrationWizardState) -> String {
    let mut display = String::from("🧙 Migration Wizard\n");
    display.push_str("═══════════════════\n\n");
    
    // Progress bar
    let progress = state.get_progress_percentage();
    let filled = (progress / 10) as usize;
    let empty = 10 - filled;
    display.push_str(&format!(
        "Progress: [{}{}] {}%\n\n",
        "█".repeat(filled),
        "░".repeat(empty),
        progress
    ));
    
    // Current step
    display.push_str(&format!(
        "Step {}: {}\n",
        state.current_step as u8 + 1,
        state.current_step.to_string()
    ));
    display.push_str(&format!("{}\n\n", state.current_step.get_description()));
    
    // Stats
    if state.total_positions > 0 {
        display.push_str(&format!(
            "Positions: {}/{} migrated\n",
            state.positions_migrated,
            state.total_positions
        ));
    }
    
    if state.estimated_total_rewards > 0 {
        display.push_str(&format!(
            "Estimated Rewards: {} MMT\n",
            state.estimated_total_rewards / 1_000_000_000
        ));
        
        if state.positions_migrated > 0 {
            display.push_str(&format!(
                "Earned So Far: {} MMT\n",
                state.actual_rewards_earned / 1_000_000_000
            ));
        }
    }
    
    // Call to action
    display.push_str(&format!("\n{}", match state.current_step {
        MigrationStep::Welcome => "🎯 Click 'Start Migration' to begin",
        MigrationStep::ConnectWallet => "🔗 Connect your Polymarket wallet",
        MigrationStep::ScanPositions => "🔍 Scanning... Please wait",
        MigrationStep::ReviewRewards => "💰 Review and click 'Confirm'",
        MigrationStep::ConfirmMigration => "✅ Click 'Begin Migration'",
        MigrationStep::MigratingPositions => "⏳ Migration in progress...",
        MigrationStep::ClaimRewards => "🎁 Click 'Claim Rewards'",
        MigrationStep::Completed => "🎉 All done! Welcome aboard!",
    }));
    
    display
}

// Event definitions
define_event!(MigrationWizardStarted, EventType::MigrationStarted, {
    user: Pubkey,
    timestamp: i64,
});

define_event!(MigrationWizardCompleted, EventType::MigrationCompleted, {
    user: Pubkey,
    positions_migrated: u32,
    total_rewards: u64,
    duration_seconds: i64,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_progress() {
        let user = Pubkey::new_unique();
        let mut wizard = MigrationWizardState::new(user);
        
        assert_eq!(wizard.get_progress_percentage(), 0);
        assert_eq!(wizard.current_step, MigrationStep::Welcome);
        
        // Advance through steps
        wizard.advance_step().unwrap();
        assert_eq!(wizard.current_step, MigrationStep::ConnectWallet);
        assert_eq!(wizard.get_progress_percentage(), 14);
        
        wizard.advance_step().unwrap();
        assert_eq!(wizard.current_step, MigrationStep::ScanPositions);
        assert_eq!(wizard.get_progress_percentage(), 28);
    }

    #[test]
    fn test_rewards_calculation() {
        let user = Pubkey::new_unique();
        let mut wizard = MigrationWizardState::new(user);
        
        // Add test positions
        wizard.positions_discovered.push(PositionInfo {
            position_id: [1u8; 32],
            market_name: "BTC > $100k by EOY".to_string(),
            size: 1000_000_000, // $1000
            notional: 10_000_000_000, // $10k with leverage
            is_migrated: false,
        });
        
        wizard.total_positions = 1;
        
        // Calculate rewards with early bird
        wizard.calculate_estimated_rewards(true).unwrap();
        
        // Base: 10k * 0.001 = $10
        // Migration: 10k * 0.003 = $30
        // Early bird: 10k * 0.001 = $10
        // Total: $50
        assert_eq!(wizard.estimated_total_rewards, 50_000_000); // $50 with decimals
    }
}