// Phase 20: Money-Making Focus UI Integration
// Optimizes the platform for maximum profit generation and trading efficiency

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
    events::{emit_event, EventType},
};

/// Profit optimization configuration
pub const MIN_PROFIT_THRESHOLD_BPS: u16 = 50; // 0.5% minimum profit
pub const OPTIMAL_LEVERAGE_RATIO: u64 = 10000; // 1.0x base, can go up to 10x
pub const QUICK_PROFIT_WINDOW: u64 = 900; // 15 minutes for quick profits
pub const COMPOUND_INTERVAL: u64 = 7200; // 2 hours for auto-compound
pub const MAX_CONCURRENT_POSITIONS: u32 = 20;
pub const PROFIT_TAKE_PERCENTAGE: u64 = 2000; // 20% profit taking
pub const CHAIN_LEVERAGE_MULTIPLIERS: [f64; 3] = [1.5, 1.2, 1.1]; // For 3-step chain
pub const CHAIN_BASE_LEVERAGE: u64 = 100; // 100x base leverage
pub const MARKET_MOVE_FACTOR: f64 = 0.2; // 20% market move assumption

/// Money-making optimizer
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MoneyMakingOptimizer {
    pub total_profit_generated: u64,
    pub total_fees_saved: u64,
    pub winning_positions: u64,
    pub losing_positions: u64,
    pub current_streak: i32,
    pub best_streak: u32,
    pub average_profit_per_trade: u64,
    pub compound_earnings: u64,
    pub active_strategies: Vec<ProfitStrategy>,
    pub performance_metrics: PerformanceMetrics,
    pub last_update_slot: u64,
}

impl MoneyMakingOptimizer {
    pub const SIZE: usize = 8 + // total_profit_generated
        8 + // total_fees_saved
        8 + // winning_positions
        8 + // losing_positions
        4 + // current_streak
        4 + // best_streak
        8 + // average_profit_per_trade
        8 + // compound_earnings
        4 + 100 * 5 + // active_strategies (up to 5)
        PerformanceMetrics::SIZE +
        8; // last_update_slot

    /// Initialize optimizer
    pub fn initialize(&mut self) -> ProgramResult {
        self.total_profit_generated = 0;
        self.total_fees_saved = 0;
        self.winning_positions = 0;
        self.losing_positions = 0;
        self.current_streak = 0;
        self.best_streak = 0;
        self.average_profit_per_trade = 0;
        self.compound_earnings = 0;
        self.active_strategies = Vec::new();
        self.performance_metrics = PerformanceMetrics::default();
        self.last_update_slot = Clock::get()?.slot;

        msg!("Money-making optimizer initialized");
        Ok(())
    }

    /// Analyze profit opportunity
    pub fn analyze_opportunity(
        &self,
        market: &MarketData,
        user_balance: u64,
        current_positions: u32,
    ) -> Result<ProfitOpportunity, ProgramError> {
        // Check if user can take more positions
        if current_positions >= MAX_CONCURRENT_POSITIONS {
            return Err(BettingPlatformError::TooManyPositions.into());
        }

        // Calculate potential profit
        let edge = self.calculate_edge(market)?;
        if edge < MIN_PROFIT_THRESHOLD_BPS {
            return Ok(ProfitOpportunity {
                should_trade: false,
                ..Default::default()
            });
        }

        // Determine optimal position size
        let kelly_fraction = self.calculate_kelly_criterion(edge, market.odds)?;
        let position_size = self.calculate_position_size(
            user_balance,
            kelly_fraction,
            current_positions,
        )?;

        // Calculate leverage
        let optimal_leverage = self.calculate_optimal_leverage(
            market.volatility_bps,
            edge,
        )?;

        Ok(ProfitOpportunity {
            should_trade: true,
            market_id: market.market_id,
            direction: if market.yes_price < market.no_price {
                TradeDirection::Yes
            } else {
                TradeDirection::No
            },
            position_size,
            leverage: optimal_leverage,
            expected_profit: (position_size * edge as u64) / 10000,
            confidence_score: self.calculate_confidence(edge, market)?,
            time_to_expiry: market.expiry_slot.saturating_sub(Clock::get()?.slot),
        })
    }

