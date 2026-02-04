# Risk Quiz Implementation Documentation

## Overview

The betting platform implements a mandatory risk quiz system for users who want to access leverage above 10x. This system ensures users understand the risks involved with high leverage trading before they can access it.

## Key Features

### 1. Leverage Tiers
- **No Quiz Required**: 1x - 10x leverage
- **Quiz Required**: 11x - 100x leverage  
- **Chain Positions**: Up to 500x effective leverage (requires quiz)

### 2. Quiz System
- **5 Questions** covering key risk concepts
- **80% Pass Rate** required (4/5 correct)
- **1 Hour Cooldown** between attempts
- **Maximum 5 Attempts** before lockout
- **Risk Acknowledgment** required after passing

### 3. Quiz Topics
1. Leverage amplification of losses
2. Maximum effective leverage through chains
3. Partial liquidation protection
4. Cross-margin risks
5. Funding rate costs

## Implementation Details

### State Management

```rust
pub struct RiskQuizState {
    pub discriminator: [u8; 8],
    pub user: Pubkey,
    pub has_passed: bool,
    pub last_score: u8,
    pub attempts: u8,
    pub last_attempt: i64,
    pub quiz_version: u16,
    pub max_leverage_unlocked: u8,
    pub risk_acknowledged: bool,
    pub created_at: i64,
    pub passed_at: Option<i64>,
}
```

### Instructions

1. **InitializeRiskQuiz**: Create quiz state for user
2. **SubmitRiskQuizAnswers**: Submit quiz answers for scoring
3. **AcknowledgeRiskDisclosure**: Acknowledge risk after passing

### Integration Points

#### Trading Module
The `leverage_validation.rs` module integrates risk quiz checks:

```rust
pub fn validate_leverage_with_risk_check(
    user: &Pubkey,
    requested_leverage: u8,
    max_system_leverage: u8,
    risk_quiz_account: Option<&AccountInfo>,
) -> Result<(), ProgramError>
```

#### Position Opening
When opening positions with leverage > 10x:
1. Check if user has risk quiz account
2. Verify quiz passed and risk acknowledged
3. Validate requested leverage <= unlocked leverage

## User Journey

### First-Time High Leverage User

1. **Attempt High Leverage**
   - User tries to open position with 50x leverage
   - System detects leverage > 10x
   - Returns `RiskQuizRequired` error

2. **Initialize Quiz**
   - User calls `InitializeRiskQuiz`
   - Creates PDA: `[b"risk_quiz", user_pubkey]`
   - Sets initial state with 10x max leverage

3. **Take Quiz**
   - User reviews 5 questions
   - Submits answers via `SubmitRiskQuizAnswers`
   - System scores answers

4. **Handle Results**
   - **Pass (≥80%)**: Unlock up to 100x leverage
   - **Fail (<80%)**: Must wait 1 hour cooldown

5. **Acknowledge Risk**
   - User reads risk disclosure
   - Calls `AcknowledgeRiskDisclosure` with hash
   - System verifies hash matches current disclosure

6. **Trade with High Leverage**
   - User can now trade up to unlocked leverage
   - System validates on each position open

## Security Considerations

### Quiz Integrity
- Questions randomized per user (future enhancement)
- Cooldown prevents brute force attempts
- Max attempts limit prevents gaming

### Risk Disclosure
- Hash verification ensures user sees current version
- Cannot trade without acknowledgment
- Disclosure stored on-chain for audit trail

### Leverage Limits
- Hard cap at system maximum (100x)
- Per-market leverage limits still apply
- Chain positions require additional validation

## Events

```rust
// Quiz initialized for new user
RiskQuizInitialized {
    user: Pubkey,
    timestamp: i64,
}

// Quiz attempt submitted
RiskQuizSubmitted {
    user: Pubkey,
    score: u8,
    passed: bool,
    attempts: u8,
    timestamp: i64,
}

// Risk disclosure acknowledged
RiskAcknowledged {
    user: Pubkey,
    risk_hash: [u8; 32],
    timestamp: i64,
}
```

## Error Handling

### Common Errors
- `RiskQuizRequired`: Leverage > 10x without quiz
- `QuizAlreadyPassed`: Cannot retake passed quiz
- `QuizCooldownActive`: Must wait between attempts
- `QuizAttemptsExceeded`: Max 5 attempts reached
- `InvalidRiskHash`: Risk disclosure hash mismatch
- `RiskNotAcknowledged`: Must acknowledge risk after passing

## Testing

### Unit Tests
- Quiz state initialization
- Score calculation
- Cooldown logic
- Leverage validation

### Integration Tests
- Full user journey simulation
- Error case handling
- Multi-user scenarios

### Test Coverage
- All quiz questions validated
- All error paths tested
- Edge cases (0 leverage, max leverage)
- Cooldown and attempt limits

## Future Enhancements

1. **Dynamic Questions**
   - Randomize question order
   - Question pool rotation
   - Difficulty scaling

2. **Educational Content**
   - Links to educational resources
   - Interactive risk calculator
   - Case studies of losses

3. **Progressive Unlocking**
   - Gradual leverage increase
   - Experience-based limits
   - Volume requirements

4. **Analytics**
   - Quiz performance metrics
   - User behavior tracking
   - Risk correlation analysis