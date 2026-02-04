//! Security Module
//!
//! Production-grade security features for attack prevention and audit compliance

pub mod reentrancy_guard;
pub mod overflow_protection;
pub mod access_control;
pub mod rate_limiter;
pub mod signature_verifier;
pub mod security_monitor;
pub mod invariant_checker;
pub mod emergency_pause;
pub mod immutability;

#[cfg(test)]
mod test_security_modules;

pub use reentrancy_guard::*;
pub use overflow_protection::*;
pub use access_control::*;
pub use rate_limiter::*;
pub use signature_verifier::*;
pub use security_monitor::*;
pub use invariant_checker::*;
pub use emergency_pause::*;