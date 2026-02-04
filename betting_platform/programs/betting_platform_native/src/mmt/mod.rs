//! MMT Token Distribution & Staking System
//! 
//! Implements the TWIST token economics with:
//! - 100M total supply (10M current season, 90M locked)
//! - 15% rebate on trading fees for stakers
//! - Maker rewards for spread improvement
//! - Early trader bonuses
//!
//! Native Solana implementation - NO ANCHOR

pub mod constants;
pub mod token;
pub mod staking;
pub mod maker_rewards;
pub mod distribution;
pub mod early_trader;
pub mod instructions;
pub mod state;
pub mod pda_setup;
pub mod security_validation;
pub mod prelaunch_airdrop;
pub mod vesting;

#[cfg(test)]
pub mod test_vesting;

pub use constants::*;
pub use token::*;
pub use staking::*;
pub use maker_rewards::*;
pub use distribution::*;
pub use early_trader::*;
pub use instructions::*;
pub use state::*;
pub use pda_setup::*;
pub use security_validation::*;
pub use prelaunch_airdrop::*;
pub use vesting::*;

// Alias for backward compatibility
pub use staking::calculate_rebate as calculate_rewards;