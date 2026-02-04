//! Performance Comparison Metrics
//!
//! Displays competitive performance metrics vs other platforms
//! Native Solana implementation - NO ANCHOR

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{Event, EventType},
    define_event,
};

/// Platform performance metrics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PlatformMetrics {
    pub name: String,
    pub tps: u64,
    pub block_time_ms: u64,
    pub finality_ms: u64,
    pub cost_per_tx_usd: f32,
    pub decentralized: bool,
}

/// Performance comparison data
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PerformanceComparison {
    pub our_platform: PlatformMetrics,
    pub competitors: Vec<PlatformMetrics>,
    pub last_updated: i64,
}

impl PerformanceComparison {
    pub fn new() -> Self {
        Self {
            our_platform: PlatformMetrics {
                name: "Solana Native Betting".to_string(),
                tps: 65_000, // 65k TPS as specified
                block_time_ms: 400,
                finality_ms: 400,
                cost_per_tx_usd: 0.00025,
                decentralized: true,
            },
            competitors: vec![
                PlatformMetrics {
                    name: "Polygon (Polymarket)".to_string(),
                    tps: 2_000, // 2k TPS as specified
                    block_time_ms: 2000,
                    finality_ms: 2000,
                    cost_per_tx_usd: 0.01,
                    decentralized: true,
                },
                PlatformMetrics {
                    name: "Ethereum L1".to_string(),
                    tps: 15,
                    block_time_ms: 12000,
                    finality_ms: 900_000, // 15 minutes
                    cost_per_tx_usd: 5.0,
                    decentralized: true,
                },
                PlatformMetrics {
                    name: "BSC".to_string(),
                    tps: 160,
                    block_time_ms: 3000,
                    finality_ms: 3000,
                    cost_per_tx_usd: 0.10,
                    decentralized: false,
                },
            ],
            last_updated: 0,
        }
    }
    
    /// Calculate performance advantage
    pub fn calculate_advantage(&self, metric: PerformanceMetric) -> Vec<(String, f64)> {
        let our_value = match metric {
            PerformanceMetric::TPS => self.our_platform.tps as f64,
            PerformanceMetric::BlockTime => self.our_platform.block_time_ms as f64,
            PerformanceMetric::Finality => self.our_platform.finality_ms as f64,
            PerformanceMetric::Cost => self.our_platform.cost_per_tx_usd as f64,
        };
        
        self.competitors.iter().map(|competitor| {
            let their_value = match metric {
                PerformanceMetric::TPS => competitor.tps as f64,
                PerformanceMetric::BlockTime => competitor.block_time_ms as f64,
                PerformanceMetric::Finality => competitor.finality_ms as f64,
                PerformanceMetric::Cost => competitor.cost_per_tx_usd as f64,
            };
            
            let advantage = match metric {
                PerformanceMetric::TPS => our_value / their_value,
                PerformanceMetric::BlockTime | PerformanceMetric::Finality | PerformanceMetric::Cost => 
                    their_value / our_value,
            };
            
            (competitor.name.clone(), advantage)
        }).collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PerformanceMetric {
    TPS,
    BlockTime,
    Finality,
    Cost,
}

/// Display performance comparison
pub fn process_display_performance_comparison(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let clock = Clock::get()?;
    let comparison = PerformanceComparison::new();
    
    msg!("=== PLATFORM PERFORMANCE COMPARISON ===");
    msg!("Last Updated: {}", clock.unix_timestamp);
    msg!("");
    
    // Display our metrics
    msg!("Our Platform (Solana Native):");
    msg!("  • TPS: {:,} transactions/second", comparison.our_platform.tps);
    msg!("  • Block Time: {}ms", comparison.our_platform.block_time_ms);
    msg!("  • Finality: {}ms", comparison.our_platform.finality_ms);
    msg!("  • Cost per TX: ${:.5}", comparison.our_platform.cost_per_tx_usd);
    msg!("  • Decentralized: ✅");
    msg!("");
    
    // Display comparisons
    msg!("Performance Advantages:");
    
    // TPS comparison
    let tps_advantages = comparison.calculate_advantage(PerformanceMetric::TPS);
    for (platform, advantage) in &tps_advantages {
        msg!("  vs {}: {:.0}x faster TPS", platform, advantage);
    }
    msg!("");
    
    // Block time comparison
    let blocktime_advantages = comparison.calculate_advantage(PerformanceMetric::BlockTime);
    for (platform, advantage) in &blocktime_advantages {
        msg!("  vs {}: {:.0}x faster blocks", platform, advantage);
    }
    msg!("");
    
    // Cost comparison
    let cost_advantages = comparison.calculate_advantage(PerformanceMetric::Cost);
    for (platform, advantage) in &cost_advantages {
        msg!("  vs {}: {:.0}x cheaper", platform, advantage);
    }
    msg!("");
    
    // Key highlights
    msg!("🚀 KEY HIGHLIGHTS:");
    msg!("  • 32.5x faster than Polymarket (65k vs 2k TPS)");
    msg!("  • 5x faster block time (400ms vs 2s)");
    msg!("  • 40x cheaper transactions ($0.00025 vs $0.01)");
    msg!("  • True decentralization with no single point of failure");
    
    // Emit event
    PerformanceComparisonDisplayed {
        timestamp: clock.unix_timestamp,
        our_tps: comparison.our_platform.tps,
        polymarket_tps: 2_000,
        advantage_multiple: 32,
    }.emit();
    
    Ok(())
}

/// Get performance comparison data for UI
pub fn get_performance_comparison_data() -> PerformanceComparison {
    let mut comparison = PerformanceComparison::new();
    comparison.last_updated = Clock::get().unwrap_or_default().unix_timestamp;
    comparison
}

/// Format performance metric for display
pub fn format_performance_metric(value: u64, metric_type: &str) -> String {
    match metric_type {
        "tps" => format!("{:,} TPS", value),
        "block_time" => format!("{}ms", value),
        "finality" => {
            if value < 1000 {
                format!("{}ms", value)
            } else if value < 60_000 {
                format!("{:.1}s", value as f64 / 1000.0)
            } else {
                format!("{:.1}m", value as f64 / 60_000.0)
            }
        }
        _ => format!("{}", value),
    }
}

// Event definition
define_event!(PerformanceComparisonDisplayed, EventType::PerformanceSnapshot, {
    timestamp: i64,
    our_tps: u64,
    polymarket_tps: u64,
    advantage_multiple: u8,
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_performance_comparison() {
        let comparison = PerformanceComparison::new();
        
        // Verify our platform metrics
        assert_eq!(comparison.our_platform.tps, 65_000);
        
        // Verify Polymarket metrics
        let polymarket = comparison.competitors.iter()
            .find(|c| c.name.contains("Polymarket"))
            .unwrap();
        assert_eq!(polymarket.tps, 2_000);
        
        // Calculate TPS advantage
        let tps_advantages = comparison.calculate_advantage(PerformanceMetric::TPS);
        let polymarket_advantage = tps_advantages.iter()
            .find(|(name, _)| name.contains("Polymarket"))
            .unwrap();
        
        // 65000 / 2000 = 32.5
        assert!(polymarket_advantage.1 > 32.0 && polymarket_advantage.1 < 33.0);
    }
    
    #[test]
    fn test_metric_formatting() {
        assert_eq!(format_performance_metric(65_000, "tps"), "65,000 TPS");
        assert_eq!(format_performance_metric(400, "block_time"), "400ms");
        assert_eq!(format_performance_metric(900_000, "finality"), "15.0m");
    }
}