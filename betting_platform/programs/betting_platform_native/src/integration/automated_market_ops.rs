// Phase 20.5: Automated Market Operations
// Implements automated market management and optimization

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
};

/// Market operations configuration
pub const REBALANCE_THRESHOLD_BPS: u16 = 500; // 5% imbalance triggers rebalance
pub const MIN_LIQUIDITY_RATIO: u64 = 2000; // 0.2 minimum liquidity ratio
pub const MAX_EXPOSURE_PER_MARKET: u64 = 1000; // 10% max exposure
pub const AUTO_HEDGE_THRESHOLD: u64 = 8000; // 80% exposure triggers hedge
pub const MARKET_MAKER_SPREAD_BPS: u16 = 30; // 0.3% MM spread
pub const INVENTORY_TARGET_RATIO: u64 = 5000; // 50% target inventory

/// Automated market operations engine
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AutomatedMarketOps {
    pub operations_enabled: bool,
    pub market_making_active: bool,
    pub auto_rebalance_active: bool,
    pub hedge_management_active: bool,
    pub total_markets_managed: u64,
    pub total_rebalances: u64,
    pub total_hedges_placed: u64,
    pub last_operation_slot: u64,
    pub operation_interval: u64,
}

impl AutomatedMarketOps {
    pub const SIZE: usize = 1 + // operations_enabled
        1 + // market_making_active
        1 + // auto_rebalance_active
        1 + // hedge_management_active
        8 + // total_markets_managed
        8 + // total_rebalances
        8 + // total_hedges_placed
        8 + // last_operation_slot
        8; // operation_interval

    /// Initialize automated operations
    pub fn initialize(&mut self) -> ProgramResult {
        self.operations_enabled = true;
        self.market_making_active = true;
        self.auto_rebalance_active = true;
        self.hedge_management_active = true;
        self.total_markets_managed = 0;
        self.total_rebalances = 0;
        self.total_hedges_placed = 0;
        self.last_operation_slot = 0;
        self.operation_interval = 150; // ~60 seconds

        msg!("Automated market operations initialized");
        Ok(())
    }

    /// Check if operation is due
    pub fn is_operation_due(&self, current_slot: u64) -> bool {
        current_slot >= self.last_operation_slot + self.operation_interval
    }

    /// Execute automated operations
    pub fn execute_operations(
        &mut self,
        markets: &[MarketInfo],
        vault_state: &VaultState,
        current_slot: u64,
    ) -> Result<OperationResult, ProgramError> {
        if !self.operations_enabled || !self.is_operation_due(current_slot) {
            return Ok(OperationResult::default());
        }

        let mut result = OperationResult::default();

        // Market making operations
        if self.market_making_active {
            let mm_actions = self.execute_market_making(markets, vault_state)?;
            result.market_making_actions = mm_actions;
        }

        // Rebalancing operations
        if self.auto_rebalance_active {
            let rebalance_actions = self.execute_rebalancing(markets, vault_state)?;
            self.total_rebalances += rebalance_actions.len() as u64;
            result.rebalance_actions = rebalance_actions;
        }

        // Hedge management
        if self.hedge_management_active {
            let hedge_actions = self.execute_hedging(markets, vault_state)?;
            self.total_hedges_placed += hedge_actions.len() as u64;
            result.hedge_actions = hedge_actions;
        }

        self.last_operation_slot = current_slot;

        Ok(result)
    }

    /// Execute market making operations
    fn execute_market_making(
        &self,
        markets: &[MarketInfo],
        vault_state: &VaultState,
    ) -> Result<Vec<MarketMakingAction>, ProgramError> {
        let mut actions = Vec::new();

        for market in markets {
            // Check if market needs liquidity
            if market.liquidity_ratio < MIN_LIQUIDITY_RATIO {
                let action = self.calculate_mm_action(market, vault_state)?;
                if action.size > 0 {
                    actions.push(action);
                }
            }
        }

        Ok(actions)
    }

