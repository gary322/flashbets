//! L2 Distribution State Structure
//! 
//! Represents the state of an L2 distribution for continuous markets

use borsh::{BorshDeserialize, BorshSerialize};
use crate::account_validation::DISCRIMINATOR_SIZE;
use crate::state::accounts::discriminators;

/// L2 Distribution state for continuous outcome markets
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct L2DistributionState {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Distribution type (Normal, LogNormal, Beta, Uniform)
    pub distribution_type: u8,
    
    /// Mean of the distribution
    pub mean: u32,
    
    /// Standard deviation
    pub std_dev: u32,
    
    /// Skewness parameter
    pub skew: i32,
    
    /// Kurtosis parameter
    pub kurtosis: i32,
    
    /// Price array for discrete buckets
    pub prices: Vec<u32>,
    
    /// Total liquidity in the distribution
    pub liquidity: u64,
    
    /// K constant for L2 norm
    pub k_constant: u64,
    
    /// Last update slot
    pub last_update_slot: u64,
}

impl L2DistributionState {
    pub const MAX_BUCKETS: usize = 100;
    
    pub fn new(
        distribution_type: u8,
        num_buckets: usize,
        liquidity: u64,
        k_constant: u64,
    ) -> Result<Self, solana_program::program_error::ProgramError> {
        if num_buckets == 0 || num_buckets > Self::MAX_BUCKETS {
            return Err(solana_program::program_error::ProgramError::InvalidArgument);
        }
        
        Ok(Self {
            discriminator: discriminators::L2_DISTRIBUTION,
            distribution_type,
            mean: 5000, // Default to center
            std_dev: 1000, // Default standard deviation
            skew: 0,
            kurtosis: 0,
            prices: vec![0; num_buckets],
            liquidity,
            k_constant,
            last_update_slot: 0,
        })
    }
    
    pub fn validate(&self) -> Result<(), solana_program::program_error::ProgramError> {
        if self.prices.len() > Self::MAX_BUCKETS {
            return Err(solana_program::program_error::ProgramError::InvalidAccountData);
        }
        
        if self.std_dev == 0 {
            return Err(solana_program::program_error::ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}