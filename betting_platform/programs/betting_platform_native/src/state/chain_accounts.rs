//! Chain execution account structures
//!
//! Account types for automated chain execution

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::account_validation::DISCRIMINATOR_SIZE;
use crate::instruction::ChainStepType;

/// Discriminators for chain account types
pub mod discriminators {
    pub const CHAIN_STATE: [u8; 8] = [201, 89, 156, 234, 45, 167, 23, 78];
    pub const CHAIN_POSITION: [u8; 8] = [156, 234, 78, 201, 45, 23, 167, 89];
    pub const CHAIN_STEP: [u8; 8] = [78, 23, 167, 45, 234, 89, 201, 156];
}

/// Chain state account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ChainState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Unique chain identifier
    pub chain_id: u128,
    
    /// User who created the chain
    pub user: Pubkey,
    
    /// Verse ID where chain operates
    pub verse_id: u128,
    
    /// Initial deposit amount
    pub initial_deposit: u64,
    
    /// Current balance in chain
    pub current_balance: u64,
    
    /// Chain steps to execute
    pub steps: Vec<ChainStepType>,
    
    /// Current step index
    pub current_step: u8,
    
    /// Chain status
    pub status: ChainStatus,
    
    /// Total profit/loss
    pub total_pnl: i64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last execution timestamp
    pub last_execution: i64,
    
    /// Positions created by this chain
    pub position_ids: Vec<u128>,
    
    /// Error count
    pub error_count: u32,
    
    /// Last error message
    pub last_error: Option<String>,
}

impl ChainState {
    pub const MAX_STEPS: usize = 10;
    pub const MAX_POSITIONS: usize = 20;
    
    pub fn new(
        chain_id: u128,
        user: Pubkey,
        verse_id: u128,
        initial_deposit: u64,
        steps: Vec<ChainStepType>,
        created_at: i64,
    ) -> Result<Self, ProgramError> {
        if steps.is_empty() || steps.len() > Self::MAX_STEPS {
            return Err(ProgramError::InvalidArgument);
        }
        
        Ok(Self {
            discriminator: discriminators::CHAIN_STATE,
            chain_id,
            user,
            verse_id,
            initial_deposit,
            current_balance: initial_deposit,
            steps,
            current_step: 0,
            status: ChainStatus::Active,
            total_pnl: 0,
            created_at,
            last_execution: created_at,
            position_ids: Vec::with_capacity(Self::MAX_POSITIONS),
            error_count: 0,
            last_error: None,
        })
    }
    
    pub fn add_position(&mut self, position_id: u128) -> Result<(), ProgramError> {
        if self.position_ids.len() >= Self::MAX_POSITIONS {
            return Err(ProgramError::AccountDataTooSmall);
        }
        
        self.position_ids.push(position_id);
        Ok(())
    }
    
