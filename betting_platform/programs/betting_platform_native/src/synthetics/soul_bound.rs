//! Soul-Bound Token Enforcement
//!
//! Ensures synthetic tokens cannot be transferred

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use spl_token_2022::{
    state::{Account as TokenAccount, Mint},
    extension::{
        BaseStateWithExtensions,
        StateWithExtensions,
        non_transferable::NonTransferable,
    },
};

use crate::error::BettingPlatformError;

/// Soul-bound restriction types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum SoulBoundRestriction {
    /// Completely non-transferable
    FullRestriction,
    /// Can only transfer back to protocol
    ProtocolOnly,
    /// Can transfer within same wallet
    SameWalletOnly,
    /// Can transfer for liquidation only
    LiquidationOnly,
    /// Temporary restriction (can be lifted)
    TemporaryRestriction { until_slot: u64 },
}

/// Transfer restriction configuration
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct TransferRestriction {
    /// Type of restriction
    pub restriction_type: SoulBoundRestriction,
    
    /// Protocol authority that can override
    pub protocol_authority: Pubkey,
    
    /// Whitelist of allowed recipients (if any)
    pub allowed_recipients: Vec<Pubkey>,
    
    /// Is restriction active
    pub is_active: bool,
    
    /// Creation slot
    pub created_at_slot: u64,
    
    /// Last update slot
    pub last_updated_slot: u64,
    
    /// Total blocked transfers
    pub blocked_transfers: u64,
    
    /// Emergency override enabled
    pub emergency_override: bool,
}

impl TransferRestriction {
    pub fn new(
        restriction_type: SoulBoundRestriction,
        protocol_authority: Pubkey,
        current_slot: u64,
    ) -> Self {
        Self {
            restriction_type,
            protocol_authority,
            allowed_recipients: vec![protocol_authority], // Protocol can always receive
            is_active: true,
            created_at_slot: current_slot,
            last_updated_slot: current_slot,
            blocked_transfers: 0,
            emergency_override: false,
        }
    }
    
    /// Check if transfer is allowed
    pub fn is_transfer_allowed(
        &self,
        sender: &Pubkey,
        recipient: &Pubkey,
        current_slot: u64,
    ) -> bool {
        if !self.is_active || self.emergency_override {
            return true;
        }
        
        match &self.restriction_type {
            SoulBoundRestriction::FullRestriction => {
                // No transfers allowed except burns (to self)
                sender == recipient
            }
            SoulBoundRestriction::ProtocolOnly => {
                // Only allow transfers to protocol
                self.allowed_recipients.contains(recipient) || sender == recipient
            }
            SoulBoundRestriction::SameWalletOnly => {
                // Only allow transfers within same wallet
                sender == recipient
            }
            SoulBoundRestriction::LiquidationOnly => {
                // Check if this is a liquidation transfer
                // In production, this would check liquidation state
                self.allowed_recipients.contains(recipient)
            }
            SoulBoundRestriction::TemporaryRestriction { until_slot } => {
                // Check if restriction has expired
                if current_slot > *until_slot {
                    true
                } else {
                    sender == recipient || self.allowed_recipients.contains(recipient)
                }
            }
        }
    }
    
    /// Record a blocked transfer
    pub fn record_blocked_transfer(&mut self) {
        self.blocked_transfers += 1;
    }
    
    /// Add an allowed recipient
    pub fn add_allowed_recipient(&mut self, recipient: Pubkey) {
        if !self.allowed_recipients.contains(&recipient) {
            self.allowed_recipients.push(recipient);
        }
    }
    
    /// Remove an allowed recipient
    pub fn remove_allowed_recipient(&mut self, recipient: &Pubkey) {
        self.allowed_recipients.retain(|r| r != recipient);
    }
    
    /// Activate emergency override
    pub fn enable_emergency_override(&mut self) {
        self.emergency_override = true;
    }
}

/// Validate that a token mint is soul-bound
pub fn validate_soul_bound(
    mint_account: &AccountInfo,
) -> Result<bool, ProgramError> {
    msg!("Validating soul-bound token");
    
    // Check if account is owned by token program
    if mint_account.owner != &spl_token_2022::id() {
        msg!("Account not owned by token program");
        return Ok(false);
    }
    
    // Parse mint data
    let mint_data = mint_account.try_borrow_data()?;
    
    // Try to unpack as Token-2022 mint with extensions
    match StateWithExtensions::<Mint>::unpack(&mint_data) {
        Ok(mint) => {
            // Check for non-transferable extension
            match mint.get_extension::<NonTransferable>() {
                Ok(_) => {
                    msg!("Token has non-transferable extension");
                    Ok(true)
                }
                Err(_) => {
                    msg!("Token does not have non-transferable extension");
                    Ok(false)
                }
            }
        }
        Err(_) => {
            msg!("Failed to parse mint with extensions");
            Ok(false)
        }
    }
}

/// Enforce non-transferable restriction
pub fn enforce_non_transferable(
    source_account: &AccountInfo,
    destination_account: &AccountInfo,
    authority: &AccountInfo,
    restriction: &TransferRestriction,
    current_slot: u64,
) -> ProgramResult {
    msg!("Enforcing soul-bound transfer restriction");
    
    // Check if transfer is allowed
    if !restriction.is_transfer_allowed(
        source_account.key,
        destination_account.key,
        current_slot,
    ) {
        msg!("Transfer blocked by soul-bound restriction");
        return Err(BettingPlatformError::TransferRestricted.into());
    }
    
    // Special case: Allow burns (transfer to self)
    if source_account.key == destination_account.key {
        msg!("Burn operation allowed");
        return Ok(());
    }
    
    // Check if authority is protocol
    if authority.key != &restriction.protocol_authority {
        msg!("Only protocol authority can initiate transfers");
        return Err(BettingPlatformError::UnauthorizedTransfer.into());
    }
    
    Ok(())
}

