//! Oracle PDA State Management
//!
//! Program Derived Addresses for oracle data storage

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

use crate::error::BettingPlatformError;
use super::sigma::SigmaCalculator;
use super::validation::PriceHistory;

/// Seed for oracle PDA derivation
pub const ORACLE_PDA_SEED: &[u8] = b"oracle";

/// Seed for oracle history PDA
pub const ORACLE_HISTORY_SEED: &[u8] = b"oracle_history";

/// Main Oracle PDA for storing current state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OraclePDA {
    /// Is initialized
    pub is_initialized: bool,
    /// Market ID this oracle tracks
    pub market_id: u128,
    /// Current probability value
    pub current_prob: f64,
    /// Current sigma (volatility)
    pub current_sigma: f64,
    /// Time-weighted average probability
    pub twap_prob: f64,
    /// Exponentially weighted moving average of probability
    pub ewma_prob: f64,
    /// Last update slot
    pub last_update_slot: u64,
    /// Senior flag for vault protection
    pub senior_flag: bool,
    /// Buffer requirement for over-collateralization
    pub buffer_req: f64,
    /// Number of oracle sources in last update
    pub num_sources: u8,
    /// Confidence level of last update
    pub confidence: f64,
    /// Is market halted
    pub is_halted: bool,
    /// Last validated scalar
    pub last_scalar: f64,
    /// Probability at last scalar calculation
    pub scalar_prob: f64,
    /// Sigma at last scalar calculation
    pub scalar_sigma: f64,
}

impl OraclePDA {
    pub const LEN: usize = 1 + 16 + 8 * 10 + 1 + 8 + 1 + 8 + 1 + 8 * 3;

    pub fn new(market_id: u128) -> Self {
        Self {
            is_initialized: true,
            market_id,
            current_prob: 0.5,
            current_sigma: 0.01,
            twap_prob: 0.5,
            ewma_prob: 0.5,
            last_update_slot: 0,
            senior_flag: false,
            buffer_req: 0.0,
            num_sources: 0,
            confidence: 0.0,
            is_halted: false,
            last_scalar: 1.0,
            scalar_prob: 0.5,
            scalar_sigma: 0.01,
        }
    }

    /// Update oracle data with new values
    pub fn update(
        &mut self,
        prob: f64,
        sigma: f64,
        twap: f64,
        ewma: f64,
        slot: u64,
        num_sources: u8,
        confidence: f64,
    ) -> Result<(), ProgramError> {
        // Validate inputs
        if prob < 0.0 || prob > 1.0 {
            msg!("Invalid probability: {}", prob);
            return Err(BettingPlatformError::InvalidProbability.into());
        }

        if sigma < 0.0 || sigma > 1.0 {
            msg!("Invalid sigma: {}", sigma);
            return Err(BettingPlatformError::InvalidSigma.into());
        }

        self.current_prob = prob;
        self.current_sigma = sigma;
        self.twap_prob = twap;
        self.ewma_prob = ewma;
        self.last_update_slot = slot;
        self.num_sources = num_sources;
        self.confidence = confidence;

        // Update buffer requirement
        self.buffer_req = 1.0 + sigma * 1.5;

        Ok(())
    }

    /// Calculate and cache scalar value
    pub fn calculate_scalar(&mut self) -> f64 {
        // Clamp probability to prevent extremes
        let prob = self.current_prob.max(0.01).min(0.99);
        let sigma = self.current_sigma.max(0.01);

        // Constants from model
        const CAP_FUSED: f64 = 20.0;
        const CAP_VAULT: f64 = 30.0;
        const BASE_RISK: f64 = 0.25;

        // Calculate risk
        let risk = prob * (1.0 - prob);

        // Unified scalar (simplified - prob terms cancel)
        let unified_scalar = (1.0 / sigma) * CAP_FUSED;

        // Premium factor
        let premium_factor = (risk / BASE_RISK) * CAP_VAULT;

        // Total scalar
        let total_scalar = unified_scalar * premium_factor;

        // Cache values
        self.last_scalar = total_scalar.min(1000.0); // Cap at 1000x
        self.scalar_prob = prob;
        self.scalar_sigma = sigma;

        self.last_scalar
    }

    /// Check if scalar needs recalculation
    pub fn needs_scalar_update(&self, prob: f64, sigma: f64) -> bool {
        let prob_change = (prob - self.scalar_prob).abs();
        let sigma_change = (sigma - self.scalar_sigma).abs();

        // Recalculate if changes exceed thresholds
        prob_change > 0.01 || sigma_change > 0.01
    }

    /// Set senior flag for vault protection
    pub fn set_senior_protection(&mut self, protected: bool) {
        self.senior_flag = protected;
    }

    /// Check if oracle should trigger halt
    pub fn should_halt(&self, max_sigma: f64) -> bool {
        self.is_halted || self.current_sigma > max_sigma
    }

    /// Get clamped probability for calculations
    pub fn get_clamped_prob(&self) -> f64 {
        self.current_prob.max(0.01).min(0.99)
    }
}

