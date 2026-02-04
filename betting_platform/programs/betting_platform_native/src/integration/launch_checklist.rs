// Phase 20: System Launch Checklist Script
// Automated checklist to ensure all systems are go for launch

use solana_program::{
    account_info::AccountInfo,
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
    events::{emit_event, EventType},
};

/// Launch checklist configuration
pub const LAUNCH_PHASES: u8 = 5;
pub const CRITICAL_CHECKS: u32 = 25;
pub const WARNING_THRESHOLD: u32 = 5;
pub const MINIMUM_GO_SCORE: u16 = 9000; // 90%
pub const ROLLBACK_WINDOW: u64 = 7200; // 1 hour in slots

/// Launch checklist coordinator
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LaunchChecklist {
    pub launch_id: u128,
    pub launch_status: LaunchStatus,
    pub current_phase: LaunchPhase,
    pub checklist_items: Vec<ChecklistItem>,
    pub automated_checks: Vec<AutomatedCheck>,
    pub manual_confirmations: Vec<ManualConfirmation>,
    pub go_no_go_decision: GoNoGoDecision,
    pub launch_metrics: LaunchMetrics,
    pub rollback_plan: RollbackPlan,
    pub start_time: i64,
    pub completion_time: Option<i64>,
}

impl LaunchChecklist {
    pub const SIZE: usize = 16 + // launch_id
        1 + // launch_status
        1 + // current_phase
        4 + 100 * ChecklistItem::SIZE + // checklist_items
        4 + 50 * AutomatedCheck::SIZE + // automated_checks
        4 + 20 * ManualConfirmation::SIZE + // manual_confirmations
        GoNoGoDecision::SIZE +
        LaunchMetrics::SIZE +
        RollbackPlan::SIZE +
        8 + // start_time
        9; // completion_time

    /// Initialize launch checklist
    pub fn initialize(&mut self, launch_id: u128) -> ProgramResult {
        self.launch_id = launch_id;
        self.launch_status = LaunchStatus::PreLaunch;
        self.current_phase = LaunchPhase::Preparation;
        self.checklist_items = self.generate_checklist_items();
        self.automated_checks = Vec::new();
        self.manual_confirmations = Vec::new();
        self.go_no_go_decision = GoNoGoDecision::default();
        self.launch_metrics = LaunchMetrics::default();
        self.rollback_plan = RollbackPlan::default();
        self.start_time = Clock::get()?.unix_timestamp;
        self.completion_time = None;

        msg!("Launch checklist {} initialized", launch_id);
        Ok(())
    }

