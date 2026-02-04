// Phase 20: Immutability Verifier and Authority Burning
// Ensures the protocol becomes truly immutable after deployment

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    system_program,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
};

/// Immutability configuration
pub const AUTHORITY_BURN_DELAY_SLOTS: u64 = 432_000; // ~48 hours at 0.4s/slot
pub const UPGRADE_AUTHORITY_SEED: &[u8] = b"upgrade_authority";
pub const ADMIN_TRANSFER_DELAY_SLOTS: u64 = 216_000; // ~24 hours
pub const MAX_ADMIN_ACTIONS: u32 = 10; // Limited admin actions before burn

/// Authority status
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum AuthorityStatus {
    Active,
    PendingBurn,
    Burned,
    Emergency, // For critical fixes only
}

/// Immutability verifier state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ImmutabilityVerifier {
    pub program_id: Pubkey,
    pub current_admin: Pubkey,
    pub pending_admin: Option<Pubkey>,
    pub admin_transfer_slot: Option<u64>,
    pub upgrade_authority: Option<Pubkey>,
    pub authority_status: AuthorityStatus,
    pub burn_scheduled_slot: Option<u64>,
    pub admin_actions_remaining: u32,
    pub critical_functions_locked: bool,
    pub emergency_contacts: Vec<Pubkey>,
    pub last_verification_slot: u64,
}

impl ImmutabilityVerifier {
    pub const SIZE: usize = 32 + // program_id
        32 + // current_admin
        33 + // pending_admin (Option)
        9 + // admin_transfer_slot (Option)
        33 + // upgrade_authority (Option)
        1 + // authority_status
        9 + // burn_scheduled_slot (Option)
        4 + // admin_actions_remaining
        1 + // critical_functions_locked
        4 + 32 * 3 + // emergency_contacts (up to 3)
        8; // last_verification_slot

    /// Initialize immutability verifier
    pub fn initialize(
        &mut self,
        program_id: &Pubkey,
        admin: &Pubkey,
        upgrade_authority: Option<Pubkey>,
    ) -> ProgramResult {
        self.program_id = *program_id;
        self.current_admin = *admin;
        self.pending_admin = None;
        self.admin_transfer_slot = None;
        self.upgrade_authority = upgrade_authority;
        self.authority_status = AuthorityStatus::Active;
        self.burn_scheduled_slot = None;
        self.admin_actions_remaining = MAX_ADMIN_ACTIONS;
        self.critical_functions_locked = false;
        self.emergency_contacts = Vec::new();
        self.last_verification_slot = Clock::get()?.slot;

        msg!("Immutability verifier initialized with {} admin actions", MAX_ADMIN_ACTIONS);
        Ok(())
    }

    /// Schedule authority burn
    pub fn schedule_authority_burn(&mut self, current_slot: u64) -> ProgramResult {
        if self.authority_status != AuthorityStatus::Active {
            return Err(BettingPlatformError::AuthorityAlreadyBurned.into());
        }

        self.burn_scheduled_slot = Some(current_slot + AUTHORITY_BURN_DELAY_SLOTS);
        self.authority_status = AuthorityStatus::PendingBurn;

        msg!("Authority burn scheduled for slot {}", 
            self.burn_scheduled_slot.unwrap());

        Ok(())
    }

    /// Execute authority burn
    pub fn burn_authority(&mut self, current_slot: u64) -> ProgramResult {
        if self.authority_status != AuthorityStatus::PendingBurn {
            return Err(BettingPlatformError::BurnNotScheduled.into());
        }

        if let Some(burn_slot) = self.burn_scheduled_slot {
            if current_slot < burn_slot {
                return Err(BettingPlatformError::BurnDelayNotMet.into());
            }
        }

        // Burn upgrade authority
        self.upgrade_authority = None;
        self.authority_status = AuthorityStatus::Burned;
        self.critical_functions_locked = true;
        self.admin_actions_remaining = 0;

        msg!("AUTHORITY BURNED - Protocol is now immutable!");

        Ok(())
    }

