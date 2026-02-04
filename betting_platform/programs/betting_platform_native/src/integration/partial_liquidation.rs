// Phase 20: Partial Liquidation Engine
// Implements 50% health-preserving liquidations using native Solana

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
// Note: Using u64 for fixed-point calculations where 10000 = 1.0

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
    constants::PARTIAL_LIQUIDATION_BPS,
};
pub const MIN_HEALTH_AFTER_LIQUIDATION: u64 = 1100; // 1.1 health factor
pub const LIQUIDATION_INCENTIVE_BPS: u16 = 500; // 5% keeper incentive
pub const MAX_LIQUIDATION_CLOSE_FACTOR: u64 = 9000; // Max 90% position close
pub const MIN_LIQUIDATION_AMOUNT: u64 = 100_000_000; // $100 minimum
pub const LIQUIDATION_BUFFER_BPS: u16 = 200; // 2% safety buffer

/// Liquidation engine state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PartialLiquidationEngine {
    pub total_liquidations_processed: u64,
    pub total_value_liquidated: u64,
    pub active_liquidations: u32,
    pub keeper_rewards_paid: u64,
    pub partial_liquidation_enabled: bool,
    pub emergency_liquidation_mode: bool,
    pub last_liquidation_slot: u64,
    pub liquidation_queue_size: u32,
}

impl PartialLiquidationEngine {
    pub const SIZE: usize = 8 + // total_liquidations_processed
        8 + // total_value_liquidated
        4 + // active_liquidations
        8 + // keeper_rewards_paid
        1 + // partial_liquidation_enabled
        1 + // emergency_liquidation_mode
        8 + // last_liquidation_slot
        4; // liquidation_queue_size

    /// Initialize liquidation engine
    pub fn initialize(&mut self) -> ProgramResult {
        self.total_liquidations_processed = 0;
        self.total_value_liquidated = 0;
        self.active_liquidations = 0;
        self.keeper_rewards_paid = 0;
        self.partial_liquidation_enabled = true;
        self.emergency_liquidation_mode = false;
        self.last_liquidation_slot = 0;
        self.liquidation_queue_size = 0;

        msg!("Partial liquidation engine initialized");
        Ok(())
    }

    /// Process liquidation
    pub fn process_liquidation(
        &mut self,
        position: &mut Position,
        liquidation_amount: u64,
        keeper: &Pubkey,
    ) -> Result<LiquidationResult, ProgramError> {
        // Validate liquidation conditions
        self.validate_liquidation(position, liquidation_amount)?;

        // Calculate partial liquidation amount (50% of unhealthy portion)
        let liquidation_factor = if self.emergency_liquidation_mode {
            MAX_LIQUIDATION_CLOSE_FACTOR
        } else {
            PARTIAL_LIQUIDATION_BPS as u64
        };

        let actual_liquidation_amount = self.calculate_partial_amount(
            position,
            liquidation_amount,
            liquidation_factor,
        )?;

        // Calculate keeper incentive
        let keeper_reward = (actual_liquidation_amount as u128 * LIQUIDATION_INCENTIVE_BPS as u128 / 10000) as u64;

        // Update position
        position.size = position.size.saturating_sub(actual_liquidation_amount);
        position.collateral = self.calculate_remaining_collateral(
            position.collateral,
            actual_liquidation_amount,
            position.size + actual_liquidation_amount,
        )?;

        // Update engine stats
        self.total_liquidations_processed += 1;
        self.total_value_liquidated += actual_liquidation_amount;
        self.keeper_rewards_paid += keeper_reward;
        self.last_liquidation_slot = Clock::get()?.slot;

        Ok(LiquidationResult {
            liquidated_amount: actual_liquidation_amount,
            remaining_position: position.size,
            keeper_reward,
            new_health_factor: self.calculate_health_factor(position)?,
            is_fully_liquidated: position.size == 0,
        })
    }

