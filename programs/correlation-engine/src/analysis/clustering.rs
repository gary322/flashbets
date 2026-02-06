use solana_program::program_error::ProgramError;
use crate::state::correlation_matrix::CorrelationMatrix;
use crate::math::fixed_point::U64F64;
use crate::error::CorrelationError;
use borsh::{BorshDeserialize, BorshSerialize};

/// Union-Find data structure for clustering correlated markets
pub struct UnionFind {
    parent: Vec<u16>,
    rank: Vec<u8>,
    size: usize,
}

impl UnionFind {
    /// Create a new Union-Find structure for n markets
    pub fn new(n: usize) -> Self {
        let parent: Vec<u16> = (0..n as u16).collect();
        let rank = vec![0; n];
        
        Self {
            parent,
            rank,
            size: n,
        }
    }
    
    /// Find the root parent of a market (with path compression)
    pub fn find(&mut self, x: u16) -> u16 {
        if self.parent[x as usize] != x {
            // Path compression: make every node point directly to root
            self.parent[x as usize] = self.find(self.parent[x as usize]);
        }
        self.parent[x as usize]
    }
    
    /// Union two markets into the same cluster
    pub fn union(&mut self, x: u16, y: u16) {
        let root_x = self.find(x);
        let root_y = self.find(y);
        
        if root_x != root_y {
            // Union by rank: attach smaller tree to larger tree
            if self.rank[root_x as usize] < self.rank[root_y as usize] {
                self.parent[root_x as usize] = root_y;
            } else if self.rank[root_x as usize] > self.rank[root_y as usize] {
                self.parent[root_y as usize] = root_x;
            } else {
                self.parent[root_y as usize] = root_x;
                self.rank[root_x as usize] += 1;
            }
        }
    }
    
    /// Get all clusters as groups of market indices
    pub fn get_clusters(&mut self) -> Vec<Vec<u16>> {
        let mut clusters: Vec<Vec<u16>> = Vec::new();
        let mut cluster_map: Vec<Option<usize>> = vec![None; self.size];
        
        for i in 0..self.size {
            let root = self.find(i as u16);
            
            if let Some(cluster_idx) = cluster_map[root as usize] {
                clusters[cluster_idx].push(i as u16);
            } else {
                cluster_map[root as usize] = Some(clusters.len());
                clusters.push(vec![i as u16]);
            }
        }
        
        clusters
    }
}

/// Correlation cluster information
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CorrelationCluster {
    pub cluster_id: u8,
    pub market_indices: Vec<u16>,
    pub average_internal_correlation: u64,  // Fixed point
    pub size: u16,
}

/// Clustering analysis results
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ClusteringResults {
    pub clusters: Vec<CorrelationCluster>,
    pub num_clusters: u8,
    pub threshold_used: u64,  // Fixed point correlation threshold
    pub timestamp: i64,
}

/// Identify correlation clusters in a verse
pub fn identify_correlation_clusters(
    matrix: &CorrelationMatrix,
    threshold: u64,  // Fixed point threshold (e.g., 0.7 = 700_000)
    market_count: u16,
) -> Result<ClusteringResults, ProgramError> {
    if market_count == 0 {
        return Err(CorrelationError::InvalidMarketCount.into());
    }
    
    let mut uf = UnionFind::new(market_count as usize);
    
    // Union markets with correlation above threshold
    for entry in &matrix.correlations {
        // Stored correlation is in mapped representation [0, 2*ONE] where ONE is 0 correlation.
        // Convert to signed [-ONE, ONE] then take absolute magnitude [0, ONE].
        const ONE: i64 = 1_000_000;
        let signed = entry.correlation - ONE;
        let abs_corr = signed.unsigned_abs();
        
        if abs_corr >= threshold {
            uf.union(entry.market_i, entry.market_j);
        }
    }
    
    // Get clusters
    let cluster_groups = uf.get_clusters();
    let mut clusters = Vec::new();
    
    for (idx, group) in cluster_groups.iter().enumerate() {
        if group.len() > 1 {  // Only include clusters with more than 1 market
            // Calculate average internal correlation for this cluster
            let avg_corr = calculate_cluster_average_correlation(matrix, group)?;
            
            clusters.push(CorrelationCluster {
                cluster_id: idx as u8,
                market_indices: group.clone(),
                average_internal_correlation: avg_corr,
                size: group.len() as u16,
            });
        }
    }
    
    // Sort clusters by size (largest first)
    clusters.sort_by(|a, b| b.size.cmp(&a.size));
    
    Ok(ClusteringResults {
        num_clusters: clusters.len() as u8,
        clusters,
        threshold_used: threshold,
        timestamp: 0,  // To be set by caller
    })
}

/// Calculate average correlation within a cluster
fn calculate_cluster_average_correlation(
    matrix: &CorrelationMatrix,
    cluster_markets: &[u16],
) -> Result<u64, ProgramError> {
    if cluster_markets.len() < 2 {
        return Ok(0);
    }
    
    let mut sum = 0u128;
    let mut count = 0u32;
    
    // Find all correlations between markets in this cluster
    for i in 0..cluster_markets.len() {
        for j in (i + 1)..cluster_markets.len() {
            let market_i = cluster_markets[i];
            let market_j = cluster_markets[j];
            
            // Find correlation entry
            if let Some(entry) = matrix.correlations.iter()
                .find(|e| (e.market_i == market_i && e.market_j == market_j) ||
                         (e.market_i == market_j && e.market_j == market_i)) {

                const ONE: i64 = 1_000_000;
                let signed = entry.correlation - ONE;
                let abs_corr = signed.unsigned_abs();
                
                sum += abs_corr as u128;
                count += 1;
            }
        }
    }
    
    if count == 0 {
        Ok(0)
    } else {
        Ok((sum / count as u128) as u64)
    }
}

