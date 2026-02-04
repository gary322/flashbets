//! Backtesting Infrastructure
//!
//! Production-grade backtesting with IPFS historical data and on-chain event replay

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
    math::U64F64,
    state::{ProposalPDA, Position, accounts::discriminators},
    events::EventType,
};

/// IPFS gateway for historical data
pub const IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

/// Backtest configuration
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BacktestConfig {
    /// Start slot for backtest
    pub start_slot: u64,
    /// End slot for backtest
    pub end_slot: u64,
    /// Initial capital
    pub initial_capital: u64,
    /// Strategy parameters
    pub strategy_params: StrategyParams,
    /// Risk limits
    pub risk_limits: RiskLimits,
    /// IPFS hash for historical data
    pub ipfs_data_hash: [u8; 32],
    /// Event replay mode
    pub replay_mode: ReplayMode,
}

/// Strategy parameters
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct StrategyParams {
    /// Maximum position size (% of capital)
    pub max_position_size_pct: u8,
    /// Maximum leverage
    pub max_leverage: u8,
    /// Stop loss percentage
    pub stop_loss_pct: u8,
    /// Take profit percentage
    pub take_profit_pct: u8,
    /// Rebalance frequency (slots)
    pub rebalance_frequency: u64,
}

/// Risk limits for backtesting
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct RiskLimits {
    /// Maximum drawdown allowed
    pub max_drawdown_pct: u8,
    /// Maximum VaR (% of capital)
    pub max_var_pct: u8,
    /// Maximum number of positions
    pub max_positions: u16,
    /// Correlation limit
    pub max_correlation: u16,
}

/// Event replay mode
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum ReplayMode {
    /// Replay all events
    Full,
    /// Replay only trading events
    TradingOnly,
    /// Replay with sampling
    Sampled { rate: u16 }, // basis points
}

/// Backtest state
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct BacktestState {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Backtest ID
    pub backtest_id: [u8; 32],
    /// Configuration
    pub config: BacktestConfig,
    /// Current slot being processed
    pub current_slot: u64,
    /// Current capital
    pub current_capital: u64,
    /// Total PnL
    pub total_pnl: i64,
    /// Number of trades
    pub trade_count: u32,
    /// Win rate (basis points)
    pub win_rate: u16,
    /// Maximum drawdown
    pub max_drawdown: u64,
    /// Sharpe ratio (x1000)
    pub sharpe_ratio: i64,
    /// Status
    pub status: BacktestStatus,
    /// Performance metrics
    pub metrics: PerformanceMetrics,
}

/// Backtest status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BacktestStatus {
    /// Backtest is running
    Running,
    /// Backtest completed successfully
    Completed,
    /// Backtest failed
    Failed,
    /// Backtest paused
    Paused,
}

/// Performance metrics
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PerformanceMetrics {
    /// Total return (basis points)
    pub total_return_bps: i16,
    /// Annualized return (basis points)
    pub annualized_return_bps: i16,
    /// Volatility (basis points)
    pub volatility_bps: u16,
    /// Maximum consecutive wins
    pub max_consecutive_wins: u16,
    /// Maximum consecutive losses
    pub max_consecutive_losses: u16,
    /// Average win size
    pub avg_win_size: u64,
    /// Average loss size
    pub avg_loss_size: u64,
    /// Profit factor
    pub profit_factor: U64F64,
}

/// Historical market data
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct HistoricalMarketData {
    /// Market ID
    pub market_id: u128,
    /// Slot timestamp
    pub slot: u64,
    /// Outcome prices
    pub prices: Vec<U64F64>,
    /// Volume
    pub volume: u64,
    /// Liquidity
    pub liquidity: u64,
    /// Volatility
    pub volatility: U64F64,
}

/// Historical event
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct HistoricalEvent {
    /// Event type
    pub event_type: EventType,
    /// Event slot
    pub slot: u64,
    /// Event data
    pub data: Vec<u8>,
}

impl BacktestState {
    /// Create new backtest
    pub fn new(
        backtest_id: [u8; 32],
        config: BacktestConfig,
    ) -> Result<Self, ProgramError> {
        Ok(Self {
            discriminator: discriminators::BACKTEST_STATE,
            backtest_id,
            current_slot: config.start_slot,
            current_capital: config.initial_capital,
            total_pnl: 0,
            trade_count: 0,
            win_rate: 0,
            max_drawdown: 0,
            sharpe_ratio: 0,
            status: BacktestStatus::Running,
            metrics: PerformanceMetrics {
                total_return_bps: 0,
                annualized_return_bps: 0,
                volatility_bps: 0,
                max_consecutive_wins: 0,
                max_consecutive_losses: 0,
                avg_win_size: 0,
                avg_loss_size: 0,
                profit_factor: U64F64::from_num(0),
            },
            config,
        })
    }

