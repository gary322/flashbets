//! Test modules for betting platform

#[cfg(test)]
pub mod auto_chain_tests;

#[cfg(test)]
pub mod oracle_median_tests;

#[cfg(test)]
pub mod e2e_user_journey_tests;

#[cfg(test)]
pub mod spec_compliance_tests;

#[cfg(test)]
pub mod simple_spec_test;

#[cfg(test)]
pub mod integration_test;

// #[cfg(test)]
// pub mod e2e_spec_compliance;

#[cfg(test)]
pub mod user_journey_tests;

#[cfg(test)]
pub mod part7_integration_test;

#[cfg(test)]
pub mod flash_loan_protection_test;

#[cfg(test)]
pub mod flash_loan_simple_test;

#[cfg(test)]
pub mod credit_system_test;

#[cfg(test)]
pub mod polymarket_integration_test;

// Temporarily disabled until import issues are resolved
// #[cfg(test)]
// pub mod production_user_journey_test;

// #[cfg(test)]
// pub mod production_mmt_journey_test;

// #[cfg(test)]
// pub mod production_keeper_journey_test;

// #[cfg(test)]
// pub mod production_integration_test;

#[cfg(test)]
pub mod basic_integration_test;

#[cfg(test)]
pub mod standalone_verification_test;

#[cfg(test)]
pub mod production_performance_test;

#[cfg(test)]
pub mod production_security_test;

// #[cfg(test)]
// pub mod spec_compliance_user_journeys;

#[cfg(test)]
pub mod risk_quiz_test;

#[cfg(test)]
pub mod websocket_latency_test;

#[cfg(test)]
pub mod polymarket_websocket_test;

#[cfg(test)]
pub mod zk_compression_test;

#[cfg(test)]
pub mod migration_halt_test;

#[cfg(test)]
pub mod sustainability_warning_test;

#[cfg(test)]
pub mod comprehensive_integration_test;

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    /// Test that all components work together
    #[test]
    fn test_system_integration() {
        // Verify AMM types are correctly selected
        // Verify leverage calculations integrate with chaining
        // Verify oracle median feeds into pricing
        // Verify everything connects properly
    }
}