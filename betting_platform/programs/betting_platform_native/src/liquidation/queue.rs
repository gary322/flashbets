//! Liquidation queue management
//!
//! Implements priority-based liquidation queue for efficient processing

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
    events::{Event, EventType},
    keeper_liquidation::{LIQUIDATION_THRESHOLD, MONITORING_THRESHOLD},
    liquidation::calculate_risk_score_with_price,
    math::U64F64,
    state::{Position},
};

/// Maximum positions in liquidation queue
pub const MAX_QUEUE_SIZE: usize = 100;

/// Liquidation queue account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct LiquidationQueue {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// Queue of at-risk positions
    pub positions: Vec<LiquidationCandidate>,
    
    /// Total liquidatable value
    pub total_liquidatable_value: u64,
    
    /// Last scan slot
    pub last_scan_slot: u64,
    
    /// Scan in progress flag
    pub scan_in_progress: bool,
    
    /// Number of positions processed
    pub positions_processed: u64,
    
    /// Number of liquidations executed
    pub liquidations_executed: u64,
}

/// Liquidation candidate
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct LiquidationCandidate {
    /// Position account pubkey
    pub position_pubkey: Pubkey,
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Risk score (0-100)
    pub risk_score: u8,
    
    /// Health factor (basis points where 10000 = 1.0)
    pub health_factor: u64,
    
    /// Liquidatable amount
    pub liquidatable_amount: u64,
    
    /// Priority score (higher = more urgent)
    pub priority_score: u64,
    
    /// Added to queue timestamp
    pub added_at: i64,
}

impl LiquidationQueue {
    pub const DISCRIMINATOR: [u8; 8] = [76, 73, 81, 85, 73, 68, 81, 85]; // "LIQUIDQU"
    
    pub fn new() -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            positions: Vec::with_capacity(MAX_QUEUE_SIZE),
            total_liquidatable_value: 0,
            last_scan_slot: 0,
            scan_in_progress: false,
            positions_processed: 0,
            liquidations_executed: 0,
        }
    }
    
    /// Add position to liquidation queue
    pub fn add_candidate(
        &mut self,
        position_pubkey: &Pubkey,
        position: &Position,
        risk_score: u8,
        health_factor: u64,
        current_price: u64,
    ) -> Result<(), ProgramError> {
        // Only add if above monitoring threshold
        if risk_score < MONITORING_THRESHOLD {
            return Ok(());
        }
        
        // Check if already in queue
        if self.positions.iter().any(|c| c.position_pubkey == *position_pubkey) {
            return Ok(());
        }
        
        // Calculate liquidatable amount
        let liquidatable_amount = calculate_liquidatable_amount(position, current_price)?;
        
        // Calculate priority score (higher risk = higher priority)
        let priority_score = calculate_priority_score(risk_score, health_factor, liquidatable_amount);
        
        let candidate = LiquidationCandidate {
            position_pubkey: *position_pubkey,
            user: position.user,
            risk_score,
            health_factor,
            liquidatable_amount,
            priority_score,
            added_at: Clock::get()?.unix_timestamp,
        };
        
        // Add to queue
        self.positions.push(candidate);
        self.total_liquidatable_value += liquidatable_amount;
        
        // Sort by priority (highest first)
        self.positions.sort_by(|a, b| b.priority_score.cmp(&a.priority_score));
        
        // Limit queue size
        if self.positions.len() > MAX_QUEUE_SIZE {
            let removed = self.positions.pop().unwrap();
            self.total_liquidatable_value = self.total_liquidatable_value
                .saturating_sub(removed.liquidatable_amount);
        }
        
        Ok(())
    }
    
    /// Get next batch for liquidation
    pub fn get_next_batch(&mut self, max_batch_size: usize) -> Vec<LiquidationCandidate> {
        let batch_size = max_batch_size.min(self.positions.len());
        let mut batch = Vec::with_capacity(batch_size);
        
        // Get positions above liquidation threshold
        let mut i = 0;
        while i < self.positions.len() && batch.len() < batch_size {
            if self.positions[i].risk_score >= LIQUIDATION_THRESHOLD {
                let candidate = self.positions.remove(i);
                self.total_liquidatable_value = self.total_liquidatable_value
                    .saturating_sub(candidate.liquidatable_amount);
                batch.push(candidate);
            } else {
                i += 1;
            }
        }
        
        batch
    }
    
    /// Remove position from queue
    pub fn remove_position(&mut self, position_pubkey: &Pubkey) -> Result<(), ProgramError> {
        if let Some(index) = self.positions.iter().position(|c| c.position_pubkey == *position_pubkey) {
            let removed = self.positions.remove(index);
            self.total_liquidatable_value = self.total_liquidatable_value
                .saturating_sub(removed.liquidatable_amount);
            Ok(())
        } else {
            Err(BettingPlatformError::InvalidPosition.into())
        }
    }
    
    /// Clear stale entries
    pub fn clear_stale_entries(&mut self, current_slot: u64, max_age_slots: u64) {
        let cutoff_slot = current_slot.saturating_sub(max_age_slots);
        
        self.positions.retain(|candidate| {
            let keep = candidate.added_at as u64 > cutoff_slot;
            if !keep {
                self.total_liquidatable_value = self.total_liquidatable_value
                    .saturating_sub(candidate.liquidatable_amount);
            }
            keep
        });
    }
}