    /// Calculate market making action
    fn calculate_mm_action(
        &self,
        market: &MarketInfo,
        vault_state: &VaultState,
    ) -> Result<MarketMakingAction, ProgramError> {
        // Calculate optimal bid/ask spread
        let mid_price = (market.bid_price + market.ask_price) / 2;
        
        // Calculate spread (MARKET_MAKER_SPREAD_BPS is basis points)
        // E.g., 30 bps = 0.3% = 0.003
        let half_spread_bps = MARKET_MAKER_SPREAD_BPS / 2;
        
        // bid_price = mid_price * (1 - spread/2)
        let bid_price = (mid_price as u128 * (10000 - half_spread_bps as u128) / 10000) as u64;
        // ask_price = mid_price * (1 + spread/2)
        let ask_price = (mid_price as u128 * (10000 + half_spread_bps as u128) / 10000) as u64;

        // Calculate size based on vault capacity
        let max_size = vault_state.available_liquidity / 10; // Max 10% per market
        let optimal_size = self.calculate_optimal_mm_size(market, max_size)?;

        Ok(MarketMakingAction {
            market_id: market.market_id,
            bid_price,
            ask_price,
            size: optimal_size,
            action_type: MMActionType::ProvideLiquidity,
        })
    }

    /// Execute rebalancing operations
    fn execute_rebalancing(
        &self,
        markets: &[MarketInfo],
        vault_state: &VaultState,
    ) -> Result<Vec<RebalanceAction>, ProgramError> {
        let mut actions = Vec::new();

        // Calculate total exposure
        let total_exposure: u64 = markets.iter()
            .map(|m| m.net_exposure.abs() as u64)
            .sum();

        for market in markets {
            let exposure_ratio = if total_exposure > 0 {
                ((market.net_exposure.abs() as u64) * 10000) / total_exposure
            } else {
                0
            };

            // Check if rebalance needed
            if exposure_ratio > MAX_EXPOSURE_PER_MARKET as u64 {
                let action = self.calculate_rebalance_action(market, vault_state)?;
                actions.push(action);
            }
        }

        Ok(actions)
    }

    /// Calculate rebalance action
    fn calculate_rebalance_action(
        &self,
        market: &MarketInfo,
        vault_state: &VaultState,
    ) -> Result<RebalanceAction, ProgramError> {
        let target_exposure = (vault_state.total_value * MAX_EXPOSURE_PER_MARKET as u64) / 10000;
        let current_exposure = market.net_exposure.abs() as u64;
        let reduction_needed = current_exposure.saturating_sub(target_exposure);

        Ok(RebalanceAction {
            market_id: market.market_id,
            current_exposure,
            target_exposure,
            amount_to_rebalance: reduction_needed,
            direction: if market.net_exposure > 0 {
                RebalanceDirection::ReduceLong
            } else {
                RebalanceDirection::ReduceShort
            },
        })
    }

    /// Execute hedging operations
    fn execute_hedging(
        &self,
        markets: &[MarketInfo],
        vault_state: &VaultState,
    ) -> Result<Vec<HedgeAction>, ProgramError> {
        let mut actions = Vec::new();

        for market in markets {
            let exposure_percentage = ((market.net_exposure.abs() as u64) * 10000) / vault_state.total_value;
            
            if exposure_percentage > AUTO_HEDGE_THRESHOLD as u64 {
                let hedge = self.calculate_hedge_action(market, vault_state)?;
                actions.push(hedge);
            }
        }

        Ok(actions)
    }

    /// Calculate hedge action
    fn calculate_hedge_action(
        &self,
        market: &MarketInfo,
        vault_state: &VaultState,
    ) -> Result<HedgeAction, ProgramError> {
        // 80% hedge ratio
        let hedge_amount = (market.net_exposure.abs() as u128 * 8 / 10) as u64;

        Ok(HedgeAction {
            market_id: market.market_id,
            exposure_to_hedge: market.net_exposure,
            hedge_amount,
            hedge_type: if market.net_exposure > 0 {
                HedgeType::Short
            } else {
                HedgeType::Long
            },
            correlation_markets: vec![], // Simplified - would use correlation matrix in production
        })
    }

