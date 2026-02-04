//! Hybrid AMM implementation
//!
//! Allows switching between LMSR, PM-AMM, and L2-AMM based on market conditions

pub mod router;
pub mod conversion;

pub use router::{process_hybrid_trade, select_optimal_amm};
pub use conversion::{convert_amm_type, migrate_liquidity};

use solana_program::program_error::ProgramError;
use crate::state::{ProposalPDA, amm_accounts::AMMType};

/// Calculate price for hybrid AMM based on the underlying AMM type
pub fn calculate_hybrid_price(
    proposal: &ProposalPDA,
    outcome: u8,
) -> Result<u64, ProgramError> {
    // Route to appropriate AMM based on type
    match proposal.amm_type {
        AMMType::LMSR => {
            use crate::amm::lmsr::LMSRAMMContext;
            let context = LMSRAMMContext::from_proposal(proposal)?;
            context.price(outcome)
        }
        AMMType::PMAMM => {
            use crate::amm::pmamm::price_discovery::PMAMMContext;
            let context = PMAMMContext::from_proposal(proposal)?;
            context.current_price(outcome)
        }
        AMMType::L2AMM => {
            use crate::amm::l2amm::types::L2AMMContext;
            let context = L2AMMContext::from_proposal(proposal)?;
            context.calculate_price(outcome)
        }
        AMMType::Hybrid => {
            // For hybrid, use the optimal AMM selection logic
            let optimal_amm = if proposal.outcomes == 2 {
                AMMType::LMSR
            } else if proposal.total_volume < 1_000_000_000 {
                AMMType::LMSR
            } else {
                AMMType::PMAMM
            };
            
            // Recursive call with the selected type
            let mut temp_proposal = proposal.clone();
            temp_proposal.amm_type = optimal_amm;
            calculate_hybrid_price(&temp_proposal, outcome)
        }
    }
}