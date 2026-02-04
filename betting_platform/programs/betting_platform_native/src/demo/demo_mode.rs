//! Demo mode account management
//!
//! Implements demo accounts with fake USDC for risk-free practice trading

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    program::invoke_signed,
    system_instruction,
    rent::Rent,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::{accounts::discriminators, UserMap},
    events::{emit_event, EventType},
};

/// Demo mode configuration
pub const DEMO_INITIAL_BALANCE: u64 = 10_000_000_000; // 10,000 USDC (6 decimals)
pub const DEMO_RESET_COOLDOWN: u64 = 43_200; // 3 hours in slots
pub const DEMO_MAX_POSITIONS: u8 = 20;
pub const DEMO_MAX_LEVERAGE: u8 = 20; // Limited leverage in demo mode

/// Demo account state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DemoAccount {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User pubkey
    pub user: Pubkey,
    
    /// Demo balance in fake USDC (6 decimals)
    pub demo_balance: u64,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Total positions opened
    pub positions_opened: u32,
    
    /// Total positions closed
    pub positions_closed: u32,
    
    /// Total profit/loss
    pub total_pnl: i64,
    
    /// Best trade profit
    pub best_trade: i64,
    
    /// Worst trade loss
    pub worst_trade: i64,
    
    /// Win rate (basis points)
    pub win_rate_bps: u16,
    
    /// Account created at
    pub created_at: i64,
    
    /// Last reset slot
    pub last_reset_slot: u64,
    
    /// Number of resets
    pub reset_count: u16,
    
    /// Is currently active
    pub is_active: bool,
    
    /// Demo positions
    pub demo_positions: Vec<DemoPosition>,
}

impl DemoAccount {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        8 + // demo_balance
        8 + // total_volume
        4 + // positions_opened
        4 + // positions_closed
        8 + // total_pnl
        8 + // best_trade
        8 + // worst_trade
        2 + // win_rate_bps
        8 + // created_at
        8 + // last_reset_slot
        2 + // reset_count
        1 + // is_active
        4 + (DEMO_MAX_POSITIONS as usize * DemoPosition::SIZE); // demo_positions
    
    /// Create new demo account
    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::DEMO_ACCOUNT,
            user,
            demo_balance: DEMO_INITIAL_BALANCE,
            total_volume: 0,
            positions_opened: 0,
            positions_closed: 0,
            total_pnl: 0,
            best_trade: 0,
            worst_trade: 0,
            win_rate_bps: 0,
            created_at: Clock::get().unwrap().unix_timestamp,
            last_reset_slot: Clock::get().unwrap().slot,
            reset_count: 0,
            is_active: true,
            demo_positions: Vec::with_capacity(DEMO_MAX_POSITIONS as usize),
        }
    }
    
    /// Reset demo account
    pub fn reset(&mut self) -> Result<(), ProgramError> {
        let current_slot = Clock::get()?.slot;
        
        // Check cooldown
        if current_slot < self.last_reset_slot + DEMO_RESET_COOLDOWN {
            return Err(BettingPlatformError::DemoResetCooldown.into());
        }
        
        // Reset balance and stats
        self.demo_balance = DEMO_INITIAL_BALANCE;
        self.total_volume = 0;
        self.positions_opened = 0;
        self.positions_closed = 0;
        self.total_pnl = 0;
        self.best_trade = 0;
        self.worst_trade = 0;
        self.win_rate_bps = 0;
        self.last_reset_slot = current_slot;
        self.reset_count += 1;
        self.demo_positions.clear();
        
        Ok(())
    }
    
    /// Update statistics after trade
    pub fn update_stats(&mut self, pnl: i64, is_win: bool) {
        self.total_pnl += pnl;
        
        if pnl > self.best_trade {
            self.best_trade = pnl;
        }
        if pnl < self.worst_trade {
            self.worst_trade = pnl;
        }
        
        self.positions_closed += 1;
        
        // Update win rate
        if self.positions_closed > 0 {
            let wins = if is_win { 
                (self.win_rate_bps as u64 * (self.positions_closed - 1) as u64 / 10000) + 1 
            } else { 
                self.win_rate_bps as u64 * (self.positions_closed - 1) as u64 / 10000 
            };
            self.win_rate_bps = ((wins * 10000) / self.positions_closed as u64) as u16;
        }
    }
    
    /// Check if can open new position
    pub fn can_open_position(&self) -> bool {
        self.demo_positions.len() < DEMO_MAX_POSITIONS as usize && 
        self.is_active
    }
}

/// Demo position tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DemoPosition {
    /// Position ID
    pub position_id: u128,
    
    /// Market
    pub market: Pubkey,
    
    /// Size
    pub size: u64,
    
    /// Entry price
    pub entry_price: u64,
    
    /// Leverage
    pub leverage: u8,
    
    /// Is long
    pub is_long: bool,
    
    /// Open timestamp
    pub opened_at: i64,
    
    /// Current PnL
    pub unrealized_pnl: i64,
}