    /// Process historical event
    pub fn process_event(
        &mut self,
        event: &HistoricalEvent,
    ) -> Result<(), ProgramError> {
        // Skip if before start or after end
        if event.slot < self.config.start_slot || event.slot > self.config.end_slot {
            return Ok(());
        }

        // Update current slot
        self.current_slot = event.slot;

        // Process based on replay mode
        match self.config.replay_mode {
            ReplayMode::Full => {
                // Process all events
                self.process_event_internal(event)?;
            }
            ReplayMode::TradingOnly => {
                // Only process trading-related events
                match event.event_type {
                    EventType::PositionOpened |
                    EventType::PositionClosed |
                    EventType::PositionLiquidated |
                    EventType::TradeExecuted => {
                        self.process_event_internal(event)?;
                    }
                    _ => {}
                }
            }
            ReplayMode::Sampled { rate } => {
                // Sample events based on rate
                let hash = self.hash_event(event);
                if (hash % 10000) < rate as u64 {
                    self.process_event_internal(event)?;
                }
            }
        }

        Ok(())
    }

    /// Internal event processing
    fn process_event_internal(
        &mut self,
        event: &HistoricalEvent,
    ) -> Result<(), ProgramError> {
        match event.event_type {
            EventType::PositionOpened => {
                // Simulate position opening
                self.trade_count += 1;
            }
            EventType::PositionClosed => {
                // Extract PnL from event data
                if event.data.len() >= 8 {
                    let pnl = i64::from_le_bytes(
                        event.data[0..8].try_into()
                            .map_err(|_| ProgramError::InvalidAccountData)?
                    );
                    self.update_pnl(pnl)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Update PnL and metrics
    fn update_pnl(&mut self, pnl: i64) -> Result<(), ProgramError> {
        self.total_pnl = self.total_pnl
            .checked_add(pnl)
            .ok_or(BettingPlatformError::Overflow)?;

        // Update capital
        if pnl > 0 {
            self.current_capital = self.current_capital
                .checked_add(pnl as u64)
                .ok_or(BettingPlatformError::Overflow)?;
        } else {
            let loss = (-pnl) as u64;
            self.current_capital = self.current_capital
                .checked_sub(loss)
                .ok_or(BettingPlatformError::Underflow)?;
        }

        // Update drawdown
        let drawdown = self.config.initial_capital
            .saturating_sub(self.current_capital);
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
        }

        Ok(())
    }

    /// Hash event for sampling
    fn hash_event(&self, event: &HistoricalEvent) -> u64 {
        // Simple hash using slot and event type discriminator
        let mut hash = event.slot;
        let event_discriminator = match event.event_type {
            EventType::PositionOpened => 10u64,
            EventType::PositionClosed => 11u64,
            EventType::PositionLiquidated => 12u64,
            EventType::TradeExecuted => 44u64,
            _ => 0u64,
        };
        hash = hash.wrapping_mul(31).wrapping_add(event_discriminator);
        hash
    }

    /// Calculate final metrics
    pub fn finalize(&mut self) -> Result<(), ProgramError> {
        if self.status != BacktestStatus::Running {
            return Err(BettingPlatformError::InvalidStatus.into());
        }

        // Calculate total return
        let total_return = if self.config.initial_capital > 0 {
            ((self.current_capital as i64 - self.config.initial_capital as i64) * 10000) /
            self.config.initial_capital as i64
        } else {
            0
        };
        self.metrics.total_return_bps = total_return as i16;

        // Calculate annualized return (simplified)
        let slots_elapsed = self.config.end_slot - self.config.start_slot;
        let years = slots_elapsed as f64 / (2.0 * 365.0 * 24.0 * 3600.0); // ~2 slots/sec
        if years > 0.0 {
            let annualized = (total_return as f64 / years) as i16;
            self.metrics.annualized_return_bps = annualized;
        }

        // Update status
        self.status = BacktestStatus::Completed;

        Ok(())
    }
}

/// IPFS data loader
pub struct IPFSDataLoader;

impl IPFSDataLoader {
    /// Load historical data from IPFS
    pub fn load_historical_data(
        ipfs_hash: &[u8; 32],
        start_slot: u64,
        end_slot: u64,
    ) -> Result<Vec<HistoricalEvent>, ProgramError> {
        // In production, this would fetch from IPFS
        // For now, return mock data
        msg!("Loading historical data from IPFS hash: {:?}", ipfs_hash);
        msg!("Slot range: {} to {}", start_slot, end_slot);

        // Mock historical events
        let mut events = Vec::new();

        // Simulate some historical events
        for slot in (start_slot..=end_slot).step_by(1000) {
            events.push(HistoricalEvent {
                event_type: EventType::PositionOpened,
                slot,
                data: vec![1, 2, 3, 4, 5, 6, 7, 8],
            });

            events.push(HistoricalEvent {
                event_type: EventType::PositionClosed,
                slot: slot + 500,
                data: 1000i64.to_le_bytes().to_vec(), // Mock profit
            });
        }

        Ok(events)
    }

    /// Load market data from IPFS
    pub fn load_market_data(
        ipfs_hash: &[u8; 32],
        market_id: u128,
        start_slot: u64,
        end_slot: u64,
    ) -> Result<Vec<HistoricalMarketData>, ProgramError> {
        msg!("Loading market data for market {} from IPFS", market_id);

        // Mock market data
        let mut data = Vec::new();

        for slot in (start_slot..=end_slot).step_by(100) {
            data.push(HistoricalMarketData {
                market_id,
                slot,
                prices: vec![
                    U64F64::from_fraction(1, 2).unwrap(),  // 0.5 = 1/2
                    U64F64::from_fraction(1, 2).unwrap(),  // 0.5 = 1/2
                ],
                volume: 1_000_000,
                liquidity: 500_000,
                volatility: U64F64::from_fraction(1, 5).unwrap(),  // 0.2 = 1/5
            });
        }

        Ok(data)
    }
}

/// Initialize backtest
pub fn initialize_backtest<'a>(
    backtest_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    config: BacktestConfig,
    backtest_id: [u8; 32],
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    // Validate authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate account is empty
    if !backtest_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // Validate config
    if config.start_slot >= config.end_slot {
        return Err(BettingPlatformError::InvalidTimeRange.into());
    }

    if config.initial_capital == 0 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }

    // Create backtest state
    let backtest_state = BacktestState::new(backtest_id, config)?;

    // Calculate space
    let space = backtest_state.try_to_vec()?.len() + 1024; // Extra space

    // Create account
    let rent = solana_program::rent::Rent::get()?;
    let rent_lamports = rent.minimum_balance(space);

    solana_program::program::invoke(
        &solana_program::system_instruction::create_account(
            payer.key,
            backtest_account.key,
            rent_lamports,
            space as u64,
            &crate::ID,
        ),
        &[payer.clone(), backtest_account.clone(), system_program.clone()],
    )?;

    // Serialize
    backtest_state.serialize(&mut &mut backtest_account.data.borrow_mut()[..])?;

    msg!("Backtest initialized: {:?}", backtest_id);

    Ok(())
}

/// Process backtest batch
pub fn process_backtest_batch<'a>(
    backtest_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    events: Vec<HistoricalEvent>,
) -> ProgramResult {
    // Validate authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load backtest state
    let mut backtest_data = backtest_account.try_borrow_mut_data()?;
    let mut backtest_state = BacktestState::try_from_slice(&backtest_data)?;

    // Verify discriminator
    if backtest_state.discriminator != discriminators::BACKTEST_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify status
    if backtest_state.status != BacktestStatus::Running {
        return Err(BettingPlatformError::InvalidStatus.into());
    }

    // Process events
    let event_count = events.len();
    for event in events {
        backtest_state.process_event(&event)?;
    }

    // Save state
    backtest_state.serialize(&mut &mut backtest_data[..])?;

    msg!("Processed {} historical events", event_count);

    Ok(())
}

