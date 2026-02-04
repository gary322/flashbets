//! AMM account structures
//!
//! Account types for various AMM implementations

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::account_validation::DISCRIMINATOR_SIZE;
use crate::BettingPlatformError;

/// Discriminators for AMM account types
pub mod discriminators {
    pub const LMSR_MARKET: [u8; 8] = [45, 189, 234, 167, 89, 23, 156, 201];
    pub const PMAMM_MARKET: [u8; 8] = [112, 78, 45, 209, 156, 34, 89, 167];
    pub const L2AMM_MARKET: [u8; 8] = [201, 156, 89, 34, 78, 112, 45, 209];
    pub const HYBRID_AMM: [u8; 8] = [167, 23, 189, 234, 45, 89, 156, 78];
}

/// LMSR (Logarithmic Market Scoring Rule) market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct LSMRMarket {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// B parameter (liquidity parameter)
    pub b_parameter: u64,
    
    /// Number of outcomes
    pub num_outcomes: u8,
    
    /// Current shares for each outcome
    pub shares: Vec<u64>,
    
    /// Total cost basis
    pub cost_basis: u64,
    
    /// Market state
    pub state: MarketState,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last update timestamp
    pub last_update: i64,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Fee percentage (basis points)
    pub fee_bps: u16,
    
    /// Oracle pubkey for resolution
    pub oracle: Pubkey,
}

impl LSMRMarket {
    pub fn new(market_id: u128, b_parameter: u64, num_outcomes: u8, oracle: Pubkey) -> Self {
        Self {
            discriminator: discriminators::LMSR_MARKET,
            market_id,
            b_parameter,
            num_outcomes,
            shares: vec![0; num_outcomes as usize],
            cost_basis: 0,
            state: MarketState::Active,
            created_at: 0,
            last_update: 0,
            total_volume: 0,
            fee_bps: 30, // 0.3%
            oracle,
        }
    }
    
    /// Calculate current price for an outcome
    pub fn calculate_price(&self, outcome: u8) -> Result<u64, ProgramError> {
        if outcome >= self.num_outcomes {
            return Err(ProgramError::InvalidArgument);
        }
        
        // LMSR price formula: p_i = e^(q_i/b) / Σ(e^(q_j/b))
        // Using fixed-point arithmetic
        let mut sum_exp = 0u128;
        
        for i in 0..self.num_outcomes as usize {
            // Simplified exponential approximation for fixed-point
            let exp = self.approximate_exp(self.shares[i], self.b_parameter)?;
            sum_exp = sum_exp.checked_add(exp as u128)
                .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        }
        
        let outcome_exp = self.approximate_exp(self.shares[outcome as usize], self.b_parameter)?;
        
        // Price = outcome_exp / sum_exp (scaled to basis points)
        let price = (outcome_exp as u128)
            .checked_mul(10000)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
            .checked_div(sum_exp)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        
        Ok(price as u64)
    }
    
    /// Simplified exponential approximation for fixed-point math
    fn approximate_exp(&self, shares: u64, b: u64) -> Result<u64, ProgramError> {
        // For small x, e^x ≈ 1 + x + x²/2
        // Here x = shares/b
        if b == 0 {
            return Err(ProgramError::InvalidArgument);
        }
        
        let x = (shares as u128 * 1000) / b as u128; // Scale by 1000 for precision
        let x_squared = x.checked_mul(x).ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())? / 2000;
        