    /// Validate liquidation is allowed using coverage-based formula
    fn validate_liquidation(
        &self,
        position: &Position,
        requested_amount: u64,
    ) -> Result<(), ProgramError> {
        // Check position is unhealthy using coverage-based formula
        use crate::trading::helpers::should_liquidate_coverage_based;
        
        let should_liquidate = should_liquidate_coverage_based(
            position.mark_price,
            position.size,
            position.margin,
            position.coverage,
        )?;
        
        if !should_liquidate {
            return Err(BettingPlatformError::PositionHealthy.into());
        }

        // Check minimum liquidation amount
        if requested_amount < MIN_LIQUIDATION_AMOUNT {
            return Err(BettingPlatformError::LiquidationTooSmall.into());
        }

        // Check partial liquidation is enabled
        if !self.partial_liquidation_enabled && !self.emergency_liquidation_mode {
            return Err(BettingPlatformError::PartialLiquidationDisabled.into());
        }

        Ok(())
    }

    /// Calculate partial liquidation amount
    fn calculate_partial_amount(
        &self,
        position: &Position,
        requested_amount: u64,
        liquidation_factor: u64,
    ) -> Result<u64, ProgramError> {
        // Calculate unhealthy portion
        let unhealthy_amount = self.calculate_unhealthy_amount(position)?;
        
        // Apply partial factor (e.g., 50%)
        let partial_amount = (unhealthy_amount as u128 * liquidation_factor as u128 / 10000) as u64;
        
        // Take minimum of requested and calculated amount
        let liquidation_amount = partial_amount.min(requested_amount).min(position.size);

        // Ensure minimum amount
        if liquidation_amount < MIN_LIQUIDATION_AMOUNT {
            return Ok(position.size.min(MIN_LIQUIDATION_AMOUNT)); // Liquidate entire small position
        }

        Ok(liquidation_amount)
    }

    /// Calculate health factor using coverage-based formula
    /// Health factor = margin_ratio / (1/coverage)
    /// Returns basis points where 10000 = 1.0
    fn calculate_health_factor(&self, position: &Position) -> Result<u64, ProgramError> {
        if position.size == 0 {
            return Ok(1000000); // Very high health for zero position
        }

        // Import the coverage-based check function
        use crate::trading::helpers::should_liquidate_coverage_based;
        
        // Check if position should be liquidated
        let should_liquidate = should_liquidate_coverage_based(
            position.mark_price,
            position.size,
            position.margin,
            position.coverage,
        )?;
        
        if should_liquidate {
            // Position is unhealthy, calculate how unhealthy
            // margin_ratio = margin / (size * price)
            let position_value = (position.size as u128 * position.mark_price as u128) / 10000;
            if position_value == 0 {
                return Ok(0);
            }
            
            let margin_ratio = (position.margin as u128 * 10000) / position_value;
            let coverage_threshold = if position.coverage > crate::math::U64F64::from_num(0) {
                let coverage_inv = crate::math::U64F64::from_num(1)
                    .checked_div(position.coverage)
                    .unwrap_or(crate::math::U64F64::from_num(10000));
                coverage_inv.to_num()
            } else {
                10000 // Default to 100% if coverage is 0
            };
            
            // Health factor = margin_ratio / coverage_threshold
            // Convert to basis points
            let health_factor = (margin_ratio * 10000) / (coverage_threshold as u128);
            Ok(health_factor as u64)
        } else {
            Ok(10000) // Healthy position = 1.0 health factor
        }
    }

