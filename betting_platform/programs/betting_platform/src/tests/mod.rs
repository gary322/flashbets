pub mod leverage_tests;

#[cfg(test)]
pub mod security;

#[cfg(test)]
pub mod economic;

#[cfg(test)]
pub mod integration;

#[cfg(test)]
pub mod attack_detection_tests;

#[cfg(test)]
pub mod circuit_breaker_tests;

#[cfg(test)]
pub mod liquidation_priority_tests;

// Phase 12 test suites
#[cfg(test)]
pub mod merkle_tests;

#[cfg(test)]
pub mod state_compression_tests;

#[cfg(test)]
pub mod keeper_tests;