    /// Find correlated markets for hedging
    fn find_correlation_markets(
        &self,
        market_id: &Pubkey,
        markets: &[MarketInfo],
    ) -> Vec<Pubkey> {
        // In production, would use correlation matrix
        // For now, return empty vec
        Vec::new()
    }

    /// Calculate optimal market making size
    fn calculate_optimal_mm_size(
        &self,
        market: &MarketInfo,
        max_size: u64,
    ) -> Result<u64, ProgramError> {
        // Base size on market volume and volatility
        let volume_factor = (market.volume_24h / 1_000_000).min(100); // Normalize to millions
        let volatility_factor = 100u64.saturating_sub(market.volatility_bps.min(100));
        
        let optimal_size = (max_size * volume_factor * volatility_factor) / 10_000;
        
        Ok(optimal_size.max(100_000_000)) // Minimum $100
    }
}

/// Market information
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketInfo {
    pub market_id: Pubkey,
    pub bid_price: u64,  // In basis points where 10000 = $1
    pub ask_price: u64,  // In basis points where 10000 = $1
    pub net_exposure: i64,
    pub liquidity_ratio: u64,
    pub volume_24h: u64,
    pub volatility_bps: u64,
}

/// Vault state
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct VaultState {
    pub total_value: u64,
    pub available_liquidity: u64,
    pub total_exposure: u64,
    pub risk_utilization: u64,
}

/// Operation result
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct OperationResult {
    pub market_making_actions: Vec<MarketMakingAction>,
    pub rebalance_actions: Vec<RebalanceAction>,
    pub hedge_actions: Vec<HedgeAction>,
}

/// Market making action
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketMakingAction {
    pub market_id: Pubkey,
    pub bid_price: u64,  // In basis points where 10000 = $1
    pub ask_price: u64,  // In basis points where 10000 = $1
    pub size: u64,
    pub action_type: MMActionType,
}

/// MM action type
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum MMActionType {
    ProvideLiquidity,
    AdjustSpread,
    CancelOrders,
}

/// Rebalance action
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct RebalanceAction {
    pub market_id: Pubkey,
    pub current_exposure: u64,
    pub target_exposure: u64,
    pub amount_to_rebalance: u64,
    pub direction: RebalanceDirection,
}

/// Rebalance direction
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum RebalanceDirection {
    ReduceLong,
    ReduceShort,
    Neutral,
}

/// Hedge action
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct HedgeAction {
    pub market_id: Pubkey,
    pub exposure_to_hedge: i64,
    pub hedge_amount: u64,
    pub hedge_type: HedgeType,
    pub correlation_markets: Vec<Pubkey>,
}

/// Hedge type
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum HedgeType {
    Long,
    Short,
    Spread,
}

/// Inventory manager for market making
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct InventoryManager {
    pub target_inventory_ratio: u64,
    pub max_inventory_value: u64,
    pub inventory_positions: Vec<InventoryPosition>,
    pub rebalance_threshold_bps: u16,
}

/// Inventory position
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct InventoryPosition {
    pub market_id: Pubkey,
    pub position_size: i64,
    pub average_price: u64,  // In basis points where 10000 = $1
    pub current_value: u64,
    pub unrealized_pnl: i64,
}

impl InventoryManager {
    /// Check if inventory needs rebalancing
    pub fn needs_rebalancing(&self, position: &InventoryPosition) -> bool {
        let position_ratio = (position.current_value * 10000) / self.max_inventory_value;
        let deviation = position_ratio.abs_diff(self.target_inventory_ratio);
        
        deviation > self.rebalance_threshold_bps as u64
    }

    /// Calculate rebalance amount
    pub fn calculate_rebalance(&self, position: &InventoryPosition) -> i64 {
        let target_value = (self.max_inventory_value * self.target_inventory_ratio) / 10000;
        let current_value = position.current_value;
        
        if current_value > target_value {
            -((current_value - target_value) as i64)
        } else {
            (target_value - current_value) as i64
        }
    }
}

