//! Vault State Management
//!
//! Core vault state structures with zero-loss guarantee

use solana_program::{
    pubkey::Pubkey,
    clock::UnixTimestamp,
    program_error::ProgramError,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
};

/// Main vault state
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct Vault {
    /// Vault ID
    pub vault_id: u128,
    
    /// Vault name
    pub name: [u8; 32],
    
    /// Vault type
    pub vault_type: VaultType,
    
    /// Total value locked (TVL)
    pub total_value_locked: u128,
    
    /// Total shares issued
    pub total_shares: u128,
    
    /// Share price (18 decimals)
    pub share_price: u128,
    
    /// Deposit token
    pub deposit_token: Pubkey,
    
    /// Synthetic token mint
    pub synthetic_mint: Pubkey,
    
    /// Oracle account
    pub oracle_account: Pubkey,
    
    /// Strategy configuration
    pub strategy: VaultStrategy,
    
    /// Performance metrics
    pub performance: PerformanceMetrics,
    
    /// Risk parameters
    pub risk_params: RiskParameters,
    
    /// Insurance fund
    pub insurance_fund: u128,
    
    /// Zero-loss guarantee enabled
    pub zero_loss_enabled: bool,
    
    /// Minimum deposit amount
    pub min_deposit: u128,
    
    /// Maximum deposit amount
    pub max_deposit: u128,
    
    /// Deposit fee (basis points)
    pub deposit_fee: u16,
    
    /// Withdrawal fee (basis points)
    pub withdrawal_fee: u16,
    
    /// Management fee (annual, basis points)
    pub management_fee: u16,
    
    /// Performance fee (basis points)
    pub performance_fee: u16,
    
    /// High water mark
    pub high_water_mark: u128,
    
    /// Status
    pub status: VaultStatus,
    
    /// Admin authority
    pub admin: Pubkey,
    
    /// Created timestamp
    pub created_at: UnixTimestamp,
    
    /// Last update
    pub last_update: UnixTimestamp,
    
    /// Epoch number
    pub epoch: u64,
    
    /// Next rebalance time
    pub next_rebalance: UnixTimestamp,
    
    /// Utilization rate
    pub utilization_rate: f64,
    
    /// Available liquidity
    pub available_liquidity: u128,
    
    /// Borrowed amount
    pub borrowed_amount: u128,
    
    /// Reserve ratio
    pub reserve_ratio: f64,
    
    /// Emergency shutdown
    pub emergency_shutdown: bool,
}

/// Vault types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum VaultType {
    /// Standard yield vault
    Standard,
    /// Leveraged yield vault
    Leveraged,
    /// Market making vault
    MarketMaking,
    /// Arbitrage vault
    Arbitrage,
    /// Liquidity provision vault
    LiquidityProvision,
    /// CDP collateral vault
    CDPCollateral,
    /// Insurance fund vault
    Insurance,
    /// Treasury vault
    Treasury,
}

/// Vault strategy configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct VaultStrategy {
    /// Strategy type
    pub strategy_type: StrategyType,
    
    /// Target APY
    pub target_apy: f64,
    
    /// Maximum leverage allowed
    pub max_leverage: u16,
    
    /// Rebalance frequency (seconds)
    pub rebalance_frequency: u64,
    
    /// Risk tolerance (0-100)
    pub risk_tolerance: u8,
    
    /// Allowed protocols for yield
    pub allowed_protocols: Vec<Protocol>,
    
    /// Diversification requirements
    pub diversification: DiversificationParams,
    
    /// Stop loss threshold
    pub stop_loss: f64,
    
    /// Take profit threshold
    pub take_profit: f64,
}

/// Strategy types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum StrategyType {
    /// Conservative - low risk, stable returns
    Conservative,
    /// Balanced - moderate risk/return
    Balanced,
    /// Aggressive - high risk, high return
    Aggressive,
    /// Custom strategy
    Custom(CustomStrategy),
}

/// Custom strategy parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CustomStrategy {
    pub id: u128,
    pub name: [u8; 32],
    pub parameters: [u64; 8],
}

/// Allowed protocols for yield generation
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum Protocol {
    /// Native CDP system
    NativeCDP,
    /// Perpetual trading
    Perpetuals,
    /// Lending/borrowing
    Lending,
    /// Liquidity provision
    LiquidityMining,
    /// Staking
    Staking,
    /// Options strategies
    Options,
}

/// Diversification parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DiversificationParams {
    /// Maximum allocation per protocol (%)
    pub max_per_protocol: u8,
    
    /// Minimum number of protocols
    pub min_protocols: u8,
    
    /// Maximum correlation allowed
    pub max_correlation: f64,
    
    /// Rebalance threshold (%)
    pub rebalance_threshold: u8,
}