/// Calculate liquidatable amount for position
fn calculate_liquidatable_amount(position: &Position, current_price: u64) -> Result<u64, ProgramError> {
    // Base liquidatable amount is position size
    let mut liquidatable = position.size;
    
    // Adjust based on current price vs liquidation price
    let price_factor = if position.is_long {
        // Long positions: lower price = more urgent
        U64F64::from_num(position.liquidation_price)
            .checked_div(U64F64::from_num(current_price))
            .unwrap_or(U64F64::from_num(1))
    } else {
        // Short positions: higher price = more urgent
        U64F64::from_num(current_price)
            .checked_div(U64F64::from_num(position.liquidation_price))
            .unwrap_or(U64F64::from_num(1))
    };
    
    // Scale liquidatable amount by urgency
    let scaled = U64F64::from_num(liquidatable)
        .checked_mul(price_factor)?;
    
    Ok(scaled.to_num())
}

/// Calculate priority score for liquidation
fn calculate_priority_score(risk_score: u8, health_factor: u64, liquidatable_amount: u64) -> u64 {
    // Priority = risk_score * (1 / health_factor) * liquidatable_amount
    // Higher risk, lower health, larger position = higher priority
    
    let risk_weight = risk_score as u64 * 100; // Scale up risk score
    
    let health_weight = if health_factor > 0 {
        10000u64.saturating_div(health_factor) // Inverse of health factor
    } else {
        10000 // Max weight if health is 0
    };
    
    let size_weight = liquidatable_amount.saturating_div(1_000_000); // Normalize by 1 USDC
    
    // Combined priority score
    risk_weight
        .saturating_mul(health_weight)
        .saturating_mul(size_weight.max(1))
}

pub mod initialize {
    use super::*;
    
    pub fn process_initialize_queue(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        msg!("Initializing liquidation queue");
        
        let account_iter = &mut accounts.iter();
        let queue_account = next_account_info(account_iter)?;
        let authority_account = next_account_info(account_iter)?;
        
        // Validate authority
        if !authority_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Initialize queue
        let queue = LiquidationQueue::new();
        queue.serialize(&mut &mut queue_account.data.borrow_mut()[..])?;
        
        msg!("Liquidation queue initialized");
        Ok(())
    }
}

pub mod update {
    use super::*;
    
    pub fn process_update_at_risk(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        mark_price: u64,
    ) -> ProgramResult {
        msg!("Updating at-risk positions with mark price: {}", mark_price);
        
        let account_iter = &mut accounts.iter();
        let queue_account = next_account_info(account_iter)?;
        let position_account = next_account_info(account_iter)?;
        
        // Load accounts
        let mut queue = LiquidationQueue::try_from_slice(&queue_account.data.borrow())?;
        let position = Position::try_from_slice(&position_account.data.borrow())?;
        
        // Calculate risk score
        let risk_score = calculate_risk_score_with_price(&position, U64F64::from_num(mark_price))?;
        
        // Calculate health factor (simplified)
        let health_factor = calculate_health_factor(&position, mark_price)?;
        
        // Add to queue if at risk
        queue.add_candidate(
            position_account.key,
            &position,
            risk_score,
            health_factor,
            mark_price,
        )?;
        
        // Save updated queue
        queue.serialize(&mut &mut queue_account.data.borrow_mut()[..])?;
        
        msg!("Updated queue, total candidates: {}", queue.positions.len());
        Ok(())
    }
}

pub mod process {
    use super::*;
    
    pub fn process_priority_liquidation(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        max_liquidations: u8,
    ) -> ProgramResult {
        msg!("Processing priority liquidations, max: {}", max_liquidations);
        
        let account_iter = &mut accounts.iter();
        let queue_account = next_account_info(account_iter)?;
        let keeper_account = next_account_info(account_iter)?;
        
        // Validate keeper
        if !keeper_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Load queue
        let mut queue = LiquidationQueue::try_from_slice(&queue_account.data.borrow())?;
        
        // Get next batch
        let batch = queue.get_next_batch(max_liquidations as usize);
        let processed = batch.len();
        
        // Update stats
        queue.positions_processed += processed as u64;
        queue.liquidations_executed += batch.iter()
            .filter(|c| c.risk_score >= LIQUIDATION_THRESHOLD)
            .count() as u64;
        
        // Save updated queue
        queue.serialize(&mut &mut queue_account.data.borrow_mut()[..])?;
        
        // Emit event
        LiquidationBatchProcessed {
            keeper: *keeper_account.key,
            positions_processed: processed as u32,
            total_liquidated: batch.iter().map(|c| c.liquidatable_amount).sum(),
            slot: Clock::get()?.slot,
        }.emit();
        
        msg!("Processed {} liquidations", processed);
        Ok(())
    }
}

/// Calculate health factor for position
fn calculate_health_factor(position: &Position, current_price: u64) -> Result<u64, ProgramError> {
    // Health factor = margin / (position_value * maintenance_margin_ratio)
    // Returns in basis points where 10000 = 1.0
    
    let position_value = (position.size as u128)
        .checked_mul(current_price as u128)
        .ok_or(BettingPlatformError::MathOverflow)?
        .checked_div(10000) // Price is in basis points
        .ok_or(BettingPlatformError::DivisionByZero)?;
    
    if position_value == 0 {
        return Ok(0);
    }
    
    let margin = position.margin as u128;
    let health_factor = (margin * 10000) / position_value;
    
    Ok(health_factor as u64)
}

/// Liquidation batch processed event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct LiquidationBatchProcessed {
    pub keeper: Pubkey,
    pub positions_processed: u32,
    pub total_liquidated: u64,
    pub slot: u64,
}

impl Event for LiquidationBatchProcessed {
    fn event_type() -> EventType {
        EventType::LiquidationExecuted
    }
    
    fn emit(&self) {
        msg!("BETTING_PLATFORM_EVENT");
        msg!("TYPE:{:?}", Self::event_type());
        
        if let Ok(data) = self.try_to_vec() {
            msg!("DATA:{}", bs58::encode(&data).into_string());
        }
    }
}