/// Risk manager for automated operations
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AutomatedRiskManager {
    pub max_daily_loss: u64,
    pub max_position_size: u64,
    pub max_market_exposure: u64,
    pub current_daily_pnl: i64,
    pub risk_limits_active: bool,
}

impl AutomatedRiskManager {
    /// Check if operation is within risk limits
    pub fn check_risk_limits(&self, action_value: u64, market_exposure: u64) -> bool {
        // Check daily loss limit
        if self.current_daily_pnl < -(self.max_daily_loss as i64) {
            return false;
        }

        // Check position size limit
        if action_value > self.max_position_size {
            return false;
        }

        // Check market exposure limit
        if market_exposure + action_value > self.max_market_exposure {
            return false;
        }

        true
    }
}

/// Performance tracker for automated operations
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PerformanceTracker {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub total_pnl: i64,
    pub best_performing_market: Option<Pubkey>,
    pub worst_performing_market: Option<Pubkey>,
    pub average_spread_captured_bps: u16,
}

impl PerformanceTracker {
    /// Update performance metrics
    pub fn update_metrics(
        &mut self,
        operation_successful: bool,
        pnl: i64,
        market: &Pubkey,
    ) {
        self.total_operations += 1;
        if operation_successful {
            self.successful_operations += 1;
        }
        self.total_pnl += pnl;

        // Update best/worst markets based on PnL
        // In production, would track per-market PnL
    }

    /// Get success rate
    pub fn success_rate(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        (self.successful_operations * 10000) / self.total_operations
    }
}

/// Process automated market operations instructions
pub fn process_market_ops_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_ops(program_id, accounts),
        1 => process_execute_operations(program_id, accounts),
        2 => process_update_parameters(program_id, accounts, &instruction_data[1..]),
        3 => process_emergency_stop(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_ops(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let ops_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut ops = AutomatedMarketOps::try_from_slice(&ops_account.data.borrow())?;
    ops.initialize()?;
    ops.serialize(&mut &mut ops_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_execute_operations(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let ops_account = next_account_info(account_iter)?;
    let vault_account = next_account_info(account_iter)?;
    let keeper_account = next_account_info(account_iter)?;

    if !keeper_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut ops = AutomatedMarketOps::try_from_slice(&ops_account.data.borrow())?;
    
    // In production, would load actual market data and vault state
    let markets = vec![]; // Placeholder
    let vault_state = VaultState {
        total_value: 10_000_000_000_000, // $10M
        available_liquidity: 1_000_000_000_000, // $1M
        total_exposure: 5_000_000_000_000, // $5M
        risk_utilization: 5000, // 50%
    };

    let result = ops.execute_operations(&markets, &vault_state, Clock::get()?.slot)?;

    msg!("Executed {} MM actions, {} rebalances, {} hedges",
        result.market_making_actions.len(),
        result.rebalance_actions.len(),
        result.hedge_actions.len());

    ops.serialize(&mut &mut ops_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_update_parameters(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let ops_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut ops = AutomatedMarketOps::try_from_slice(&ops_account.data.borrow())?;

    // Parse parameter updates
    let param_type = data[0];
    match param_type {
        0 => ops.operation_interval = u64::from_le_bytes(data[1..9].try_into().unwrap()),
        1 => ops.market_making_active = data[1] != 0,
        2 => ops.auto_rebalance_active = data[1] != 0,
        3 => ops.hedge_management_active = data[1] != 0,
        _ => return Err(ProgramError::InvalidInstructionData),
    }

    ops.serialize(&mut &mut ops_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_emergency_stop(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let ops_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut ops = AutomatedMarketOps::try_from_slice(&ops_account.data.borrow())?;
    
    ops.operations_enabled = false;
    ops.market_making_active = false;
    ops.auto_rebalance_active = false;
    ops.hedge_management_active = false;

    msg!("EMERGENCY: All automated operations stopped");

    ops.serialize(&mut &mut ops_account.data.borrow_mut()[..])?;

    Ok(())
}

use solana_program::account_info::next_account_info;