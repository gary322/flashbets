use betting_platform_native::amm::lmsr::optimized_math::fast_exp_lookup;

#[test]
fn test_exp_debug() {
    println!("Testing fast_exp_lookup...");
    
    let val0 = fast_exp_lookup(0).unwrap();
    println!("fast_exp_lookup(0) = {}", val0);
    
    let val1000 = fast_exp_lookup(1000).unwrap();
    println!("fast_exp_lookup(1000) = {}", val1000);
    
    // Expected: e^0 = 1 (scaled by 1000) = 1000
    // Expected: e^1 ≈ 2.718 (scaled by 1000) ≈ 2718
}