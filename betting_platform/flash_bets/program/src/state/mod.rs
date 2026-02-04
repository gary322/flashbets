pub mod flash_verse;
pub mod quantum_flash;
pub mod migration_flags;

pub use flash_verse::*;
pub use quantum_flash::*;
pub use migration_flags::*;

use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum ChainAction {
    Borrow,
    Liquidate,
    Stake,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ChainStep {
    pub action: ChainAction,
    pub amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Outcome {
    pub name: String,
    pub probability: f64,
    pub volume: u64,
    pub odds: f64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct QuantumState {
    pub outcome: String,
    pub probability: f64,
    pub amplitude: f64,
    pub phase: f64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum CollapseTrigger {
    TimeExpiry { slot: u64 },
    EventOccurrence { threshold: f64 },
    MaxProbability { value: f64 },
}