    /// Generate comprehensive checklist items
    fn generate_checklist_items(&self) -> Vec<ChecklistItem> {
        vec![
            // Phase 1: Preparation
            ChecklistItem {
                phase: LaunchPhase::Preparation,
                category: CheckCategory::Infrastructure,
                description: "Verify all Solana accounts initialized".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Preparation,
                category: CheckCategory::Infrastructure,
                description: "Confirm program deployed to mainnet".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::Preparation,
                category: CheckCategory::Security,
                description: "Security audit completed and passed".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::Preparation,
                category: CheckCategory::Configuration,
                description: "Circuit breakers configured and tested".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Preparation,
                category: CheckCategory::Oracle,
                description: "Polymarket oracle connection verified".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Preparation,
                category: CheckCategory::Oracle,
                description: "Fallback oracle configured".to_string(),
                is_critical: false,
                status: CheckStatus::Pending,
                automated: true,
            },
            
            // Phase 2: Testing
            ChecklistItem {
                phase: LaunchPhase::Testing,
                category: CheckCategory::Testing,
                description: "All unit tests passing".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Testing,
                category: CheckCategory::Testing,
                description: "Integration tests complete".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Testing,
                category: CheckCategory::Testing,
                description: "Stress tests passed at 2x expected load".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Testing,
                category: CheckCategory::Testing,
                description: "E2E user journey tests complete".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Testing,
                category: CheckCategory::Performance,
                description: "TPS meets minimum requirements (1000+)".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Testing,
                category: CheckCategory::Performance,
                description: "Latency under 100ms p99".to_string(),
                is_critical: false,
                status: CheckStatus::Pending,
                automated: true,
            },
            
            // Phase 3: Final Configuration
            ChecklistItem {
                phase: LaunchPhase::FinalConfig,
                category: CheckCategory::Authority,
                description: "Multisig authorities configured".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::FinalConfig,
                category: CheckCategory::Authority,
                description: "Upgrade authority to be burned".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::FinalConfig,
                category: CheckCategory::Vault,
                description: "Vault funded with minimum balance".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::FinalConfig,
                category: CheckCategory::Keeper,
                description: "Minimum 3 keepers registered".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::FinalConfig,
                category: CheckCategory::Monitoring,
                description: "Monitoring and alerting configured".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::FinalConfig,
                category: CheckCategory::Documentation,
                description: "User documentation published".to_string(),
                is_critical: false,
                status: CheckStatus::Pending,
                automated: false,
            },
            
            // Phase 4: Launch
            ChecklistItem {
                phase: LaunchPhase::Launch,
                category: CheckCategory::System,
                description: "System state set to active".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Launch,
                category: CheckCategory::Markets,
                description: "Initial markets created".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::Launch,
                category: CheckCategory::Authority,
                description: "Upgrade authority burned".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::Launch,
                category: CheckCategory::Communication,
                description: "Launch announcement published".to_string(),
                is_critical: false,
                status: CheckStatus::Pending,
                automated: false,
            },
            
            // Phase 5: Post-Launch
            ChecklistItem {
                phase: LaunchPhase::PostLaunch,
                category: CheckCategory::Monitoring,
                description: "24-hour monitoring active".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: false,
            },
            ChecklistItem {
                phase: LaunchPhase::PostLaunch,
                category: CheckCategory::Performance,
                description: "Performance metrics within bounds".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
            ChecklistItem {
                phase: LaunchPhase::PostLaunch,
                category: CheckCategory::System,
                description: "No critical incidents in first 24h".to_string(),
                is_critical: true,
                status: CheckStatus::Pending,
                automated: true,
            },
        ]
    }

    /// Execute launch checklist
    pub fn execute_checklist(&mut self) -> Result<LaunchReport, ProgramError> {
        msg!("Executing launch checklist for phase {:?}", self.current_phase);

        // Run automated checks for current phase
        self.run_automated_checks()?;

        // Validate manual confirmations
        self.validate_manual_confirmations()?;

        // Calculate phase completion
        let phase_completion = self.calculate_phase_completion()?;

        // Check if phase is complete
        if phase_completion >= 100 {
            // Move to next phase
            self.advance_phase()?;
        }

        // Update go/no-go decision
        self.update_go_no_go_decision()?;

        // Generate report
        let report = self.generate_launch_report()?;

        msg!("Launch checklist progress: {}%", self.calculate_overall_progress());

        Ok(report)
    }

    /// Run automated checks
    fn run_automated_checks(&mut self) -> Result<(), ProgramError> {
        let current_time = Clock::get()?.unix_timestamp;
        let current_phase = self.current_phase.clone();
        
        // Collect indices of items to check
        let indices_to_check: Vec<usize> = self.checklist_items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.phase == current_phase && 
                item.automated && 
                item.status == CheckStatus::Pending
            })
            .map(|(i, _)| i)
            .collect();

        // Process each item
        for &index in &indices_to_check {
            let item = self.checklist_items[index].clone();
            let check_result = self.perform_automated_check(&item)?;
            let check_details = self.get_check_details(&item)?;
            
            self.automated_checks.push(AutomatedCheck {
                item_description: item.description.clone(),
                check_time: current_time,
                result: check_result.clone(),
                details: check_details,
            });

            self.checklist_items[index].status = if check_result == CheckResult::Pass {
                CheckStatus::Completed
            } else if item.is_critical {
                CheckStatus::Failed
            } else {
                CheckStatus::Warning
            };
        }

