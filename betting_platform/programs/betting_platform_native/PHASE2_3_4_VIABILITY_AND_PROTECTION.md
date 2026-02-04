# Phase 2.3 & 2.4: Minimum Viable Vault and Vampire Attack Protection

## Overview
Successfully implemented comprehensive vault viability management and vampire attack protection mechanisms, ensuring the platform can safely grow from $0 to operational status while protecting against malicious actors.

## Phase 2.3: Minimum Viable Vault Size ($10k) Logic

### Implementation Details

#### 1. Vault Viability Tracker (`minimum_viable_vault.rs`)

**Core Features:**
- **State Management**: Tracks vault progression through 5 distinct states
- **Feature Gating**: Controls platform features based on vault balance
- **Automatic Transitions**: Handles state changes with proper events
- **Degradation Handling**: Manages scenarios where vault falls below minimum

#### Viability States:
```rust
pub enum VaultViabilityState {
    Bootstrap,           // Building towards $10k
    NearingViability,    // 90%+ of target ($9k+)
    MinimumViable,       // $10k reached, core features enabled
    FullyOperational,    // $20k+, all features enabled
    Degraded,           // Fell below $10k after being viable
}
```

#### Feature Control System:
```rust
pub struct EnabledFeatures {
    pub trading_enabled: bool,
    pub leverage_enabled: bool,
    pub max_leverage: u8,        // 0-10x based on vault size
    pub liquidations_enabled: bool,
    pub chain_positions_enabled: bool,
    pub advanced_orders_enabled: bool,
    pub fee_distribution_enabled: bool,
}
```

#### Leverage Scaling:
- **$0 - $1k**: No leverage (0x)
- **$1k - $10k**: Linear scaling (1x to 10x)
- **$10k+**: Full 10x leverage

### Key Mechanisms

#### 1. Automatic Viability Checks
- Runs every 60 seconds (150 slots)
- Updates feature availability
- Emits state transition events
- Tracks degradation history

#### 2. State Transition Handling
```rust
Bootstrap → NearingViability (at 90% = $9k)
NearingViability → MinimumViable (at 100% = $10k)
MinimumViable → FullyOperational (at 200% = $20k)
MinimumViable/FullyOperational → Degraded (below $10k)
Degraded → MinimumViable/FullyOperational (recovery)
```

#### 3. Feature Enablement Logic
- **Bootstrap**: No trading, deposits only
- **NearingViability**: Basic trading if >$1k
- **MinimumViable**: Trading, leverage, liquidations
- **FullyOperational**: All features including chains
- **Degraded**: Limited to closing positions

## Phase 2.4: Vampire Attack Protection

### Implementation Details

#### 1. Vampire Attack Detector (`vampire_attack_protection.rs`)

**Protection Mechanisms:**
1. **Coverage Ratio Check**: Halt if coverage < 0.5
2. **Large Withdrawal Detection**: Block >20% vault withdrawals
3. **Rapid Withdrawal Prevention**: Max 3 withdrawals per 60 seconds
4. **Suspicious Address Tracking**: Blacklist attackers

#### Detection Constants:
```rust
VAMPIRE_ATTACK_COVERAGE_THRESHOLD: 0.5 (50%)
SUSPICIOUS_WITHDRAWAL_THRESHOLD: 20% of vault
RAPID_WITHDRAWAL_WINDOW: 60 seconds
MAX_WITHDRAWALS_PER_WINDOW: 3
RECOVERY_COOLDOWN: 20 minutes
```

### Attack Scenarios Protected Against

#### 1. Coverage Drain Attack
- **Scenario**: Withdrawals that would drop coverage below 0.5
- **Protection**: Immediate block with cooldown
- **Recovery**: 20-minute cooldown before withdrawals resume

#### 2. Large Single Withdrawal
- **Scenario**: Single withdrawal >20% of vault
- **Protection**: Flagged as suspicious, blocked
- **Recovery**: Admin review required

#### 3. Rapid Withdrawal Attack
- **Scenario**: Multiple small withdrawals in quick succession
- **Protection**: Rate limiting (3 per minute max)
- **Recovery**: Window resets after 60 seconds

#### 4. Coordinated Attack
- **Scenario**: Multiple addresses attacking simultaneously
- **Protection**: Suspicious address tracking
- **Recovery**: Addresses permanently blacklisted

### Security Features

#### 1. Attack Detection & Response
```rust
fn handle_vampire_attack() {
    // Log attack details
    // Add attacker to suspicious list
    // Set recovery cooldown
    // Emit security event
    // Block withdrawal
}
```

#### 2. Admin Controls
- Reset protection state
- Remove addresses from blacklist
- Override cooldown in emergencies
- Full audit trail via events

#### 3. Automatic Recovery
- Cooldown expires after 20 minutes
- Withdrawal windows reset automatically
- Coverage recalculated on each deposit

## Integration Architecture

### Component Interaction Flow:
```
User Withdrawal Request
    ↓
Vampire Attack Check
    ↓ (if safe)
Vault Balance Update
    ↓
Viability State Check
    ↓
Feature Availability Update
    ↓
Event Emission
```

### Event System

#### Viability Events:
- `VaultViabilityChecked`: Regular status updates
- `VaultViabilityReached`: First time hitting $10k
- `VaultDegraded`: Falling below minimum
- `VaultRecovered`: Returning to viable state
- `VaultNearingViability`: 90% threshold reached

#### Security Events:
- `VampireAttackDetected`: Attack blocked
- `VampireProtectionReset`: Admin intervention

## Testing & Validation

### Unit Tests Implemented:
1. **Viability State Determination**: All state transitions
2. **Leverage Calculation**: Progressive scaling
3. **Feature Enablement**: State-based feature control
4. **Coverage Calculations**: Attack threshold testing
5. **Withdrawal Percentage**: Large withdrawal detection

### Integration Points Validated:
- Bootstrap coordinator updates
- MMT reward distribution
- Deposit/withdrawal flows
- Event emission consistency

## Production Considerations

### Performance:
- O(1) viability checks
- Minimal storage overhead
- Event-driven architecture
- No loops or complex calculations

### Safety:
- Multiple attack vectors covered
- Fail-safe defaults (protection on)
- Admin override capabilities
- Comprehensive audit trail

### Scalability:
- Fixed-size state accounts
- Efficient bitmap encoding for events
- No growing data structures
- Predictable compute usage

## Summary

The implementation provides robust protection for the bootstrap phase while enabling smooth progression to full platform functionality:

✅ **Minimum Viable Vault**:
- Clear state progression model
- Feature gating based on vault size
- Automatic degradation handling
- Progressive leverage unlocking

✅ **Vampire Attack Protection**:
- Multi-layered defense mechanisms
- Automatic attack detection
- Recovery procedures
- Admin controls for emergencies

The system ensures that the platform can safely grow from $0 to a sustainable trading venue while protecting early liquidity providers from exploitation.