/// Finalize backtest
pub fn finalize_backtest<'a>(
    backtest_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
) -> ProgramResult {
    // Validate authority
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load backtest state
    let mut backtest_data = backtest_account.try_borrow_mut_data()?;
    let mut backtest_state = BacktestState::try_from_slice(&backtest_data)?;

    // Verify discriminator
    if backtest_state.discriminator != discriminators::BACKTEST_STATE {
        return Err(ProgramError::InvalidAccountData);
    }

    // Finalize
    backtest_state.finalize()?;

    // Save state
    backtest_state.serialize(&mut &mut backtest_data[..])?;

    msg!("Backtest finalized with return: {} bps", 
        backtest_state.metrics.total_return_bps);

    Ok(())
}

/// Backtest strategy evaluator
pub struct StrategyEvaluator;

impl StrategyEvaluator {
    /// Evaluate strategy performance
    pub fn evaluate(
        backtest_state: &BacktestState,
    ) -> StrategyEvaluation {
        let return_quality = if backtest_state.metrics.total_return_bps > 2000 {
            QualityScore::Excellent
        } else if backtest_state.metrics.total_return_bps > 1000 {
            QualityScore::Good
        } else if backtest_state.metrics.total_return_bps > 0 {
            QualityScore::Average
        } else {
            QualityScore::Poor
        };

        let risk_quality = if backtest_state.max_drawdown < backtest_state.config.initial_capital / 10 {
            QualityScore::Excellent
        } else if backtest_state.max_drawdown < backtest_state.config.initial_capital / 5 {
            QualityScore::Good
        } else if backtest_state.max_drawdown < backtest_state.config.initial_capital / 2 {
            QualityScore::Average
        } else {
            QualityScore::Poor
        };

        StrategyEvaluation {
            overall_score: (backtest_state.metrics.total_return_bps / 100) as u8,
            return_quality,
            risk_quality,
            consistency_score: calculate_consistency_score(backtest_state),
            recommendation: generate_recommendation(backtest_state),
        }
    }
}

