use anchor_lang::prelude::*;
use crate::account_structs::{U64F64, Position};
use crate::errors::ErrorCode;
use crate::state::ProposalPDA;

// Events
#[event]
pub struct PriceUpdateProcessed {
    pub market_id: [u8; 32],
    pub keeper_id: [u8; 32],
    pub timestamp: i64,
}

// Types that should be imported from other modules but are defined here for now
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AtRiskPosition {
    pub position_id: [u8; 32],
    pub account: Pubkey,
    pub risk_score: u8,
    pub distance_to_liquidation: u64,
    pub notional: u64,
    pub leverage: u64,
}

#[account]
pub struct StopOrder {
    pub order_id: [u8; 32],
    pub market_id: [u8; 32],
    pub user: Pubkey,
    pub order_type: StopOrderType,
    pub side: OrderSide,
    pub size: u64,
    pub trigger_price: u64,
    pub is_active: bool,
    pub prepaid_bounty: u64,
    pub position_entry_price: u64,
    pub trailing_distance: u64,
    pub trailing_price: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum StopOrderType {
    StopLoss,
    TakeProfit,
    TrailingStop,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Long,
    Short,
}

// Risk calculation function
pub fn calculate_risk_score(position: &ExtendedPosition) -> Result<u8> {
    // Simplified risk score calculation
    let leverage_risk = (position.effective_leverage as u8).min(50);
    let margin_risk = if position.margin_at_risk > position.collateral / 2 { 40 } else { 20 };
    Ok(leverage_risk + margin_risk)
}

// Constants
pub const KEEPER_REWARD_BPS: u64 = 5;  // 0.05%
pub const STOP_KEEPER_BOUNTY_BPS: u64 = 2;  // 0.02%
pub const MAX_LIQUIDATION_PERCENT: u64 = 800;  // 8%
pub const LIQUIDATION_THRESHOLD: u8 = 90;
pub const MONITORING_THRESHOLD: u8 = 80;
pub const SUSPENSION_THRESHOLD: u64 = 8000;  // 80% success rate

// CLAUDE.md: "Permissionless keepers (bots, anyone)"
#[account]
pub struct KeeperRegistry {
    pub total_keepers: u32,
    pub active_keepers: u32,
    pub total_rewards_distributed: u64,
    pub performance_threshold: u64,  // Min successful operations
    pub slash_threshold: u64,        // Max failed operations
}

impl KeeperRegistry {
    pub const LEN: usize = 8 + 4 + 4 + 8 + 8 + 8;
}

#[account]
pub struct KeeperAccount {
    pub keeper_id: [u8; 32],
    pub authority: Pubkey,
    pub mmt_stake: u64,              // Staked MMT for priority
    pub performance_score: u64,       // Success rate
    pub total_operations: u64,
    pub successful_operations: u64,
    pub total_rewards_earned: u64,
    pub last_operation_slot: u64,
    pub status: KeeperStatus,
    pub specializations: Vec<KeeperSpecialization>,
}

impl KeeperAccount {
    pub const BASE_LEN: usize = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 1 + 4;
    
    pub fn space(specializations: usize) -> usize {
        Self::BASE_LEN + specializations
    }
    
    pub fn calculate_priority(&self) -> u64 {
        // Priority = stake * performance_score / 10000
        self.mmt_stake
            .saturating_mul(self.performance_score)
            .saturating_div(10000)
    }

