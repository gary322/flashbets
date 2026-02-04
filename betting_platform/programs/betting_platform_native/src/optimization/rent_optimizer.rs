//! Solana Rent Cost Calculator and Optimizer
//!
//! Calculates and optimizes rent costs for the betting platform's on-chain storage
//! with strategies to minimize rent while maintaining performance.

use solana_program::{
    rent::Rent,
    pubkey::Pubkey,
    msg,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    state::*,
    compression::ZKCompressedState,
};

/// Solana rent constants (mainnet values)
pub const LAMPORTS_PER_BYTE_YEAR: u64 = 3_480; // ~0.00348 SOL per byte per year
pub const RENT_EXEMPT_MINIMUM: u64 = 890_880; // Minimum account balance
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Account size constants
pub const DISCRIMINATOR_SIZE: usize = 8;
pub const PUBKEY_SIZE: usize = 32;
pub const U64_SIZE: usize = 8;
pub const U32_SIZE: usize = 4;
pub const U16_SIZE: usize = 2;
pub const U8_SIZE: usize = 1;
pub const BOOL_SIZE: usize = 1;

/// Rent optimization configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RentOptimizationConfig {
    pub enable_compression: bool,
    pub archive_inactive_after_days: u32,
    pub use_minimal_accounts: bool,
    pub batch_similar_accounts: bool,
    pub target_compression_ratio: f32,
}

impl Default for RentOptimizationConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            archive_inactive_after_days: 30,
            use_minimal_accounts: true,
            batch_similar_accounts: true,
            target_compression_ratio: 10.0,
        }
    }
}

/// Rent calculator for different account types
pub struct RentCalculator;

impl RentCalculator {
    /// Calculate annual rent for an account
    pub fn calculate_annual_rent(size_bytes: usize) -> u64 {
        (size_bytes as u64) * LAMPORTS_PER_BYTE_YEAR
    }
    
    /// Calculate rent-exempt balance for an account
    pub fn calculate_rent_exempt(size_bytes: usize) -> u64 {
        // Rent = 2 years of rent payments
        let annual_rent = Self::calculate_annual_rent(size_bytes);
        annual_rent * 2 + RENT_EXEMPT_MINIMUM
    }
    
    /// Calculate rent in SOL
    pub fn calculate_rent_sol(size_bytes: usize) -> f64 {
        let lamports = Self::calculate_rent_exempt(size_bytes);
        lamports as f64 / LAMPORTS_PER_SOL as f64
    }
    
    /// Calculate position account rent
    pub fn position_account_rent() -> RentAnalysis {
        let size = std::mem::size_of::<Position>();
        let compressed_size = std::mem::size_of::<CompressedPosition>();
        
        RentAnalysis {
            account_type: "Position".to_string(),
            uncompressed_size: size,
            compressed_size: compressed_size,
            uncompressed_rent_sol: Self::calculate_rent_sol(size),
            compressed_rent_sol: Self::calculate_rent_sol(compressed_size),
            compression_ratio: size as f32 / compressed_size as f32,
            annual_cost_sol: Self::calculate_annual_rent(size) as f64 / LAMPORTS_PER_SOL as f64,
        }
    }
    
    /// Calculate proposal account rent
    pub fn proposal_account_rent() -> RentAnalysis {
        let size = std::mem::size_of::<ProposalPDA>();
        let compressed_size = std::mem::size_of::<CompressedProposal>();
        
        RentAnalysis {
            account_type: "Proposal".to_string(),
            uncompressed_size: size,
            compressed_size: compressed_size,
            uncompressed_rent_sol: Self::calculate_rent_sol(size),
            compressed_rent_sol: Self::calculate_rent_sol(compressed_size),
            compression_ratio: size as f32 / compressed_size as f32,
            annual_cost_sol: Self::calculate_annual_rent(size) as f64 / LAMPORTS_PER_SOL as f64,
        }
    }
    
