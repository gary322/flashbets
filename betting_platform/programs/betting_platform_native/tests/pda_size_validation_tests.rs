// Tests for PDA Size Validation

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    state::{
        accounts::*,
        pda_size_validation::*,
    },
};
use borsh::BorshSerialize;

#[test]
fn test_verse_pda_size_validation() {
    // Test that VersePDA meets 83 byte requirement
    let verse = OptimizedVersePDA {
        discriminator: [1; 8],
        verse_id: 12345678,
        parent_id: 87654321,
        children_root: [0; 16],
        packed_data: OptimizedVersePDA::pack_status_depth_count(1, 5, 100),
        last_update_slot_slot: 1000,
        total_oi: 1_000_000,
        derived_prob_bp: 5000, // 50%
        correlation_bp: 500,   // 5%
        bump: 255,
        _reserved: [0; 8],
    };
    
    let serialized = verse.try_to_vec().unwrap();
    assert_eq!(serialized.len(), VERSE_PDA_SIZE);
    assert_eq!(serialized.len(), 83);
    
    // Test packing/unpacking
    let (status, depth, child_count) = OptimizedVersePDA::unpack_status_depth_count(verse.packed_data);
    assert_eq!(status, 1);
    assert_eq!(depth, 5);
    assert_eq!(child_count, 100);
}

#[test]
fn test_proposal_pda_size_validation() {
    // Test that ProposalPDA meets 520 byte requirement
    let proposal = OptimizedProposalPDA {
        discriminator: [2; 8],
        proposal_id: [3; 32],
        verse_id: [4; 32],
        market_id: [5; 32],
        packed_config: OptimizedProposalPDA::pack_amm_outcomes(1, 8),
        prices: [5000; 8],
        volumes: [100_000; 8],
        liquidity_depth: 1_000_000,
        state_metadata: 0,
        settle_slot: 2000,
        resolution_data: [0; 73],
        partial_liq_accumulator: 0,
        chain_count: 0,
        chain_data: [0; 177],
    };
    
    let serialized = proposal.try_to_vec().unwrap();
    assert_eq!(serialized.len(), PROPOSAL_PDA_SIZE);
    assert_eq!(serialized.len(), 520);
    
    // Test packing/unpacking
    let (amm_type, outcomes) = OptimizedProposalPDA::unpack_amm_outcomes(proposal.packed_config);
    assert_eq!(amm_type, 1);
    assert_eq!(outcomes, 8);
}

#[test]
fn test_original_pda_sizes() {
    // Verify original PDAs don't meet size requirements
    let original_verse = VersePDA::new(12345, Some(67890), 255);
    let serialized = original_verse.try_to_vec().unwrap();
    
    // Original VersePDA is larger than 83 bytes due to Option<QuantumState> and U64F64 fields
    assert!(serialized.len() > 83);
    
    let original_proposal = ProposalPDA::new([1; 32], [2; 32], 4);
    let serialized = original_proposal.try_to_vec().unwrap();
    
    // Original ProposalPDA uses Vec which adds overhead
    assert!(serialized.len() != 520);
}

#[test]
fn test_account_size_validation_on_create() {
    use solana_sdk::account_info::AccountInfo;
    use std::cell::RefCell;
    use std::rc::Rc;
    
    let key = Pubkey::new_unique();
    let mut lamports = 0;
    let mut data = vec![0u8; 83];
    let owner = Pubkey::new_unique();
    
    let account = AccountInfo {
        key: &key,
        is_signer: false,
        is_writable: true,
        lamports: Rc::new(RefCell::new(&mut lamports)),
        data: Rc::new(RefCell::new(&mut data[..])),
        owner: &owner,
        executable: false,
        rent_epoch: 0,
    };
    
    // Should succeed with correct size
    validate_account_size_on_create(&account, 83).unwrap();
    
    // Should fail with incorrect size
    let result = validate_account_size_on_create(&account, 100);
    assert!(result.is_err());
}