        let result = 1000u128
            .checked_add(x)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?
            .checked_add(x_squared)
            .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        
        Ok((result * b as u128 / 1000) as u64)
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::LMSR_MARKET {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.num_outcomes < 2 || self.num_outcomes > 64 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.shares.len() != self.num_outcomes as usize {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// PM-AMM (Polynomial Market AMM) market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct PMAMMMarket {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// Pool ID (same as market_id for compatibility)
    pub pool_id: u128,
    
    /// L parameter (liquidity depth)
    pub l_parameter: u64,
    
    /// Expiry time
    pub expiry_time: i64,
    
    /// Number of outcomes
    pub num_outcomes: u8,
    
    /// Current reserves for each outcome
    pub reserves: Vec<u64>,
    
    /// Total liquidity
    pub total_liquidity: u64,
    
    /// Total LP token supply
    pub total_lp_supply: u64,
    
    /// Liquidity providers count
    pub liquidity_providers: u32,
    
    /// Market state
    pub state: MarketState,
    
    /// Initial price
    pub initial_price: u64,
    
    /// Current implied probabilities
    pub probabilities: Vec<u64>,
    
    /// Fee percentage (basis points)
    pub fee_bps: u16,
    
    /// Oracle pubkey
    pub oracle: Pubkey,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last update timestamp  
    pub last_update: i64,
    
    /// Use uniform LVR instead of scaled LVR
    pub use_uniform_lvr: bool,
}

impl PMAMMMarket {
    pub fn new(
        market_id: u128,
        l_parameter: u64,
        expiry_time: i64,
        num_outcomes: u8,
        initial_price: u64,
        oracle: Pubkey,
    ) -> Self {
        let initial_prob = 10000 / num_outcomes as u64; // Equal probability
        
        Self {
            discriminator: discriminators::PMAMM_MARKET,
            market_id,
            pool_id: market_id, // Same as market_id
            l_parameter,
            expiry_time,
            num_outcomes,
            reserves: vec![l_parameter / num_outcomes as u64; num_outcomes as usize],
            total_liquidity: l_parameter,
            total_lp_supply: l_parameter, // Initial LP supply equals initial liquidity
            liquidity_providers: 1, // Start with 1 (the initializer)
            state: MarketState::Active,
            initial_price,
            probabilities: vec![initial_prob; num_outcomes as usize],
            fee_bps: 30,
            oracle,
            total_volume: 0,
            created_at: 0, // Will be set by caller
            last_update: 0, // Will be set by caller
            use_uniform_lvr: true, // Default to uniform LVR per spec
        }
    }
    
    /// Calculate price for outcome using constant product formula
    pub fn calculate_price(&self, outcome: u8, is_buy: bool) -> Result<u64, ProgramError> {
        if outcome >= self.num_outcomes {
            return Err(ProgramError::InvalidArgument);
        }
        
        let reserve = self.reserves[outcome as usize];
        let other_reserves: u64 = self.reserves.iter()
            .enumerate()
            .filter(|(i, _)| *i != outcome as usize)
            .map(|(_, &r)| r)
            .sum();
        
        // Simplified PM-AMM pricing
        let price = if is_buy {
            // Price increases when buying
            (reserve as u128 * 10000) / self.total_liquidity as u128
        } else {
            // Price decreases when selling
            (other_reserves as u128 * 10000) / self.total_liquidity as u128
        };
        
        Ok((price as u64).min(10000).max(1))
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::PMAMM_MARKET {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.num_outcomes < 2 || self.num_outcomes > 64 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.reserves.len() != self.num_outcomes as usize {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// L2-norm AMM market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct L2AMMMarket {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// Pool ID (same as market_id for compatibility)
    pub pool_id: u128,
    
    /// K parameter (constant for L2 norm)
    pub k_parameter: u64,
    
    /// Liquidity parameter (same as k for compatibility)
    pub liquidity_parameter: u64,
    
    /// Bounded region parameter
    pub b_bound: u64,
    
    /// Distribution type
    pub distribution_type: DistributionType,
    
    /// Number of discretization points
    pub discretization_points: u16,
    
    /// Price range minimum
    pub range_min: u64,
    
    /// Min value (same as range_min for compatibility)
    pub min_value: u64,
    
    /// Price range maximum
    pub range_max: u64,
    
    /// Max value (same as range_max for compatibility)
    pub max_value: u64,
    
    /// Current position in each discretized bucket
    pub positions: Vec<u64>,
    
    /// Distribution bins
    pub distribution: Vec<DistributionBin>,
    
    /// Total liquidity
    pub total_liquidity: u64,
    
    /// Total shares
    pub total_shares: u64,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Market state
    pub state: MarketState,
    
    /// Fee percentage
    pub fee_bps: u16,
    
    /// Oracle
    pub oracle: Pubkey,
    
    /// Last update timestamp
    pub last_update: i64,
}

impl L2AMMMarket {
    pub fn new(
        market_id: u128,
        k_parameter: u64,
        b_bound: u64,
        distribution_type: DistributionType,
        discretization_points: u16,
        range_min: u64,
        range_max: u64,
        oracle: Pubkey,
    ) -> Self {
        Self {
            discriminator: discriminators::L2AMM_MARKET,
            market_id,
            pool_id: market_id,
            k_parameter,
            liquidity_parameter: k_parameter,
            b_bound,
            distribution_type,
            discretization_points,
            range_min,
            min_value: range_min,
            range_max,
            max_value: range_max,
            positions: vec![0; discretization_points as usize],
            distribution: {
                let mut bins = Vec::new();
                let bin_width = (range_max - range_min) / discretization_points as u64;
                for i in 0..discretization_points {
                    bins.push(DistributionBin {
                        lower_bound: range_min + (i as u64 * bin_width),
                        upper_bound: range_min + ((i + 1) as u64 * bin_width),
                        weight: 0,
                    });
                }
                bins
            },
            total_liquidity: 0,
            total_shares: 0,
            total_volume: 0,
            state: MarketState::Active,
            fee_bps: 30,
            oracle,
            last_update: 0,
        }
    }
    
    /// Calculate L2 norm constraint
    pub fn calculate_l2_norm(&self) -> Result<u64, ProgramError> {
        let mut sum_squares = 0u128;
        
        for &position in &self.positions {
            let squared = (position as u128)
                .checked_mul(position as u128)
                .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
            sum_squares = sum_squares
                .checked_add(squared)
                .ok_or::<ProgramError>(BettingPlatformError::ArithmeticOverflow.into())?;
        }
        
        // Integer square root approximation
        let mut x = sum_squares;
        let mut y = (x + 1) / 2;
        
        while y < x {
            x = y;
            y = (x + sum_squares / x) / 2;
        }
        
        Ok(x as u64)
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::L2AMM_MARKET {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.discretization_points == 0 || self.discretization_points > 1000 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.range_max <= self.range_min {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Distribution type for L2 AMM
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum DistributionType {
    Normal,
    LogNormal,
    Custom,
}

/// Hybrid AMM market
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct HybridAMM {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// AMM type being used
    pub amm_type: AMMType,
    
    /// Number of outcomes
    pub num_outcomes: u8,
    
    /// Expiry time
    pub expiry_time: i64,
    
    /// Is continuous market
    pub is_continuous: bool,
    
    /// AMM-specific data (serialized)
    pub amm_data: Vec<u8>,
    
    /// Total volume
    pub total_volume: u64,
    
    /// State
    pub state: MarketState,
    
    /// Fee
    pub fee_bps: u16,
    
    /// Oracle
    pub oracle: Pubkey,
}

impl HybridAMM {
    pub fn new(
        market_id: u128,
        amm_type: AMMType,
        num_outcomes: u8,
        expiry_time: i64,
        is_continuous: bool,
        oracle: Pubkey,
    ) -> Self {
        Self {
            discriminator: discriminators::HYBRID_AMM,
            market_id,
            amm_type,
            num_outcomes,
            expiry_time,
            is_continuous,
            amm_data: vec![],
            total_volume: 0,
            state: MarketState::Active,
            fee_bps: 30,
            oracle,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::HYBRID_AMM {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Market state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MarketState {
    Active,
    Paused,
    Resolved,
    Disputed,
}

/// L2AMM distribution type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum L2Distribution {
    Normal,
    LogNormal,
    Beta,
    Uniform,
}

// Type aliases for compatibility
pub type L2AMMPool = L2AMMMarket;
pub type PoolState = MarketState;
pub type PMAMMPool = PMAMMMarket;

/// Distribution bin for L2-AMM
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct DistributionBin {
    pub lower_bound: u64,
    pub upper_bound: u64,
    pub weight: u64,
}

/// L2 position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct L2Position {
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    pub position_id: u128,
    pub trader: Pubkey,
    pub pool_id: u128,
    pub lower_bound: u64,
    pub upper_bound: u64,
    pub shares: u64,
    pub entry_cost: u64,
    pub last_update: i64,
    pub realized_pnl: i64,
    pub fees_paid: u64,
}

impl L2Position {
    pub fn generate_id(trader: &Pubkey, pool_id: u128, lower: u64, upper: u64) -> u128 {
        use solana_program::keccak;
        let mut data = vec![];
        data.extend_from_slice(trader.as_ref());
        data.extend_from_slice(&pool_id.to_le_bytes());
        data.extend_from_slice(&lower.to_le_bytes());
        data.extend_from_slice(&upper.to_le_bytes());
        let hash = keccak::hash(&data);
        u128::from_le_bytes(hash.0[..16].try_into().unwrap())
    }
}

/// LP position for liquidity providers
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct LPPosition {
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    pub provider: Pubkey,
    pub pool_id: u128,
    pub lp_tokens: u64,
    pub initial_investment: u64,
    pub withdrawn_amount: u64,
    pub last_update: i64,
}

/// Hybrid market with switchable AMM
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct HybridMarket {
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    pub market_id: u128,
    pub current_amm: AMMType,
    pub target_amm: AMMType,
    pub num_outcomes: u8,
    pub total_volume: u64,
    pub total_liquidity: u64,
    pub state: MarketState,
    pub last_conversion: i64,
    pub conversion_count: u32,
}

// Import and re-export AMMType from accounts module
pub use super::accounts::AMMType;