        Ok(())
    }

    /// Perform specific automated check
    fn perform_automated_check(&self, item: &ChecklistItem) -> Result<CheckResult, ProgramError> {
        match item.category {
            CheckCategory::Infrastructure => {
                // Check infrastructure readiness
                if item.description.contains("accounts initialized") {
                    Ok(if self.verify_accounts_initialized()? { 
                        CheckResult::Pass 
                    } else { 
                        CheckResult::Fail 
                    })
                } else {
                    Ok(CheckResult::Pass)
                }
            },
            CheckCategory::Oracle => {
                // Check oracle connectivity
                if item.description.contains("Polymarket") {
                    Ok(if self.verify_polymarket_connection()? { 
                        CheckResult::Pass 
                    } else { 
                        CheckResult::Fail 
                    })
                } else {
                    Ok(CheckResult::Pass)
                }
            },
            CheckCategory::Testing => {
                // Check test results
                Ok(if self.verify_test_results(&item.description)? { 
                    CheckResult::Pass 
                } else { 
                    CheckResult::Fail 
                })
            },
            CheckCategory::Performance => {
                // Check performance metrics
                Ok(if self.verify_performance_metrics(&item.description)? { 
                    CheckResult::Pass 
                } else { 
                    CheckResult::Fail 
                })
            },
            CheckCategory::Vault => {
                // Check vault balance
                Ok(if self.verify_vault_balance()? { 
                    CheckResult::Pass 
                } else { 
                    CheckResult::Fail 
                })
            },
            CheckCategory::Keeper => {
                // Check keeper count
                Ok(if self.verify_keeper_count()? { 
                    CheckResult::Pass 
                } else { 
                    CheckResult::Fail 
                })
            },
            _ => Ok(CheckResult::Pass),
        }
    }

    /// Validate manual confirmations
    fn validate_manual_confirmations(&mut self) -> Result<(), ProgramError> {
        for item in &mut self.checklist_items {
            if item.phase == self.current_phase && 
               !item.automated && 
               item.status == CheckStatus::Pending {
                
                // Check if manual confirmation exists
                if let Some(confirmation) = self.manual_confirmations.iter()
                    .find(|c| c.item_description == item.description) {
                    
                    item.status = if confirmation.confirmed {
                        CheckStatus::Completed
                    } else if item.is_critical {
                        CheckStatus::Failed
                    } else {
                        CheckStatus::Warning
                    };
                }
            }
        }

        Ok(())
    }

    /// Calculate phase completion percentage
    fn calculate_phase_completion(&self) -> Result<u16, ProgramError> {
        let phase_items: Vec<&ChecklistItem> = self.checklist_items.iter()
            .filter(|i| i.phase == self.current_phase)
            .collect();

        if phase_items.is_empty() {
            return Ok(10000); // 100%
        }

        let completed = phase_items.iter()
            .filter(|i| i.status == CheckStatus::Completed)
            .count() as u16;

        Ok((completed * 10000) / phase_items.len() as u16)
    }

    /// Calculate overall progress
    fn calculate_overall_progress(&self) -> u16 {
        let total_items = self.checklist_items.len() as u16;
        let completed_items = self.checklist_items.iter()
            .filter(|i| i.status == CheckStatus::Completed)
            .count() as u16;

        if total_items == 0 {
            0
        } else {
            (completed_items * 100) / total_items
        }
    }

    /// Advance to next phase
    fn advance_phase(&mut self) -> Result<(), ProgramError> {
        self.current_phase = match self.current_phase {
            LaunchPhase::Preparation => LaunchPhase::Testing,
            LaunchPhase::Testing => LaunchPhase::FinalConfig,
            LaunchPhase::FinalConfig => LaunchPhase::Launch,
            LaunchPhase::Launch => LaunchPhase::PostLaunch,
            LaunchPhase::PostLaunch => {
                self.launch_status = LaunchStatus::Completed;
                self.completion_time = Some(Clock::get()?.unix_timestamp);
                LaunchPhase::PostLaunch
            },
        };

        msg!("Advanced to phase: {:?}", self.current_phase);
        Ok(())
    }

    /// Update go/no-go decision
    fn update_go_no_go_decision(&mut self) -> Result<(), ProgramError> {
        let critical_failures = self.checklist_items.iter()
            .filter(|i| i.is_critical && i.status == CheckStatus::Failed)
            .count() as u32;

        let warnings = self.checklist_items.iter()
            .filter(|i| i.status == CheckStatus::Warning)
            .count() as u32;

        let score = self.calculate_go_score()?;

        self.go_no_go_decision = GoNoGoDecision {
            decision: if critical_failures > 0 {
                GoNoGo::NoGo
            } else if score >= MINIMUM_GO_SCORE {
                GoNoGo::Go
            } else {
                GoNoGo::ConditionalGo
            },
            score,
            critical_failures,
            warnings,
            decision_time: Clock::get()?.unix_timestamp,
            approvers: Vec::new(), // Would be populated by manual process
        };

        Ok(())
    }

    /// Calculate go/no-go score
    fn calculate_go_score(&self) -> Result<u16, ProgramError> {
        let total_critical = self.checklist_items.iter()
            .filter(|i| i.is_critical)
            .count() as u16;

        let completed_critical = self.checklist_items.iter()
            .filter(|i| i.is_critical && i.status == CheckStatus::Completed)
            .count() as u16;

        if total_critical == 0 {
            Ok(10000)
        } else {
            Ok((completed_critical * 10000) / total_critical)
        }
    }

    /// Generate launch report
    fn generate_launch_report(&self) -> Result<LaunchReport, ProgramError> {
        let report = LaunchReport {
            launch_id: self.launch_id,
            current_phase: self.current_phase.clone(),
            launch_status: self.launch_status.clone(),
            overall_progress: self.calculate_overall_progress(),
            phase_progress: self.calculate_phase_completion()? / 100,
            critical_items: self.get_critical_items(),
            pending_items: self.get_pending_items(),
            failed_items: self.get_failed_items(),
            warnings: self.get_warnings(),
            go_no_go_decision: self.go_no_go_decision.clone(),
            estimated_completion: self.estimate_completion_time()?,
            next_actions: self.get_next_actions(),
        };

        Ok(report)
    }

    /// Get critical items
    fn get_critical_items(&self) -> Vec<ChecklistItem> {
        self.checklist_items.iter()
            .filter(|i| i.is_critical && i.phase == self.current_phase)
            .cloned()
            .collect()
    }

    /// Get pending items
    fn get_pending_items(&self) -> Vec<ChecklistItem> {
        self.checklist_items.iter()
            .filter(|i| i.status == CheckStatus::Pending && i.phase == self.current_phase)
            .cloned()
            .collect()
    }

    /// Get failed items
    fn get_failed_items(&self) -> Vec<ChecklistItem> {
        self.checklist_items.iter()
            .filter(|i| i.status == CheckStatus::Failed)
            .cloned()
            .collect()
    }

    /// Get warnings
    fn get_warnings(&self) -> Vec<ChecklistItem> {
        self.checklist_items.iter()
            .filter(|i| i.status == CheckStatus::Warning)
            .cloned()
            .collect()
    }

    /// Estimate completion time
    fn estimate_completion_time(&self) -> Result<i64, ProgramError> {
        let remaining_items = self.checklist_items.iter()
            .filter(|i| i.status == CheckStatus::Pending)
            .count() as i64;

        let avg_time_per_item = 300; // 5 minutes average
        let estimated_seconds = remaining_items * avg_time_per_item;
        
        Ok(Clock::get()?.unix_timestamp + estimated_seconds)
    }

    /// Get next actions
    fn get_next_actions(&self) -> Vec<String> {
        let mut actions = Vec::new();

        // Get pending critical items
        for item in self.get_pending_items() {
            if item.is_critical {
                actions.push(format!("Complete: {}", item.description));
            }
        }

        // Add phase-specific actions
        match self.current_phase {
            LaunchPhase::Preparation => {
                actions.push("Complete all infrastructure setup".to_string());
            },
            LaunchPhase::Testing => {
                actions.push("Run full test suite".to_string());
            },
            LaunchPhase::FinalConfig => {
                actions.push("Configure production parameters".to_string());
            },
            LaunchPhase::Launch => {
                actions.push("Execute launch sequence".to_string());
            },
            LaunchPhase::PostLaunch => {
                actions.push("Monitor system performance".to_string());
            },
        }

        actions
    }

    /// Add manual confirmation
    pub fn add_manual_confirmation(
        &mut self,
        item_description: String,
        confirmed: bool,
        approver: Pubkey,
    ) -> ProgramResult {
        self.manual_confirmations.push(ManualConfirmation {
            item_description,
            confirmed,
            approver,
            confirmation_time: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Execute rollback if needed
    pub fn execute_rollback(&mut self) -> Result<(), ProgramError> {
        if self.launch_status != LaunchStatus::Failed {
            return Err(BettingPlatformError::InvalidInput.into());
        }

        msg!("Executing rollback plan...");

        // Revert system state
        self.rollback_plan.execute()?;

        // Update status
        self.launch_status = LaunchStatus::RolledBack;

        Ok(())
    }

    // Mock verification functions
    fn verify_accounts_initialized(&self) -> Result<bool, ProgramError> { Ok(true) }
    fn verify_polymarket_connection(&self) -> Result<bool, ProgramError> { Ok(true) }
    fn verify_test_results(&self, _test_name: &str) -> Result<bool, ProgramError> { Ok(true) }
    fn verify_performance_metrics(&self, _metric: &str) -> Result<bool, ProgramError> { Ok(true) }
    fn verify_vault_balance(&self) -> Result<bool, ProgramError> { Ok(true) }
    fn verify_keeper_count(&self) -> Result<bool, ProgramError> { Ok(true) }
    fn get_check_details(&self, _item: &ChecklistItem) -> Result<String, ProgramError> { 
        Ok("Check completed successfully".to_string()) 
    }
}

/// Launch status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum LaunchStatus {
    PreLaunch,
    InProgress,
    Completed,
    Failed,
    RolledBack,
}

/// Launch phases
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum LaunchPhase {
    Preparation,
    Testing,
    FinalConfig,
    Launch,
    PostLaunch,
}

/// Checklist item
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ChecklistItem {
    pub phase: LaunchPhase,
    pub category: CheckCategory,
    pub description: String,
    pub is_critical: bool,
    pub status: CheckStatus,
    pub automated: bool,
}

impl ChecklistItem {
    pub const SIZE: usize = 1 + 1 + 100 + 1 + 1 + 1;
}

/// Check categories
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum CheckCategory {
    Infrastructure,
    Security,
    Configuration,
    Oracle,
    Testing,
    Performance,
    Authority,
    Vault,
    Keeper,
    Monitoring,
    Documentation,
    System,
    Markets,
    Communication,
}

/// Check status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum CheckStatus {
    Pending,
    Completed,
    Failed,
    Warning,
    Skipped,
}

