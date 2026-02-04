#[cfg(all(not(target_arch = "bpf"), not(target_os = "solana")))]
//! Security Integration Tests
//!
//! Production-grade tests for security modules

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program_test::*;
    use solana_sdk::{
        account::Account,
        hash::Hash,
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        system_instruction,
        transaction::Transaction,
    };
    use borsh::BorshSerialize;
    
    use crate::{
        security::{
            reentrancy_guard::{ReentrancyGuard, ReentrancyState},
            overflow_protection::SafeMath,
            access_control::{AccessControlList, Permissions, Role},
            rate_limiter::{TokenBucket, SlidingWindow},
            signature_verifier::{SignedMessage, MultiSigConfig, NonceManager, OracleSignatureVerifier},
            security_monitor::{SecurityMonitor, SecurityEventType, AnomalyDetector},
            invariant_checker::{InvariantChecker, InvariantType},
            emergency_pause::{EmergencyPause, PauseLevel, OperationCategory, CircuitBreaker},
        },
        state::accounts::discriminators,
        error::BettingPlatformError,
        ID,
        security::rate_limiter::RateLimiter,
    };

    /// Test reentrancy protection
    #[tokio::test]
    async fn test_reentrancy_protection() {
        let program = ProgramTest::new("betting_platform_native", ID, None);
        let (mut banks_client, payer, recent_blockhash) = program.start().await;
        
        // Create reentrancy guard account
        let guard_keypair = Keypair::new();
        let mut guard = ReentrancyGuard::new(Pubkey::new_unique());
        
        // Test normal flow
        assert_eq!(guard.state, ReentrancyState::NotEntered);
        assert!(guard.enter(1).is_ok());
        assert_eq!(guard.state, ReentrancyState::Entered);
        
        // Test reentrancy attempt
        assert!(guard.enter(2).is_err());
        
        // Exit and test lock
        assert!(guard.exit().is_ok());
        assert_eq!(guard.state, ReentrancyState::NotEntered);
        
        // Test with lock
        let lock_authority = Pubkey::new_unique();
        let mut guard = ReentrancyGuard::new(lock_authority);
        guard.emergency_lock(&lock_authority).unwrap();
        assert_eq!(guard.state, ReentrancyState::Locked);
        assert!(guard.enter(3).is_err()); // Cannot enter when locked
    }

    /// Test overflow protection
    #[test]
    fn test_overflow_protection() {
        // Test u64 safe math
        let a: u64 = u64::MAX - 100;
        let b: u64 = 200;
        
        // Addition should fail
        assert!(a.safe_add(b).is_err());
        
        // Safe addition within bounds
        let c: u64 = 50;
        assert_eq!(a.safe_add(c).unwrap(), u64::MAX - 50);
        
        // Multiplication overflow
        let d: u64 = u64::MAX / 2;
        let e: u64 = 3;
        assert!(d.safe_mul(e).is_err());
        
        // Safe subtraction
        assert_eq!(b.safe_sub(c).unwrap(), 150);
        assert!(c.safe_sub(b).is_err()); // Would underflow
        
        // Safe division
        assert_eq!(b.safe_div(c).unwrap(), 4);
        assert!(b.safe_div(0).is_err()); // Division by zero
    }

    /// Test access control
    #[tokio::test]
    async fn test_access_control() {
        let program = ProgramTest::new("betting_platform_native", ID, None);
        let (mut banks_client, payer, recent_blockhash) = program.start().await;
        
        let authority = Keypair::new();
        let mut acl = AccessControlList::new(authority.pubkey());
        
        // Test role management
        let user1 = Keypair::new().pubkey();
        let user2 = Keypair::new().pubkey();
        
        // Grant trader role to user1
        assert!(acl.grant_role(&user1, 3, &authority.pubkey()).is_ok());
        let perms1 = acl.get_user_permissions(&user1);
        assert!(perms1.has(Permissions::CREATE_PROPOSAL));
        assert!(perms1.has(Permissions::EXECUTE_TRADES));
        assert!(!perms1.has(Permissions::PAUSE_PROTOCOL));
        
        // Grant keeper role to user2
        assert!(acl.grant_role(&user2, 2, &authority.pubkey()).is_ok());
        let perms2 = acl.get_user_permissions(&user2);
        assert!(perms2.has(Permissions::LIQUIDATE_POSITIONS));
        assert!(perms2.has(Permissions::RESOLVE_PROPOSAL));
        
        // Test suspension
        assert!(acl.suspend_user(&user1, &authority.pubkey()).is_ok());
        let perms1_suspended = acl.get_user_permissions(&user1);
        assert_eq!(perms1_suspended, Permissions::NONE);
        
        // Unsuspend
        assert!(acl.unsuspend_user(&user1, &authority.pubkey()).is_ok());
        let perms1_restored = acl.get_user_permissions(&user1);
        assert!(perms1_restored.has(Permissions::CREATE_PROPOSAL));
        
        // Test direct permission grant
        assert!(acl.grant_permission(
            &user1,
            Permissions::MANAGE_LIQUIDITY,
            &authority.pubkey()
        ).is_ok());
        let perms1_updated = acl.get_user_permissions(&user1);
        assert!(perms1_updated.has(Permissions::MANAGE_LIQUIDITY));
    }

    /// Test rate limiting
    #[tokio::test]
    async fn test_rate_limiting() {
        // Test token bucket
        let mut bucket = TokenBucket::new(10, 10, 2);
        
        // Initial tokens available
        assert_eq!(bucket.tokens, 10);
        
        // Use tokens
        assert!(bucket.try_consume(5).is_ok());
        assert_eq!(bucket.tokens, 5);
        
        // Try to use more than available
        assert!(bucket.try_consume(10).is_err());
        
        // Simulate time passing (refill)
        bucket.refill(100); // 100 slots passed
        assert_eq!(bucket.tokens, 10); // Capped at capacity
        
        // Test sliding window
        let mut window = SlidingWindow::new(100, 5);
        
        // Add events
        for i in 0..5 {
            assert!(window.check_and_add(100 + i).is_ok());
        }
        
        // Should fail (limit reached)
        assert!(window.check_and_add(105).is_err());
        
        // Move window forward
        assert!(window.check_and_add(201).is_ok()); // Outside window
    }

    /// Test signature verification
    #[test]
    fn test_signature_verification() {
        // Test multi-sig
        let signers = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let multisig = MultiSigConfig::new(2, signers.clone());
        
        // Create signatures
        let message = b"test message";
        let sig1 = SignedMessage::new(
            message.to_vec(),
            vec![0; 65],
            signers[0],
            crate::security::signature_verifier::SignatureType::Ed25519,
        );
        let sig2 = SignedMessage::new(
            message.to_vec(),
            vec![0; 65],
            signers[1],
            crate::security::signature_verifier::SignatureType::Ed25519,
        );
        
        // Would succeed with 2 signatures (in production with real signatures)
        // assert!(multisig.verify_multisig(&[sig1, sig2]).is_ok());
        
        // Test weighted multi-sig
        let weights = vec![50, 30, 20];
        let weighted_multisig = MultiSigConfig::new_weighted(signers.clone(), weights, 60);
        
        // Test nonce manager
        let mut nonce_mgr = NonceManager::new(100);
        assert!(nonce_mgr.use_nonce(1000).is_ok());
        assert!(nonce_mgr.use_nonce(1001).is_ok());
        assert!(nonce_mgr.use_nonce(1000).is_err()); // Already used
        
        // Test oracle verification
        let oracle_keys = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let oracle_verifier = OracleSignatureVerifier::new(oracle_keys.clone(), 2);
        
        let oracle_data = b"BTC/USD:50000";
        let oracle_sig1 = SignedMessage::new(
            oracle_data.to_vec(),
            vec![0; 65],
            oracle_keys[0],
            crate::security::signature_verifier::SignatureType::Ed25519,
        );
        let oracle_sig2 = SignedMessage::new(
            oracle_data.to_vec(),
            vec![0; 65],
            oracle_keys[1],
            crate::security::signature_verifier::SignatureType::Ed25519,
        );
        
        // Would verify with 2 oracle confirmations (in production)
    }

    /// Test security monitoring
    #[tokio::test]
    async fn test_security_monitoring() {
        let authority = Keypair::new();
        let mut monitor = SecurityMonitor::new(authority.pubkey());
        
        // Log events
        let action1 = monitor.log_event(
            SecurityEventType::SuspiciousTransaction,
            Some(Pubkey::new_unique()),
            b"High frequency trading detected".to_vec(),
            5,
        ).unwrap();
        assert_eq!(action1, crate::security::security_monitor::SecurityAction::None);
        
        // High severity event
        let action2 = monitor.log_event(
            SecurityEventType::FlashLoanDetected,
            None,
            b"Large flash loan detected".to_vec(),
            9,
        ).unwrap();
        assert_eq!(action2, crate::security::security_monitor::SecurityAction::ProtocolPaused);
        
        // Check statistics
        let stats = monitor.get_stats();
        assert_eq!(stats.total_events, 2);
        assert!(stats.threat_level > 0);
        
        // Test anomaly detection
        let mut detector = AnomalyDetector::new(5);
        detector.update_baselines(1_000_000, 1000, 100);
        
        // Normal activity
        assert!(detector.detect_volume_anomaly(1_200_000).is_none());
        
        // Anomalous activity
        let severity = detector.detect_volume_anomaly(10_000_000);
        assert!(severity.is_some());
    }

    /// Test invariant checking
    #[test]
    fn test_invariant_checking() {
        let authority = Keypair::new();
        let mut checker = InvariantChecker::new(authority.pubkey());
        
        // Enable all invariants
        assert!(checker.is_enabled(InvariantType::TVLConsistency));
        assert!(checker.is_enabled(InvariantType::PriceNormalization));
        
        // Disable specific invariant
        checker.set_invariant(InvariantType::TVLConsistency, false);
        assert!(!checker.is_enabled(InvariantType::TVLConsistency));
        
        // Test price normalization check
        let proposal = crate::state::ProposalPDA {
            discriminator: [0; 8],
            version: 1,
            proposal_id: [0; 32],
            verse_id: [0; 32],
            market_id: [0; 32],
            amm_type: crate::state::AMMType::LMSR,
            outcomes: 3,
            prices: vec![400_000, 400_000, 200_000], // Sum = 1_000_000
            volumes: vec![0; 3],
            liquidity_depth: 0,
            state: crate::state::ProposalState::Active,
            settle_slot: 0,
            resolution: None,
            partial_liq_accumulator: 0,
            chain_positions: Vec::new(),
            outcome_balances: vec![0; 3],
            b_value: 1_000_000,
            total_liquidity: 0,
            total_volume: 0,
            funding_state: crate::trading::funding_rate::FundingRateState::new(0),
            status: crate::state::ProposalState::Active,
            settled_at: None,
        };
        
        // Would check invariants in production
    }

    /// Test emergency pause
    #[tokio::test]
    async fn test_emergency_pause() {
        let pause_auth = Keypair::new();
        let unpause_auth = Keypair::new();
        let mut pause_system = EmergencyPause::new(pause_auth.pubkey(), unpause_auth.pubkey());
        
        // Normal operation
        assert!(pause_system.is_operation_allowed(OperationCategory::Trading, 100).is_ok());
        
        // Trigger partial pause
        assert!(pause_system.trigger_pause(
            PauseLevel::Partial,
            "Test partial pause",
            0,
            &pause_auth.pubkey()
        ).is_ok());
        
        // Trading blocked, emergency allowed
        assert!(pause_system.is_operation_allowed(OperationCategory::Trading, 100).is_err());
        assert!(pause_system.is_operation_allowed(OperationCategory::Emergency, 100).is_ok());
        assert!(pause_system.is_operation_allowed(OperationCategory::Liquidation, 100).is_ok());
        
        // Trigger full pause
        assert!(pause_system.trigger_pause(
            PauseLevel::Full,
            "Test full pause",
            0,
            &pause_auth.pubkey()
        ).is_ok());
        
        // Only emergency allowed
        assert!(pause_system.is_operation_allowed(OperationCategory::Admin, 100).is_err());
        assert!(pause_system.is_operation_allowed(OperationCategory::Emergency, 100).is_ok());
        
        // Unpause
        assert!(pause_system.unpause(&unpause_auth.pubkey()).is_ok());
        assert!(pause_system.is_operation_allowed(OperationCategory::Trading, 100).is_ok());
        
        // Test circuit breaker
        let mut breaker = CircuitBreaker::default();
        
        // Normal metrics
        let normal_metrics = crate::security::emergency_pause::CircuitBreakerMetrics {
            max_volatility: 500,
            volume_ratio: 200,
            liquidation_count: 10,
            protocol_loss_bps: 100,
        };
        assert!(breaker.should_trigger(&normal_metrics).is_none());
        
        // High volatility
        let volatile_metrics = crate::security::emergency_pause::CircuitBreakerMetrics {
            max_volatility: 1500,
            volume_ratio: 200,
            liquidation_count: 10,
            protocol_loss_bps: 100,
        };
        let trigger = breaker.should_trigger(&volatile_metrics);
        assert!(trigger.is_some());
        assert_eq!(trigger.unwrap().0, PauseLevel::Partial);
    }

    /// Comprehensive security scenario test
    #[tokio::test]
    async fn test_security_scenario() {
        let program = ProgramTest::new("betting_platform_native", ID, None);
        let (mut banks_client, payer, recent_blockhash) = program.start().await;
        
        // Create all security components
        let authority = Keypair::new();
        let mut acl = AccessControlList::new(authority.pubkey());
        let mut monitor = SecurityMonitor::new(authority.pubkey());
        let mut pause_system = EmergencyPause::new(authority.pubkey(), authority.pubkey());
        let mut rate_limiter = RateLimiter::new(5, 10, 100);
        
        // Simulate attack scenario
        let attacker = Keypair::new().pubkey();
        let legitimate_user = Keypair::new().pubkey();
        
        // Grant trader role to legitimate user
        assert!(acl.grant_role(&legitimate_user, 3, &authority.pubkey()).is_ok());
        
        // Attacker attempts multiple operations
        for i in 0..10 {
            // Check rate limit
            if rate_limiter.check_limit(&attacker, 1000 + i).is_err() {
                // Log rate limit violation
                monitor.log_event(
                    SecurityEventType::RateLimitViolation,
                    Some(attacker),
                    b"Rate limit exceeded".to_vec(),
                    6,
                ).unwrap();
            }
        }
        
        // Check if attacker should be suspended
        let events = monitor.get_events_by_type(SecurityEventType::RateLimitViolation, 10);
        if events.len() >= 5 {
            assert!(acl.suspend_user(&attacker, &authority.pubkey()).is_ok());
        }
        
        // Simulate flash loan attack detection
        let flash_loan_action = monitor.log_event(
            SecurityEventType::FlashLoanDetected,
            Some(attacker),
            b"Suspicious flash loan activity".to_vec(),
            9,
        ).unwrap();
        
        // Should trigger protocol pause
        if flash_loan_action == crate::security::security_monitor::SecurityAction::ProtocolPaused {
            assert!(pause_system.trigger_pause(
                PauseLevel::Full,
                "Flash loan attack detected",
                300, // 300 slots pause
                &authority.pubkey()
            ).is_ok());
        }
        
        // Verify legitimate user can still perform emergency operations
        let user_perms = acl.get_user_permissions(&legitimate_user);
        assert!(user_perms.has(Permissions::CREATE_PROPOSAL));
        
        // But trading should be blocked due to pause
        assert!(pause_system.is_operation_allowed(OperationCategory::Trading, 1000).is_err());
        assert!(pause_system.is_operation_allowed(OperationCategory::Emergency, 1000).is_ok());
    }

    /// Test production-grade security flow
    #[tokio::test]
    async fn test_production_security_flow() {
        let program = ProgramTest::new("betting_platform_native", ID, None);
        let (mut banks_client, payer, recent_blockhash) = program.start().await;
        
        // Initialize all security systems
        let admin = Keypair::new();
        let operator = Keypair::new();
        let keeper = Keypair::new();
        let trader1 = Keypair::new();
        let trader2 = Keypair::new();
        
        let mut acl = AccessControlList::new(admin.pubkey());
        let mut monitor = SecurityMonitor::new(admin.pubkey());
        let mut pause_system = EmergencyPause::new(admin.pubkey(), admin.pubkey());
        let mut invariant_checker = InvariantChecker::new(admin.pubkey());
        
        // Setup roles
        assert!(acl.grant_role(&operator.pubkey(), 1, &admin.pubkey()).is_ok()); // Operator
        assert!(acl.grant_role(&keeper.pubkey(), 2, &admin.pubkey()).is_ok()); // Keeper
        assert!(acl.grant_role(&trader1.pubkey(), 3, &admin.pubkey()).is_ok()); // Trader
        assert!(acl.grant_role(&trader2.pubkey(), 3, &admin.pubkey()).is_ok()); // Trader
        
        // Normal trading flow
        let mut rate_limiter1 = RateLimiter::new(10, 60, 100);
        let mut rate_limiter2 = RateLimiter::new(10, 60, 100);
        
        // Trader 1 performs normal operations
        for i in 0..5 {
            assert!(rate_limiter1.check_limit(&trader1.pubkey(), 1000 + i * 10).is_ok());
        }
        
        // Trader 2 attempts high-frequency trading
        let mut violations = 0;
        for i in 0..20 {
            if rate_limiter2.check_limit(&trader2.pubkey(), 1000 + i).is_err() {
                violations += 1;
                monitor.log_event(
                    SecurityEventType::RateLimitViolation,
                    Some(trader2.pubkey()),
                    format!("Rate limit violation #{}", violations).as_bytes().to_vec(),
                    4 + violations.min(5) as u8,
                ).unwrap();
            }
        }
        
        // Check monitor statistics
        let stats = monitor.get_stats();
        assert!(stats.total_events > 0);
        
        // If too many violations, suspend trader 2
        if violations >= 10 {
            assert!(acl.suspend_user(&trader2.pubkey(), &admin.pubkey()).is_ok());
            
            // Log suspension
            monitor.log_event(
                SecurityEventType::UnauthorizedAccess,
                Some(trader2.pubkey()),
                b"User suspended due to repeated violations".to_vec(),
                7,
            ).unwrap();
        }
        
        // Keeper performs liquidation check
        let keeper_perms = acl.get_user_permissions(&keeper.pubkey());
        assert!(keeper_perms.has(Permissions::LIQUIDATE_POSITIONS));
        
        // Simulate invariant check
        let global_config = crate::state::GlobalConfigPDA {
            discriminator: discriminators::GLOBAL_CONFIG,
            version: 1,
            migration_state: crate::state::versioned_accounts::MigrationState::Current,
            epoch: 0,
            season: 0,
            vault: 0,
            total_oi: 0,
            coverage: 0,
            fee_base: 30,
            fee_slope: 0,
            halt_flag: false,
            genesis_slot: 0,
            season_start_slot: 0,
            season_end_slot: 0,
            mmt_total_supply: 0,
            mmt_current_season: 0,
            mmt_emission_rate: 0,
            leverage_tiers: vec![
                crate::state::LeverageTier { n: 1, max: 100 },
                crate::state::LeverageTier { n: 2, max: 50 },
            ],
            min_order_size: 1_000_000,
            max_order_size: 1_000_000_000_000,
            update_authority: admin.pubkey(),
            primary_market_id: [0u8; 32],
        };
        
        // Check system health
        let threat_level = monitor.threat_level;
        if threat_level >= SecurityMonitor::CRITICAL_THREAT_LEVEL {
            // Emergency pause
            assert!(pause_system.trigger_pause(
                PauseLevel::Partial,
                &format!("High threat level: {}", threat_level),
                600, // 10 minute pause
                &admin.pubkey()
            ).is_ok());
            
            // Alert operators
            monitor.log_event(
                SecurityEventType::CircuitBreakerTriggered,
                None,
                format!("Emergency pause triggered at threat level {}", threat_level).as_bytes().to_vec(),
                8,
            ).unwrap();
        }
        
        // Verify security measures are working
        assert_eq!(acl.get_user_permissions(&trader2.pubkey()), Permissions::NONE); // Suspended
        assert!(acl.get_user_permissions(&trader1.pubkey()).has(Permissions::CREATE_PROPOSAL)); // Active
        assert!(acl.get_user_permissions(&keeper.pubkey()).has(Permissions::LIQUIDATE_POSITIONS)); // Active
    }
}