    /// Calculate edge (expected value)
    fn calculate_edge(&self, market: &MarketData) -> Result<u16, ProgramError> {
        // Edge = (Probability * Payout) - Cost
        // In basis points where 10000 = 100%
        
        let implied_prob_yes = (10000 * 10000) / market.yes_price as u128;
        let implied_prob_no = (10000 * 10000) / market.no_price as u128;
        
        // Normalize probabilities
        let total_prob = implied_prob_yes + implied_prob_no;
        let norm_prob_yes = (implied_prob_yes * 10000 / total_prob) as u64;
        
        // Calculate true probability (using our proprietary model)
        let true_prob = self.calculate_true_probability(market)?;
        
        // Edge = True Probability - Market Probability
        let edge = if true_prob > norm_prob_yes {
            true_prob - norm_prob_yes
        } else {
            0
        };
        
        Ok(edge as u16)
    }

    /// Calculate true probability using advanced model
    fn calculate_true_probability(&self, market: &MarketData) -> Result<u64, ProgramError> {
        // Factors: volume, liquidity, sentiment, correlation
        let base_prob = 5000; // Start at 50%
        
        // Volume factor
        let volume_factor = (market.volume_24h / 1_000_000_000).min(1000) as i64;
        
        // Liquidity factor
        let liquidity_factor = (market.liquidity / 10_000_000_000).min(500) as i64;
        
        // Adjust probability
        let adjusted_prob = base_prob + volume_factor - liquidity_factor / 2;
        
        Ok(adjusted_prob.max(100).min(9900) as u64)
    }

    /// Kelly Criterion for optimal bet sizing
    fn calculate_kelly_criterion(&self, edge_bps: u16, odds: u64) -> Result<u64, ProgramError> {
        // Kelly % = (p*b - q) / b
        // Where p = probability of winning, b = odds, q = probability of losing
        
        let p = (5000 + edge_bps) as u128; // Probability in bps
        let q = 10000 - p;
        let b = odds as u128;
        
        let kelly_bps = (p * b - q * 10000) / b;
        
        // Apply Kelly fraction (usually 25% of full Kelly for safety)
        let safe_kelly = kelly_bps / 4;
        
        Ok(safe_kelly.min(2500) as u64) // Cap at 25% of bankroll
    }

    /// Calculate position size
    fn calculate_position_size(
        &self,
        balance: u64,
        kelly_fraction: u64,
        current_positions: u32,
    ) -> Result<u64, ProgramError> {
        // Diversification factor
        let diversification_factor = 10000 / (current_positions + 1) as u64;
        
        // Base position size
        let base_size = (balance as u128 * kelly_fraction as u128 / 10000) as u64;
        
        // Apply diversification
        let adjusted_size = (base_size as u128 * diversification_factor as u128 / 10000) as u64;
        
        // Minimum and maximum bounds
        let min_size = balance / 100; // 1% minimum
        let max_size = balance / 4;    // 25% maximum
        
        Ok(adjusted_size.max(min_size).min(max_size))
    }

    /// Calculate optimal leverage
    fn calculate_optimal_leverage(
        &self,
        volatility_bps: u64,
        edge_bps: u16,
    ) -> Result<u64, ProgramError> {
        // Lower leverage for high volatility
        let vol_factor = 10000_u128.saturating_sub(volatility_bps as u128);
        
        // Higher leverage for higher edge
        let edge_factor = 10000 + (edge_bps as u128 * 2);
        
        // Combined leverage = base * vol_factor * edge_factor
        let leverage_bps = (OPTIMAL_LEVERAGE_RATIO as u128 * vol_factor * edge_factor) 
            / (10000 * 10000);
        
        // Cap at 10x
        Ok(leverage_bps.min(100000) as u64)
    }