/// Automated check result
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AutomatedCheck {
    pub item_description: String,
    pub check_time: i64,
    pub result: CheckResult,
    pub details: String,
}

impl AutomatedCheck {
    pub const SIZE: usize = 100 + 8 + 1 + 200;
}

/// Check result
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum CheckResult {
    Pass,
    Fail,
    Warning,
}

/// Manual confirmation
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ManualConfirmation {
    pub item_description: String,
    pub confirmed: bool,
    pub approver: Pubkey,
    pub confirmation_time: i64,
}

impl ManualConfirmation {
    pub const SIZE: usize = 100 + 1 + 32 + 8;
}

/// Go/No-Go decision
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct GoNoGoDecision {
    pub decision: GoNoGo,
    pub score: u16,
    pub critical_failures: u32,
    pub warnings: u32,
    pub decision_time: i64,
    pub approvers: Vec<Pubkey>,
}

impl GoNoGoDecision {
    pub const SIZE: usize = 1 + 2 + 4 + 4 + 8 + 4 + 32 * 5;
}

/// Go/No-Go enum
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum GoNoGo {
    Go,
    NoGo,
    ConditionalGo,
}

impl Default for GoNoGo {
    fn default() -> Self {
        GoNoGo::NoGo
    }
}

/// Launch metrics
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct LaunchMetrics {
    pub checks_completed: u32,
    pub checks_failed: u32,
    pub warnings_count: u32,
    pub time_elapsed: u64,
    pub phases_completed: u8,
}

