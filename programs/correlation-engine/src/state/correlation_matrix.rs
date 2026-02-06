use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    program_error::ProgramError,
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CorrelationEntry {
    pub market_i: u16,
    pub market_j: u16,
    pub correlation: i64,  // Fixed point in [0, 2*ONE] where ONE represents 0 correlation
    pub last_updated: i64,
    pub sample_size: u32,
}

/// Correlation matrix for a verse
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CorrelationMatrix {
    pub is_initialized: bool,
    pub verse_id: [u8; 16],
    pub correlations: Vec<CorrelationEntry>,  // Upper triangular matrix
    pub average_correlation: u64,             // Average correlation factor
    pub last_calculated: i64,
    pub calculation_version: u8,
    pub market_count: u16,
    pub bump: u8,
}

impl CorrelationMatrix {
    pub const BASE_LEN: usize = 1 + 16 + 4 + 8 + 8 + 1 + 2 + 1;
    
    pub fn new(verse_id: [u8; 16], bump: u8) -> Self {
        Self {
            is_initialized: true,
            verse_id,
            correlations: Vec::new(),
            average_correlation: 0,
            last_calculated: 0,
            calculation_version: 0,
            market_count: 0,
            bump,
        }
    }
    
    /// Calculate the maximum number of correlations for n markets
    pub fn max_correlations(n: u16) -> usize {
        if n <= 1 {
            0
        } else {
            ((n as usize) * (n as usize - 1)) / 2
        }
    }
    
    /// Add or update a correlation entry
    pub fn update_correlation(
        &mut self,
        market_i: u16,
        market_j: u16,
        correlation: i64,
        timestamp: i64,
        sample_size: u32,
    ) -> Result<(), ProgramError> {
        // Ensure i < j for consistency
        let (i, j) = if market_i < market_j {
            (market_i, market_j)
        } else {
            (market_j, market_i)
        };
        
        // Find existing entry or create new one
        if let Some(entry) = self.correlations.iter_mut()
            .find(|e| e.market_i == i && e.market_j == j) {
            entry.correlation = correlation;
            entry.last_updated = timestamp;
            entry.sample_size = sample_size;
        } else {
            self.correlations.push(CorrelationEntry {
                market_i: i,
                market_j: j,
                correlation,
                last_updated: timestamp,
                sample_size,
            });
        }

        // Track the highest referenced market index (assuming 0..N-1 indexing).
        self.market_count = self.market_count.max(j.saturating_add(1));
        
        Ok(())
    }
    
    /// Get correlation between two markets
    pub fn get_correlation(&self, market_i: u16, market_j: u16) -> Option<i64> {
        let (i, j) = if market_i < market_j {
            (market_i, market_j)
        } else {
            (market_j, market_i)
        };
        
        self.correlations.iter()
            .find(|e| e.market_i == i && e.market_j == j)
            .map(|e| e.correlation)
    }
    
    /// Calculate average absolute correlation
    pub fn calculate_average_correlation(&mut self) -> Result<(), ProgramError> {
        if self.correlations.is_empty() {
            self.average_correlation = 0;
            return Ok(());
        }

        // Convert mapped correlation [0, 2*ONE] into absolute correlation factor [0, ONE]
        // where -1 => ONE, 0 => 0, +1 => ONE.
        const ONE: u64 = 1_000_000;
        let sum: u128 = self
            .correlations
            .iter()
            .map(|e| {
                let rep = u64::try_from(e.correlation).unwrap_or(0);
                let abs_corr = if rep > ONE { rep - ONE } else { ONE - rep };
                abs_corr as u128
            })
            .sum();

        self.average_correlation = (sum / self.correlations.len() as u128) as u64;
        Ok(())
    }
    
    /// Clean up old correlations based on timestamp
    pub fn cleanup_old_correlations(&mut self, cutoff_timestamp: i64) {
        self.correlations.retain(|e| e.last_updated >= cutoff_timestamp);
    }
    
    /// Calculate the size needed for this account
    pub fn calculate_size(max_markets: u16) -> usize {
        let max_correlations = Self::max_correlations(max_markets);
        Self::BASE_LEN 
            + (max_correlations * std::mem::size_of::<CorrelationEntry>())
            + 100 // Buffer for vec overhead
    }
}

/// Statistics about correlations
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CorrelationStats {
    pub min_correlation: i64,
    pub max_correlation: i64,
    pub median_correlation: i64,
    pub std_deviation: u64,
    pub high_correlation_pairs: u32,  // Count of pairs with |corr| > 0.7
}

impl CorrelationStats {
    pub fn calculate(matrix: &CorrelationMatrix) -> Result<Self, ProgramError> {
        if matrix.correlations.is_empty() {
            return Ok(Self {
                min_correlation: 0,
                max_correlation: 0,
                median_correlation: 0,
                std_deviation: 0,
                high_correlation_pairs: 0,
            });
        }
        
        let mut correlations: Vec<i64> = matrix.correlations.iter()
            .map(|c| c.correlation)
            .collect();
        
        correlations.sort_unstable();
        
        let min = correlations[0];
        let max = correlations[correlations.len() - 1];
        let median = if correlations.len() % 2 == 0 {
            (correlations[correlations.len() / 2 - 1] + correlations[correlations.len() / 2]) / 2
        } else {
            correlations[correlations.len() / 2]
        };
        
        // Count high correlation pairs (|corr| > 0.7 in fixed point)
        let high_corr_threshold = 700_000; // 0.7 in fixed point
        let high_corr_count = matrix.correlations.iter()
            .filter(|c| c.correlation.unsigned_abs() > high_corr_threshold)
            .count() as u32;
        
        // TODO: Calculate standard deviation
        let std_dev = 0;
        
        Ok(Self {
            min_correlation: min,
            max_correlation: max,
            median_correlation: median,
            std_deviation: std_dev,
            high_correlation_pairs: high_corr_count,
        })
    }
}
