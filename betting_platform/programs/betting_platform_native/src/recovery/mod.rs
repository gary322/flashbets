//! Disaster recovery module
//!
//! Handles system recovery, checkpoints, and emergency procedures

pub mod disaster;
pub mod checkpoint;

pub use disaster::{DisasterRecoveryState, RecoveryMode, RecoveryManager, EmergencyAction};
pub use checkpoint::{Checkpoint, CheckpointManager, StateSnapshot};