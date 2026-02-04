//! Betting Platform - Native Solana Program
//! 
//! A fully-featured prediction market platform implemented as a native Solana program.
//! This is a production-grade migration from Anchor framework to native Solana.

pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod math;
pub mod events;
pub mod validation;
pub mod constants;

// Core modules
pub mod account_validation;
pub mod pda;

// Feature modules
pub mod amm;
pub mod synthetics;
pub mod priority;
pub mod trading;
pub mod advanced_orders;
pub mod keeper_network;
pub mod chain_execution;
pub mod liquidation;
pub mod resolution;
pub mod safety;
pub mod fees;
pub mod coverage;
pub mod protection;
pub mod verse;
pub mod mmt;
pub mod dark_pool;
pub mod economics;
pub mod collapse;
pub mod credits;
pub mod oracle;
pub mod anti_mev;
pub mod risk;
pub mod privacy;
pub mod portfolio;
pub mod margin;
pub mod api;
pub mod demo;
pub mod risk_warnings;
pub mod ux;

// Monitoring and recovery
pub mod monitoring;
pub mod recovery;

// Bootstrap phase
pub mod bootstrap;

// Security modules
pub mod security;

// Migration framework
pub mod migration;

// Phase 20: Integration
pub mod integration;

// Part 7: Fee system modules are already declared above

// Performance and simulations
pub mod simulations;
pub mod sharding;
pub mod optimization;
pub mod ingestion;

// Analytics and metrics
pub mod analytics;

// State management
pub mod merkle;
pub mod state_traversal;
pub mod verse_classification;
pub mod state_compression;
pub mod compression;
pub mod state_pruning;
pub mod market_ingestion;
pub mod market_hierarchy;

// Keeper systems
pub mod keeper_liquidation;
pub mod keeper_stop_loss;
pub mod keeper_price_update;

// Tests
#[cfg(test)]
pub mod tests;
pub mod keeper_coordination;
pub mod keeper_registration;
pub mod keeper_ingestor;

// Performance metrics
// pub mod metrics; // Temporarily disabled due to compilation issues
pub mod performance;

// User journeys
pub mod user_journeys;

// Error handling and recovery
pub mod error_handling;

// Edge case testing
#[cfg(test)]
pub mod edge_cases;

// Integration testing
#[cfg(test)]
pub mod integration_tests;

// Security audit
#[cfg(test)]
pub mod security_audit;

// Cross-Program Invocation layer
pub mod cpi;
pub mod circuit_breaker;
pub mod attack_detection;


// Re-exports for convenience
pub use solana_program;
pub use error::BettingPlatformError;
pub use instruction::BettingPlatformInstruction;
pub use processor::process_instruction;

// Program ID - same as Anchor version for compatibility
solana_program::declare_id!("Hr6kfa5dvGU8sHQ9qNpFXkkJQmUSzjSZxdZ9BGRPPSa4");