impl Sealed for OraclePDA {}

impl IsInitialized for OraclePDA {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for OraclePDA {
    const LEN: usize = Self::LEN;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }
}

/// Oracle History PDA for storing compressed historical data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OracleHistoryPDA {
    /// Is initialized
    pub is_initialized: bool,
    /// Market ID this history tracks
    pub market_id: u128,
    /// Price history for TWAP
    pub price_history: PriceHistory,
    /// Sigma calculator with compressed history
    pub sigma_calculator: SigmaCalculator,
    /// Last processed slot (to avoid duplicates)
    pub last_processed_slot: u64,
    /// Total samples collected
    pub total_samples: u64,
}

impl OracleHistoryPDA {
    pub const LEN: usize = 1024; // Adjust based on actual needs

    pub fn new(market_id: u128) -> Self {
        Self {
            is_initialized: true,
            market_id,
            price_history: PriceHistory::new(),
            sigma_calculator: SigmaCalculator::new(),
            last_processed_slot: 0,
            total_samples: 0,
        }
    }

    /// Add new price sample
    pub fn add_sample(&mut self, prob: f64, slot: u64) -> Result<(), ProgramError> {
        // Avoid duplicate processing
        if slot <= self.last_processed_slot {
            return Ok(());
        }

        // Update price history
        self.price_history.add_price(prob, slot);

        // Update sigma calculator
        self.sigma_calculator.add_sample(prob)?;

        // Update metadata
        self.last_processed_slot = slot;
        self.total_samples += 1;

        Ok(())
    }

    /// Get current statistics
    pub fn get_statistics(&self) -> (f64, f64, f64) {
        let twap = self.price_history.calculate_twap();
        let ewma = self.price_history.calculate_ewma();
        let sigma = self.sigma_calculator.get_sigma();

        (twap, ewma, sigma)
    }
}

/// Derive Oracle PDA address
pub fn derive_oracle_pda(
    program_id: &Pubkey,
    market_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            ORACLE_PDA_SEED,
            &market_id.to_le_bytes(),
        ],
        program_id,
    )
}

/// Derive Oracle History PDA address
pub fn derive_oracle_history_pda(
    program_id: &Pubkey,
    market_id: u128,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            ORACLE_HISTORY_SEED,
            &market_id.to_le_bytes(),
        ],
        program_id,
    )
}

/// Initialize Oracle PDA
pub fn initialize_oracle_pda(
    oracle_account: &AccountInfo,
    market_id: u128,
) -> Result<(), ProgramError> {
    let mut oracle = OraclePDA::unpack_unchecked(&oracle_account.data.borrow())?;
    
    if oracle.is_initialized() {
        return Err(BettingPlatformError::AlreadyInitialized.into());
    }

    oracle = OraclePDA::new(market_id);
    OraclePDA::pack(oracle, &mut oracle_account.data.borrow_mut())?;

    Ok(())
}

/// Initialize Oracle History PDA
pub fn initialize_oracle_history_pda(
    history_account: &AccountInfo,
    market_id: u128,
) -> Result<(), ProgramError> {
    let mut history_data = history_account.data.borrow_mut();
    
    // Check if already initialized
    if history_data[0] != 0 {
        return Err(BettingPlatformError::AlreadyInitialized.into());
    }

    let history = OracleHistoryPDA::new(market_id);
    let serialized = history.try_to_vec()?;
    
    if serialized.len() > history_data.len() {
        return Err(ProgramError::AccountDataTooSmall);
    }

    history_data[..serialized.len()].copy_from_slice(&serialized);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oracle_pda_initialization() {
        let oracle = OraclePDA::new(12345);
        assert!(oracle.is_initialized);
        assert_eq!(oracle.market_id, 12345);
        assert_eq!(oracle.current_prob, 0.5);
    }

    #[test]
    fn test_scalar_calculation() {
        let mut oracle = OraclePDA::new(1);
        oracle.current_prob = 0.5;
        oracle.current_sigma = 0.2;
        
        let scalar = oracle.calculate_scalar();
        // With prob=0.5, sigma=0.2: scalar should be significant
        assert!(scalar > 100.0);
        assert!(scalar <= 1000.0); // Capped at 1000x
    }

    #[test]
    fn test_buffer_requirement() {
        let mut oracle = OraclePDA::new(1);
        oracle.update(0.5, 0.4, 0.5, 0.5, 100, 3, 0.95).unwrap();
        
        // buffer = 1 + 0.4 * 1.5 = 1.6
        assert!((oracle.buffer_req - 1.6).abs() < 0.001);
    }

    #[test]
    fn test_probability_clamping() {
        let mut oracle = OraclePDA::new(1);
        
        oracle.current_prob = 0.001;
        assert_eq!(oracle.get_clamped_prob(), 0.01);
        
        oracle.current_prob = 0.999;
        assert_eq!(oracle.get_clamped_prob(), 0.99);
    }
}