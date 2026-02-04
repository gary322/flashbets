//! Tests for state pruning functionality

use solana_program::{
    pubkey::Pubkey,
    clock::Clock,
};
use betting_platform_native::{
    state::{ProposalPDA, ProposalState},
    state_pruning::{StatePruner, PRUNE_GRACE_PERIOD},
};

#[test]
fn test_pruning_eligibility() {
    let mut proposal = ProposalPDA::new([1u8; 32], [0u8; 32], 2);
    proposal.state = ProposalState::Resolved;
    proposal.settle_slot = 1000;
    
    // Not ready immediately after resolution
    assert!(!StatePruner::is_ready_for_pruning(&proposal, 1000));
    
    // Not ready just before grace period
    assert!(!StatePruner::is_ready_for_pruning(&proposal, 1000 + PRUNE_GRACE_PERIOD - 1));
    
    // Ready after grace period
    assert!(StatePruner::is_ready_for_pruning(&proposal, 1000 + PRUNE_GRACE_PERIOD));
    
    // Active proposals should never be pruned
    proposal.state = ProposalState::Active;
    assert!(!StatePruner::is_ready_for_pruning(&proposal, u64::MAX));
}

#[test]
fn test_hex_encoding() {
    use betting_platform_native::state_pruning::hex;
    
    let data = vec![0x12, 0x34, 0xAB, 0xCD];
    let encoded = hex::encode(&data);
    assert_eq!(encoded, "1234abcd");
    
    let empty = vec![];
    assert_eq!(hex::encode(&empty), "");
}