impl DemoPosition {
    pub const SIZE: usize = 16 + // position_id
        32 + // market
        8 + // size
        8 + // entry_price
        1 + // leverage
        1 + // is_long
        8 + // opened_at
        8; // unrealized_pnl
}

/// Initialize demo account for user
pub fn process_initialize_demo_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let demo_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive demo account PDA
    let (demo_account_key, bump) = Pubkey::find_program_address(
        &[b"demo", user.key.as_ref()],
        program_id,
    );
    
    if demo_account_key != *demo_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Create account if it doesn't exist
    if demo_account.data_is_empty() {
        let rent = Rent::get()?;
        let space = DemoAccount::SIZE;
        let lamports = rent.minimum_balance(space);
        
        invoke_signed(
            &system_instruction::create_account(
                user.key,
                demo_account.key,
                lamports,
                space as u64,
                program_id,
            ),
            &[user.clone(), demo_account.clone(), system_program.clone()],
            &[&[b"demo", user.key.as_ref(), &[bump]]],
        )?;
    }
    
    // Initialize demo account
    let mut demo_data = DemoAccount::new(*user.key);
    demo_data.serialize(&mut &mut demo_account.data.borrow_mut()[..])?;
    
    msg!("Demo account initialized for user {}", user.key);
    msg!("Initial balance: {} fake USDC", DEMO_INITIAL_BALANCE / 1_000_000);
    
    // Emit event
    DemoAccountCreated {
        user: *user.key,
        initial_balance: DEMO_INITIAL_BALANCE,
        timestamp: demo_data.created_at,
    }.emit();
    
    Ok(())
}

/// Reset demo account balance
pub fn process_reset_demo_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let demo_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate PDA
    let (demo_account_key, _) = Pubkey::find_program_address(
        &[b"demo", user.key.as_ref()],
        program_id,
    );
    
    if demo_account_key != *demo_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load and reset account
    let mut demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    
    // Verify ownership
    if demo_data.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    let old_stats = DemoStats {
        total_pnl: demo_data.total_pnl,
        win_rate: demo_data.win_rate_bps,
        positions_closed: demo_data.positions_closed,
    };
    
    demo_data.reset()?;
    demo_data.serialize(&mut &mut demo_account.data.borrow_mut()[..])?;
    
    msg!("Demo account reset for user {}", user.key);
    msg!("Previous stats - PnL: {}, Win rate: {}%", 
        old_stats.total_pnl, 
        old_stats.win_rate as f64 / 100.0
    );
    
    // Emit event
    DemoAccountReset {
        user: *user.key,
        reset_count: demo_data.reset_count,
        previous_stats: old_stats,
        timestamp: Clock::get()?.unix_timestamp,
    }.emit();
    
    Ok(())
}

/// Get demo account state (for UI)
pub fn get_demo_account_state(
    demo_account: &AccountInfo,
) -> Result<DemoAccountState, ProgramError> {
    let demo_data = DemoAccount::try_from_slice(&demo_account.data.borrow())?;
    
    Ok(DemoAccountState {
        balance: demo_data.demo_balance,
        total_pnl: demo_data.total_pnl,
        win_rate_bps: demo_data.win_rate_bps,
        positions_opened: demo_data.positions_opened,
        positions_closed: demo_data.positions_closed,
        active_positions: demo_data.demo_positions.len() as u8,
        can_reset: Clock::get()?.slot >= demo_data.last_reset_slot + DEMO_RESET_COOLDOWN,
        reset_count: demo_data.reset_count,
        created_at: demo_data.created_at,
    })
}

/// Demo account state for UI
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoAccountState {
    pub balance: u64,
    pub total_pnl: i64,
    pub win_rate_bps: u16,
    pub positions_opened: u32,
    pub positions_closed: u32,
    pub active_positions: u8,
    pub can_reset: bool,
    pub reset_count: u16,
    pub created_at: i64,
}

/// Demo stats for reset event
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DemoStats {
    pub total_pnl: i64,
    pub win_rate: u16,
    pub positions_closed: u32,
}

/// Demo account created event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoAccountCreated {
    pub user: Pubkey,
    pub initial_balance: u64,
    pub timestamp: i64,
}

impl DemoAccountCreated {
    pub fn emit(&self) {
        emit_event(EventType::DemoAccountCreated, self);
    }
}

/// Demo account reset event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DemoAccountReset {
    pub user: Pubkey,
    pub reset_count: u16,
    pub previous_stats: DemoStats,
    pub timestamp: i64,
}

impl DemoAccountReset {
    pub fn emit(&self) {
        emit_event(EventType::DemoAccountReset, self);
    }
}