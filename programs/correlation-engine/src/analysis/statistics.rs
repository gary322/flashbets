use solana_program::program_error::ProgramError;
use crate::state::correlation_matrix::CorrelationMatrix;
use crate::math::fixed_point::U64F64;
use borsh::{BorshDeserialize, BorshSerialize};

/// Comprehensive correlation statistics for a verse
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CorrelationStatistics {
    pub min_correlation: i64,
    pub max_correlation: i64,
    pub mean_correlation: i64,
    pub median_correlation: i64,
    pub std_deviation: u64,  // Always positive
    pub high_correlation_pairs: u16,  // Count of pairs with |corr| > 0.7
    pub negative_correlation_pairs: u16,
    pub total_pairs: u16,
    pub timestamp: i64,
}

/// Calculate comprehensive statistics from correlation matrix
pub fn calculate_correlation_statistics(
    matrix: &CorrelationMatrix,
) -> Result<CorrelationStatistics, ProgramError> {
    if matrix.correlations.is_empty() {
        return Ok(CorrelationStatistics {
            min_correlation: 0,
            max_correlation: 0,
            mean_correlation: 0,
            median_correlation: 0,
            std_deviation: 0,
            high_correlation_pairs: 0,
            negative_correlation_pairs: 0,
            total_pairs: 0,
            timestamp: 0,
        });
    }
    
    // Extract all correlations
    let mut correlations: Vec<i64> = matrix.correlations
        .iter()
        .map(|e| e.correlation)
        .collect();
    
    // Sort for median calculation
    correlations.sort();
    
    // Calculate min and max
    let min_correlation = *correlations.first().unwrap();
    let max_correlation = *correlations.last().unwrap();
    
    // Calculate mean
    let sum: i128 = correlations.iter().map(|&c| c as i128).sum();
    let mean_correlation = (sum / correlations.len() as i128) as i64;
    
    // Calculate median
    let median_correlation = if correlations.len() % 2 == 0 {
        let mid = correlations.len() / 2;
        ((correlations[mid - 1] as i128 + correlations[mid] as i128) / 2) as i64
    } else {
        correlations[correlations.len() / 2]
    };
    
    // Calculate standard deviation
    let variance_sum: u128 = correlations.iter()
        .map(|&c| {
            let diff = (c as i128 - mean_correlation as i128).abs() as u128;
            diff * diff
        })
        .sum();
    
    let variance = variance_sum / correlations.len() as u128;
    let std_deviation = integer_sqrt(variance) as u64;
    
    // Count high correlation pairs and negative pairs
    let high_correlation_pairs = matrix.correlations.iter()
        .filter(|e| e.correlation.abs() > 700_000)  // |corr| > 0.7
        .count() as u16;
    
    let negative_correlation_pairs = matrix.correlations.iter()
        .filter(|e| e.correlation < 0)
        .count() as u16;
    
    Ok(CorrelationStatistics {
        min_correlation,
        max_correlation,
        mean_correlation,
        median_correlation,
        std_deviation,
        high_correlation_pairs,
        negative_correlation_pairs,
        total_pairs: matrix.correlations.len() as u16,
        timestamp: 0,
    })
}

/// Integer square root for variance calculation
fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    x
}

