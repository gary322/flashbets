//! Mandatory leverage quiz implementation
//!
//! Users must pass this quiz before accessing leverage > 10x

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
    program::invoke_signed,
    system_instruction,
    rent::Rent,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::discriminators,
    events::{EventType, Event},
    define_event,
};

/// Quiz configuration
pub const QUIZ_PASS_THRESHOLD: u8 = 80; // 80% correct answers required
pub const QUIZ_COOLDOWN_SLOTS: u64 = 7200; // 1 hour cooldown between attempts
pub const MAX_QUIZ_ATTEMPTS: u8 = 5; // Max attempts before lockout

/// Risk quiz state for a user
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct RiskQuizState {
    /// Account discriminator
    pub discriminator: [u8; 8],
    
    /// User who took the quiz
    pub user: Pubkey,
    
    /// Has passed the quiz
    pub has_passed: bool,
    
    /// Quiz score (percentage)
    pub last_score: u8,
    
    /// Number of attempts
    pub attempts: u8,
    
    /// Last attempt timestamp
    pub last_attempt: i64,
    
    /// Quiz version (for updates)
    pub quiz_version: u16,
    
    /// Highest leverage unlocked
    pub max_leverage_unlocked: u8,
    
    /// Risk acknowledgment signed
    pub risk_acknowledged: bool,
    
    /// Account created at
    pub created_at: i64,
    
    /// Quiz pass timestamp
    pub passed_at: Option<i64>,
}

impl RiskQuizState {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        1 + // has_passed
        1 + // last_score
        1 + // attempts
        8 + // last_attempt
        2 + // quiz_version
        1 + // max_leverage_unlocked
        1 + // risk_acknowledged
        8 + // created_at
        9; // passed_at (Option<i64>)
    
    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::RISK_QUIZ_STATE,
            user,
            has_passed: false,
            last_score: 0,
            attempts: 0,
            last_attempt: 0,
            quiz_version: 1,
            max_leverage_unlocked: 10, // Default max 10x
            risk_acknowledged: false,
            created_at: Clock::get().unwrap().unix_timestamp,
            passed_at: None,
        }
    }
    
    /// Check if user can take quiz again
    pub fn can_retake(&self) -> Result<bool, ProgramError> {
        if self.has_passed {
            return Ok(false);
        }
        
        if self.attempts >= MAX_QUIZ_ATTEMPTS {
            return Ok(false);
        }
        
        let current_slot = Clock::get()?.slot;
        let last_attempt_slot = self.last_attempt as u64;
        
        Ok(current_slot >= last_attempt_slot + QUIZ_COOLDOWN_SLOTS)
    }
    
    /// Get allowed leverage based on quiz results
    pub fn get_allowed_leverage(&self) -> u8 {
        if !self.has_passed {
            10 // Max 10x without passing quiz
        } else {
            self.max_leverage_unlocked
        }
    }
}

/// Quiz question
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct QuizQuestion {
    pub id: u8,
    pub question: String,
    pub answers: Vec<String>,
    pub correct_answer: u8,
    pub explanation: String,
}

