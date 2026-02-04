//! Bootstrap Module
//!
//! Handles bootstrap phase initialization and management

// Temporarily disabled - depends on integration module
// pub mod handlers;
// pub use handlers::*;

// // Re-export integration bootstrap types
// pub use crate::integration::{
//     BootstrapCoordinator,
//     BootstrapParticipant,
//     BootstrapState,
// };

// Re-export constants for public use
pub use crate::constants::{
    BOOTSTRAP_TARGET_VAULT,
    BOOTSTRAP_FEE_BPS,
    BOOTSTRAP_MMT_MULTIPLIER,
};