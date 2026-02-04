#!/bin/bash

echo "Testing Enhanced Rate Limiter Implementation..."
echo "============================================="

# Create a simple test file
cat > test_rate_limiter.rs << 'EOF'
use betting_platform_api::security::rate_limiter::*;
use std::net::{IpAddr, Ipv4Addr};
use tokio;

#[tokio::main]
async fn main() {
    println!("Creating enhanced rate limiter...");
    
    let mut config = EnhancedRateLimitConfig::default();
    config.per_ip_rps = 2;
    config.per_ip_burst = 2;
    
    let limiter = EnhancedRateLimiter::new(config).await;
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
    
    println!("Testing rate limits...");
    
    // Test 1: Should allow initial requests
    match limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await {
        Ok(()) => println!("✅ Request 1: Allowed"),
        Err(e) => println!("❌ Request 1: Blocked - {:?}", e),
    }
    
    match limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await {
        Ok(()) => println!("✅ Request 2: Allowed (burst)"),
        Err(e) => println!("❌ Request 2: Blocked - {:?}", e),
    }
    
    // Test 2: Should be rate limited after burst
    match limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await {
        Ok(()) => println!("❌ Request 3: Should be blocked but was allowed"),
        Err(e) => println!("✅ Request 3: Correctly blocked - {:?}", e),
    }
    
    // Test 3: Test different tiers
    let pro_user_id = "pro_user_123";
    match limiter.check_rate_limit(ip, Some(pro_user_id), "/api/test", RateLimitTier::Pro).await {
        Ok(()) => println!("✅ Pro user request: Allowed (higher limits)"),
        Err(e) => println!("❌ Pro user request: Blocked - {:?}", e),
    }
    
    // Test 4: Test endpoint-specific limits
    println!("\nTesting endpoint-specific limits...");
    let login_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    
    for i in 1..=7 {
        match limiter.check_rate_limit(login_ip, None, "/api/auth/login", RateLimitTier::Free).await {
            Ok(()) => println!("✅ Login request {}: Allowed", i),
            Err(e) => println!("✅ Login request {}: Correctly blocked - {:?}", i, e),
        }
    }
    
    println!("\nRate limiter tests completed!");
}
EOF

# Compile and run the test
echo -e "\nCompiling test..."
rustc --edition 2021 test_rate_limiter.rs -L ../target/debug/deps --extern betting_platform_api=../target/debug/libbetting_platform_api.rlib --extern tokio=../target/debug/deps/libtokio-*.rlib

echo -e "\nRunning rate limiter test..."
./test_rate_limiter

# Clean up
rm -f test_rate_limiter test_rate_limiter.rs

echo -e "\nDone!"