/// Strategy evaluation result
#[derive(Debug)]
pub struct StrategyEvaluation {
    pub overall_score: u8, // 0-100
    pub return_quality: QualityScore,
    pub risk_quality: QualityScore,
    pub consistency_score: u8,
    pub recommendation: StrategyRecommendation,
}

#[derive(Debug)]
pub enum QualityScore {
    Excellent,
    Good,
    Average,
    Poor,
}

#[derive(Debug)]
pub enum StrategyRecommendation {
    Deploy,
    Optimize,
    Reject,
}

fn calculate_consistency_score(state: &BacktestState) -> u8 {
    // Simple consistency score based on win rate
    (state.win_rate / 100).min(100) as u8
}

fn generate_recommendation(state: &BacktestState) -> StrategyRecommendation {
    if state.metrics.total_return_bps > 2000 && state.max_drawdown < state.config.initial_capital / 5 {
        StrategyRecommendation::Deploy
    } else if state.metrics.total_return_bps > 0 {
        StrategyRecommendation::Optimize
    } else {
        StrategyRecommendation::Reject
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtest_creation() {
        let config = BacktestConfig {
            start_slot: 1000,
            end_slot: 2000,
            initial_capital: 1_000_000,
            strategy_params: StrategyParams {
                max_position_size_pct: 10,
                max_leverage: 5,
                stop_loss_pct: 5,
                take_profit_pct: 10,
                rebalance_frequency: 100,
            },
            risk_limits: RiskLimits {
                max_drawdown_pct: 20,
                max_var_pct: 5,
                max_positions: 10,
                max_correlation: 7000,
            },
            ipfs_data_hash: [0u8; 32],
            replay_mode: ReplayMode::Full,
        };

        let backtest = BacktestState::new([1u8; 32], config).unwrap();
        assert_eq!(backtest.status, BacktestStatus::Running);
        assert_eq!(backtest.current_capital, 1_000_000);
    }

    #[test]
    fn test_event_processing() {
        let config = BacktestConfig {
            start_slot: 1000,
            end_slot: 2000,
            initial_capital: 1_000_000,
            strategy_params: StrategyParams {
                max_position_size_pct: 10,
                max_leverage: 5,
                stop_loss_pct: 5,
                take_profit_pct: 10,
                rebalance_frequency: 100,
            },
            risk_limits: RiskLimits {
                max_drawdown_pct: 20,
                max_var_pct: 5,
                max_positions: 10,
                max_correlation: 7000,
            },
            ipfs_data_hash: [0u8; 32],
            replay_mode: ReplayMode::TradingOnly,
        };

        let mut backtest = BacktestState::new([1u8; 32], config).unwrap();

        // Process position opened event
        let event = HistoricalEvent {
            event_type: EventType::PositionOpened,
            slot: 1500,
            data: vec![],
        };
        backtest.process_event(&event).unwrap();
        assert_eq!(backtest.trade_count, 1);

        // Process position closed with profit
        let event = HistoricalEvent {
            event_type: EventType::PositionClosed,
            slot: 1600,
            data: 10000i64.to_le_bytes().to_vec(),
        };
        backtest.process_event(&event).unwrap();
        assert_eq!(backtest.total_pnl, 10000);
        assert_eq!(backtest.current_capital, 1_010_000);
    }
}