/// Performance metrics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PerformanceMetrics {
    /// Current APY
    pub current_apy: f64,
    
    /// 7-day average APY
    pub avg_7d_apy: f64,
    
    /// 30-day average APY
    pub avg_30d_apy: f64,
    
    /// All-time return
    pub total_return: f64,
    
    /// Sharpe ratio
    pub sharpe_ratio: f64,
    
    /// Maximum drawdown
    pub max_drawdown: f64,
    
    /// Win rate
    pub win_rate: f64,
    
    /// Total fees earned
    pub total_fees_earned: u128,
    
    /// Total yield generated
    pub total_yield_generated: u128,
}

/// Risk parameters
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RiskParameters {
    /// Maximum leverage
    pub max_leverage: u16,
    
    /// Liquidation threshold
    pub liquidation_threshold: f64,
    
    /// Maximum exposure per position
    pub max_position_size: u128,
    
    /// Value at risk (VaR)
    pub value_at_risk: f64,
    
    /// Stress test score
    pub stress_test_score: u8,
    
    /// Risk score (0-100)
    pub risk_score: u8,
}

/// Vault status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum VaultStatus {
    /// Active and accepting deposits
    Active,
    /// Paused - no new deposits
    Paused,
    /// Withdrawals only
    WithdrawOnly,
    /// Emergency shutdown
    Shutdown,
    /// Rebalancing
    Rebalancing,
}

/// User deposit record
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct UserDeposit {
    /// User pubkey
    pub user: Pubkey,
    
    /// Vault ID
    pub vault_id: u128,
    
    /// Deposited amount
    pub deposited_amount: u128,
    
    /// Shares owned
    pub shares: u128,
    
    /// Average entry price
    pub avg_entry_price: u128,
    
    /// Deposit timestamp
    pub deposit_time: UnixTimestamp,
    
    /// Last claim time
    pub last_claim: UnixTimestamp,
    
    /// Unclaimed rewards
    pub unclaimed_rewards: u128,
    
    /// Lock period end (if any)
    pub lock_until: Option<UnixTimestamp>,
    
    /// Performance
    pub performance: UserPerformance,
    
    /// Zero-loss protection active
    pub zero_loss_protected: bool,
    
    /// Protection floor price
    pub protection_floor: u128,
}

/// User performance tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct UserPerformance {
    /// Realized profit/loss
    pub realized_pnl: i128,
    
    /// Unrealized profit/loss
    pub unrealized_pnl: i128,
    
    /// Total withdrawn
    pub total_withdrawn: u128,
    
    /// Total rewards claimed
    pub total_rewards_claimed: u128,
    
    /// Current value
    pub current_value: u128,
}

/// Vault epoch for tracking periods
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct VaultEpoch {
    /// Epoch number
    pub epoch_number: u64,
    
    /// Start timestamp
    pub start_time: UnixTimestamp,
    
    /// End timestamp
    pub end_time: UnixTimestamp,
    
    /// Starting TVL
    pub starting_tvl: u128,
    
    /// Ending TVL
    pub ending_tvl: u128,
    
    /// Yield generated
    pub yield_generated: u128,
    
    /// Fees collected
    pub fees_collected: u128,
    
    /// Performance fee
    pub performance_fee: u128,
    
    /// Share price at start
    pub start_share_price: u128,
    
    /// Share price at end
    pub end_share_price: u128,
    
    /// Deposits in epoch
    pub total_deposits: u128,
    
    /// Withdrawals in epoch
    pub total_withdrawals: u128,
}

impl Vault {
    /// Create new vault
    pub fn new(
        vault_id: u128,
        name: [u8; 32],
        vault_type: VaultType,
        deposit_token: Pubkey,
        synthetic_mint: Pubkey,
        oracle_account: Pubkey,
        admin: Pubkey,
    ) -> Self {
        Self {
            vault_id,
            name,
            vault_type,
            total_value_locked: 0,
            total_shares: 0,
            share_price: 1_000_000_000_000_000_000, // 1e18 (1:1 initial)
            deposit_token,
            synthetic_mint,
            oracle_account,
            strategy: VaultStrategy::default(),
            performance: PerformanceMetrics::default(),
            risk_params: RiskParameters::default(),
            insurance_fund: 0,
            zero_loss_enabled: true,
            min_deposit: 100_000_000, // 100 USDC
            max_deposit: 100_000_000_000_000, // 100M USDC
            deposit_fee: 10, // 0.1%
            withdrawal_fee: 50, // 0.5%
            management_fee: 200, // 2% annual
            performance_fee: 2000, // 20%
            high_water_mark: 1_000_000_000_000_000_000,
            status: VaultStatus::Active,
            admin,
            created_at: 0,
            last_update: 0,
            epoch: 0,
            next_rebalance: 0,
            utilization_rate: 0.0,
            available_liquidity: 0,
            borrowed_amount: 0,
            reserve_ratio: 0.1, // 10% reserve
            emergency_shutdown: false,
        }
    }
    
