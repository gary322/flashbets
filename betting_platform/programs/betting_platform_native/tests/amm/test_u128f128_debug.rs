use betting_platform_native::math::U128F128;

#[test]
fn test_u128f128_debug() {
    println!("Testing U128F128...");
    
    let one = U128F128::from_num(1u128);
    println!("U128F128::from_num(1) = {:?}", one);
    
    let thousand = U128F128::from_num(1000u128);
    println!("U128F128::from_num(1000) = {:?}", thousand);
    
    // Try division
    match one.checked_div(thousand) {
        Some(result) => println!("1 / 1000 = {:?}, to_num = {}", result, result.to_num()),
        None => println!("Division failed!"),
    }
    
    // Test multiplication
    println!("\nTesting multiplication:");
    let a = U128F128::from_num(1000u128);
    let b = U128F128::from_num(2000u128);
    let c = U128F128::from_num(3000u128);
    
    match a.checked_mul(b) {
        Some(result) => {
            println!("1000 * 2000 = {:?}", result);
            println!("to_num = {}", result.to_num());
            
            // Try multiplying by c
            match result.checked_mul(c) {
                Some(final_result) => {
                    println!("(1000 * 2000) * 3000 = {:?}", final_result);
                    println!("to_num = {}", final_result.to_num());
                },
                None => println!("Second multiplication failed!"),
            }
        },
        None => println!("First multiplication failed!"),
    }
}