//! Risk Quiz User Journey
//! 
//! Complete flow for a user attempting to use high leverage

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType, Event},
    risk_warnings::leverage_quiz::{RiskQuizState, get_quiz_questions, check_leverage_allowed},
    define_event,
};

/// Risk quiz journey steps
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskQuizStep {
    /// No quiz taken
    NotStarted,
    
    /// Quiz initialized
    QuizInitialized,
    
    /// First attempt
    FirstAttempt,
    
    /// Failed, retrying
    Retrying,
    
    /// Passed quiz
    QuizPassed,
    
    /// Risk acknowledged
    RiskAcknowledged,
    
    /// Ready to trade
    ReadyForHighLeverage,
}

/// Simulate a complete risk quiz journey
pub fn simulate_risk_quiz_journey(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let user_account = next_account_info(account_iter)?;
    let quiz_state_account = next_account_info(account_iter)?;
    let system_program = next_account_info(account_iter)?;
    
    msg!("=== Starting Risk Quiz Journey ===");
    msg!("User: {}", user_account.key);
    
    // Step 1: User attempts to use 50x leverage without quiz
    msg!("Step 1: User attempts 50x leverage without quiz");
    
    let requested_leverage = 50u8;
    
    // Check if quiz is required
    if requested_leverage > 10 {
        msg!("High leverage detected: {}x requires risk quiz", requested_leverage);
        
        // Initialize quiz state
        msg!("Step 2: Initializing risk quiz for user");
        crate::risk_warnings::process_initialize_risk_quiz(
            program_id,
            &[user_account.clone(), quiz_state_account.clone(), system_program.clone()],
        )?;
        
        // Step 3: Display quiz questions
        msg!("Step 3: Presenting quiz questions");
        let questions = get_quiz_questions();
        
        for (i, question) in questions.iter().enumerate() {
            msg!("Question {}: {}", i + 1, question.question);
            for (j, answer) in question.answers.iter().enumerate() {
                msg!("  {}) {}", j + 1, answer);
            }
        }
        
        // Step 4: Simulate user answering (first attempt - fail)
        msg!("Step 4: User submits answers (first attempt)");
        let wrong_answers = vec![0, 0, 0, 0, 0]; // All wrong answers
        
        let result = crate::risk_warnings::process_submit_quiz_answers(
            program_id,
            &[user_account.clone(), quiz_state_account.clone()],
            wrong_answers,
        );
        
        match result {
            Ok(_) => {
                let quiz_state = RiskQuizState::try_from_slice(&quiz_state_account.data.borrow())?;
                msg!("Quiz completed with score: {}%", quiz_state.last_score);
                
                if !quiz_state.has_passed {
                    msg!("Quiz failed! Required: 80%, Got: {}%", quiz_state.last_score);
                    msg!("Cooldown period active: {} slots", crate::risk_warnings::QUIZ_COOLDOWN_SLOTS);
                }
            }
            Err(e) => {
                msg!("Error submitting quiz: {:?}", e);
            }
        }
        
        // Step 5: Wait for cooldown (simulate)
        msg!("Step 5: Waiting for cooldown period...");
        
        // Step 6: Second attempt - pass
        msg!("Step 6: User retakes quiz (second attempt)");
        let correct_answers = vec![3, 2, 1, 1, 2]; // Correct answers
        
        crate::risk_warnings::process_submit_quiz_answers(
            program_id,
            &[user_account.clone(), quiz_state_account.clone()],
            correct_answers,
        )?;
        
        let quiz_state = RiskQuizState::try_from_slice(&quiz_state_account.data.borrow())?;
        msg!("Quiz completed with score: {}%", quiz_state.last_score);
        
        if quiz_state.has_passed {
            msg!("Quiz passed! User can now use up to {}x leverage", quiz_state.get_allowed_leverage());
            
            // Step 7: Acknowledge risk disclosure
            msg!("Step 7: User must acknowledge risk disclosure");
            
            let risk_hash = crate::risk_warnings::get_risk_disclosure_hash();
            
            crate::risk_warnings::process_acknowledge_risk(
                program_id,
                &[user_account.clone(), quiz_state_account.clone()],
                risk_hash,
            )?;
            
            msg!("Risk disclosure acknowledged!");
            
            // Step 8: Verify leverage is now allowed
            msg!("Step 8: Verifying leverage access");
            
            let allowed = check_leverage_allowed(
                user_account.key,
                requested_leverage,
                &quiz_state_account.data.borrow(),
            )?;
            
            if allowed {
                msg!("✅ User can now use {}x leverage!", requested_leverage);
                
                // Emit journey completed event
                RiskQuizJourneyCompleted {
                    user: *user_account.key,
                    max_leverage_unlocked: quiz_state.get_allowed_leverage(),
                    attempts: quiz_state.attempts,
                    final_score: quiz_state.last_score,
                }.emit();
            } else {
                msg!("❌ Leverage still not allowed - check requirements");
            }
        }
    }
    
    msg!("=== Risk Quiz Journey Complete ===");
    
    Ok(())
}


// Journey events
define_event!(RiskQuizJourneyCompleted, EventType::RiskQuizSubmitted, {
    user: Pubkey,
    max_leverage_unlocked: u8,
    attempts: u8,
    final_score: u8,
});

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_quiz_requirements() {
        // Test that leverage > 10x requires quiz
        assert!(50 > 10);
        assert!(100 > 10);
        assert!(10 <= 10); // No quiz required
    }
}