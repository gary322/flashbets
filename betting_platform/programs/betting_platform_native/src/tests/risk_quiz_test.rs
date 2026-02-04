//! Risk Quiz Tests
//!
//! Tests for mandatory leverage risk quiz functionality

#[cfg(test)]
mod tests {
    use solana_program::{
        clock::Clock,
        pubkey::Pubkey,
        sysvar::Sysvar,
    };
    
    use crate::{
        risk_warnings::{
            RiskQuizState, 
            get_quiz_questions, 
            QUIZ_PASS_THRESHOLD,
            check_leverage_allowed,
        },
        error::BettingPlatformError,
    };
    
    #[test]
    fn test_quiz_state_initialization() {
        let user = Pubkey::new_unique();
        let quiz_state = RiskQuizState::new(user);
        
        assert_eq!(quiz_state.user, user);
        assert!(!quiz_state.has_passed);
        assert_eq!(quiz_state.last_score, 0);
        assert_eq!(quiz_state.attempts, 0);
        assert_eq!(quiz_state.max_leverage_unlocked, 10);
        assert!(!quiz_state.risk_acknowledged);
    }
    
    #[test]
    fn test_quiz_questions() {
        let questions = get_quiz_questions();
        
        // Should have 5 questions
        assert_eq!(questions.len(), 5);
        
        // Each question should have 4 answers
        for question in &questions {
            assert_eq!(question.answers.len(), 4);
            assert!(question.correct_answer < 4);
        }
        
        // Test specific questions exist
        assert!(questions[0].question.contains("100x leverage"));
        assert!(questions[1].question.contains("maximum effective leverage"));
        assert!(questions[2].question.contains("liquidation"));
        assert!(questions[3].question.contains("cross-margin"));
        assert!(questions[4].question.contains("funding rate"));
    }
    
    #[test]
    fn test_leverage_limits() {
        let user = Pubkey::new_unique();
        
        // Test without quiz
        let mut quiz_state = RiskQuizState::new(user);
        assert_eq!(quiz_state.get_allowed_leverage(), 10);
        
        // Test after passing quiz
        quiz_state.has_passed = true;
        quiz_state.max_leverage_unlocked = 100;
        assert_eq!(quiz_state.get_allowed_leverage(), 100);
    }
    
    #[test]
    fn test_quiz_cooldown() {
        let user = Pubkey::new_unique();
        let mut quiz_state = RiskQuizState::new(user);
        
        // First attempt should be allowed
        assert!(quiz_state.can_retake().unwrap());
        
        // After an attempt, should need cooldown
        quiz_state.attempts = 1;
        quiz_state.last_attempt = Clock::get().unwrap_or_default().unix_timestamp;
        
        // Immediate retake should not be allowed
        assert!(!quiz_state.can_retake().unwrap());
        
        // After passing, should not allow retake
        quiz_state.has_passed = true;
        assert!(!quiz_state.can_retake().unwrap());
    }
    
    #[test]
    fn test_quiz_scoring() {
        let questions = get_quiz_questions();
        let total_questions = questions.len();
        
        // All correct should pass
        let all_correct = total_questions;
        let score = (all_correct as u16 * 100 / total_questions as u16) as u8;
        assert!(score >= QUIZ_PASS_THRESHOLD);
        
        // 4/5 correct should pass (80%)
        let four_correct = 4;
        let score = (four_correct as u16 * 100 / total_questions as u16) as u8;
        assert_eq!(score, 80);
        assert!(score >= QUIZ_PASS_THRESHOLD);
        
        // 3/5 correct should fail (60%)
        let three_correct = 3;
        let score = (three_correct as u16 * 100 / total_questions as u16) as u8;
        assert_eq!(score, 60);
        assert!(score < QUIZ_PASS_THRESHOLD);
    }
    
    #[test]
    fn test_risk_disclosure_hash() {
        use crate::risk_warnings::get_risk_disclosure_hash;
        
        // Hash should be deterministic
        let hash1 = get_risk_disclosure_hash();
        let hash2 = get_risk_disclosure_hash();
        assert_eq!(hash1, hash2);
        
        // Hash should not be zero
        assert_ne!(hash1, [0u8; 32]);
    }
    
    #[test]
    fn test_leverage_validation() {
        use crate::trading::leverage_validation::validate_leverage_with_risk_check;
        use crate::constants::MAX_LEVERAGE_NO_QUIZ as NO_QUIZ_MAX_LEVERAGE;
        
        let user = Pubkey::new_unique();
        
        // Test leverage <= 10x (no quiz required)
        let result = validate_leverage_with_risk_check(
            &user,
            10,
            100,
            None,
        );
        assert!(result.is_ok());
        
        // Test leverage > 10x without quiz
        let result = validate_leverage_with_risk_check(
            &user,
            50,
            100,
            None,
        );
        assert!(result.is_err());
        
        // Test zero leverage
        let result = validate_leverage_with_risk_check(
            &user,
            0,
            100,
            None,
        );
        assert!(result.is_err());
        
        // Test leverage exceeding system max
        let result = validate_leverage_with_risk_check(
            &user,
            150,
            100,
            None,
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_risk_levels() {
        use crate::risk_warnings::RiskLevel;
        
        assert_eq!(RiskLevel::from_leverage(5), RiskLevel::Low);
        assert_eq!(RiskLevel::from_leverage(20), RiskLevel::Medium);
        assert_eq!(RiskLevel::from_leverage(40), RiskLevel::High);
        assert_eq!(RiskLevel::from_leverage(75), RiskLevel::Extreme);
        assert_eq!(RiskLevel::from_leverage(150), RiskLevel::Insane);
        
        // Test color codes
        assert_eq!(RiskLevel::Low.color_code(), "green");
        assert_eq!(RiskLevel::Extreme.color_code(), "red");
        assert_eq!(RiskLevel::Insane.color_code(), "darkred");
    }
}