/// Transfer hook for soul-bound tokens
pub struct SoulBoundTransferHook;

impl SoulBoundTransferHook {
    /// Execute transfer hook
    pub fn execute(
        source: &AccountInfo,
        mint: &AccountInfo,
        destination: &AccountInfo,
        authority: &AccountInfo,
        amount: u64,
    ) -> ProgramResult {
        msg!("Executing soul-bound transfer hook");
        
        // Always reject transfers except burns
        if source.key != destination.key {
            msg!("Soul-bound tokens cannot be transferred");
            return Err(BettingPlatformError::TransferRestricted.into());
        }
        
        msg!("Transfer hook passed (burn operation)");
        Ok(())
    }
}

/// Create transfer restriction for a token
pub fn create_transfer_restriction(
    restriction_type: SoulBoundRestriction,
    protocol_authority: Pubkey,
    current_slot: u64,
) -> TransferRestriction {
    TransferRestriction::new(restriction_type, protocol_authority, current_slot)
}

/// Check if account can receive tokens
pub fn can_receive_tokens(
    account: &AccountInfo,
    mint: &AccountInfo,
    restriction: &TransferRestriction,
) -> Result<bool, ProgramError> {
    // Parse token account
    let account_data = account.try_borrow_data()?;
    
    // Check if account exists and is initialized
    if account_data.len() < TokenAccount::LEN {
        return Ok(false);
    }
    
    let token_account = TokenAccount::unpack(&account_data)?;
    
    // Check if mint matches
    if token_account.mint != *mint.key {
        return Ok(false);
    }
    
    // Check if account is frozen
    if token_account.is_frozen() {
        return Ok(false);
    }
    
    // Check restriction allows receiving
    Ok(restriction.allowed_recipients.contains(account.key))
}

/// Validate transfer attempt
pub fn validate_transfer(
    source: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    restriction: &TransferRestriction,
    current_slot: u64,
) -> Result<(), ProgramError> {
    // Check basic transfer rules
    if !restriction.is_transfer_allowed(source, destination, current_slot) {
        msg!("Transfer not allowed by restriction rules");
        return Err(BettingPlatformError::TransferRestricted.into());
    }
    
    // Check authority
    if authority != &restriction.protocol_authority && source != authority {
        msg!("Invalid transfer authority");
        return Err(BettingPlatformError::UnauthorizedTransfer.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Clock;
    
    #[test]
    fn test_transfer_restrictions() {
        let protocol = Pubkey::new_unique();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();
        
        // Test full restriction
        let full_restriction = TransferRestriction::new(
            SoulBoundRestriction::FullRestriction,
            protocol,
            1000,
        );
        
        // Should block all transfers except self
        assert!(!full_restriction.is_transfer_allowed(&user1, &user2, 1000));
        assert!(full_restriction.is_transfer_allowed(&user1, &user1, 1000));
        
        // Test protocol-only restriction
        let mut protocol_restriction = TransferRestriction::new(
            SoulBoundRestriction::ProtocolOnly,
            protocol,
            1000,
        );
        
        // Should allow transfers to protocol
        assert!(protocol_restriction.is_transfer_allowed(&user1, &protocol, 1000));
        assert!(!protocol_restriction.is_transfer_allowed(&user1, &user2, 1000));
        
        // Test temporary restriction
        let temp_restriction = TransferRestriction::new(
            SoulBoundRestriction::TemporaryRestriction { until_slot: 2000 },
            protocol,
            1000,
        );
        
        // Should block before expiry
        assert!(!temp_restriction.is_transfer_allowed(&user1, &user2, 1500));
        // Should allow after expiry
        assert!(temp_restriction.is_transfer_allowed(&user1, &user2, 2001));
    }
    
    #[test]
    fn test_allowed_recipients() {
        let protocol = Pubkey::new_unique();
        let user1 = Pubkey::new_unique();
        let liquidator = Pubkey::new_unique();
        
        let mut restriction = TransferRestriction::new(
            SoulBoundRestriction::LiquidationOnly,
            protocol,
            1000,
        );
        
        // Add liquidator as allowed recipient
        restriction.add_allowed_recipient(liquidator);
        
        // Should allow transfer to liquidator
        assert!(restriction.is_transfer_allowed(&user1, &liquidator, 1000));
        
        // Remove liquidator
        restriction.remove_allowed_recipient(&liquidator);
        
        // Should no longer allow transfer
        assert!(!restriction.is_transfer_allowed(&user1, &liquidator, 1000));
    }
    
    #[test]
    fn test_emergency_override() {
        let protocol = Pubkey::new_unique();
        let user1 = Pubkey::new_unique();
        let user2 = Pubkey::new_unique();
        
        let mut restriction = TransferRestriction::new(
            SoulBoundRestriction::FullRestriction,
            protocol,
            1000,
        );
        
        // Should block transfers normally
        assert!(!restriction.is_transfer_allowed(&user1, &user2, 1000));
        
        // Enable emergency override
        restriction.enable_emergency_override();
        
        // Should now allow transfers
        assert!(restriction.is_transfer_allowed(&user1, &user2, 1000));
    }
}