#!/usr/bin/env rust-script
//! Test AMM Auto-Selection Logic
//! 
//! Verifies that AMM type is selected correctly based on specification:
//! - N=1 → LMSR
//! - N=2 → PM-AMM
//! - N>2 → PM-AMM or L2 based on conditions

use std::fmt;

#[derive(Debug, PartialEq, Copy, Clone)]
enum AMMType {
    LMSR,
    PMAMM,
    L2AMM,
    Hybrid,
}

impl fmt::Display for AMMType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AMMType::LMSR => write!(f, "LMSR"),
            AMMType::PMAMM => write!(f, "PM-AMM"),
            AMMType::L2AMM => write!(f, "L2-norm"),
            AMMType::Hybrid => write!(f, "Hybrid"),
        }
    }
}

#[derive(Debug, PartialEq)]
struct ProgramError(&'static str);

/// Automatically select AMM type based on number of outcomes
fn select_amm_type(
    outcome_count: u8,
    outcome_type: Option<&str>,
    _expiry_time: Option<i64>,
    _current_time: i64,
) -> Result<AMMType, ProgramError> {
    println!("Selecting AMM type for {} outcomes, type: {:?}", outcome_count, outcome_type);
    
    match outcome_count {
        0 => {
            println!("Invalid outcome count: 0");
            Err(ProgramError("InvalidOutcomeCount"))
        },
        1 => {
            println!("Selected LMSR for single outcome");
            Ok(AMMType::LMSR)
        },
        2 => {
            println!("Selected PM-AMM for binary outcome");
            Ok(AMMType::PMAMM)
        },
        3..=64 => {
            // Check if this is a continuous outcome type
            if let Some(otype) = outcome_type {
                if otype == "range" || otype == "continuous" || otype == "distribution" {
                    println!("Selected L2-norm AMM for continuous outcome type");
                    return Ok(AMMType::L2AMM);
                }
            }
            
            // Per specification: 2≤N≤64 → PM-AMM
            println!("Selected PM-AMM for {} outcomes", outcome_count);
            Ok(AMMType::PMAMM)
        },
        65..=100 => {
            // Per specification: continuous → L2
            println!("Selected L2-norm AMM for {} outcomes (>64)", outcome_count);
            Ok(AMMType::L2AMM)
        },
        _ => {
            println!("Too many outcomes: {}", outcome_count);
            Err(ProgramError("TooManyOutcomes"))
        }
    }
}

fn test_basic_selection() {
    println!("\n=== Testing Basic AMM Selection Rules ===");
    
    let test_cases = vec![
        (1, AMMType::LMSR, "Single outcome"),
        (2, AMMType::PMAMM, "Binary outcome"),
        (3, AMMType::PMAMM, "3 outcomes"),
        (5, AMMType::PMAMM, "5 outcomes"),
        (10, AMMType::PMAMM, "10 outcomes"),
        (20, AMMType::PMAMM, "20 outcomes"),
        (64, AMMType::PMAMM, "64 outcomes (max for PM-AMM)"),
        (65, AMMType::L2AMM, "65 outcomes (switches to L2)"),
        (80, AMMType::L2AMM, "80 outcomes"),
        (100, AMMType::L2AMM, "100 outcomes"),
    ];
    
    for (outcomes, expected, description) in test_cases {
        let result = select_amm_type(outcomes, None, None, 0).unwrap();
        assert_eq!(result, expected);
        println!("✓ {}: {} -> {}", description, outcomes, expected);
    }
}

fn test_continuous_types() {
    println!("\n=== Testing Continuous Outcome Types ===");
    
    let continuous_types = vec!["range", "continuous", "distribution"];
    
    for outcome_type in continuous_types {
        println!("\nTesting outcome type: {}", outcome_type);
        
        // Even with low outcome counts, continuous types should use L2
        for count in vec![3, 5, 10, 20, 50] {
            let result = select_amm_type(count, Some(outcome_type), None, 0).unwrap();
            assert_eq!(result, AMMType::L2AMM);
            println!("  ✓ {} outcomes with '{}' type -> L2-norm", count, outcome_type);
        }
    }
}

fn test_edge_cases() {
    println!("\n=== Testing Edge Cases ===");
    
    // Test invalid outcome counts
    let result = select_amm_type(0, None, None, 0);
    assert_eq!(result, Err(ProgramError("InvalidOutcomeCount")));
    println!("✓ 0 outcomes correctly rejected");
    
    let result = select_amm_type(101, None, None, 0);
    assert_eq!(result, Err(ProgramError("TooManyOutcomes")));
    println!("✓ 101 outcomes correctly rejected");
    
    let result = select_amm_type(255, None, None, 0);
    assert_eq!(result, Err(ProgramError("TooManyOutcomes")));
    println!("✓ 255 outcomes correctly rejected");
}

fn test_boundary_conditions() {
    println!("\n=== Testing Boundary Conditions ===");
    
    // Test boundaries between AMM types
    let boundaries = vec![
        (1, AMMType::LMSR, "LMSR boundary"),
        (2, AMMType::PMAMM, "LMSR/PM-AMM boundary"),
        (64, AMMType::PMAMM, "PM-AMM upper boundary"),
        (65, AMMType::L2AMM, "PM-AMM/L2 boundary"),
        (100, AMMType::L2AMM, "L2 upper boundary"),
    ];
    
    for (count, expected, description) in boundaries {
        let result = select_amm_type(count, None, None, 0).unwrap();
        assert_eq!(result, expected);
        println!("✓ {}: {} outcomes -> {}", description, count, expected);
    }
}

fn test_real_world_scenarios() {
    println!("\n=== Testing Real-World Market Scenarios ===");
    
    struct Scenario {
        name: &'static str,
        outcomes: u8,
        outcome_type: Option<&'static str>,
        expected: AMMType,
    }
    
    let scenarios = vec![
        Scenario {
            name: "Yes/No election outcome",
            outcomes: 2,
            outcome_type: None,
            expected: AMMType::PMAMM,
        },
        Scenario {
            name: "Sports match winner (3 outcomes: Win/Draw/Loss)",
            outcomes: 3,
            outcome_type: None,
            expected: AMMType::PMAMM,
        },
        Scenario {
            name: "Temperature range prediction",
            outcomes: 10,
            outcome_type: Some("range"),
            expected: AMMType::L2AMM,
        },
        Scenario {
            name: "Stock price buckets",
            outcomes: 20,
            outcome_type: Some("continuous"),
            expected: AMMType::L2AMM,
        },
        Scenario {
            name: "Presidential primary (8 candidates)",
            outcomes: 8,
            outcome_type: None,
            expected: AMMType::PMAMM,
        },
        Scenario {
            name: "Rainfall distribution",
            outcomes: 50,
            outcome_type: Some("distribution"),
            expected: AMMType::L2AMM,
        },
    ];
    
    for scenario in scenarios {
        let result = select_amm_type(
            scenario.outcomes, 
            scenario.outcome_type, 
            None, 
            0
        ).unwrap();
        assert_eq!(result, scenario.expected);
        println!("✓ {}: {} outcomes -> {}", 
            scenario.name, scenario.outcomes, scenario.expected);
    }
}

fn main() {
    println!("AMM Auto-Selection Test Suite");
    println!("=============================");
    println!("\nSpecification Rules:");
    println!("- N=1 → LMSR");
    println!("- N=2 → PM-AMM");
    println!("- 3≤N≤64 → PM-AMM (unless continuous)");
    println!("- N>64 → L2-norm");
    println!("- Continuous types → L2-norm");
    
    test_basic_selection();
    test_continuous_types();
    test_edge_cases();
    test_boundary_conditions();
    test_real_world_scenarios();
    
    println!("\n✅ All AMM auto-selection tests passed!");
    println!("\nSummary:");
    println!("- Basic selection rules working correctly");
    println!("- Continuous outcome types properly detected");
    println!("- Edge cases handled appropriately");
    println!("- Boundary conditions validated");
    println!("- Real-world scenarios tested");
}