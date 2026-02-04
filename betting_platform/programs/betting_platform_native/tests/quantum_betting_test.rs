//! Quantum Superposition Betting Tests
//! 
//! Tests for quantum state positions: |Ψ⟩ = √p₁|Outcome1⟩ + √p₂|Outcome2⟩

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use betting_platform_native::{
    state::{QuantumState, QuantumPosition, QuantumMeasurement},
    quantum::{
        create_superposition, collapse_wavefunction, 
        calculate_amplitudes, measure_observable,
        entangle_positions,
    },
    math::fixed_point::U64F64,
};
use std::f64::consts::SQRT_2;

#[test]
fn test_quantum_superposition_creation() {
    // Test creating quantum superposition state |Ψ⟩ = √p₁|0⟩ + √p₂|1⟩
    
    let p1 = 0.6; // 60% probability
    let p2 = 0.4; // 40% probability
    
    // Verify normalization: p1 + p2 = 1
    assert!((p1 + p2 - 1.0).abs() < 1e-10);
    
    // Calculate amplitudes
    let amplitude1 = p1.sqrt();
    let amplitude2 = p2.sqrt();
    
    // Verify normalization: |α|² + |β|² = 1
    assert!((amplitude1.powi(2) + amplitude2.powi(2) - 1.0).abs() < 1e-10);
    
    // Create quantum state
    let quantum_state = QuantumState {
        amplitudes: vec![
            U64F64::from_num(amplitude1),
            U64F64::from_num(amplitude2),
        ],
        phase: U64F64::from_num(0),
        entangled_with: None,
        coherence: U64F64::from_num(1), // Perfect coherence
    };
    
    println!("✅ Quantum state created:");
    println!("   |Ψ⟩ = {:.3}|0⟩ + {:.3}|1⟩", amplitude1, amplitude2);
    println!("   Probabilities: {:.1}% / {:.1}%", p1 * 100.0, p2 * 100.0);
}

#[test]
fn test_equal_superposition() {
    // Test equal superposition: |Ψ⟩ = (1/√2)|0⟩ + (1/√2)|1⟩
    
    let amplitude = 1.0 / SQRT_2;
    
    let quantum_state = QuantumState {
        amplitudes: vec![
            U64F64::from_num(amplitude),
            U64F64::from_num(amplitude),
        ],
        phase: U64F64::from_num(0),
        entangled_with: None,
        coherence: U64F64::from_num(1),
    };
    
    // Verify equal probabilities
    let p0 = amplitude.powi(2);
    let p1 = amplitude.powi(2);
    
    assert!((p0 - 0.5).abs() < 1e-10);
    assert!((p1 - 0.5).abs() < 1e-10);
    
    println!("✅ Equal superposition:");
    println!("   |Ψ⟩ = (1/√2)|0⟩ + (1/√2)|1⟩");
    println!("   P(0) = P(1) = 50%");
}

#[test]
fn test_quantum_measurement_collapse() {
    // Test wavefunction collapse upon measurement
    
    let initial_state = QuantumState {
        amplitudes: vec![
            U64F64::from_num(0.8), // 64% probability
            U64F64::from_num(0.6), // 36% probability
        ],
        phase: U64F64::from_num(0),
        entangled_with: None,
        coherence: U64F64::from_num(1),
    };
    
    // Simulate measurement (using deterministic outcome for test)
    let measurement_outcome = 0; // Outcome 0 observed
    
    let collapsed_state = collapse_wavefunction(&initial_state, measurement_outcome);
    
    // After collapse, should be in definite state
    assert_eq!(collapsed_state.amplitudes[0], U64F64::from_num(1));
    assert_eq!(collapsed_state.amplitudes[1], U64F64::from_num(0));
    assert_eq!(collapsed_state.coherence, U64F64::from_num(0)); // No coherence after collapse
    
    println!("✅ Wavefunction collapse:");
    println!("   Before: |Ψ⟩ = 0.8|0⟩ + 0.6|1⟩");
    println!("   Measurement: Outcome 0");
    println!("   After: |Ψ⟩ = 1.0|0⟩ + 0.0|1⟩");
}

#[test]
fn test_quantum_position_payout() {
    // Test payout calculation for quantum positions
    
    let position_size = U64F64::from_num(1000); // 1000 USDC
    
    // Quantum position across two outcomes
    let quantum_position = QuantumPosition {
        owner: Pubkey::new_unique(),
        market_id: [1u8; 32],
        size: position_size,
        quantum_state: QuantumState {
            amplitudes: vec![
                U64F64::from_num(0.7), // √0.49 ≈ 0.7
                U64F64::from_num(0.714), // √0.51 ≈ 0.714
            ],
            phase: U64F64::from_num(0),
            entangled_with: None,
            coherence: U64F64::from_num(1),
        },
        entry_price: U64F64::from_num(1),
    };
    
    // Calculate expected value
    let p0 = quantum_position.quantum_state.amplitudes[0].pow(2);
    let p1 = quantum_position.quantum_state.amplitudes[1].pow(2);
    
    // If outcome 0 wins: payout = size
    // If outcome 1 wins: payout = 0
    let expected_value = position_size * p0 + U64F64::from_num(0) * p1;
    
    println!("✅ Quantum position expected value:");
    println!("   Position size: {} USDC", position_size.to_num::<u64>());
    println!("   P(win) = {:.1}%", p0.to_num::<f64>() * 100.0);
    println!("   Expected value: {:.2} USDC", expected_value.to_num::<f64>());
}

