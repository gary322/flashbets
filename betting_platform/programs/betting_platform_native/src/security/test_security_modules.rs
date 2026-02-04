//! Test Security Modules
//!
//! Simple tests for security modules without external dependencies

#[cfg(test)]
mod tests {
    use super::super::*;
    use solana_program::pubkey::Pubkey;
    
    #[test]
    fn test_reentrancy_guard() {
        use crate::security::reentrancy_guard::{ReentrancyGuard, ReentrancyState};
        
        let lock_authority = Pubkey::new_unique();
        let mut guard = ReentrancyGuard::new(lock_authority);
        
        // Test normal flow
        assert_eq!(guard.state, ReentrancyState::NotEntered);
        assert!(guard.enter(100).is_ok());
        assert_eq!(guard.state, ReentrancyState::Entered);
        
        // Test reentrancy prevention
        assert!(guard.enter(101).is_err());
        
        // Exit
        assert!(guard.exit().is_ok());
        assert_eq!(guard.state, ReentrancyState::NotEntered);
    }
    
    #[test]
    fn test_overflow_protection() {
        use crate::security::overflow_protection::SafeMath;
        
        // Test safe addition
        let a: u64 = 100;
        let b: u64 = 200;
        assert_eq!(a.safe_add(b).unwrap(), 300);
        
        // Test overflow
        let c: u64 = u64::MAX - 50;
        let d: u64 = 100;
        assert!(c.safe_add(d).is_err());
        
        // Test safe multiplication
        assert_eq!(a.safe_mul(b).unwrap(), 20000);
        
        // Test multiplication overflow
        let e: u64 = u64::MAX / 2;
        let f: u64 = 3;
        assert!(e.safe_mul(f).is_err());
    }
    
    #[test]
    fn test_permissions() {
        use crate::security::access_control::Permissions;
        
        let mut perms = Permissions::NONE;
        assert!(!perms.has(Permissions::CREATE_PROPOSAL));
        
        // Add permission
        perms.add(Permissions::CREATE_PROPOSAL);
        assert!(perms.has(Permissions::CREATE_PROPOSAL));
        
        // Add another permission
        perms.add(Permissions::EXECUTE_TRADES);
        assert!(perms.has(Permissions::CREATE_PROPOSAL));
        assert!(perms.has(Permissions::EXECUTE_TRADES));
        
        // Remove permission
        perms.remove(Permissions::CREATE_PROPOSAL);
        assert!(!perms.has(Permissions::CREATE_PROPOSAL));
        assert!(perms.has(Permissions::EXECUTE_TRADES));
        
        // Test combined permissions
        let trader_perms = Permissions::TRADER;
        assert!(trader_perms.has(Permissions::CREATE_PROPOSAL));
        assert!(trader_perms.has(Permissions::EXECUTE_TRADES));
    }
    
    #[test]
    fn test_token_bucket() {
        use crate::security::rate_limiter::TokenBucket;
        
        let mut bucket = TokenBucket::new(10, 10, 2);
        
        // Initial tokens
        assert_eq!(bucket.tokens, 10);
        
        // Consume tokens
        assert!(bucket.try_consume(5).is_ok());
        assert_eq!(bucket.tokens, 5);
        
        // Try to consume more than available
        assert!(bucket.try_consume(6).is_err());
        
        // Refill
        bucket.refill(105); // 5 slots later
        assert_eq!(bucket.tokens, 10); // 5 + (5 * 2) = 15, capped at 10
    }
    
    #[test]
    fn test_sliding_window() {
        use crate::security::rate_limiter::SlidingWindow;
        
        let mut window = SlidingWindow::new(10, 3); // 3 requests per 10 slots
        
        // Add requests
        assert!(window.check_and_add(100).is_ok());
        assert!(window.check_and_add(102).is_ok());
        assert!(window.check_and_add(105).is_ok());
        
        // Hit limit
        assert!(window.check_and_add(107).is_err());
        
        // Old request expires
        assert!(window.check_and_add(111).is_ok()); // Request at 100 is now outside window
    }
    
    #[test]
    fn test_pause_levels() {
        use crate::security::emergency_pause::{EmergencyPause, PauseLevel, OperationCategory};
        
        let authority = Pubkey::new_unique();
        let mut pause = EmergencyPause::new(authority, authority);
        
        // Normal operation
        assert!(pause.is_operation_allowed(OperationCategory::Trading, 100).is_ok());
        
        // Partial pause
        assert!(pause.trigger_pause(
            PauseLevel::Partial,
            "Test pause",
            0,
            &authority
        ).is_ok());
        
        // Trading blocked, emergency allowed
        assert!(pause.is_operation_allowed(OperationCategory::Trading, 100).is_err());
        assert!(pause.is_operation_allowed(OperationCategory::Emergency, 100).is_ok());
        
        // Unpause
        assert!(pause.unpause(&authority).is_ok());
        assert!(pause.is_operation_allowed(OperationCategory::Trading, 100).is_ok());
    }
    
    #[test]
    fn test_security_monitoring() {
        use crate::security::security_monitor::{SecurityMonitor, SecurityEventType, SecurityAction};
        
        let authority = Pubkey::new_unique();
        let mut monitor = SecurityMonitor::new(authority);
        
        // Log low severity event
        let action = monitor.log_event(
            SecurityEventType::SuspiciousTransaction,
            Some(Pubkey::new_unique()),
            b"Test event".to_vec(),
            3,
        ).unwrap();
        assert_eq!(action, SecurityAction::None);
        
        // Log high severity event
        let action = monitor.log_event(
            SecurityEventType::FlashLoanDetected,
            None,
            b"Flash loan detected".to_vec(),
            9,
        ).unwrap();
        assert_eq!(action, SecurityAction::ProtocolPaused);
        
        assert_eq!(monitor.total_events, 2);
    }
    
    #[test]
    fn test_invariant_checker() {
        use crate::security::invariant_checker::{InvariantChecker, InvariantType};
        
        let authority = Pubkey::new_unique();
        let mut checker = InvariantChecker::new(authority);
        
        // Check enabled invariants
        assert!(checker.is_enabled(InvariantType::TVLConsistency));
        assert!(checker.is_enabled(InvariantType::PriceNormalization));
        
        // Disable invariant
        checker.set_invariant(InvariantType::TVLConsistency, false);
        assert!(!checker.is_enabled(InvariantType::TVLConsistency));
        
        // Re-enable
        checker.set_invariant(InvariantType::TVLConsistency, true);
        assert!(checker.is_enabled(InvariantType::TVLConsistency));
    }
}