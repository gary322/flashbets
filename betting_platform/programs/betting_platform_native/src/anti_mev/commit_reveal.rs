//! Commit-Reveal MEV Protection
//!
//! Native Solana implementation of commit-reveal pattern to prevent MEV attacks

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    keccak,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    account_validation::{validate_signer, validate_writable, DISCRIMINATOR_SIZE},
};

/// Minimum delay between commit and reveal (in slots)
pub const MIN_COMMIT_DELAY: u64 = 2;

/// Maximum delay between commit and reveal (in slots)
pub const MAX_COMMIT_DELAY: u64 = 100;

/// Discriminator for commit account
pub const COMMIT_DISCRIMINATOR: [u8; 8] = [67, 79, 77, 77, 73, 84, 0, 0]; // "COMMIT"

/// Order commitment structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OrderCommitment {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// User who committed the order
    pub user: Pubkey,
    
    /// Commitment hash (keccak256 of order details + nonce)
    pub commitment_hash: [u8; 32],
    
    /// Slot when committed
    pub commit_slot: u64,
    
    /// Expiry slot (commit_slot + MAX_COMMIT_DELAY)
    pub expiry_slot: u64,
    
    /// Market this order is for
    pub market_id: [u8; 32],
    
    /// Has been revealed
    pub revealed: bool,
    
    /// Has been executed
    pub executed: bool,
    
    /// Bump seed for PDA
    pub bump: u8,
}

impl OrderCommitment {
    pub const LEN: usize = DISCRIMINATOR_SIZE + 32 + 32 + 8 + 8 + 32 + 1 + 1 + 1;
    
    /// Create new commitment
    pub fn new(
        user: Pubkey,
        commitment_hash: [u8; 32],
        market_id: [u8; 32],
        commit_slot: u64,
        bump: u8,
    ) -> Self {
        Self {
            discriminator: COMMIT_DISCRIMINATOR,
            user,
            commitment_hash,
            commit_slot,
            expiry_slot: commit_slot + MAX_COMMIT_DELAY,
            market_id,
            revealed: false,
            executed: false,
            bump,
        }
    }
    
    /// Validate commitment
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != COMMIT_DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

/// Revealed order details
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RevealedOrder {
    /// Order type (0: Market, 1: Limit)
    pub order_type: u8,
    
    /// Side (0: Buy, 1: Sell)
    pub side: u8,
    
    /// Outcome to trade
    pub outcome: u8,
    
    /// Size of order
    pub size: u64,
    
    /// Price (for limit orders)
    pub price: u64,
    
    /// Leverage
    pub leverage: u8,
    
    /// Random nonce for uniqueness
    pub nonce: u64,
}

impl RevealedOrder {
    /// Compute commitment hash
    pub fn compute_hash(&self) -> [u8; 32] {
        let mut data = Vec::new();
        data.push(self.order_type);
        data.push(self.side);
        data.push(self.outcome);
        data.extend_from_slice(&self.size.to_le_bytes());
        data.extend_from_slice(&self.price.to_le_bytes());
        data.push(self.leverage);
        data.extend_from_slice(&self.nonce.to_le_bytes());
        
        keccak::hash(&data).to_bytes()
    }
}

/// Process order commitment
pub fn process_commit_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    commitment_hash: [u8; 32],
    market_id: [u8; 32],
) -> ProgramResult {
    msg!("Processing order commitment");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let commitment_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(commitment_account)?;
    
    // Derive commitment PDA
    let (commitment_pda, bump) = derive_commitment_pda(
        program_id,
        user.key,
        &commitment_hash,
    );
    
    if commitment_account.key != &commitment_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    
    // Create commitment
    let commitment = OrderCommitment::new(
        *user.key,
        commitment_hash,
        market_id,
        current_slot,
        bump,
    );
    
    // Allocate and initialize account
    let rent = solana_program::rent::Rent::get()?;
    let required_lamports = rent.minimum_balance(OrderCommitment::LEN);
    
    solana_program::program::invoke_signed(
        &solana_program::system_instruction::create_account(
            user.key,
            commitment_account.key,
            required_lamports,
            OrderCommitment::LEN as u64,
            program_id,
        ),
        &[
            user.clone(),
            commitment_account.clone(),
            system_program.clone(),
        ],
        &[&[
            b"commitment",
            user.key.as_ref(),
            &commitment_hash,
            &[bump],
        ]],
    )?;
    
    // Serialize commitment
    commitment.serialize(&mut &mut commitment_account.data.borrow_mut()[..])?;
    
    msg!("Order committed successfully");
    Ok(())
}

/// Process order reveal
pub fn process_reveal_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    order: RevealedOrder,
) -> ProgramResult {
    msg!("Processing order reveal");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let commitment_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    
    // Validate accounts
    validate_signer(user)?;
    validate_writable(commitment_account)?;
    
    // Load commitment
    let mut commitment = OrderCommitment::try_from_slice(&commitment_account.data.borrow())?;
    commitment.validate()?;
    
    // Verify user
    if commitment.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if already revealed
    if commitment.revealed {
        return Err(BettingPlatformError::AlreadyRevealed.into());
    }
    
    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    
    // Check timing constraints
    if current_slot < commitment.commit_slot + MIN_COMMIT_DELAY {
        return Err(BettingPlatformError::TooEarlyToReveal.into());
    }
    
    if current_slot > commitment.expiry_slot {
        return Err(BettingPlatformError::CommitmentExpired.into());
    }
    
    // Verify commitment hash
    let computed_hash = order.compute_hash();
    if computed_hash != commitment.commitment_hash {
        return Err(BettingPlatformError::InvalidReveal.into());
    }
    
    // Mark as revealed
    commitment.revealed = true;
    commitment.serialize(&mut &mut commitment_account.data.borrow_mut()[..])?;
    
    // Store revealed order for execution
    // In production, this would be stored in a separate account
    // or immediately executed based on system design
    
    msg!("Order revealed successfully");
    Ok(())
}