    /// Calculate confidence score
    fn calculate_confidence(
        &self,
        edge_bps: u16,
        market: &MarketData,
    ) -> Result<u64, ProgramError> {
        let mut confidence = 5000u64; // Start at 50%
        
        // Edge contribution
        confidence += (edge_bps as u64 * 2).min(2000);
        
        // Volume contribution
        if market.volume_24h > 1_000_000_000_000 { // $1M+
            confidence += 1000;
        }
        
        // Liquidity contribution
        if market.liquidity > 10_000_000_000_000 { // $10M+
            confidence += 1000;
        }
        
        // Time contribution (prefer markets with more time)
        let time_factor = (market.expiry_slot.saturating_sub(Clock::get()?.slot) / 1000).min(500);
        confidence += time_factor;
        
        Ok(confidence.min(9500)) // Cap at 95%
    }

    /// Execute profit strategy
    pub fn execute_strategy(
        &mut self,
        opportunity: &ProfitOpportunity,
        execution_result: &ExecutionResult,
    ) -> ProgramResult {
        // Update metrics
        if execution_result.profit > 0 {
            self.winning_positions += 1;
            self.current_streak = self.current_streak.saturating_add(1);
            if self.current_streak > self.best_streak as i32 {
                self.best_streak = self.current_streak as u32;
            }
        } else {
            self.losing_positions += 1;
            self.current_streak = 0;
        }

        self.total_profit_generated += execution_result.profit;
        self.total_fees_saved += execution_result.fees_saved;

        // Update average
        let total_trades = self.winning_positions + self.losing_positions;
        if total_trades > 0 {
            self.average_profit_per_trade = self.total_profit_generated / total_trades;
        }

        // Check for compound opportunity
        if self.should_compound(execution_result.profit)? {
            self.compound_earnings += execution_result.profit;
        }

        self.last_update_slot = Clock::get()?.slot;

        msg!("Strategy executed. Profit: {}, Total: {}", 
            execution_result.profit, 
            self.total_profit_generated
        );

        Ok(())
    }

    /// Check if profits should be compounded
    fn should_compound(&self, profit: u64) -> Result<bool, ProgramError> {
        let current_slot = Clock::get()?.slot;
        let slots_since_update = current_slot.saturating_sub(self.last_update_slot);
        
        // Compound if enough time passed and profit is significant
        Ok(slots_since_update > COMPOUND_INTERVAL && profit > 100_000_000) // $100+
    }

    /// Get profit recommendations
    pub fn get_recommendations(&self, user_stats: &UserStats) -> ProfitRecommendations {
        let mut recommendations = ProfitRecommendations {
            strategies: Vec::new(),
            risk_level: RiskLevel::Medium,
            suggested_leverage: OPTIMAL_LEVERAGE_RATIO,
            take_profit_levels: vec![2000, 5000, 10000], // 20%, 50%, 100%
            stop_loss_level: 1000, // 10%
        };

        // Adjust based on user performance
        if user_stats.win_rate > 6000 { // 60%+ win rate
            recommendations.risk_level = RiskLevel::High;
            recommendations.suggested_leverage = 30000; // 3x
        } else if user_stats.win_rate < 4000 { // <40% win rate
            recommendations.risk_level = RiskLevel::Low;
            recommendations.suggested_leverage = 5000; // 0.5x
        }

        // Add active strategies
        recommendations.strategies.push(ProfitStrategy::MomentumTrading);
        recommendations.strategies.push(ProfitStrategy::ArbOpportunities);
        
        if self.performance_metrics.sharpe_ratio > 15000 { // 1.5+
            recommendations.strategies.push(ProfitStrategy::CompoundGrowth);
        }

        recommendations
    }