    /// Calculate total platform rent costs
    pub fn calculate_platform_costs(
        num_positions: u64,
        num_proposals: u64,
        num_users: u64,
        num_markets: u64,
        config: &RentOptimizationConfig,
    ) -> PlatformRentCosts {
        let position_rent = Self::position_account_rent();
        let proposal_rent = Self::proposal_account_rent();
        
        // Calculate costs based on compression settings
        let position_cost = if config.enable_compression {
            position_rent.compressed_rent_sol
        } else {
            position_rent.uncompressed_rent_sol
        };
        
        let proposal_cost = if config.enable_compression {
            proposal_rent.compressed_rent_sol
        } else {
            proposal_rent.uncompressed_rent_sol
        };
        
        // User accounts (relatively small)
        let user_account_size = DISCRIMINATOR_SIZE + PUBKEY_SIZE + U64_SIZE * 10; // ~120 bytes
        let user_cost = Self::calculate_rent_sol(user_account_size);
        
        // Market accounts
        let market_account_size = if config.enable_compression { 200 } else { 1024 };
        let market_cost = Self::calculate_rent_sol(market_account_size);
        
        // Calculate totals
        let total_positions_cost = position_cost * num_positions as f64;
        let total_proposals_cost = proposal_cost * num_proposals as f64;
        let total_users_cost = user_cost * num_users as f64;
        let total_markets_cost = market_cost * num_markets as f64;
        
        let total_cost = total_positions_cost + total_proposals_cost + 
                        total_users_cost + total_markets_cost;
        
        PlatformRentCosts {
            positions: CostBreakdown {
                count: num_positions,
                cost_per_account: position_cost,
                total_cost: total_positions_cost,
                percentage: (total_positions_cost / total_cost) * 100.0,
            },
            proposals: CostBreakdown {
                count: num_proposals,
                cost_per_account: proposal_cost,
                total_cost: total_proposals_cost,
                percentage: (total_proposals_cost / total_cost) * 100.0,
            },
            users: CostBreakdown {
                count: num_users,
                cost_per_account: user_cost,
                total_cost: total_users_cost,
                percentage: (total_users_cost / total_cost) * 100.0,
            },
            markets: CostBreakdown {
                count: num_markets,
                cost_per_account: market_cost,
                total_cost: total_markets_cost,
                percentage: (total_markets_cost / total_cost) * 100.0,
            },
            total_cost_sol: total_cost,
            annual_cost_sol: total_cost * 0.5, // Rent-exempt requires 2 years upfront
            compression_enabled: config.enable_compression,
            potential_savings: if !config.enable_compression {
                Self::calculate_compression_savings(
                    num_positions, num_proposals, num_users, num_markets
                )
            } else {
                0.0
            },
        }
    }
    
    /// Calculate potential savings from compression
    fn calculate_compression_savings(
        num_positions: u64,
        num_proposals: u64,
        num_users: u64,
        num_markets: u64,
    ) -> f64 {
        let mut config = RentOptimizationConfig::default();
        config.enable_compression = false;
        let uncompressed = Self::calculate_platform_costs(
            num_positions, num_proposals, num_users, num_markets, &config
        );
        
        config.enable_compression = true;
        let compressed = Self::calculate_platform_costs(
            num_positions, num_proposals, num_users, num_markets, &config
        );
        
        uncompressed.total_cost_sol - compressed.total_cost_sol
    }
}

/// Rent optimization strategies
pub struct RentOptimizer;

impl RentOptimizer {
    /// Optimize account layout for minimal size
    pub fn optimize_account_layout() -> Vec<OptimizationStrategy> {
        vec![
            OptimizationStrategy {
                name: "Use u32 for timestamps".to_string(),
                description: "Store as offset from epoch instead of i64".to_string(),
                space_saved: 4,
                implementation_complexity: Complexity::Low,
            },
            OptimizationStrategy {
                name: "Pack boolean flags".to_string(),
                description: "Pack 8 booleans into single u8".to_string(),
                space_saved: 7,
                implementation_complexity: Complexity::Medium,
            },
            OptimizationStrategy {
                name: "Truncate pubkeys".to_string(),
                description: "Store first 20 bytes for non-critical keys".to_string(),
                space_saved: 12,
                implementation_complexity: Complexity::Medium,
            },
            OptimizationStrategy {
                name: "Use basis points".to_string(),
                description: "Store percentages as u16 instead of f64".to_string(),
                space_saved: 6,
                implementation_complexity: Complexity::Low,
            },
            OptimizationStrategy {
                name: "Enum discriminants".to_string(),
                description: "Use u8 for enums instead of default size".to_string(),
                space_saved: 3,
                implementation_complexity: Complexity::Low,
            },
        ]
    }
    
    /// Calculate optimal batch sizes for account creation
    pub fn calculate_optimal_batch_size(
        available_sol: f64,
        account_type: &str,
    ) -> BatchingRecommendation {
        let account_cost = match account_type {
            "Position" => RentCalculator::position_account_rent().compressed_rent_sol,
            "Proposal" => RentCalculator::proposal_account_rent().compressed_rent_sol,
            _ => 0.01, // Default small account
        };
        
        let max_accounts = (available_sol / account_cost) as u32;
        let recommended_batch = max_accounts.min(1000); // Cap at 1000 for safety
        
        BatchingRecommendation {
            account_type: account_type.to_string(),
            cost_per_account: account_cost,
            max_accounts_possible: max_accounts,
            recommended_batch_size: recommended_batch,
            total_cost: account_cost * recommended_batch as f64,
        }
    }
    