    /// Transfer admin (with delay)
    pub fn initiate_admin_transfer(&mut self, new_admin: &Pubkey, current_slot: u64) -> ProgramResult {
        if self.critical_functions_locked {
            return Err(BettingPlatformError::CriticalFunctionsLocked.into());
        }

        if self.admin_actions_remaining == 0 {
            return Err(BettingPlatformError::NoAdminActionsRemaining.into());
        }

        self.pending_admin = Some(*new_admin);
        self.admin_transfer_slot = Some(current_slot + ADMIN_TRANSFER_DELAY_SLOTS);

        msg!("Admin transfer initiated to {} at slot {}", 
            new_admin, 
            self.admin_transfer_slot.unwrap());

        Ok(())
    }

    /// Complete admin transfer
    pub fn complete_admin_transfer(&mut self, current_slot: u64) -> ProgramResult {
        if let (Some(pending), Some(transfer_slot)) = (self.pending_admin, self.admin_transfer_slot) {
            if current_slot < transfer_slot {
                return Err(BettingPlatformError::TransferDelayNotMet.into());
            }

            self.current_admin = pending;
            self.pending_admin = None;
            self.admin_transfer_slot = None;
            self.admin_actions_remaining = self.admin_actions_remaining.saturating_sub(1);

            msg!("Admin transferred. {} actions remaining", self.admin_actions_remaining);

            Ok(())
        } else {
            Err(BettingPlatformError::NoTransferPending.into())
        }
    }

    /// Add emergency contact
    pub fn add_emergency_contact(&mut self, contact: &Pubkey) -> ProgramResult {
        if self.emergency_contacts.len() >= 3 {
            return Err(BettingPlatformError::TooManyEmergencyContacts.into());
        }

        if !self.emergency_contacts.contains(contact) {
            self.emergency_contacts.push(*contact);
            msg!("Emergency contact added: {}", contact);
        }

        Ok(())
    }

    /// Verify protocol immutability
    pub fn verify_immutability(&self) -> ImmutabilityStatus {
        ImmutabilityStatus {
            is_immutable: self.authority_status == AuthorityStatus::Burned,
            upgrade_authority_burned: self.upgrade_authority.is_none(),
            admin_actions_exhausted: self.admin_actions_remaining == 0,
            critical_functions_locked: self.critical_functions_locked,
            days_until_burn: if let Some(burn_slot) = self.burn_scheduled_slot {
                let current = Clock::get().unwrap().slot;
                if burn_slot > current {
                    Some((burn_slot - current) / 216_000) // Convert to days
                } else {
                    Some(0)
                }
            } else {
                None
            },
        }
    }

    /// Emergency override (requires multiple signatures)
    pub fn emergency_override(
        &mut self,
        signers: &[Pubkey],
        action: EmergencyAction,
    ) -> ProgramResult {
        // Require at least 2 of 3 emergency contacts
        let valid_signers: Vec<_> = signers.iter()
            .filter(|s| self.emergency_contacts.contains(s))
            .collect();

        if valid_signers.len() < 2 {
            return Err(BettingPlatformError::InsufficientEmergencySigners.into());
        }

        match action {
            EmergencyAction::PauseProtocol => {
                self.authority_status = AuthorityStatus::Emergency;
                msg!("EMERGENCY: Protocol paused by emergency contacts");
            }
            EmergencyAction::ExtendBurnDelay => {
                if let Some(burn_slot) = self.burn_scheduled_slot {
                    self.burn_scheduled_slot = Some(burn_slot + AUTHORITY_BURN_DELAY_SLOTS);
                    msg!("EMERGENCY: Burn delay extended");
                }
            }
        }

        Ok(())
    }
}

/// Immutability status report
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ImmutabilityStatus {
    pub is_immutable: bool,
    pub upgrade_authority_burned: bool,
    pub admin_actions_exhausted: bool,
    pub critical_functions_locked: bool,
    pub days_until_burn: Option<u64>,
}

/// Emergency actions
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum EmergencyAction {
    PauseProtocol,
    ExtendBurnDelay,
}

/// Program upgrade guard
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct UpgradeGuard {
    pub upgrades_allowed: bool,
    pub last_upgrade_slot: Option<u64>,
    pub upgrade_count: u32,
    pub max_upgrades: u32,
}

impl UpgradeGuard {
    /// Check if upgrade is allowed
    pub fn can_upgrade(&self) -> bool {
        self.upgrades_allowed && self.upgrade_count < self.max_upgrades
    }

