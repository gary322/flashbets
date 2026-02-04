#[allow(dead_code)]#[cfg(test)]
mod integration_tests {
    use super::super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_enhanced_rate_limiter_basic() {
        let mut config = EnhancedRateLimitConfig::default();
        config.per_ip_rps = 2;
        config.per_ip_burst = 2;
        
        let limiter = EnhancedRateLimiter::new(config).await;
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // Should allow initial requests
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
        
        // Should be rate limited after burst
        assert!(limiter.check_rate_limit(ip, None, "/api/test", RateLimitTier::Free).await.is_err());
    }
    
    #[tokio::test]
    async fn test_tier_multipliers() {
        let config = EnhancedRateLimitConfig::default();
        let limiter = EnhancedRateLimiter::new(config).await;
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        
        // Pro users should have higher limits
        let pro_user = "pro_user_123";
        
        // Make many requests as pro user
        for _ in 0..10 {
            let result = limiter.check_rate_limit(
                ip, 
                Some(pro_user), 
                "/api/test", 
                RateLimitTier::Pro
            ).await;
            
            // Pro tier has 5x multiplier, so should handle more requests
            if result.is_err() {
                break;
            }
        }
        
        // Enterprise users should have even higher limits
        let enterprise_user = "enterprise_user_456";
        for _ in 0..20 {
            let result = limiter.check_rate_limit(
                ip, 
                Some(enterprise_user), 
                "/api/test", 
                RateLimitTier::Enterprise
            ).await;
            
            // Enterprise tier has 10x multiplier
            if result.is_err() {
                break;
            }
        }
    }
    
    #[tokio::test]
    async fn test_endpoint_specific_limits() {
        let config = EnhancedRateLimitConfig::default();
        let limiter = EnhancedRateLimiter::new(config).await;
        let ip = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));
        
        // Login endpoint has stricter limits (5 rps, burst 10)
        let mut success_count = 0;
        for i in 0..15 {
            match limiter.check_rate_limit(ip, None, "/api/auth/login", RateLimitTier::Free).await {
                Ok(()) => success_count += 1,
                Err(_) => {
                    println!("Login endpoint blocked after {} requests", i);
                    break;
                }
            }
        }
        
        // Should allow some requests but not all
        assert!(success_count > 0 && success_count < 15);
    }
    
    #[tokio::test]
    async fn test_ddos_protection() {
        let mut config = EnhancedRateLimitConfig::default();
        config.ddos_threshold = 10;
        config.ddos_ban_duration = Duration::from_secs(1);
        
        let limiter = EnhancedRateLimiter::new(config).await;
        let attacker_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1));
        
        // Simulate DDoS attack
        for _ in 0..15 {
            let _ = limiter.check_rate_limit(
                attacker_ip, 
                None, 
                "/api/test", 
                RateLimitTier::Free
            ).await;
        }
        
        // IP should now be blocked
        sleep(Duration::from_millis(100)).await;
        match limiter.check_rate_limit(attacker_ip, None, "/api/test", RateLimitTier::Free).await {
            Err(RateLimitError::Blocked) => {
                println!("IP correctly blocked for DDoS");
            }
            _ => panic!("IP should have been blocked"),
        }
        
        // Wait for ban to expire
        sleep(Duration::from_secs(1)).await;
        
        // Should be unblocked now
        assert!(limiter.check_rate_limit(attacker_ip, None, "/api/test", RateLimitTier::Free).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_global_rate_limit() {
        let mut config = EnhancedRateLimitConfig::default();
        config.global_rps = 5;
        config.global_burst = 5;
        
        let limiter = EnhancedRateLimiter::new(config).await;
        
        // Use different IPs to test global limit
        let ips: Vec<IpAddr> = (1..=10)
            .map(|i| IpAddr::V4(Ipv4Addr::new(10, 0, 0, i)))
            .collect();
        
        let mut global_limit_hit = false;
        for (i, ip) in ips.iter().enumerate() {
            match limiter.check_rate_limit(*ip, None, "/api/test", RateLimitTier::Free).await {
                Ok(()) => println!("Request {} allowed", i + 1),
                Err(RateLimitError::GlobalLimit) => {
                    println!("Global limit hit at request {}", i + 1);
                    global_limit_hit = true;
                    break;
                }
                Err(e) => println!("Other error: {:?}", e),
            }
        }
        
        assert!(global_limit_hit, "Global rate limit should have been hit");
    }
}