#[test]
fn test_initialize_account_with_size() {
    use solana_sdk::account_info::AccountInfo;
    use std::cell::RefCell;
    use std::rc::Rc;
    
    let key = Pubkey::new_unique();
    let mut lamports = 0;
    let mut data = vec![0u8; 83];
    let owner = Pubkey::new_unique();
    
    let account = AccountInfo {
        key: &key,
        is_signer: false,
        is_writable: true,
        lamports: Rc::new(RefCell::new(&mut lamports)),
        data: Rc::new(RefCell::new(&mut data[..])),
        owner: &owner,
        executable: false,
        rent_epoch: 0,
    };
    
    let verse = OptimizedVersePDA {
        discriminator: [1; 8],
        verse_id: 12345678,
        parent_id: 0,
        children_root: [0; 16],
        packed_data: 0,
        last_update_slot_slot: 0,
        total_oi: 0,
        derived_prob_bp: 0,
        correlation_bp: 0,
        bump: 0,
        _reserved: [0; 8],
    };
    
    // Initialize with exact size
    initialize_account_with_size(&account, &verse, 83).unwrap();
    
    // Verify data was written
    let written_data = account.data.borrow();
    assert_eq!(written_data[0], 1); // First byte of discriminator
}

#[test]
fn test_compact_representation_limits() {
    // Test maximum values for packed fields
    let max_packed = OptimizedVersePDA::pack_status_depth_count(3, 63, 4095);
    let (status, depth, child_count) = OptimizedVersePDA::unpack_status_depth_count(max_packed);
    
    assert_eq!(status, 3);      // 2 bits max = 3
    assert_eq!(depth, 63);      // 6 bits max = 63
    assert_eq!(child_count, 4095); // 12 bits max = 4095
    
    // Test that values beyond limits are truncated
    let overflow_packed = OptimizedVersePDA::pack_status_depth_count(4, 64, 4096);
    let (status, depth, child_count) = OptimizedVersePDA::unpack_status_depth_count(overflow_packed);
    
    assert_eq!(status, 0);      // 4 & 0b11 = 0
    assert_eq!(depth, 0);       // 64 & 0b111111 = 0
    assert_eq!(child_count, 0); // 4096 & 0xFFF = 0
}

#[test]
fn test_resolution_data_packing() {
    let mut resolution_data = [0u8; 73];
    
    // Pack resolution info
    resolution_data[0] = 2; // outcome
    resolution_data[1..9].copy_from_slice(&1234567890i64.to_le_bytes()); // timestamp
    resolution_data[9..73].copy_from_slice(&[0xFF; 64]); // signature
    
    // Unpack and verify
    let outcome = resolution_data[0];
    let timestamp = i64::from_le_bytes(resolution_data[1..9].try_into().unwrap());
    let signature = &resolution_data[9..73];
    
    assert_eq!(outcome, 2);
    assert_eq!(timestamp, 1234567890);
    assert_eq!(signature.len(), 64);
    assert!(signature.iter().all(|&b| b == 0xFF));
}

#[test]
fn test_chain_data_capacity() {
    // Chain data can store up to 11 chain position references (16 bytes each)
    let chain_data_size = 177;
    let position_ref_size = 16;
    let max_positions = chain_data_size / position_ref_size;
    
    assert_eq!(max_positions, 11);
    
    // Test packing position references
    let mut chain_data = [0u8; 177];
    for i in 0..max_positions {
        let start = i * position_ref_size;
        let end = start + position_ref_size;
        chain_data[start..end].copy_from_slice(&[i as u8; 16]);
    }
    
    // Verify unpacking
    for i in 0..max_positions {
        let start = i * position_ref_size;
        let end = start + position_ref_size;
        let position_ref = &chain_data[start..end];
        assert!(position_ref.iter().all(|&b| b == i as u8));
    }
}

#[cfg(test)]
mod size_optimization_tests {
    use super::*;
    
    #[test]
    fn test_size_reduction_techniques() {
        // Original u128 (16 bytes) reduced to u64 (8 bytes)
        assert_eq!(std::mem::size_of::<u128>(), 16);
        assert_eq!(std::mem::size_of::<u64>(), 8);
        
        // Option<u128> (17 bytes) replaced with flag + u64 (9 bytes)
        assert_eq!(std::mem::size_of::<Option<u128>>(), 17);
        
        // Vec overhead eliminated by using fixed arrays
        let vec_overhead = std::mem::size_of::<Vec<u8>>(); // 24 bytes on 64-bit
        assert!(vec_overhead >= 24);
        
        // Bitpacking multiple fields into u32
        assert_eq!(std::mem::size_of::<u8>() * 3 + std::mem::size_of::<u16>(), 5);
        assert_eq!(std::mem::size_of::<u32>(), 4); // Saved 1 byte
    }
}