    /// Generate rent report for current platform state
    pub fn generate_rent_report(
        total_accounts: u64,
        compression_enabled: bool,
    ) -> RentReport {
        // Estimate account distribution
        let positions = total_accounts * 60 / 100; // 60% positions
        let proposals = total_accounts * 20 / 100; // 20% proposals
        let users = total_accounts * 15 / 100;     // 15% users
        let markets = total_accounts * 5 / 100;    // 5% markets
        
        let config = RentOptimizationConfig {
            enable_compression: compression_enabled,
            ..Default::default()
        };
        
        let costs = RentCalculator::calculate_platform_costs(
            positions, proposals, users, markets, &config
        );
        
        let optimization_strategies = Self::optimize_account_layout();
        let total_potential_savings = optimization_strategies.iter()
            .map(|s| s.space_saved)
            .sum::<usize>();
        
        RentReport {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            total_accounts,
            platform_costs: costs,
            optimization_strategies,
            total_potential_space_savings: total_potential_savings,
            recommendations: vec![
                "Enable ZK compression for 10x reduction in rent costs".to_string(),
                "Archive inactive positions after 30 days".to_string(),
                "Use batch account creation to optimize transaction costs".to_string(),
                "Implement account recycling for closed positions".to_string(),
                "Consider off-chain storage for historical data".to_string(),
            ],
        }
    }
}

// Analysis structures
#[derive(Debug)]
pub struct RentAnalysis {
    pub account_type: String,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub uncompressed_rent_sol: f64,
    pub compressed_rent_sol: f64,
    pub compression_ratio: f32,
    pub annual_cost_sol: f64,
}

#[derive(Debug)]
pub struct PlatformRentCosts {
    pub positions: CostBreakdown,
    pub proposals: CostBreakdown,
    pub users: CostBreakdown,
    pub markets: CostBreakdown,
    pub total_cost_sol: f64,
    pub annual_cost_sol: f64,
    pub compression_enabled: bool,
    pub potential_savings: f64,
}

#[derive(Debug)]
pub struct CostBreakdown {
    pub count: u64,
    pub cost_per_account: f64,
    pub total_cost: f64,
    pub percentage: f64,
}

#[derive(Debug)]
pub struct OptimizationStrategy {
    pub name: String,
    pub description: String,
    pub space_saved: usize,
    pub implementation_complexity: Complexity,
}

#[derive(Debug)]
pub enum Complexity {
    Low,
    Medium,
    High,
}

#[derive(Debug)]
pub struct BatchingRecommendation {
    pub account_type: String,
    pub cost_per_account: f64,
    pub max_accounts_possible: u32,
    pub recommended_batch_size: u32,
    pub total_cost: f64,
}

#[derive(Debug)]
pub struct RentReport {
    pub timestamp: u64,
    pub total_accounts: u64,
    pub platform_costs: PlatformRentCosts,
    pub optimization_strategies: Vec<OptimizationStrategy>,
    pub total_potential_space_savings: usize,
    pub recommendations: Vec<String>,
}

// Compressed account structures from compression module
use crate::compression::{CompressedPosition, CompressedProposal};

/// Example usage and cost calculations
pub fn print_rent_analysis() {
    msg!("=== Solana Rent Cost Analysis ===");
    
    // Single account analysis
    let position_analysis = RentCalculator::position_account_rent();
    msg!("Position Account:");
    msg!("  Size: {} bytes (compressed: {} bytes)", 
        position_analysis.uncompressed_size,
        position_analysis.compressed_size
    );
    msg!("  Rent: {:.4} SOL (compressed: {:.4} SOL)",
        position_analysis.uncompressed_rent_sol,
        position_analysis.compressed_rent_sol
    );
    msg!("  Compression ratio: {:.1}x", position_analysis.compression_ratio);
    
    // Platform-wide analysis
    let platform_costs = RentCalculator::calculate_platform_costs(
        100_000,  // positions
        1_000,    // proposals
        10_000,   // users
        21_000,   // markets
        &RentOptimizationConfig::default(),
    );
    
    msg!("\nPlatform Costs (with compression):");
    msg!("  Total: {:.2} SOL", platform_costs.total_cost_sol);
    msg!("  Annual: {:.2} SOL", platform_costs.annual_cost_sol);
    msg!("  Potential savings: {:.2} SOL", platform_costs.potential_savings);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rent_calculations() {
        let size = 1000;
        let annual_rent = RentCalculator::calculate_annual_rent(size);
        assert_eq!(annual_rent, 3_480_000); // 1000 bytes * 3480 lamports
        
        let rent_exempt = RentCalculator::calculate_rent_exempt(size);
        assert!(rent_exempt > annual_rent * 2);
    }
    
    #[test]
    fn test_compression_savings() {
        let position_rent = RentCalculator::position_account_rent();
        assert!(position_rent.compression_ratio > 5.0);
        
        let savings_percent = (1.0 - (position_rent.compressed_rent_sol / 
                                      position_rent.uncompressed_rent_sol)) * 100.0;
        assert!(savings_percent > 80.0); // Should save >80% with compression
    }
}