    /// Calculate current share price
    pub fn calculate_share_price(&self) -> u128 {
        if self.total_shares == 0 {
            return 1_000_000_000_000_000_000; // 1e18
        }
        
        // Share price = TVL / Total Shares
        (self.total_value_locked * 1_000_000_000_000_000_000) / self.total_shares
    }
    
    /// Update TVL
    pub fn update_tvl(&mut self, new_tvl: u128) {
        self.total_value_locked = new_tvl;
        self.share_price = self.calculate_share_price();
        
        // Update high water mark if new high
        if self.share_price > self.high_water_mark {
            self.high_water_mark = self.share_price;
        }
    }
    
    /// Calculate deposit shares
    pub fn calculate_deposit_shares(&self, amount: u128) -> u128 {
        if self.total_shares == 0 || self.total_value_locked == 0 {
            // First deposit, 1:1 ratio
            return amount;
        }
        
        // Shares = amount * total_shares / TVL
        (amount * self.total_shares) / self.total_value_locked
    }
    
    /// Calculate withdrawal amount
    pub fn calculate_withdrawal_amount(&self, shares: u128) -> u128 {
        if self.total_shares == 0 {
            return 0;
        }
        
        // Amount = shares * TVL / total_shares
        (shares * self.total_value_locked) / self.total_shares
    }
    
    /// Apply deposit fee
    pub fn apply_deposit_fee(&self, amount: u128) -> (u128, u128) {
        let fee = (amount * self.deposit_fee as u128) / 10000;
        let net_amount = amount - fee;
        (net_amount, fee)
    }
    
    /// Apply withdrawal fee
    pub fn apply_withdrawal_fee(&self, amount: u128) -> (u128, u128) {
        let fee = (amount * self.withdrawal_fee as u128) / 10000;
        let net_amount = amount - fee;
        (net_amount, fee)
    }
    
    /// Check if vault is accepting deposits
    pub fn is_accepting_deposits(&self) -> bool {
        self.status == VaultStatus::Active && !self.emergency_shutdown
    }
    
    /// Check if vault is allowing withdrawals
    pub fn is_allowing_withdrawals(&self) -> bool {
        self.status != VaultStatus::Shutdown && !self.emergency_shutdown
    }
    
    /// Update utilization rate
    pub fn update_utilization(&mut self) {
        if self.total_value_locked == 0 {
            self.utilization_rate = 0.0;
        } else {
            self.utilization_rate = self.borrowed_amount as f64 / self.total_value_locked as f64;
        }
        
        self.available_liquidity = self.total_value_locked.saturating_sub(self.borrowed_amount);
    }
}

impl Default for VaultStrategy {
    fn default() -> Self {
        Self {
            strategy_type: StrategyType::Balanced,
            target_apy: 0.15, // 15%
            max_leverage: 3,
            rebalance_frequency: 86400, // Daily
            risk_tolerance: 50,
            allowed_protocols: vec![
                Protocol::NativeCDP,
                Protocol::Perpetuals,
                Protocol::Lending,
            ],
            diversification: DiversificationParams::default(),
            stop_loss: 0.1, // 10%
            take_profit: 0.5, // 50%
        }
    }
}

impl Default for DiversificationParams {
    fn default() -> Self {
        Self {
            max_per_protocol: 40, // 40% max per protocol
            min_protocols: 2,
            max_correlation: 0.7,
            rebalance_threshold: 5, // 5% deviation triggers rebalance
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            current_apy: 0.0,
            avg_7d_apy: 0.0,
            avg_30d_apy: 0.0,
            total_return: 0.0,
            sharpe_ratio: 0.0,
            max_drawdown: 0.0,
            win_rate: 0.0,
            total_fees_earned: 0,
            total_yield_generated: 0,
        }
    }
}

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            max_leverage: 5,
            liquidation_threshold: 0.8,
            max_position_size: 1_000_000_000_000, // 1M USDC
            value_at_risk: 0.05, // 5% VaR
            stress_test_score: 80,
            risk_score: 30,
        }
    }
}

/// Derive vault PDA
pub fn derive_vault_pda(
    program_id: &Pubkey,
    vault_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"vault",
            &vault_id.to_le_bytes(),
        ],
        program_id,
    )
}

/// Derive user deposit PDA
pub fn derive_user_deposit_pda(
    program_id: &Pubkey,
    user: &Pubkey,
    vault_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"user_deposit",
            user.as_ref(),
            &vault_id.to_le_bytes(),
        ],
        program_id,
    )
}