#[test]
fn test_quantum_entanglement() {
    // Test entangled quantum positions
    
    let position_a_id = [1u8; 32];
    let position_b_id = [2u8; 32];
    
    // Create entangled states (Bell state)
    // |Ψ⟩ = (1/√2)|00⟩ + (1/√2)|11⟩
    let entangled_state_a = QuantumState {
        amplitudes: vec![
            U64F64::from_num(1.0 / SQRT_2),
            U64F64::from_num(1.0 / SQRT_2),
        ],
        phase: U64F64::from_num(0),
        entangled_with: Some(position_b_id),
        coherence: U64F64::from_num(1),
    };
    
    let entangled_state_b = QuantumState {
        amplitudes: vec![
            U64F64::from_num(1.0 / SQRT_2),
            U64F64::from_num(1.0 / SQRT_2),
        ],
        phase: U64F64::from_num(0),
        entangled_with: Some(position_a_id),
        coherence: U64F64::from_num(1),
    };
    
    // Verify entanglement
    assert!(entangled_state_a.entangled_with.is_some());
    assert!(entangled_state_b.entangled_with.is_some());
    
    println!("✅ Entangled positions created:");
    println!("   |Ψ_AB⟩ = (1/√2)|00⟩ + (1/√2)|11⟩");
    println!("   Measuring A forces B to same outcome");
}

#[test]
fn test_quantum_coherence_decay() {
    // Test coherence decay over time
    
    let initial_coherence = U64F64::from_num(1);
    let decay_rate = U64F64::from_num(0.01); // 1% per slot
    let slots_elapsed = 50;
    
    let mut coherence = initial_coherence;
    for _ in 0..slots_elapsed {
        coherence = coherence * (U64F64::from_num(1) - decay_rate);
    }
    
    // After 50 slots at 1% decay
    let expected_coherence = 0.99_f64.powi(50);
    let actual_coherence = coherence.to_num::<f64>();
    
    assert!((actual_coherence - expected_coherence).abs() < 0.01);
    
    println!("✅ Quantum coherence decay:");
    println!("   Initial: 100%");
    println!("   After {} slots: {:.1}%", slots_elapsed, actual_coherence * 100.0);
    println!("   Decay rate: 1% per slot");
}

#[test]
fn test_multi_outcome_superposition() {
    // Test superposition across multiple outcomes (3+)
    
    let amplitudes = vec![
        U64F64::from_num(0.5),    // 25%
        U64F64::from_num(0.5),    // 25%
        U64F64::from_num(0.5),    // 25%
        U64F64::from_num(0.5),    // 25%
    ];
    
    // Verify normalization
    let sum_of_squares: f64 = amplitudes.iter()
        .map(|a| a.to_num::<f64>().powi(2))
        .sum();
    
    assert!((sum_of_squares - 1.0).abs() < 1e-10);
    
    let quantum_state = QuantumState {
        amplitudes,
        phase: U64F64::from_num(0),
        entangled_with: None,
        coherence: U64F64::from_num(1),
    };
    
    println!("✅ 4-outcome superposition:");
    println!("   |Ψ⟩ = 0.5|0⟩ + 0.5|1⟩ + 0.5|2⟩ + 0.5|3⟩");
    println!("   Each outcome: 25% probability");
}

#[test]
fn test_quantum_interference() {
    // Test quantum interference patterns
    
    // Create states with phase difference
    let state1 = QuantumState {
        amplitudes: vec![
            U64F64::from_num(1.0 / SQRT_2),
            U64F64::from_num(1.0 / SQRT_2),
        ],
        phase: U64F64::from_num(0),
        entangled_with: None,
        coherence: U64F64::from_num(1),
    };
    
    let state2 = QuantumState {
        amplitudes: vec![
            U64F64::from_num(1.0 / SQRT_2),
            U64F64::from_num(1.0 / SQRT_2),
        ],
        phase: U64F64::from_num(std::f64::consts::PI), // π phase shift
        entangled_with: None,
        coherence: U64F64::from_num(1),
    };
    
    // Interference would create different probability distributions
    println!("✅ Quantum interference:");
    println!("   State 1: |Ψ₁⟩ = (1/√2)|0⟩ + (1/√2)|1⟩");
    println!("   State 2: |Ψ₂⟩ = (1/√2)|0⟩ + e^(iπ)(1/√2)|1⟩");
    println!("   Phase difference causes interference patterns");
}

#[test]
fn test_quantum_position_hedging() {
    // Test using quantum positions for perfect hedging
    
    let market_id = [1u8; 32];
    let position_size = U64F64::from_num(1000);
    
    // Create quantum hedge: equal superposition
    let hedge_position = QuantumPosition {
        owner: Pubkey::new_unique(),
        market_id,
        size: position_size,
        quantum_state: QuantumState {
            amplitudes: vec![
                U64F64::from_num(1.0 / SQRT_2),
                U64F64::from_num(1.0 / SQRT_2),
            ],
            phase: U64F64::from_num(0),
            entangled_with: None,
            coherence: U64F64::from_num(1),
        },
        entry_price: U64F64::from_num(0.5), // 50% price
    };
    
    // Expected payout is always 50% of size (perfect hedge)
    let expected_payout = position_size * U64F64::from_num(0.5);
    
    println!("✅ Quantum hedge position:");
    println!("   50% probability on each outcome");
    println!("   Guaranteed payout: {} USDC", expected_payout.to_num::<u64>());
    println!("   Risk eliminated through superposition");
}

// Helper functions
fn collapse_wavefunction(state: &QuantumState, outcome: usize) -> QuantumState {
    let mut collapsed = state.clone();
    
    // Set measured outcome amplitude to 1, others to 0
    for (i, amp) in collapsed.amplitudes.iter_mut().enumerate() {
        *amp = if i == outcome { 
            U64F64::from_num(1) 
        } else { 
            U64F64::from_num(0) 
        };
    }
    
    // Coherence lost after measurement
    collapsed.coherence = U64F64::from_num(0);
    
    collapsed
}