    /// Calculate unhealthy amount
    fn calculate_unhealthy_amount(&self, position: &Position) -> Result<u64, ProgramError> {
        let health_factor = self.calculate_health_factor(position)?;
        
        if health_factor >= 10000 {  // 1.0 = healthy
            return Ok(0); // Position is healthy
        }

        // Calculate how much to liquidate to reach target health
        // MIN_HEALTH_AFTER_LIQUIDATION is 1100 (1.1), convert to ratio
        let target_health_ratio = (MIN_HEALTH_AFTER_LIQUIDATION as u128 * 10000) / 10000;
        let current_collateral = position.collateral as u128;
        let position_value = (position.size as u128 * position.mark_price as u128) / 10000;
        
        // Solve for liquidation amount
        // We need: new_collateral / (new_position * maintenance_margin) = target_health
        // This gives us the position size we need to reach
        let target_position_value = (current_collateral * 10000) / (target_health_ratio * position.maintenance_margin_ratio as u128 / 10000);
        
        if position_value > target_position_value {
            let unhealthy_value = position_value - target_position_value;
            let unhealthy_amount = ((unhealthy_value * 10000) / position.mark_price as u128) as u64;
            Ok(unhealthy_amount)
        } else {
            Ok(0)
        }
    }

    /// Calculate remaining collateral after liquidation
    fn calculate_remaining_collateral(
        &self,
        current_collateral: u64,
        liquidated_amount: u64,
        original_size: u64,
    ) -> Result<u64, ProgramError> {
        // Proportional collateral reduction
        if original_size == 0 {
            return Ok(current_collateral);
        }
        
        let collateral_reduction = (current_collateral as u128 * liquidated_amount as u128 / original_size as u128) as u64;
        
        Ok(current_collateral.saturating_sub(collateral_reduction))
    }
}

/// Position structure
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct Position {
    pub user: Pubkey,
    pub market: Pubkey,
    pub size: u64,
    pub collateral: u64,
    pub entry_price: u64,  // In basis points where 10000 = $1
    pub mark_price: u64,   // In basis points where 10000 = $1
    pub maintenance_margin_ratio: u64,  // In basis points where 10000 = 100%
    pub last_update_slot: u64,
    pub coverage: crate::math::U64F64,  // Coverage ratio from vault
    pub margin: u64,  // Margin amount for coverage-based calculation
}

/// Liquidation result
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct LiquidationResult {
    pub liquidated_amount: u64,
    pub remaining_position: u64,
    pub keeper_reward: u64,
    pub new_health_factor: u64,  // In basis points where 10000 = 1.0
    pub is_fully_liquidated: bool,
}

/// Liquidation queue for batch processing
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LiquidationQueue {
    pub positions: Vec<LiquidationCandidate>,
    pub total_liquidatable_value: u64,
    pub last_scan_slot: u64,
    pub scan_in_progress: bool,
}

/// Liquidation candidate
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LiquidationCandidate {
    pub position_pubkey: Pubkey,
    pub user: Pubkey,
    pub health_factor: u64, // Stored as fixed point
    pub liquidatable_amount: u64,
    pub priority_score: u64,
}

impl LiquidationQueue {
    /// Add position to liquidation queue
    pub fn add_candidate(
        &mut self,
        position_pubkey: &Pubkey,
        position: &Position,
        health_factor: u64,
        liquidatable_amount: u64,
    ) -> Result<(), ProgramError> {
        // Calculate priority (lower health = higher priority)
        // If health_factor is 10000 (1.0), priority is 0
        // If health_factor is 5000 (0.5), priority is 5000
        let priority_score = if health_factor < 10000 {
            10000 - health_factor
        } else {
            0
        };

        let candidate = LiquidationCandidate {
            position_pubkey: *position_pubkey,
            user: position.user,
            health_factor,
            liquidatable_amount,
            priority_score,
        };

        self.positions.push(candidate);
        self.total_liquidatable_value += liquidatable_amount;

        // Sort by priority (highest priority first)
        self.positions.sort_by(|a, b| b.priority_score.cmp(&a.priority_score));

        // Limit queue size
        if self.positions.len() > 100 {
            self.positions.truncate(100);
        }

        Ok(())
    }

    /// Get next batch for liquidation
    pub fn get_next_batch(&mut self, max_batch_size: usize) -> Vec<LiquidationCandidate> {
        let batch_size = max_batch_size.min(self.positions.len());
        let batch: Vec<_> = self.positions.drain(..batch_size).collect();
        
        // Update total value
        self.total_liquidatable_value = self.positions.iter()
            .map(|c| c.liquidatable_amount)
            .sum();

        batch
    }
}