    pub fn advance_step(&mut self) -> Result<(), ProgramError> {
        if self.current_step as usize >= self.steps.len() - 1 {
            self.status = ChainStatus::Completed;
        } else {
            self.current_step += 1;
        }
        
        Ok(())
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::CHAIN_STATE {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.steps.is_empty() || self.steps.len() > Self::MAX_STEPS {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.position_ids.len() > Self::MAX_POSITIONS {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Chain status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ChainStatus {
    Active,
    Paused,
    Completed,
    Failed,
    Liquidated,
    Closed,
}

/// Chain position (position created by chain)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ChainPosition {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Chain ID that created this position
    pub chain_id: u128,
    
    /// Position ID
    pub position_id: u128,
    
    /// Proposal ID
    pub proposal_id: u128,
    
    /// Step index that created this position
    pub step_index: u8,
    
    /// Position details
    pub outcome: u8,
    pub size: u64,
    pub leverage: u64,
    pub entry_price: u64,
    pub is_long: bool,
    
    /// Position status
    pub status: PositionStatus,
    
    /// PnL if closed
    pub realized_pnl: i64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Close timestamp
    pub closed_at: Option<i64>,
    
    /// Total payout accumulated
    pub total_payout: u64,
    
    /// Chain legs for multi-leg positions
    pub legs: Vec<ChainLeg>,
    
    /// Initial stake amount
    pub initial_stake: u64,
}

impl ChainPosition {
    pub fn new(
        chain_id: u128,
        position_id: u128,
        proposal_id: u128,
        step_index: u8,
        outcome: u8,
        size: u64,
        leverage: u64,
        entry_price: u64,
        is_long: bool,
        created_at: i64,
    ) -> Self {
        Self {
            discriminator: discriminators::CHAIN_POSITION,
            chain_id,
            position_id,
            proposal_id,
            step_index,
            outcome,
            size,
            leverage,
            entry_price,
            is_long,
            status: PositionStatus::Open,
            realized_pnl: 0,
            created_at,
            closed_at: None,
            total_payout: 0,
            legs: Vec::new(),
            initial_stake: size / leverage, // Calculate initial stake from size and leverage
        }
    }
    
    pub fn close(&mut self, exit_price: u64, timestamp: i64) {
        let price_diff = if self.is_long {
            exit_price as i64 - self.entry_price as i64
        } else {
            self.entry_price as i64 - exit_price as i64
        };
        
        self.realized_pnl = (price_diff * self.size as i64 * self.leverage as i64) / self.entry_price as i64;
        self.status = PositionStatus::Closed;
        self.closed_at = Some(timestamp);
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::CHAIN_POSITION {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.leverage == 0 || self.leverage > 100 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Position status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum PositionStatus {
    Open,
    Closed,
    Liquidated,
}

/// Chain leg for multi-leg positions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ChainLeg {
    pub proposal_id: u128,
    pub outcome: u8,
    pub size: u64,
    pub leverage: u64,
    pub allocation_bps: u16,
    pub executed: bool,
    pub pnl: i64,
}

/// Chain type enum
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ChainType {
    Sequential,
    Conditional,
    Loop,
    Parallel,
}

/// Position info for chain unwind operations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct PositionInfo {
    pub position_id: u128,
    pub proposal_id: u128,
    pub outcome: u8,
    pub size: u64,
    pub leverage: u64,
    pub entry_price: u64,
    pub is_long: bool,
    pub unrealized_pnl: i64,
}

/// Chain execution tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ChainExecution {
    /// Current verse path
    pub verse_path: Vec<u128>,
    
    /// Positions per verse
    pub verse_positions: Vec<VersePositions>,
    
    /// Total verses visited
    pub verses_visited: u32,
    
    /// Cycle detection
    pub cycle_detector: Vec<u128>,
}

impl ChainExecution {
    pub fn new() -> Self {
        Self {
            verse_path: Vec::with_capacity(10),
            verse_positions: Vec::with_capacity(10),
            verses_visited: 0,
            cycle_detector: Vec::with_capacity(32),
        }
    }
    
    pub fn enter_verse(&mut self, verse_id: u128) -> Result<(), ProgramError> {
        // Check for cycles
        if self.cycle_detector.contains(&verse_id) {
            return Err(ProgramError::InvalidAccountData); // ChainCycle error
        }
        
        self.verse_path.push(verse_id);
        self.cycle_detector.push(verse_id);
        self.verses_visited += 1;
        
        // Limit depth
        if self.verse_path.len() > 10 {
            self.verse_path.remove(0);
        }
        
        Ok(())
    }
    
    pub fn exit_verse(&mut self) -> Option<u128> {
        if let Some(verse_id) = self.verse_path.pop() {
            self.cycle_detector.retain(|&id| id != verse_id);
            Some(verse_id)
        } else {
            None
        }
    }
}

/// Verse positions tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct VersePositions {
    pub verse_id: u128,
    pub position_count: u32,
    pub total_exposure: u64,
}

/// Chain safety parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct ChainSafety {
    /// Maximum positions per chain
    pub max_positions: u32,
    
    /// Maximum leverage per position
    pub max_leverage: u8,
    
    /// Maximum total exposure
    pub max_exposure: u64,
    
    /// Stop loss percentage (basis points)
    pub stop_loss_bps: u16,
    
    /// Take profit percentage (basis points)
    pub take_profit_bps: u16,
    
    /// Maximum chain duration (slots)
    pub max_duration: u64,
}

impl Default for ChainSafety {
    fn default() -> Self {
        Self {
            max_positions: 20,
            max_leverage: 10,
            max_exposure: 1_000_000_000, // 1000 USDC
            stop_loss_bps: 500,           // 5%
            take_profit_bps: 2000,        // 20%
            max_duration: 432_000,        // ~2 days
        }
    }
}