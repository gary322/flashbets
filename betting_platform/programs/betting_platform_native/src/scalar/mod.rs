//! Unified Scalar Calculation Module
//!
//! Provides consistent pricing, risk assessment, and fee calculations
//! across all platform modules (Oracle, Synthetics, CDP, Perpetual, Vault)

pub mod state;
pub mod calculation;
pub mod integration;
pub mod risk_model;

pub use state::*;
pub use calculation::*;
pub use integration::*;
pub use risk_model::*;

use solana_program::pubkey::Pubkey;

/// Derive PDA for unified scalar state
pub fn derive_scalar_pda(program_id: &Pubkey, market_id: u128) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"scalar",
            &market_id.to_le_bytes(),
        ],
        program_id,
    )
}

/// Derive PDA for risk parameters
pub fn derive_risk_params_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"risk_params"],
        program_id,
    )
}