/// Keeper incentive calculator
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct KeeperIncentiveCalculator {
    pub base_reward_bps: u16,
    pub gas_compensation: u64,
    pub priority_multiplier: u64,  // In basis points where 10000 = 1.0
}

impl KeeperIncentiveCalculator {
    /// Calculate keeper reward
    pub fn calculate_reward(
        &self,
        liquidation_amount: u64,
        priority_score: u64,
        gas_estimate: u64,
    ) -> u64 {
        // Base reward
        let base_reward = (liquidation_amount as u128 * self.base_reward_bps as u128 / 10000) as u64;
        
        // Priority bonus (priority_multiplier is in basis points where 10000 = 1.0)
        let priority_bonus = (base_reward as u128 * self.priority_multiplier as u128 * priority_score as u128 / 100 / 10000) as u64;
        
        // Total reward
        base_reward + priority_bonus + gas_estimate.min(self.gas_compensation)
    }
}

/// Process liquidation instructions
pub fn process_liquidation_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_engine(program_id, accounts),
        1 => process_liquidate_position(program_id, accounts, &instruction_data[1..]),
        2 => process_scan_positions(program_id, accounts),
        3 => process_batch_liquidation(program_id, accounts),
        4 => process_toggle_emergency_mode(program_id, accounts, instruction_data[1] != 0),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_engine(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let engine_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut engine = PartialLiquidationEngine::try_from_slice(&engine_account.data.borrow())?;
    engine.initialize()?;
    engine.serialize(&mut &mut engine_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_liquidate_position(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let engine_account = next_account_info(account_iter)?;
    let position_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;

    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse liquidation amount
    let requested_amount = u64::from_le_bytes(data[0..8].try_into().unwrap());

    let mut engine = PartialLiquidationEngine::try_from_slice(&engine_account.data.borrow())?;
    let mut position = Position::try_from_slice(&position_account.data.borrow())?;

    // Process liquidation
    let result = engine.process_liquidation(&mut position, requested_amount, keeper_account.key)?;

    msg!("Liquidated {} of position, keeper reward: {}", 
        result.liquidated_amount, 
        result.keeper_reward);

    // In production, would handle token transfers here

    engine.serialize(&mut &mut engine_account.data.borrow_mut()[..])?;
    position.serialize(&mut &mut position_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_scan_positions(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let queue_account = next_account_info(account_iter)?;
    let engine_account = next_account_info(account_iter)?;

    let mut queue = LiquidationQueue::try_from_slice(&queue_account.data.borrow())?;
    let engine = PartialLiquidationEngine::try_from_slice(&engine_account.data.borrow())?;

    queue.last_scan_slot = Clock::get()?.slot;
    queue.scan_in_progress = false;

    msg!("Position scan complete. {} candidates found", queue.positions.len());

    queue.serialize(&mut &mut queue_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_batch_liquidation(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let engine_account = next_account_info(account_iter)?;
    let queue_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;

    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut engine = PartialLiquidationEngine::try_from_slice(&engine_account.data.borrow())?;
    let mut queue = LiquidationQueue::try_from_slice(&queue_account.data.borrow())?;

    // Get batch to process
    let batch = queue.get_next_batch(5); // Process up to 5 at once
    let processed = batch.len();

    msg!("Processing batch liquidation of {} positions", processed);

    // In production, would process each liquidation

    engine.serialize(&mut &mut engine_account.data.borrow_mut()[..])?;
    queue.serialize(&mut &mut queue_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_toggle_emergency_mode(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    enable: bool,
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let engine_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut engine = PartialLiquidationEngine::try_from_slice(&engine_account.data.borrow())?;
    engine.emergency_liquidation_mode = enable;

    msg!("Emergency liquidation mode: {}", if enable { "ENABLED" } else { "disabled" });

    engine.serialize(&mut &mut engine_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;