impl LaunchMetrics {
    pub const SIZE: usize = 4 + 4 + 4 + 8 + 1;
}

/// Rollback plan
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct RollbackPlan {
    pub rollback_steps: Vec<RollbackStep>,
    pub estimated_rollback_time: u64,
    pub rollback_authority: Option<Pubkey>,
}

impl RollbackPlan {
    pub const SIZE: usize = 4 + 10 * RollbackStep::SIZE + 8 + 33;

    pub fn execute(&self) -> Result<(), ProgramError> {
        msg!("Executing {} rollback steps", self.rollback_steps.len());
        // In production, would execute actual rollback
        Ok(())
    }
}

/// Rollback step
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RollbackStep {
    pub step_description: String,
    pub is_automated: bool,
    pub estimated_time: u64,
}

impl RollbackStep {
    pub const SIZE: usize = 100 + 1 + 8;
}

/// Launch report
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LaunchReport {
    pub launch_id: u128,
    pub current_phase: LaunchPhase,
    pub launch_status: LaunchStatus,
    pub overall_progress: u16,
    pub phase_progress: u16,
    pub critical_items: Vec<ChecklistItem>,
    pub pending_items: Vec<ChecklistItem>,
    pub failed_items: Vec<ChecklistItem>,
    pub warnings: Vec<ChecklistItem>,
    pub go_no_go_decision: GoNoGoDecision,
    pub estimated_completion: i64,
    pub next_actions: Vec<String>,
}