/// Distribution analysis of correlations
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CorrelationDistribution {
    pub buckets: Vec<DistributionBucket>,
    pub total_count: u16,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct DistributionBucket {
    pub range_start: i64,  // Inclusive
    pub range_end: i64,    // Exclusive
    pub count: u16,
    pub percentage: u16,   // Basis points (100 = 1%)
}

/// Analyze the distribution of correlations
pub fn analyze_correlation_distribution(
    matrix: &CorrelationMatrix,
) -> Result<CorrelationDistribution, ProgramError> {
    // Define buckets for correlation ranges
    let bucket_ranges = vec![
        (-1_000_000, -800_000),  // -1.0 to -0.8 (strong negative)
        (-800_000, -600_000),    // -0.8 to -0.6
        (-600_000, -400_000),    // -0.6 to -0.4
        (-400_000, -200_000),    // -0.4 to -0.2
        (-200_000, 0),           // -0.2 to 0.0
        (0, 200_000),            // 0.0 to 0.2
        (200_000, 400_000),      // 0.2 to 0.4
        (400_000, 600_000),      // 0.4 to 0.6
        (600_000, 800_000),      // 0.6 to 0.8
        (800_000, 1_000_000),    // 0.8 to 1.0 (strong positive)
    ];
    
    let total_count = matrix.correlations.len() as u16;
    let mut buckets = Vec::new();
    
    for (start, end) in bucket_ranges {
        let count = matrix.correlations.iter()
            .filter(|e| e.correlation >= start && e.correlation < end)
            .count() as u16;
        
        let percentage = if total_count > 0 {
            (count as u32 * 10000 / total_count as u32) as u16
        } else {
            0
        };
        
        buckets.push(DistributionBucket {
            range_start: start,
            range_end: end,
            count,
            percentage,
        });
    }
    
    Ok(CorrelationDistribution {
        buckets,
        total_count,
    })
}

/// Time series analysis of correlation stability
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct CorrelationStability {
    pub stable_pairs: u16,      // Pairs with low variance over time
    pub volatile_pairs: u16,    // Pairs with high variance over time
    pub stability_ratio: u64,   // Fixed point
}

/// Market connectivity analysis
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MarketConnectivity {
    pub market_id: u16,
    pub connection_count: u16,      // Number of significant correlations
    pub avg_correlation: u64,       // Average absolute correlation
    pub max_correlation: u64,       // Maximum absolute correlation
    pub connectivity_score: u64,    // Overall connectivity metric
}

/// Analyze connectivity of individual markets
pub fn analyze_market_connectivity(
    matrix: &CorrelationMatrix,
    market_id: u16,
    significance_threshold: u64,  // e.g., 300_000 for |corr| > 0.3
) -> Result<MarketConnectivity, ProgramError> {
    let connections: Vec<&crate::state::correlation_matrix::CorrelationEntry> = matrix.correlations
        .iter()
        .filter(|e| (e.market_i == market_id || e.market_j == market_id) && 
                    e.correlation.abs() as u64 > significance_threshold)
        .collect();
    
    if connections.is_empty() {
        return Ok(MarketConnectivity {
            market_id,
            connection_count: 0,
            avg_correlation: 0,
            max_correlation: 0,
            connectivity_score: 0,
        });
    }
    
    let mut sum = 0u128;
    let mut max_corr = 0u64;
    
    for entry in &connections {
        let abs_corr = entry.correlation.abs() as u64;
        sum += abs_corr as u128;
        max_corr = max_corr.max(abs_corr);
    }
    
    let avg_correlation = (sum / connections.len() as u128) as u64;
    let connection_count = connections.len() as u16;
    
    // Connectivity score: weighted average of connection count and average correlation
    // Score = 0.5 * normalized_count + 0.5 * avg_correlation
    let max_possible_connections = matrix.market_count.saturating_sub(1);
    let normalized_count = if max_possible_connections > 0 {
        (connection_count as u64 * U64F64::ONE) / max_possible_connections as u64
    } else {
        0
    };
    
    let connectivity_score = (normalized_count + avg_correlation) / 2;
    
    Ok(MarketConnectivity {
        market_id,
        connection_count,
        avg_correlation,
        max_correlation: max_corr,
        connectivity_score,
    })
}

/// Identify the most connected markets in a verse
pub fn find_hub_markets(
    matrix: &CorrelationMatrix,
    top_n: usize,
    significance_threshold: u64,
) -> Result<Vec<MarketConnectivity>, ProgramError> {
    let mut connectivities = Vec::new();
    
    for market_id in 0..matrix.market_count {
        let connectivity = analyze_market_connectivity(matrix, market_id, significance_threshold)?;
        connectivities.push(connectivity);
    }
    
    // Sort by connectivity score (descending)
    connectivities.sort_by(|a, b| b.connectivity_score.cmp(&a.connectivity_score));
    
    // Return top N
    Ok(connectivities.into_iter().take(top_n).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::correlation_matrix::CorrelationEntry;
    
    #[test]
    fn test_correlation_statistics() {
        let matrix = CorrelationMatrix {
            is_initialized: true,
            verse_id: [0u8; 16],
            correlations: vec![
                CorrelationEntry {
                    market_i: 0,
                    market_j: 1,
                    correlation: 900_000,  // 0.9
                    last_updated: 0,
                    sample_size: 7,
                },
                CorrelationEntry {
                    market_i: 0,
                    market_j: 2,
                    correlation: -500_000,  // -0.5
                    last_updated: 0,
                    sample_size: 7,
                },
                CorrelationEntry {
                    market_i: 1,
                    market_j: 2,
                    correlation: 200_000,  // 0.2
                    last_updated: 0,
                    sample_size: 7,
                },
            ],
            average_correlation: 0,
            last_calculated: 0,
            calculation_version: 1,
            market_count: 3,
            bump: 0,
        };
        
        let stats = calculate_correlation_statistics(&matrix).unwrap();
        
        assert_eq!(stats.min_correlation, -500_000);
        assert_eq!(stats.max_correlation, 900_000);
        assert_eq!(stats.total_pairs, 3);
        assert_eq!(stats.high_correlation_pairs, 1);
        assert_eq!(stats.negative_correlation_pairs, 1);
    }
    
    #[test]
    fn test_correlation_distribution() {
        let matrix = CorrelationMatrix {
            is_initialized: true,
            verse_id: [0u8; 16],
            correlations: vec![
                CorrelationEntry { market_i: 0, market_j: 1, correlation: 900_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 0, market_j: 2, correlation: 750_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 0, market_j: 3, correlation: 300_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 1, market_j: 2, correlation: -200_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 1, market_j: 3, correlation: -700_000, last_updated: 0, sample_size: 7 },
            ],
            average_correlation: 0,
            last_calculated: 0,
            calculation_version: 1,
            market_count: 4,
            bump: 0,
        };
        
        let dist = analyze_correlation_distribution(&matrix).unwrap();
        
        assert_eq!(dist.total_count, 5);
        
        // Check specific buckets
        let high_positive_bucket = dist.buckets.iter()
            .find(|b| b.range_start == 800_000 && b.range_end == 1_000_000)
            .unwrap();
        assert_eq!(high_positive_bucket.count, 1);  // One correlation in 0.8-1.0 range
        
        let negative_bucket = dist.buckets.iter()
            .find(|b| b.range_start == -800_000 && b.range_end == -600_000)
            .unwrap();
        assert_eq!(negative_bucket.count, 1);  // One correlation in -0.8 to -0.6 range
    }
    
    #[test]
    fn test_market_connectivity() {
        let matrix = CorrelationMatrix {
            is_initialized: true,
            verse_id: [0u8; 16],
            correlations: vec![
                CorrelationEntry { market_i: 0, market_j: 1, correlation: 800_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 0, market_j: 2, correlation: 700_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 0, market_j: 3, correlation: 600_000, last_updated: 0, sample_size: 7 },
                CorrelationEntry { market_i: 1, market_j: 2, correlation: 100_000, last_updated: 0, sample_size: 7 },
            ],
            average_correlation: 0,
            last_calculated: 0,
            calculation_version: 1,
            market_count: 4,
            bump: 0,
        };
        
        // Market 0 is highly connected
        let connectivity = analyze_market_connectivity(&matrix, 0, 500_000).unwrap();
        assert_eq!(connectivity.connection_count, 3);
        assert_eq!(connectivity.market_id, 0);
        assert!(connectivity.avg_correlation > 600_000);
        
        // Market 3 has only one significant connection
        let connectivity = analyze_market_connectivity(&matrix, 3, 500_000).unwrap();
        assert_eq!(connectivity.connection_count, 1);
    }
}