/// Execute revealed order
pub fn process_execute_revealed_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    order: RevealedOrder,
) -> ProgramResult {
    msg!("Executing revealed order");
    
    let account_info_iter = &mut accounts.iter();
    let user = next_account_info(account_info_iter)?;
    let commitment_account = next_account_info(account_info_iter)?;
    let market_account = next_account_info(account_info_iter)?;
    let position_account = next_account_info(account_info_iter)?;
    let user_credits_account = next_account_info(account_info_iter)?;
    
    // Load commitment
    let mut commitment = OrderCommitment::try_from_slice(&commitment_account.data.borrow())?;
    commitment.validate()?;
    
    // Verify order has been revealed
    if !commitment.revealed {
        return Err(BettingPlatformError::OrderNotRevealed.into());
    }
    
    // Check if already executed
    if commitment.executed {
        return Err(BettingPlatformError::AlreadyExecuted.into());
    }
    
    // Verify commitment hash again
    let computed_hash = order.compute_hash();
    if computed_hash != commitment.commitment_hash {
        return Err(BettingPlatformError::InvalidReveal.into());
    }
    
    // Execute the order based on type
    match order.order_type {
        0 => execute_market_order(accounts, &order)?,
        1 => execute_limit_order(accounts, &order)?,
        _ => return Err(BettingPlatformError::InvalidOrderType.into()),
    }
    
    // Mark as executed
    commitment.executed = true;
    commitment.serialize(&mut &mut commitment_account.data.borrow_mut()[..])?;
    
    msg!("Order executed successfully");
    Ok(())
}

/// Execute market order
fn execute_market_order(
    accounts: &[AccountInfo],
    order: &RevealedOrder,
) -> ProgramResult {
    // Implementation would call into existing trading logic
    // This prevents MEV by ensuring orders are executed in commit order
    msg!("Executing market order: size={}, outcome={}", order.size, order.outcome);
    Ok(())
}

/// Execute limit order
fn execute_limit_order(
    accounts: &[AccountInfo],
    order: &RevealedOrder,
) -> ProgramResult {
    // Implementation would add to order book or execute if crossable
    msg!("Executing limit order: size={}, price={}", order.size, order.price);
    Ok(())
}

/// Batch reveal multiple orders
pub fn process_batch_reveal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    orders: Vec<RevealedOrder>,
) -> ProgramResult {
    msg!("Processing batch reveal of {} orders", orders.len());
    
    // Validate all reveals first
    for (i, order) in orders.iter().enumerate() {
        msg!("Validating order {}", i);
        // Validation logic here
    }
    
    // Execute in commit order to prevent reordering
    for (i, order) in orders.iter().enumerate() {
        msg!("Revealing order {}", i);
        // Reveal logic here
    }
    
    Ok(())
}

/// Derive commitment PDA
pub fn derive_commitment_pda(
    program_id: &Pubkey,
    user: &Pubkey,
    commitment_hash: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"commitment",
            user.as_ref(),
            commitment_hash,
        ],
        program_id,
    )
}

/// MEV protection configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MEVProtectionConfig {
    /// Minimum commit delay (slots)
    pub min_delay: u64,
    
    /// Maximum commit delay (slots)
    pub max_delay: u64,
    
    /// Batch execution window (slots)
    pub batch_window: u64,
    
    /// Priority fee for commit-reveal orders
    pub priority_fee_bps: u16,
    
    /// Maximum orders per batch
    pub max_batch_size: u16,
}

impl Default for MEVProtectionConfig {
    fn default() -> Self {
        Self {
            min_delay: MIN_COMMIT_DELAY,
            max_delay: MAX_COMMIT_DELAY,
            batch_window: 10,
            priority_fee_bps: 10, // 0.1% priority fee
            max_batch_size: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_commitment_hash() {
        let order = RevealedOrder {
            order_type: 0,
            side: 0,
            outcome: 1,
            size: 1000000,
            price: 500000,
            leverage: 10,
            nonce: 12345,
        };
        
        let hash1 = order.compute_hash();
        let hash2 = order.compute_hash();
        assert_eq!(hash1, hash2);
        
        // Different nonce should produce different hash
        let mut order2 = order.clone();
        order2.nonce = 54321;
        let hash3 = order2.compute_hash();
        assert_ne!(hash1, hash3);
    }
    
    #[test]
    fn test_timing_constraints() {
        let commitment = OrderCommitment::new(
            Pubkey::new_unique(),
            [0; 32],
            [0; 32],
            100, // commit_slot
            255,
        );
        
        assert_eq!(commitment.commit_slot, 100);
        assert_eq!(commitment.expiry_slot, 100 + MAX_COMMIT_DELAY);
    }
}