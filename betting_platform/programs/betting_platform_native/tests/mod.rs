//! Test module for betting platform
//!
//! Comprehensive test suite for specification compliance and functionality

pub mod spec_compliance;
pub mod performance_benchmarks;

// AMM tests
#[cfg(test)]
mod amm {
    mod lmsr_optimized_math_tests;
    mod pmamm_math_tests;
}

// Math tests
#[cfg(test)]
mod math {
    mod fixed_point_tests;
}

// Fees tests
#[cfg(test)]
mod fees {
    mod elastic_fee_tests;
}

// Liquidation tests
#[cfg(test)]
mod liquidation {
    mod helpers_tests;
}

// Trading tests
#[cfg(test)]
mod trading {
    mod multi_collateral_tests;
}

// Re-export test utilities
pub use spec_compliance::helpers;