    /// Calculate leverage chain return (3955% example from spec)
    pub fn calculate_chain_return(
        &self,
        deposit: u64,
        chain_steps: u8,
    ) -> Result<u64, ProgramError> {
        // Base case: deposit * base_leverage * product(multipliers) * market_move - fees
        // Example: 100 * 100 * (1.5 * 1.2 * 1.1) * 0.2 - 5 = 3955
        
        let mut effective_multiplier = 1.0;
        
        // Apply chain multipliers based on number of steps
        for i in 0..chain_steps.min(3) {
            effective_multiplier *= CHAIN_LEVERAGE_MULTIPLIERS[i as usize];
        }
        
        // Calculate return
        let base_return = (deposit as f64) * (CHAIN_BASE_LEVERAGE as f64) * effective_multiplier * MARKET_MOVE_FACTOR;
        
        // Subtract fees (approximately 5 basis points per step)
        let fees = (chain_steps as f64) * 5.0;
        let net_return = base_return - fees;
        
        // Calculate percentage return
        let percentage_return = ((net_return / deposit as f64) * 100.0) as u64;
        
        msg!("Chain return calculation: deposit={}, steps={}, return={}%", 
            deposit, chain_steps, percentage_return);
        
        Ok(percentage_return)
    }

    /// Calculate daily volume edge for arbitrage opportunities
    pub fn calculate_daily_volume_edge(
        &self,
        daily_volume: u64,
        edge_percentage: u16,
    ) -> Result<u64, ProgramError> {
        // Edge calculation: $10k daily volume at 1% edge = $100 profit
        let edge_amount = (daily_volume as u128 * edge_percentage as u128) / 10000;
        
        msg!("Daily volume edge: volume={}, edge={}%, profit={}", 
            daily_volume, edge_percentage / 100, edge_amount);
        
        Ok(edge_amount as u64)
    }

    /// Calculate true probability using alternative model (for comparison)
    fn calculate_true_probability_alt(&self, market: &MarketData) -> Result<u64, ProgramError> {
        // Simple model based on volume and liquidity signals
        // In production, this would use advanced ML models
        
        let volume_signal = if market.volume_24h > 1_000_000_000 { // $1M+
            500 // 5% adjustment
        } else {
            0
        };
        
        let liquidity_signal = if market.liquidity > 500_000_000 { // $500k+
            300 // 3% adjustment
        } else {
            0
        };
        
        // Base probability from market price
        let base_prob = (10000 * 10000) / market.yes_price as u128;
        let adjusted_prob = base_prob + volume_signal as u128 + liquidity_signal as u128;
        
        Ok((adjusted_prob % 10000) as u64)
    }
}

/// Market data for analysis
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketData {
    pub market_id: Pubkey,
    pub yes_price: u64,
    pub no_price: u64,
    pub volume_24h: u64,
    pub liquidity: u64,
    pub volatility_bps: u64,
    pub expiry_slot: u64,
    pub odds: u64,
}

/// Profit opportunity
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct ProfitOpportunity {
    pub should_trade: bool,
    pub market_id: Pubkey,
    pub direction: TradeDirection,
    pub position_size: u64,
    pub leverage: u64,
    pub expected_profit: u64,
    pub confidence_score: u64,
    pub time_to_expiry: u64,
}

/// Trade direction
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq)]
pub enum TradeDirection {
    Yes,
    No,
}

impl Default for TradeDirection {
    fn default() -> Self {
        TradeDirection::Yes
    }
}

/// Execution result
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ExecutionResult {
    pub profit: u64,
    pub fees_saved: u64,
    pub execution_time: u64,
    pub slippage_bps: u16,
}

/// User statistics
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct UserStats {
    pub total_trades: u64,
    pub winning_trades: u64,
    pub total_profit: i64,
    pub win_rate: u64, // In basis points
    pub average_leverage: u64,
}