/// Process launch checklist instructions
pub fn process_launch_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_checklist(program_id, accounts, &instruction_data[1..]),
        1 => process_execute_checklist(program_id, accounts),
        2 => process_add_confirmation(program_id, accounts, &instruction_data[1..]),
        3 => process_advance_phase(program_id, accounts),
        4 => process_execute_rollback(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_checklist(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let checklist_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let launch_id = u128::from_le_bytes(data[0..16].try_into().unwrap());

    let mut checklist = LaunchChecklist::try_from_slice(&checklist_account.data.borrow())?;
    checklist.initialize(launch_id)?;
    checklist.serialize(&mut &mut checklist_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_execute_checklist(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let checklist_account = next_account_info(account_iter)?;
    let report_account = next_account_info(account_iter)?;

    let mut checklist = LaunchChecklist::try_from_slice(&checklist_account.data.borrow())?;
    let report = checklist.execute_checklist()?;
    
    checklist.serialize(&mut &mut checklist_account.data.borrow_mut()[..])?;
    report.serialize(&mut &mut report_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_add_confirmation(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let checklist_account = next_account_info(account_iter)?;
    let approver_account = next_account_info(account_iter)?;

    if !approver_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let item_description = String::from_utf8(data[0..100].to_vec())
        .map_err(|_| ProgramError::InvalidInstructionData)?
        .trim_end_matches('\0')
        .to_string();
    let confirmed = data[100] != 0;

    let mut checklist = LaunchChecklist::try_from_slice(&checklist_account.data.borrow())?;
    checklist.add_manual_confirmation(item_description, confirmed, *approver_account.key)?;
    checklist.serialize(&mut &mut checklist_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_advance_phase(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let checklist_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut checklist = LaunchChecklist::try_from_slice(&checklist_account.data.borrow())?;
    checklist.advance_phase()?;
    checklist.serialize(&mut &mut checklist_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_execute_rollback(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let checklist_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut checklist = LaunchChecklist::try_from_slice(&checklist_account.data.borrow())?;
    checklist.execute_rollback()?;
    checklist.serialize(&mut &mut checklist_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;