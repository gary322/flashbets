use betting_platform_native::amm::lmsr::optimized_math::*;
use betting_platform_native::math::U64F64;

#[test]
fn test_debug_exponential_function() {
    println!("\n=== LMSR Price Calculation Debug ===");
    
    // Test parameters matching the failing test
    let shares = vec![100u64, 100u64];
    let b_parameter = 1000u64;
    let outcome = 0u8;
    
    println!("Input parameters:");
    println!("  shares: {:?}", shares);
    println!("  b_parameter: {}", b_parameter);
    println!("  outcome: {}", outcome);
    
    // Calculate the exponent for shares[0] / b_parameter
    let exponent0 = shares[0] as f64 / b_parameter as f64;
    let exponent1 = shares[1] as f64 / b_parameter as f64;
    println!("\nExponents:");
    println!("  shares[0]/b = {}/{} = {}", shares[0], b_parameter, exponent0);
    println!("  shares[1]/b = {}/{} = {}", shares[1], b_parameter, exponent1);
    
    // Expected values using standard math
    let exp0 = exponent0.exp();
    let exp1 = exponent1.exp();
    let sum_exp = exp0 + exp1;
    let price0 = exp0 / sum_exp;
    let price1 = exp1 / sum_exp;
    
    println!("\nExpected values (using f64):");
    println!("  exp(0.1) = {}", exp0);
    println!("  exp(0.1) = {}", exp1);
    println!("  sum = {}", sum_exp);
    println!("  price[0] = {:.4} = {:.1}%", price0, price0 * 100.0);
    println!("  price[1] = {:.4} = {:.1}%", price1, price1 * 100.0);
    println!("  price[0] in bps = {}", (price0 * 10000.0) as u64);
    
    // Now test the actual function
    println!("\n=== Testing fast_exp_lookup ===");
    
    // Test exp(0) = 1
    match fast_exp_lookup(0) {
        Ok(val) => println!("fast_exp_lookup(0) = {}", val),
        Err(e) => println!("fast_exp_lookup(0) failed: {:?}", e),
    }
    
    // Test exp(100) which should be exp(0.01) since input is in basis points
    match fast_exp_lookup(100) {
        Ok(val) => println!("fast_exp_lookup(100) = {}", val),
        Err(e) => println!("fast_exp_lookup(100) failed: {:?}", e),
    }
    
    // Test exp(1000) which should be exp(0.1)
    match fast_exp_lookup(1000) {
        Ok(val) => println!("fast_exp_lookup(1000) = {}", val),
        Err(e) => println!("fast_exp_lookup(1000) failed: {:?}", e),
    }
    
    println!("\n=== Testing calculate_price_optimized ===");
    
    match calculate_price_optimized(&shares, outcome, b_parameter) {
        Ok(price) => {
            println!("calculate_price_optimized returned: {} bps", price);
            println!("Expected: ~5000 bps (50%)");
            println!("Difference: {} bps", (price as i64 - 5000).abs());
        }
        Err(e) => {
            println!("calculate_price_optimized failed: {:?}", e);
        }
    }
    
    // Let's also test U64F64 exp function directly
    println!("\n=== Testing U64F64::exp ===");
    
    let x = U64F64::from_fraction(1, 10).unwrap(); // 0.1
    println!("Created U64F64 value for 0.1: raw = {:#x}", x.raw);
    
    match x.exp() {
        Ok(exp_val) => {
            println!("U64F64(0.1).exp() = {} (raw: {:#x})", exp_val, exp_val.raw);
            println!("Expected e^0.1 ≈ 1.105");
        }
        Err(e) => {
            println!("U64F64(0.1).exp() failed: {:?}", e);
        }
    }
}

#[test]
fn test_simple_exponential_approximation() {
    println!("\n=== Testing simple exp approximation ===");
    
    // For small x, e^x ≈ 1 + x + x²/2 + x³/6
    let x = 0.1f64;
    let approx1 = 1.0;
    let approx2 = 1.0 + x;
    let approx3 = 1.0 + x + x*x/2.0;
    let approx4 = 1.0 + x + x*x/2.0 + x*x*x/6.0;
    let exact = x.exp();
    
    println!("Approximations for e^0.1:");
    println!("  1st order: {:.6}", approx1);
    println!("  2nd order: {:.6}", approx2);
    println!("  3rd order: {:.6}", approx3);
    println!("  4th order: {:.6}", approx4);
    println!("  Exact:     {:.6}", exact);
}

#[test]
fn test_manual_lmsr_calculation() {
    println!("\n=== Manual LMSR calculation ===");
    
    // Manually calculate LMSR price without using lookup tables
    let shares = vec![100u64, 100u64];
    let b_parameter = 1000u64;
    
    // Use U64F64 for calculations
    let b_fp = U64F64::from_num(b_parameter);
    
    // Calculate exp(shares[i]/b) for each outcome
    let mut exp_values = Vec::new();
    let mut sum_exp = U64F64::from_num(0);
    
    for (i, &share) in shares.iter().enumerate() {
        let share_fp = U64F64::from_num(share);
        let normalized = share_fp.checked_div(b_fp).unwrap();
        
        println!("Outcome {}: share={}, normalized={}", i, share, normalized);
        
        // Use the exp function from U64F64
        match normalized.exp() {
            Ok(exp_val) => {
                println!("  exp({}) = {}", normalized, exp_val);
                exp_values.push(exp_val);
                sum_exp = sum_exp.checked_add(exp_val).unwrap();
            }
            Err(e) => {
                println!("  exp({}) failed: {:?}", normalized, e);
                exp_values.push(U64F64::from_num(1)); // fallback
                sum_exp = sum_exp.checked_add(U64F64::from_num(1)).unwrap();
            }
        }
    }
    
    println!("\nSum of exponentials: {}", sum_exp);
    
    // Calculate prices
    for (i, &exp_val) in exp_values.iter().enumerate() {
        let price = exp_val.checked_mul(U64F64::from_num(10000))
            .and_then(|p| p.checked_div(sum_exp))
            .unwrap_or(U64F64::from_num(0));
        
        println!("Price[{}] = {} bps", i, price.to_num());
    }
}