/// Performance metrics
#[derive(BorshSerialize, BorshDeserialize, Clone, Default)]
pub struct PerformanceMetrics {
    pub total_return: i64,
    pub sharpe_ratio: u64, // In basis points
    pub max_drawdown: u64,
    pub profit_factor: u64,
    pub recovery_factor: u64,
}

impl PerformanceMetrics {
    pub const SIZE: usize = 8 + 8 + 8 + 8 + 8;
}

/// Profit strategy types
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq)]
pub enum ProfitStrategy {
    MomentumTrading,
    MeanReversion,
    ArbOpportunities,
    CompoundGrowth,
    YieldFarming,
}

/// Risk levels
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Degen,
}

/// Profit recommendations
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ProfitRecommendations {
    pub strategies: Vec<ProfitStrategy>,
    pub risk_level: RiskLevel,
    pub suggested_leverage: u64,
    pub take_profit_levels: Vec<u64>,
    pub stop_loss_level: u64,
}

/// Profit tracker for UI
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct ProfitTracker {
    pub daily_profit: i64,
    pub weekly_profit: i64,
    pub monthly_profit: i64,
    pub all_time_profit: i64,
    pub current_positions_pnl: i64,
    pub fees_saved_today: u64,
    pub best_trade_today: u64,
    pub worst_trade_today: i64,
}

impl ProfitTracker {
    /// Update daily metrics
    pub fn update_daily(&mut self, profit: i64, fees_saved: u64) {
        self.daily_profit += profit;
        self.fees_saved_today += fees_saved;
        
        if profit > 0 && profit as u64 > self.best_trade_today {
            self.best_trade_today = profit as u64;
        } else if profit < self.worst_trade_today {
            self.worst_trade_today = profit;
        }
    }

    /// Reset daily metrics
    pub fn reset_daily(&mut self) {
        self.daily_profit = 0;
        self.fees_saved_today = 0;
        self.best_trade_today = 0;
        self.worst_trade_today = 0;
    }
}

/// Quick profit scanner
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct QuickProfitScanner {
    pub scan_interval: u64,
    pub min_profit_bps: u16,
    pub max_scan_markets: u32,
    pub priority_markets: Vec<Pubkey>,
}

impl QuickProfitScanner {
    /// Scan for quick profit opportunities
    pub fn scan_markets(
        &self,
        markets: &[MarketData],
        user_balance: u64,
    ) -> Vec<ProfitOpportunity> {
        let mut opportunities = Vec::new();
        let optimizer = MoneyMakingOptimizer::default();
        
        // Prioritize specific markets
        let mut sorted_markets = markets.to_vec();
        sorted_markets.sort_by(|a, b| {
            let a_priority = self.priority_markets.contains(&a.market_id);
            let b_priority = self.priority_markets.contains(&b.market_id);
            b_priority.cmp(&a_priority)
        });
        
        // Scan up to max markets
        for market in sorted_markets.iter().take(self.max_scan_markets as usize) {
            if let Ok(opp) = optimizer.analyze_opportunity(market, user_balance, 0) {
                if opp.should_trade && opp.expected_profit > 0 {
                    opportunities.push(opp);
                }
            }
        }
        
        // Sort by expected profit
        opportunities.sort_by(|a, b| b.expected_profit.cmp(&a.expected_profit));
        
        opportunities
    }
}

/// Auto-compound engine
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AutoCompoundEngine {
    pub enabled: bool,
    pub min_compound_amount: u64,
    pub compound_percentage: u64, // Basis points
    pub last_compound_slot: u64,
    pub total_compounded: u64,
}

impl AutoCompoundEngine {
    /// Process auto-compound
    pub fn process_compound(
        &mut self,
        profit: u64,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        if !self.enabled || profit < self.min_compound_amount {
            return Ok(0);
        }
        
        let compound_amount = (profit as u128 * self.compound_percentage as u128 / 10000) as u64;
        
        self.total_compounded += compound_amount;
        self.last_compound_slot = current_slot;
        
        msg!("Auto-compounded {} ({} bps of profit)", 
            compound_amount, 
            self.compound_percentage
        );
        
        Ok(compound_amount)
    }
}