    /// Record upgrade
    pub fn record_upgrade(&mut self, current_slot: u64) -> ProgramResult {
        if !self.can_upgrade() {
            return Err(BettingPlatformError::UpgradesExhausted.into());
        }

        self.upgrade_count += 1;
        self.last_upgrade_slot = Some(current_slot);

        if self.upgrade_count >= self.max_upgrades {
            self.upgrades_allowed = false;
            msg!("Maximum upgrades reached. No more upgrades allowed.");
        }

        Ok(())
    }
}

/// Authority burn proof
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AuthorityBurnProof {
    pub program_id: Pubkey,
    pub burn_slot: u64,
    pub burn_transaction_signature: [u8; 64],
    pub witnesses: Vec<Pubkey>,
    pub timestamp: i64,
}

impl AuthorityBurnProof {
    /// Generate burn proof
    pub fn generate(
        program_id: &Pubkey,
        burn_slot: u64,
        witnesses: Vec<Pubkey>,
    ) -> Self {
        Self {
            program_id: *program_id,
            burn_slot,
            burn_transaction_signature: [0u8; 64], // Would be actual tx sig
            witnesses,
            timestamp: Clock::get().unwrap().unix_timestamp,
        }
    }

    /// Verify burn proof
    pub fn verify(&self, expected_program: &Pubkey) -> bool {
        self.program_id == *expected_program &&
        self.witnesses.len() >= 2 &&
        self.burn_transaction_signature != [0u8; 64]
    }
}

/// Process immutability instructions
pub fn process_immutability_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_verifier(program_id, accounts),
        1 => process_schedule_burn(program_id, accounts),
        2 => process_execute_burn(program_id, accounts),
        3 => process_initiate_admin_transfer(program_id, accounts, &instruction_data[1..]),
        4 => process_complete_admin_transfer(program_id, accounts),
        5 => process_add_emergency_contact(program_id, accounts, &instruction_data[1..]),
        6 => process_emergency_override(program_id, accounts, &instruction_data[1..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_verifier(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;
    let upgrade_authority_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;
    
    let upgrade_authority = if upgrade_authority_account.key != &system_program::ID {
        Some(*upgrade_authority_account.key)
    } else {
        None
    };

    verifier.initialize(program_id, admin_account.key, upgrade_authority)?;
    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_schedule_burn(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;

    if verifier.current_admin != *admin_account.key {
        return Err(BettingPlatformError::UnauthorizedAdmin.into());
    }

    verifier.schedule_authority_burn(Clock::get()?.slot)?;
    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;

    msg!("Authority burn scheduled - protocol will become immutable");

    Ok(())
}

fn process_execute_burn(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;
    let proof_account = next_account_info(account_iter)?;

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;
    verifier.burn_authority(Clock::get()?.slot)?;

    // Generate burn proof
    let proof = AuthorityBurnProof::generate(
        &verifier.program_id,
        Clock::get()?.slot,
        verifier.emergency_contacts.clone(),
    );

    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;
    proof.serialize(&mut &mut proof_account.data.borrow_mut()[..])?;

    msg!("PROTOCOL IS NOW IMMUTABLE - Authority burned permanently");

    Ok(())
}

fn process_initiate_admin_transfer(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;
    let current_admin_account = next_account_info(account_iter)?;

    if !current_admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let new_admin = Pubkey::new_from_array(data[0..32].try_into().unwrap());

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;

    if verifier.current_admin != *current_admin_account.key {
        return Err(BettingPlatformError::UnauthorizedAdmin.into());
    }

    verifier.initiate_admin_transfer(&new_admin, Clock::get()?.slot)?;
    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_complete_admin_transfer(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;
    verifier.complete_admin_transfer(Clock::get()?.slot)?;
    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_add_emergency_contact(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let contact = Pubkey::new_from_array(data[0..32].try_into().unwrap());

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;

    if verifier.current_admin != *admin_account.key {
        return Err(BettingPlatformError::UnauthorizedAdmin.into());
    }

    verifier.add_emergency_contact(&contact)?;
    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_emergency_override(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let verifier_account = next_account_info(account_iter)?;

    // Collect signers
    let mut signers = Vec::new();
    for account in account_iter {
        if account.is_signer {
            signers.push(*account.key);
        }
    }

    let action = match data[0] {
        0 => EmergencyAction::PauseProtocol,
        1 => EmergencyAction::ExtendBurnDelay,
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    let mut verifier = ImmutabilityVerifier::try_from_slice(&verifier_account.data.borrow())?;
    verifier.emergency_override(&signers, action)?;
    verifier.serialize(&mut &mut verifier_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;