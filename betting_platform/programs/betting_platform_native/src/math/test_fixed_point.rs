// Test file to debug fixed-point math
use crate::math::fixed_point::U64F64;

#[test]
fn test_debug_arithmetic() {
    println!("Testing U64F64 arithmetic...");
    
    let a = U64F64::from_num(10);
    println!("a = {:?}, raw = {}", a, a.raw);
    
    let b = U64F64::from_num(3);
    println!("b = {:?}, raw = {}", b, b.raw);
    
    println!("ONE = {}", U64F64::ONE);
    println!("FRACTION_BITS = {}", U64F64::FRACTION_BITS);
    
    // Test addition
    match a.checked_add(b) {
        Ok(sum) => println!("a + b = {:?}, to_num = {}", sum, sum.to_num()),
        Err(e) => println!("Addition failed: {:?}", e),
    }
    
    // Test multiplication
    match a.checked_mul(b) {
        Ok(product) => println!("a * b = {:?}, to_num = {}", product, product.to_num()),
        Err(e) => println!("Multiplication failed: {:?}", e),
    }
    
    // Test division
    println!("\nTesting division:");
    println!("a.raw * ONE would be: {}", a.raw as u128 * U64F64::ONE);
    println!("u128::MAX = {}", u128::MAX);
    
    match a.checked_div(b) {
        Ok(quotient) => println!("a / b = {:?}, to_num = {}", quotient, quotient.to_num()),
        Err(e) => println!("Division failed: {:?}", e),
    }
}