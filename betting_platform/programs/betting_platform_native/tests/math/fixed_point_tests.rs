use betting_platform_native::math::fixed_point::*;

#[test]
fn test_basic_arithmetic() {
    let a = U64F64::from_num(10);
    let b = U64F64::from_num(3);
    
    let sum = a.checked_add(b).unwrap();
    assert_eq!(sum.to_num(), 13);
    
    let diff = a.checked_sub(b).unwrap();
    assert_eq!(diff.to_num(), 7);
    
    let product = a.checked_mul(b).unwrap();
    assert_eq!(product.to_num(), 30);
    
    let quotient = a.checked_div(b).unwrap();
    assert_eq!(quotient.to_num(), 3);
}

#[test]
fn test_sqrt() {
    let val = U64F64::from_num(16);
    let sqrt = val.sqrt().unwrap();
    assert_eq!(sqrt.to_num(), 4);
    
    let val2 = U64F64::from_num(100);
    let sqrt2 = val2.sqrt().unwrap();
    assert_eq!(sqrt2.to_num(), 10);
}

#[test]
fn test_percentage() {
    let value = 1000;
    let bps = 250; // 2.5%
    
    let result = helpers::calculate_percentage(value, bps).unwrap();
    assert_eq!(result, 25);
}