/// Get quiz questions
pub fn get_quiz_questions() -> Vec<QuizQuestion> {
    vec![
        QuizQuestion {
            id: 1,
            question: "What happens to your position if you use 100x leverage and the price moves 1% against you?".to_string(),
            answers: vec![
                "You lose 1% of your position".to_string(),
                "You lose 10% of your position".to_string(),
                "You lose 50% of your position".to_string(),
                "Your position is liquidated".to_string(),
            ],
            correct_answer: 3,
            explanation: "With 100x leverage, a 1% adverse price movement results in 100% loss and liquidation.".to_string(),
        },
        QuizQuestion {
            id: 2,
            question: "What is the maximum effective leverage achievable through chain positions?".to_string(),
            answers: vec![
                "100x".to_string(),
                "250x".to_string(),
                "500x".to_string(),
                "1000x".to_string(),
            ],
            correct_answer: 2,
            explanation: "Chain positions can achieve up to 500x effective leverage through compounding.".to_string(),
        },
        QuizQuestion {
            id: 3,
            question: "What protects your position from instant liquidation during volatile markets?".to_string(),
            answers: vec![
                "Nothing, liquidations are instant".to_string(),
                "Partial liquidation system".to_string(),
                "Insurance fund".to_string(),
                "Market makers".to_string(),
            ],
            correct_answer: 1,
            explanation: "The partial liquidation system gradually reduces position size instead of full liquidation.".to_string(),
        },
        QuizQuestion {
            id: 4,
            question: "When using cross-margin, what can happen to your profitable positions?".to_string(),
            answers: vec![
                "They are protected from losses".to_string(),
                "They can offset losses in other positions".to_string(),
                "They earn extra rewards".to_string(),
                "They become risk-free".to_string(),
            ],
            correct_answer: 1,
            explanation: "Cross-margin allows profitable positions to offset losses but also puts them at risk.".to_string(),
        },
        QuizQuestion {
            id: 5,
            question: "What is the funding rate impact on leveraged positions?".to_string(),
            answers: vec![
                "No impact".to_string(),
                "Only affects short positions".to_string(),
                "Amplified by leverage amount".to_string(),
                "Fixed 0.01% per hour".to_string(),
            ],
            correct_answer: 2,
            explanation: "Funding rates are amplified by your leverage, increasing holding costs.".to_string(),
        },
        QuizQuestion {
            id: 6,
            question: "With 500x leverage, what percentage price movement against you results in total loss?".to_string(),
            answers: vec![
                "0.1%".to_string(),
                "0.2%".to_string(),
                "0.5%".to_string(),
                "1.0%".to_string(),
            ],
            correct_answer: 1, // 0.2% because 100/500 = 0.2%
            explanation: "At 500x leverage, a mere 0.2% price movement against your position results in 100% loss. This is why 500x is extremely risky.".to_string(),
        },
    ]
}

/// Initialize risk quiz state for user
pub fn process_initialize_risk_quiz(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let quiz_state_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Derive PDA
    let (quiz_state_key, bump) = Pubkey::find_program_address(
        &[b"risk_quiz", user.key.as_ref()],
        program_id,
    );
    
    if quiz_state_key != *quiz_state_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Create account if needed
    if quiz_state_account.data_is_empty() {
        let rent = Rent::get()?;
        let space = RiskQuizState::SIZE;
        let lamports = rent.minimum_balance(space);
        
        invoke_signed(
            &system_instruction::create_account(
                user.key,
                quiz_state_account.key,
                lamports,
                space as u64,
                program_id,
            ),
            &[user.clone(), quiz_state_account.clone(), system_program.clone()],
            &[&[b"risk_quiz", user.key.as_ref(), &[bump]]],
        )?;
    }
    
    // Initialize state
    let quiz_state = RiskQuizState::new(*user.key);
    quiz_state.serialize(&mut &mut quiz_state_account.data.borrow_mut()[..])?;
    
    msg!("Risk quiz state initialized for user {}", user.key);
    
    // Emit event
    let event = RiskQuizInitialized {
        user: *user.key,
        timestamp: quiz_state.created_at,
    };
    event.emit();
    
    Ok(())
}