/// Process money-making optimizer instructions
pub fn process_optimizer_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data[0] {
        0 => process_initialize_optimizer(program_id, accounts),
        1 => process_analyze_opportunity(program_id, accounts, &instruction_data[1..]),
        2 => process_execute_strategy(program_id, accounts, &instruction_data[1..]),
        3 => process_update_recommendations(program_id, accounts, &instruction_data[1..]),
        4 => process_enable_auto_compound(program_id, accounts, &instruction_data[1..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_initialize_optimizer(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let optimizer_account = next_account_info(account_iter)?;
    let admin_account = next_account_info(account_iter)?;

    if !admin_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let mut optimizer = MoneyMakingOptimizer::try_from_slice(&optimizer_account.data.borrow())?;
    optimizer.initialize()?;
    optimizer.serialize(&mut &mut optimizer_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_analyze_opportunity(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let optimizer_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;

    let market_data: MarketData = BorshDeserialize::try_from_slice(data)?;
    let user_balance = u64::from_le_bytes(data[100..108].try_into().unwrap());
    let current_positions = u32::from_le_bytes(data[108..112].try_into().unwrap());

    let optimizer = MoneyMakingOptimizer::try_from_slice(&optimizer_account.data.borrow())?;
    let opportunity = optimizer.analyze_opportunity(&market_data, user_balance, current_positions)?;

    msg!("Opportunity analysis: should_trade={}, expected_profit={}", 
        opportunity.should_trade, 
        opportunity.expected_profit
    );

    Ok(())
}

fn process_execute_strategy(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let optimizer_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;

    let opportunity: ProfitOpportunity = BorshDeserialize::try_from_slice(&data[..200])?;
    let execution_result: ExecutionResult = BorshDeserialize::try_from_slice(&data[200..])?;

    let mut optimizer = MoneyMakingOptimizer::try_from_slice(&optimizer_account.data.borrow())?;
    optimizer.execute_strategy(&opportunity, &execution_result)?;
    optimizer.serialize(&mut &mut optimizer_account.data.borrow_mut()[..])?;

    Ok(())
}

fn process_update_recommendations(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let optimizer_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;

    let user_stats: UserStats = BorshDeserialize::try_from_slice(data)?;

    let optimizer = MoneyMakingOptimizer::try_from_slice(&optimizer_account.data.borrow())?;
    let recommendations = optimizer.get_recommendations(&user_stats);

    msg!("Updated recommendations: risk_level={:?}, leverage={}", 
        recommendations.risk_level, 
        recommendations.suggested_leverage
    );

    Ok(())
}

fn process_enable_auto_compound(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let compound_account = next_account_info(account_iter)?;
    let user_account = next_account_info(account_iter)?;

    if !user_account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let enabled = data[0] != 0;
    let compound_percentage = u64::from_le_bytes(data[1..9].try_into().unwrap());

    let mut engine = AutoCompoundEngine::try_from_slice(&compound_account.data.borrow())?;
    engine.enabled = enabled;
    engine.compound_percentage = compound_percentage;
    engine.serialize(&mut &mut compound_account.data.borrow_mut()[..])?;

    msg!("Auto-compound {}: {}%", 
        if enabled { "enabled" } else { "disabled" },
        compound_percentage as f64 / 100.0
    );

    Ok(())
}

impl Default for MoneyMakingOptimizer {
    fn default() -> Self {
        Self {
            total_profit_generated: 0,
            total_fees_saved: 0,
            winning_positions: 0,
            losing_positions: 0,
            current_streak: 0,
            best_streak: 0,
            average_profit_per_trade: 0,
            compound_earnings: 0,
            active_strategies: Vec::new(),
            performance_metrics: PerformanceMetrics::default(),
            last_update_slot: 0,
        }
    }
}

use solana_program::account_info::next_account_info;