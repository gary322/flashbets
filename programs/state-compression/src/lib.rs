use solana_program::entrypoint;

pub mod compression;
pub mod error;
pub mod instructions;
pub mod processor;
pub mod state;

// Program ID
solana_program::declare_id!("CompZK11111111111111111111111111111111111111");

// Export for use in tests and other programs
pub use crate::compression::*;
pub use crate::error::*;
pub use crate::instructions::*;
pub use crate::processor::*;
pub use crate::state::*;

// Program entrypoint
#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);