/// Submit quiz answers
pub fn process_submit_quiz_answers(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    answers: Vec<u8>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let quiz_state_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Validate PDA
    let (quiz_state_key, _) = Pubkey::find_program_address(
        &[b"risk_quiz", user.key.as_ref()],
        program_id,
    );
    
    if quiz_state_key != *quiz_state_account.key {
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    // Load state
    let mut quiz_state = RiskQuizState::try_from_slice(&quiz_state_account.data.borrow())?;
    
    // Verify ownership
    if quiz_state.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check if can retake
    if !quiz_state.can_retake()? {
        msg!("Cannot retake quiz: passed={}, attempts={}", quiz_state.has_passed, quiz_state.attempts);
        return Err(BettingPlatformError::QuizAlreadyPassed.into());
    }
    
    // Get questions and validate answers
    let questions = get_quiz_questions();
    if answers.len() != questions.len() {
        return Err(BettingPlatformError::InvalidInput.into());
    }
    
    // Calculate score
    let mut correct = 0u8;
    for (i, answer) in answers.iter().enumerate() {
        if *answer == questions[i].correct_answer {
            correct += 1;
        }
    }
    
    let score = (correct as u16 * 100 / questions.len() as u16) as u8;
    
    // Update state
    quiz_state.last_score = score;
    quiz_state.attempts += 1;
    quiz_state.last_attempt = Clock::get()?.unix_timestamp;
    
    if score >= QUIZ_PASS_THRESHOLD {
        quiz_state.has_passed = true;
        quiz_state.passed_at = Some(Clock::get()?.unix_timestamp);
        
        // Unlock leverage based on score
        quiz_state.max_leverage_unlocked = if score >= 100 {
            255 // Allow up to 500x for perfect score (stored as u8, actual is multiplied)
        } else if score >= 90 {
            100 // Up to 100x for 90%+
        } else {
            50  // Up to 50x for 80%+
        };
        
        msg!("Quiz passed with score {}%! Max leverage unlocked: {}x", score, quiz_state.max_leverage_unlocked);
    } else {
        msg!("Quiz failed with score {}%. Required: {}%", score, QUIZ_PASS_THRESHOLD);
    }
    
    // Save state
    quiz_state.serialize(&mut &mut quiz_state_account.data.borrow_mut()[..])?;
    
    // Emit event
    let event = RiskQuizSubmitted {
        user: *user.key,
        score,
        passed: quiz_state.has_passed,
        attempts: quiz_state.attempts,
        timestamp: quiz_state.last_attempt,
    };
    event.emit();
    
    Ok(())
}

/// Acknowledge risk disclosure
pub fn process_acknowledge_risk(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    risk_hash: [u8; 32], // Hash of risk disclosure text
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let quiz_state_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Expected risk disclosure hash
    let expected_hash = crate::risk_warnings::get_risk_disclosure_hash();
    if risk_hash != expected_hash {
        return Err(BettingPlatformError::InvalidRiskHash.into());
    }
    
    // Load and update state
    let mut quiz_state = RiskQuizState::try_from_slice(&quiz_state_account.data.borrow())?;
    
    if quiz_state.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    quiz_state.risk_acknowledged = true;
    quiz_state.serialize(&mut &mut quiz_state_account.data.borrow_mut()[..])?;
    
    msg!("Risk disclosure acknowledged by user {}", user.key);
    
    // Emit event
    let event = RiskAcknowledged {
        user: *user.key,
        risk_hash,
        timestamp: Clock::get()?.unix_timestamp,
    };
    event.emit();
    
    Ok(())
}

/// Check if user can use requested leverage
pub fn check_leverage_allowed(
    user: &Pubkey,
    requested_leverage: u8,
    quiz_state_data: &[u8],
) -> Result<bool, ProgramError> {
    if requested_leverage <= 10 {
        // No quiz required for 10x or less
        return Ok(true);
    }
    
    let quiz_state = RiskQuizState::try_from_slice(quiz_state_data)?;
    
    if quiz_state.user != *user {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    if !quiz_state.has_passed {
        msg!("User must pass risk quiz to use {}x leverage", requested_leverage);
        return Ok(false);
    }
    
    if !quiz_state.risk_acknowledged {
        msg!("User must acknowledge risk disclosure");
        return Ok(false);
    }
    
    if requested_leverage > quiz_state.max_leverage_unlocked {
        msg!("Requested leverage {} exceeds maximum allowed {}", 
            requested_leverage, quiz_state.max_leverage_unlocked);
        return Ok(false);
    }
    
    Ok(true)
}

/// Get risk disclosure hash
fn get_risk_disclosure_hash() -> [u8; 32] {
    // Hash of the full risk disclosure text
    use solana_program::keccak;
    let disclosure = include_str!("../../docs/RISK_DISCLOSURE.md");
    keccak::hash(disclosure.as_bytes()).to_bytes()
}

// Event definitions
define_event!(RiskQuizInitialized, EventType::RiskQuizInitialized, {
    user: Pubkey,
    timestamp: i64,
});

define_event!(RiskQuizSubmitted, EventType::RiskQuizSubmitted, {
    user: Pubkey,
    score: u8,
    passed: bool,
    attempts: u8,
    timestamp: i64,
});

define_event!(RiskAcknowledged, EventType::RiskAcknowledged, {
    user: Pubkey,
    risk_hash: [u8; 32],
    timestamp: i64,
});