    pub fn has_specialization(&self, work_type: &WorkType) -> bool {
        let required_spec = match work_type {
            WorkType::Liquidations => KeeperSpecialization::Liquidations,
            WorkType::StopOrders => KeeperSpecialization::StopLosses,
            WorkType::PriceUpdates => KeeperSpecialization::PriceUpdates,
            WorkType::Resolutions => KeeperSpecialization::MarketResolution,
        };

        self.specializations.contains(&required_spec)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum KeeperStatus {
    Active,
    Suspended,      // Failed operations exceeded threshold
    Slashed,        // Malicious behavior detected
    Inactive,       // Voluntary pause
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum KeeperSpecialization {
    Liquidations,
    StopLosses,
    PriceUpdates,
    MarketResolution,
    ChainExecution,
    CircuitBreakers,
}

// Liquidation Keeper System
pub struct LiquidationKeeper;

impl LiquidationKeeper {
    // CLAUDE.md: "Incentive: 5bp bounty from liq fee (hardcoded, from vault)"
    pub fn execute_liquidation(
        ctx: Context<ExecuteLiquidation>,
        position_id: [u8; 32],
    ) -> Result<()> {
        // In production, deserialize position from account data
        // For now, create a dummy position to show the logic
        let base_position = Position {
            proposal_id: 0u128,
            outcome: 0,
            size: 1000,
            leverage: 10,
            entry_price: 100,
            liquidation_price: 90,
            is_long: true,
            created_at: 0,
        };
        let mut position = ExtendedPosition::from_position(&base_position, position_id);
        let keeper = &mut ctx.accounts.keeper;

        // Verify position is at risk
        let risk_score = calculate_risk_score(&position)?;
        require!(
            risk_score >= LIQUIDATION_THRESHOLD,
            ErrorCode::PositionNotAtRisk
        );

        // Calculate liquidation amount (max 8% per slot)
        let max_liquidation = position.notional
            .checked_mul(MAX_LIQUIDATION_PERCENT)
            .unwrap()
            .checked_div(10000)
            .unwrap();

        let liquidation_amount = std::cmp::min(
            position.margin_at_risk,
            max_liquidation
        );

        // Execute partial liquidation
        position.execute_partial_liquidation(liquidation_amount)?;

        // Calculate keeper reward (5bp of liquidated amount)
        let keeper_reward = liquidation_amount
            .checked_mul(KEEPER_REWARD_BPS)
            .unwrap()
            .checked_div(10000)
            .unwrap();

        // Transfer reward from vault
        **ctx.accounts.vault.lamports.borrow_mut() -= keeper_reward;
        **keeper.to_account_info().lamports.borrow_mut() += keeper_reward;

        // Update keeper stats
        keeper.successful_operations += 1;
        keeper.total_operations += 1;
        keeper.total_rewards_earned += keeper_reward;
        keeper.last_operation_slot = Clock::get()?.slot;

        // Update performance score
        keeper.performance_score = (keeper.successful_operations * 10000) / keeper.total_operations;

        emit!(LiquidationExecuted {
            position_id,
            keeper_id: keeper.keeper_id,
            amount_liquidated: liquidation_amount,
            keeper_reward,
            risk_score,
            slot: Clock::get()?.slot,
        });

        Ok(())
    }

    // Monitor positions approaching liquidation
    pub fn scan_at_risk_positions(
        ctx: Context<ScanPositions>,
        batch_size: u8,
    ) -> Result<Vec<AtRiskPosition>> {
        let mut at_risk = Vec::new();
        // Price should be passed as parameter or loaded from price cache
        let current_price = 50_000_000; // TODO: Load from price cache or pass as parameter

        for account in ctx.remaining_accounts.iter() {
            if at_risk.len() >= batch_size as usize {
                break;
            }

            // In production, deserialize position from account data
            // For now, skip if account is not the right size
            if account.data_len() >= Position::LEN {
                let base_position = Position {
                    proposal_id: 0u128,
                    outcome: 0,
                    size: 1000,
                    leverage: 10,
                    entry_price: 100,
                    liquidation_price: 90,
                    is_long: true,
                    created_at: 0,
                };
                let position_id = [0u8; 32]; // In practice, this would be derived from the account
                let extended_position = ExtendedPosition::from_position(&base_position, position_id);
                let risk_score = calculate_risk_score_with_price(&extended_position, current_price)?;

                if risk_score >= MONITORING_THRESHOLD {
                    at_risk.push(AtRiskPosition {
                        position_id: extended_position.position_id,
                        account: account.key(),
                        risk_score,
                        distance_to_liquidation: extended_position.calculate_distance_to_liq(current_price)?,
                        notional: extended_position.notional,
                        leverage: extended_position.effective_leverage,
                    });
                }
            }
        }

        // Sort by risk score (highest first)
        at_risk.sort_by(|a, b| b.risk_score.cmp(&a.risk_score));

        Ok(at_risk)
    }
}

// Stop-Loss Keeper System
pub struct StopLossKeeper;

impl StopLossKeeper {
    // CLAUDE.md: "For stops, user-paid 2bp bounty"
    pub fn execute_stop_loss(
        ctx: Context<ExecuteStopLoss>,
        order_id: [u8; 32],
    ) -> Result<()> {
        let order = &mut ctx.accounts.stop_order;
        let keeper = &mut ctx.accounts.keeper;
        // Price should be passed as parameter or loaded from price cache
        let current_price = 50_000_000; // TODO: Load from price cache or pass as parameter

        // Verify stop condition met
        let triggered = match order.order_type {
            StopOrderType::StopLoss => current_price <= order.trigger_price,
            StopOrderType::TakeProfit => current_price >= order.trigger_price,
            StopOrderType::TrailingStop => {
                let distance = order.position_entry_price
                    .checked_sub(current_price)
                    .unwrap_or(0);
                distance >= order.trailing_distance
            }
        };

        require!(triggered, ErrorCode::StopConditionNotMet);

        // Execute the stop order
        let execution_result = order.execute(current_price)?;

        // Calculate keeper bounty (2bp of order value)
        let keeper_bounty = execution_result.executed_value
            .checked_mul(STOP_KEEPER_BOUNTY_BPS)
            .unwrap()
            .checked_div(10000)
            .unwrap();

        // Transfer bounty from user's prepaid amount
        require!(
            order.prepaid_bounty >= keeper_bounty,
            ErrorCode::InsufficientPrepaidBounty
        );

        **order.to_account_info().lamports.borrow_mut() -= keeper_bounty;
        **keeper.to_account_info().lamports.borrow_mut() += keeper_bounty;

        // Update keeper stats
        keeper.successful_operations += 1;
        keeper.total_operations += 1;
        keeper.total_rewards_earned += keeper_bounty;

        emit!(StopLossExecuted {
            order_id,
            keeper_id: keeper.keeper_id,
            trigger_price: order.trigger_price,
            execution_price: current_price,
            keeper_bounty,
            order_type: order.order_type,
        });

        Ok(())
    }

    // Scan for triggered stop orders
    pub fn scan_stop_orders(
        ctx: Context<ScanStopOrders>,
        market_id: [u8; 32],
    ) -> Result<Vec<TriggeredOrder>> {
        let mut triggered = Vec::new();
        // Price should be passed as parameter or loaded from price cache
        let current_price = 50_000_000; // TODO: Load from price cache or pass as parameter

        for i in 0..ctx.remaining_accounts.len() {
            if let Ok(order) = Account::<StopOrder>::try_from(&ctx.remaining_accounts[i]) {
                if order.market_id == market_id && order.is_active {
                    let should_trigger = match order.order_type {
                        StopOrderType::StopLoss => current_price <= order.trigger_price,
                        StopOrderType::TakeProfit => current_price >= order.trigger_price,
                        StopOrderType::TrailingStop => {
                            // Check if stop hit
                            let distance = if order.side == OrderSide::Long {
                                order.trailing_price.saturating_sub(current_price)
                            } else {
                                current_price.saturating_sub(order.trailing_price)
                            };
                            distance >= order.trailing_distance
                        }
                    };

                    if should_trigger {
                        triggered.push(TriggeredOrder {
                            order_id: order.order_id,
                            account: ctx.remaining_accounts[i].key(),
                            order_type: order.order_type,
                            trigger_price: order.trigger_price,
                            current_price,
                            priority: order.calculate_priority(),
                        });
                    }
                }
            }
        }

        // Sort by priority (user stake, order age, etc.)
        triggered.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(triggered)
    }
}

// Price Update Keeper
pub struct PriceUpdateKeeper;

impl PriceUpdateKeeper {
    // Update prices from Polymarket WebSocket
    pub fn update_market_prices(
        ctx: Context<UpdatePrices>,
        updates: Vec<PriceUpdate>,
    ) -> Result<()> {
        let keeper = &mut ctx.accounts.keeper;
        let clock = Clock::get()?;

        for update in updates.iter() {
            // Verify update freshness (<1s old)
            require!(
                clock.unix_timestamp - update.timestamp < 1,
                ErrorCode::StalePriceUpdate
            );

            // Find and update market
            // Note: Due to Anchor lifetime constraints with mutable borrows from remaining_accounts,
            // we need to handle updates differently. In production, you would:
            // 1. Validate the account is a ProposalPDA
            // 2. Update prices via CPI or direct account manipulation
            // 3. Check circuit breakers
            
            // For now, we'll iterate and emit events for matching markets
            let mut found = false;
            for i in 0..ctx.remaining_accounts.len() {
                // In production: properly deserialize and validate this is a ProposalPDA
                // Check if market_id matches update.market_id
                // Update prices: proposal.prices = update.prices.clone()
                // Check circuit breakers based on price movement
                
                // Emit success event for the first match (simplified)
                if !found {
                    emit!(PriceUpdateProcessed {
                        market_id: update.market_id,
                        keeper_id: keeper.keeper_id,
                        timestamp: update.timestamp,
                    });
                    found = true;
                    break;
                }
            }
            
            if !found {
                // Market not found in remaining accounts
                return Err(ErrorCode::MarketNotFound.into());
            }
        }

        // Update keeper stats
        keeper.successful_operations += updates.len() as u64;
        keeper.total_operations += updates.len() as u64;
        keeper.last_operation_slot = clock.slot;

        Ok(())
    }

    // Monitor WebSocket connection health
    pub fn monitor_websocket_health(
        ctx: Context<MonitorWebSocket>,
    ) -> Result<WebSocketHealth> {
        // TODO: Load WebSocketState from the account
        let last_update = 0; // Placeholder - should load from websocket_state account
        let current_slot = Clock::get()?.slot;

        let slots_since_update = current_slot.saturating_sub(last_update);

        let health = if slots_since_update < 150 {  // ~1 minute
            WebSocketHealth::Healthy
        } else if slots_since_update < 750 {  // ~5 minutes
            WebSocketHealth::Degraded
        } else {
            WebSocketHealth::Failed
        };

        if health != WebSocketHealth::Healthy {
            emit!(WebSocketHealthAlert {
                health,
                slots_since_update,
                fallback_active: health == WebSocketHealth::Failed,
            });
        }

        Ok(health)
    }
}

// Keeper Coordination
pub struct KeeperCoordinator;

impl KeeperCoordinator {
    // Distribute work among multiple keepers
    pub fn assign_work_batch(
        ctx: Context<AssignWork>,
        work_type: WorkType,
    ) -> Result<()> {
        let registry = &ctx.accounts.registry;
        
        // Get active keepers using index-based approach to avoid lifetime issues
        let mut active_keepers = Vec::new();
        
        for i in 0..ctx.remaining_accounts.len() {
            let account_info = &ctx.remaining_accounts[i];
            match Account::<KeeperAccount>::try_from(account_info) {
                Ok(keeper) => {
                    if keeper.status == KeeperStatus::Active {
                        active_keepers.push(keeper.into_inner());
                    }
                }
                Err(_) => continue,
            }
        }

        if active_keepers.is_empty() {
            return Err(ErrorCode::NoActiveKeepers.into());
        }

        // Sort keepers by priority (stake * performance)
        active_keepers.sort_by(|a, b| {
            let a_priority = a.calculate_priority();
            let b_priority = b.calculate_priority();
            b_priority.cmp(&a_priority)
        });

        // Get pending work items inline
        let work_items: Vec<WorkItem> = match work_type {
            WorkType::Liquidations => vec![], // Placeholder
            WorkType::StopOrders => vec![],   // Placeholder
            WorkType::PriceUpdates => vec![], // Placeholder
            WorkType::Resolutions => vec![],  // Placeholder
        };
        
        let num_keepers = active_keepers.len();
        let items_per_keeper = if num_keepers > 0 { work_items.len() / num_keepers } else { 0 };

        for (i, keeper) in active_keepers.iter().enumerate() {
            let start = i * items_per_keeper;
            let end = if i == num_keepers - 1 {
                work_items.len()
            } else {
                (i + 1) * items_per_keeper
            };

            let assigned_items = &work_items[start..end];

            emit!(WorkAssigned {
                keeper_id: keeper.keeper_id,
                work_type,
                items_count: assigned_items.len() as u32,
                priority: keeper.calculate_priority(),
            });
        }

        Ok(())
    }

    // Handle keeper failures and reassignment
    pub fn handle_keeper_failure(
        ctx: Context<HandleFailure>,
        failed_keeper_id: [u8; 32],
        work_item: WorkItem,
    ) -> Result<()> {
        let failed_keeper = &mut ctx.accounts.failed_keeper;
        let registry = &mut ctx.accounts.registry;

        // Update failure stats
        failed_keeper.total_operations += 1;
        failed_keeper.performance_score =
            (failed_keeper.successful_operations * 10000) / failed_keeper.total_operations;

        // Check if suspension needed
        if failed_keeper.performance_score < SUSPENSION_THRESHOLD {
            failed_keeper.status = KeeperStatus::Suspended;
            registry.active_keepers -= 1;

            emit!(KeeperSuspended {
                keeper_id: failed_keeper.keeper_id,
                performance_score: failed_keeper.performance_score,
                total_failures: failed_keeper.total_operations - failed_keeper.successful_operations,
            });
        }

        // Reassign work to next available keeper
        // Use index-based approach to avoid lifetime issues
        let mut best_keeper = None;
        let mut best_score = 0u64;

        for i in 0..ctx.remaining_accounts.len() {
            let account_info = &ctx.remaining_accounts[i];
            match Account::<KeeperAccount>::try_from(account_info) {
                Ok(keeper) => {
                    if keeper.status == KeeperStatus::Active &&
                       keeper.keeper_id != failed_keeper_id &&
                       keeper.has_specialization(&work_item.work_type) {

                        let score = keeper.calculate_priority();
                        if score > best_score {
                            best_score = score;
                            best_keeper = Some(keeper.into_inner());
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        let backup_keeper = best_keeper.ok_or(ErrorCode::NoBackupKeeperAvailable)?;

        emit!(WorkReassigned {
            original_keeper: failed_keeper_id,
            new_keeper: backup_keeper.keeper_id,
            work_item: work_item.id,
        });

        Ok(())
    }

    fn get_active_keepers<'info>(
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<Vec<KeeperAccount>> {
        let mut keepers = Vec::new();

        for account in accounts {
            if let Ok(keeper) = Account::<KeeperAccount>::try_from(account) {
                if keeper.status == KeeperStatus::Active {
                    keepers.push(keeper.into_inner());
                }
            }
        }

        Ok(keepers)
    }

    fn get_pending_work<'info>(
        work_type: WorkType,
        accounts: &'info [AccountInfo<'info>],
    ) -> Result<Vec<WorkItem>> {
        match work_type {
            WorkType::Liquidations => Self::get_liquidation_work(accounts),
            WorkType::StopOrders => Self::get_stop_order_work(accounts),
            WorkType::PriceUpdates => Self::get_price_update_work(accounts),
            WorkType::Resolutions => Self::get_resolution_work(accounts),
        }
    }

    fn find_backup_keeper<'info>(
        accounts: &'info [AccountInfo<'info>],
        work_item: &WorkItem,
        exclude: Option<[u8; 32]>,
    ) -> Result<KeeperAccount> {
        let mut best_keeper = None;
        let mut best_score = 0u64;

        for account in accounts {
            if let Ok(keeper) = Account::<KeeperAccount>::try_from(account) {
                if keeper.status == KeeperStatus::Active &&
                   Some(keeper.keeper_id) != exclude &&
                   keeper.has_specialization(&work_item.work_type) {

                    let score = keeper.calculate_priority();
                    if score > best_score {
                        best_score = score;
                        best_keeper = Some(keeper.into_inner());
                    }
                }
            }
        }

        best_keeper.ok_or(ErrorCode::NoBackupKeeperAvailable.into())
    }
    
    // Placeholder implementations for work getters
    fn get_liquidation_work<'info>(_accounts: &'info [AccountInfo<'info>]) -> Result<Vec<WorkItem>> {
        Ok(vec![])
    }
    
    fn get_stop_order_work<'info>(_accounts: &'info [AccountInfo<'info>]) -> Result<Vec<WorkItem>> {
        Ok(vec![])
    }
    
    fn get_price_update_work<'info>(_accounts: &'info [AccountInfo<'info>]) -> Result<Vec<WorkItem>> {
        Ok(vec![])
    }
    
    fn get_resolution_work<'info>(_accounts: &'info [AccountInfo<'info>]) -> Result<Vec<WorkItem>> {
        Ok(vec![])
    }
}

// Supporting types and structs
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum WorkType {
    Liquidations,
    StopOrders,
    PriceUpdates,
    Resolutions,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WorkItem {
    pub id: [u8; 32],
    pub work_type: WorkType,
    pub priority: u64,
    pub data: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PriceUpdate {
    pub market_id: [u8; 32],
    pub prices: Vec<U64F64>,
    pub volumes: Vec<u64>,
    pub timestamp: i64,
    pub signature: [u8; 64],  // Polymarket signature
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TriggeredOrder {
    pub order_id: [u8; 32],
    pub account: Pubkey,
    pub order_type: StopOrderType,
    pub trigger_price: U64F64,
    pub current_price: U64F64,
    pub priority: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Clone, Copy)]
pub enum WebSocketHealth {
    Healthy,
    Degraded,
    Failed,
}

// Context structs
#[derive(Accounts)]
pub struct ExecuteLiquidation<'info> {
    #[account(mut)]
    pub keeper: Account<'info, KeeperAccount>,
    
    /// CHECK: Position account - validated in handler
    #[account(mut)]
    pub position: AccountInfo<'info>,
    
    /// CHECK: Vault to pay keeper rewards
    #[account(mut)]
    pub vault: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ScanPositions<'info> {
    pub keeper: Account<'info, KeeperAccount>,
    
    /// CHECK: Price feed account
    pub price_feed: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ExecuteStopLoss<'info> {
    #[account(mut)]
    pub keeper: Account<'info, KeeperAccount>,
    
    #[account(mut)]
    pub stop_order: Account<'info, StopOrder>,
    
    /// CHECK: Price feed account
    pub price_feed: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ScanStopOrders<'info> {
    pub keeper: Account<'info, KeeperAccount>,
    
    /// CHECK: Price feed account
    pub price_feed: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct UpdatePrices<'info> {
    #[account(mut)]
    pub keeper: Account<'info, KeeperAccount>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MonitorWebSocket<'info> {
    /// CHECK: WebSocket state account
    pub websocket_state: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AssignWork<'info> {
    pub registry: Account<'info, KeeperRegistry>,
}

#[derive(Accounts)]
pub struct HandleFailure<'info> {
    #[account(mut)]
    pub failed_keeper: Account<'info, KeeperAccount>,
    
    #[account(mut)]
    pub registry: Account<'info, KeeperRegistry>,
}

// Events
#[event]
pub struct LiquidationExecuted {
    pub position_id: [u8; 32],
    pub keeper_id: [u8; 32],
    pub amount_liquidated: u64,
    pub keeper_reward: u64,
    pub risk_score: u8,
    pub slot: u64,
}

#[event]
pub struct StopLossExecuted {
    pub order_id: [u8; 32],
    pub keeper_id: [u8; 32],
    pub trigger_price: u64,
    pub execution_price: u64,
    pub keeper_bounty: u64,
    pub order_type: StopOrderType,
}

#[event]
pub struct CircuitBreakerTriggered {
    pub market_id: [u8; 32],
    pub trigger_type: CircuitBreakerType,
    pub keeper_id: [u8; 32],
}

#[event]
pub struct WebSocketHealthAlert {
    pub health: WebSocketHealth,
    pub slots_since_update: u64,
    pub fallback_active: bool,
}

#[event]
pub struct WorkAssigned {
    pub keeper_id: [u8; 32],
    pub work_type: WorkType,
    pub items_count: u32,
    pub priority: u64,
}

#[event]
pub struct KeeperSuspended {
    pub keeper_id: [u8; 32],
    pub performance_score: u64,
    pub total_failures: u64,
}

#[event]
pub struct WorkReassigned {
    pub original_keeper: [u8; 32],
    pub new_keeper: [u8; 32],
    pub work_item: [u8; 32],
}

// Placeholder types that need to be imported/defined
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct Market {
    pub market_id: [u8; 32],
    pub prices: Vec<U64F64>,
}

impl Market {
    pub fn update_prices(&mut self, prices: Vec<U64F64>, _timestamp: i64) -> Result<()> {
        self.prices = prices;
        Ok(())
    }
    
    pub fn check_price_movement_breaker(&self) -> Result<bool> {
        // Placeholder implementation
        Ok(false)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum CircuitBreakerType {
    PriceMovement,
    VolumeSpike,
    LiquidationCascade,
}

// Extended Position struct for keeper operations
#[derive(Clone)]
pub struct ExtendedPosition {
    pub base: Position,
    pub position_id: [u8; 32],
    pub notional: u64,
    pub margin_at_risk: u64,
    pub collateral: u64,
    pub effective_leverage: u64,
}

impl ExtendedPosition {
    pub fn from_position(position: &Position, position_id: [u8; 32]) -> Self {
        Self {
            base: position.clone(),
            position_id,
            notional: position.size * position.leverage,
            margin_at_risk: position.size,
            collateral: position.size,
            effective_leverage: position.leverage,
        }
    }
    
    pub fn execute_partial_liquidation(&mut self, amount: u64) -> Result<()> {
        self.notional = self.notional.saturating_sub(amount);
        self.margin_at_risk = self.margin_at_risk.saturating_sub(amount);
        Ok(())
    }
    
    pub fn calculate_distance_to_liq(&self, current_price: u64) -> Result<u64> {
        let distance = if self.base.is_long {
            current_price.saturating_sub(self.base.liquidation_price)
        } else {
            self.base.liquidation_price.saturating_sub(current_price)
        };
        Ok(distance)
    }
}

// Extension traits for StopOrder
impl StopOrder {
    pub fn execute(&mut self, _current_price: u64) -> Result<ExecutionResult> {
        self.is_active = false;
        Ok(ExecutionResult {
            executed_value: self.size,
        })
    }
    
    pub fn calculate_priority(&self) -> u64 {
        // Placeholder - in production would consider user stake, order age, etc
        100
    }
}

pub struct ExecutionResult {
    pub executed_value: u64,
}

// Helper function for risk calculation with price
pub fn calculate_risk_score_with_price(position: &ExtendedPosition, _current_price: u64) -> Result<u8> {
    calculate_risk_score(position)
}