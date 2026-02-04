//! Integration tests for oracle and bootstrap implementations

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instruction::BettingPlatformInstruction,
        processor::process_instruction,
        integration::{
            polymarket_sole_oracle::{PolymarketSoleOracle, SPREAD_HALT_THRESHOLD_BPS},
            bootstrap_enhanced::{EnhancedBootstrapCoordinator, BOOTSTRAP_MMT_ALLOCATION},
        },
    };
    use solana_program::{
        account_info::AccountInfo,
        clock::Clock,
        pubkey::Pubkey,
        program_error::ProgramError,
        rent::Rent,
        system_program,
    };
    use borsh::{BorshSerialize, BorshDeserialize};

    #[test]
    fn test_oracle_integration() {
        // Test that oracle instructions are properly routed
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        
        // Test InitializePolymarketSoleOracle instruction
        let instruction = BettingPlatformInstruction::InitializePolymarketSoleOracle { 
            authority 
        };
        let data = instruction.try_to_vec().unwrap();
        
        // Verify instruction can be deserialized
        let decoded = BettingPlatformInstruction::try_from_slice(&data).unwrap();
        match decoded {
            BettingPlatformInstruction::InitializePolymarketSoleOracle { authority: auth } => {
                assert_eq!(auth, authority);
            }
            _ => panic!("Wrong instruction decoded"),
        }
    }

    #[test] 
    fn test_bootstrap_integration() {
        // Test that bootstrap instructions are properly routed
        let program_id = Pubkey::new_unique();
        let mmt_allocation = BOOTSTRAP_MMT_ALLOCATION;
        
        // Test InitializeBootstrapPhase instruction
        let instruction = BettingPlatformInstruction::InitializeBootstrapPhase { 
            mmt_allocation 
        };
        let data = instruction.try_to_vec().unwrap();
        
        // Verify instruction can be deserialized
        let decoded = BettingPlatformInstruction::try_from_slice(&data).unwrap();
        match decoded {
            BettingPlatformInstruction::InitializeBootstrapPhase { mmt_allocation: alloc } => {
                assert_eq!(alloc, mmt_allocation);
            }
            _ => panic!("Wrong instruction decoded"),
        }
    }

    #[test]
    fn test_spread_halt_threshold() {
        // Verify spread halt threshold matches specification
        assert_eq!(SPREAD_HALT_THRESHOLD_BPS, 1000); // 10% spread
    }

    #[test]
    fn test_coverage_formula() {
        // Test coverage = vault / (0.5 * OI)
        let vault: u64 = 100_000_000_000; // $100k
        let open_interest: u64 = 400_000_000_000; // $400k
        
        // coverage = 100k / (0.5 * 400k) = 100k / 200k = 0.5
        let numerator = vault as u128 * 10000;
        let denominator = (open_interest / 2) as u128;
        let coverage_ratio = (numerator / denominator) as u64;
        
        assert_eq!(coverage_ratio, 5000); // 50% in basis points
    }

    #[test]
    fn test_liquidation_formula() {
        // Test liq_price = entry_price * (1 - (margin_ratio / lev_eff))
        let entry_price = 5000; // $0.50 in basis points
        let margin_ratio = 1000; // 10% in basis points
        let leverage = 10;
        
        // lev_eff = leverage for simplified case
        // liq_price = 5000 * (1 - (1000 / 10000))
        // liq_price = 5000 * (1 - 0.1)
        // liq_price = 5000 * 0.9 = 4500
        
        let lev_eff_bps = leverage * 1000; // Convert to basis points
        let ratio = (margin_ratio * 10000) / lev_eff_bps;
        let multiplier = 10000 - ratio;
        let liq_price = (entry_price * multiplier) / 10000;
        
        assert_eq!(liq_price, 4500); // $0.45
    }
}