/// Analyze cluster risk concentration
pub fn analyze_cluster_risk(
    clusters: &ClusteringResults,
    total_markets: u16,
) -> ClusterRiskAnalysis {
    let largest_cluster_size = clusters.clusters
        .iter()
        .map(|c| c.size)
        .max()
        .unwrap_or(0);
    
    let concentration_ratio = if total_markets > 0 {
        (largest_cluster_size as u64 * U64F64::ONE) / total_markets as u64
    } else {
        0
    };
    
    // Count markets in high correlation clusters (>3 markets)
    let high_risk_market_count: u16 = clusters.clusters
        .iter()
        .filter(|c| c.size > 3)
        .map(|c| c.size)
        .sum();
    
    let high_risk_ratio = if total_markets > 0 {
        (high_risk_market_count as u64 * U64F64::ONE) / total_markets as u64
    } else {
        0
    };
    
    ClusterRiskAnalysis {
        largest_cluster_size,
        concentration_ratio,
        high_risk_market_count,
        high_risk_ratio,
        risk_level: determine_risk_level(concentration_ratio),
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ClusterRiskAnalysis {
    pub largest_cluster_size: u16,
    pub concentration_ratio: u64,  // Fixed point
    pub high_risk_market_count: u16,
    pub high_risk_ratio: u64,  // Fixed point
    pub risk_level: RiskLevel,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

fn determine_risk_level(concentration_ratio: u64) -> RiskLevel {
    if concentration_ratio < 200_000 {  // < 20%
        RiskLevel::Low
    } else if concentration_ratio < 400_000 {  // < 40%
        RiskLevel::Medium
    } else if concentration_ratio < 600_000 {  // < 60%
        RiskLevel::High
    } else {
        RiskLevel::Critical
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::correlation_matrix::CorrelationEntry;
    
    #[test]
    fn test_union_find() {
        let mut uf = UnionFind::new(5);
        
        // Initially, each element is its own parent
        assert_eq!(uf.find(0), 0);
        assert_eq!(uf.find(1), 1);
        
        // Union 0 and 1
        uf.union(0, 1);
        assert_eq!(uf.find(0), uf.find(1));
        
        // Union 2 and 3
        uf.union(2, 3);
        assert_eq!(uf.find(2), uf.find(3));
        
        // Union clusters
        uf.union(1, 2);
        assert_eq!(uf.find(0), uf.find(3));
        
        // Get clusters
        let clusters = uf.get_clusters();
        assert_eq!(clusters.len(), 2);  // One big cluster and element 4 alone
    }
    
    #[test]
    fn test_correlation_clustering() {
        let matrix = CorrelationMatrix {
            is_initialized: true,
            verse_id: [0u8; 16],
            correlations: vec![
                // Cluster 1: markets 0, 1, 2 (high correlation)
                CorrelationEntry {
                    market_i: 0,
                    market_j: 1,
                    correlation: 1_900_000,  // +0.9 in mapped representation
                    last_updated: 0,
                    sample_size: 7,
                },
                CorrelationEntry {
                    market_i: 1,
                    market_j: 2,
                    correlation: 1_850_000,  // +0.85
                    last_updated: 0,
                    sample_size: 7,
                },
                CorrelationEntry {
                    market_i: 0,
                    market_j: 2,
                    correlation: 1_800_000,  // +0.8
                    last_updated: 0,
                    sample_size: 7,
                },
                // Cluster 2: markets 3, 4 (high correlation)
                CorrelationEntry {
                    market_i: 3,
                    market_j: 4,
                    correlation: 1_750_000,  // +0.75
                    last_updated: 0,
                    sample_size: 7,
                },
                // Low correlation between clusters
                CorrelationEntry {
                    market_i: 0,
                    market_j: 3,
                    correlation: 1_200_000,  // +0.2
                    last_updated: 0,
                    sample_size: 7,
                },
            ],
            average_correlation: 0,
            last_calculated: 0,
            calculation_version: 1,
            market_count: 5,
            bump: 0,
        };
        
        let results = identify_correlation_clusters(&matrix, 700_000, 5).unwrap();
        
        assert_eq!(results.num_clusters, 2);
        assert_eq!(results.clusters[0].size, 3);  // Cluster with markets 0, 1, 2
        assert_eq!(results.clusters[1].size, 2);  // Cluster with markets 3, 4
    }
    
    #[test]
    fn test_cluster_risk_analysis() {
        let clusters = ClusteringResults {
            clusters: vec![
                CorrelationCluster {
                    cluster_id: 0,
                    market_indices: vec![0, 1, 2, 3, 4],
                    average_internal_correlation: 800_000,
                    size: 5,
                },
                CorrelationCluster {
                    cluster_id: 1,
                    market_indices: vec![5, 6],
                    average_internal_correlation: 750_000,
                    size: 2,
                },
            ],
            num_clusters: 2,
            threshold_used: 700_000,
            timestamp: 0,
        };
        
        let risk = analyze_cluster_risk(&clusters, 10);
        
        assert_eq!(risk.largest_cluster_size, 5);
        assert_eq!(risk.concentration_ratio, 500_000);  // 50%
        assert_eq!(risk.high_risk_market_count, 5);  // Only the cluster with >3 markets
        assert_eq!(risk.risk_level, RiskLevel::High);
    }
}
