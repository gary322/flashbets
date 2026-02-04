//! Security account structures
//!
//! Account types for circuit breakers, attack detection, and liquidation management

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::account_validation::DISCRIMINATOR_SIZE;

/// Discriminators for security account types
pub mod discriminators {
    pub const CIRCUIT_BREAKER: [u8; 8] = [167, 234, 89, 201, 45, 156, 78, 23];
    pub const ATTACK_DETECTOR: [u8; 8] = [78, 201, 156, 23, 234, 45, 167, 89];
    pub const LIQUIDATION_QUEUE: [u8; 8] = [234, 89, 167, 156, 45, 78, 23, 201];
    pub const AT_RISK_POSITION: [u8; 8] = [156, 23, 78, 234, 201, 89, 45, 167];
    pub const RESOLUTION_STATE: [u8; 8] = [89, 156, 201, 234, 167, 23, 78, 45];
    pub const DISPUTE_STATE: [u8; 8] = [201, 78, 45, 167, 23, 234, 156, 89];
    pub const PRICE_CACHE: [u8; 8] = [45, 234, 167, 89, 156, 201, 23, 78];
}

/// Circuit breaker configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct CircuitBreaker {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Coverage ratio threshold (basis points)
    pub coverage_threshold: u16,
    
    /// Price movement threshold (basis points)
    pub price_movement_threshold: u16,
    
    /// Volume spike threshold (multiplier)
    pub volume_spike_threshold: u16,
    
    /// Liquidation cascade threshold
    pub liquidation_cascade_threshold: u32,
    
    /// Network congestion threshold (failed tx percentage)
    pub congestion_threshold: u16,
    
    /// Open interest rate threshold (basis points per slot)
    pub oi_rate_threshold: u16,
    
    /// Cooldown period between triggers (slots)
    pub cooldown_period: u64,
    
    /// Last trigger slot
    pub last_trigger_slot: u64,
    
    /// Halt durations for each breaker type
    pub coverage_halt_duration: u64,
    pub price_halt_duration: u64,
    pub volume_halt_duration: u64,
    pub liquidation_halt_duration: u64,
    pub congestion_halt_duration: u64,
    pub oi_rate_halt_duration: u64,
    
    /// Current breaker states
    pub coverage_breaker_active: bool,
    pub price_breaker_active: bool,
    pub volume_breaker_active: bool,
    pub liquidation_breaker_active: bool,
    pub congestion_breaker_active: bool,
    pub oi_rate_breaker_active: bool,
    
    /// Breaker activation timestamps
    pub coverage_activated_at: Option<i64>,
    pub price_activated_at: Option<i64>,
    pub volume_activated_at: Option<i64>,
    pub liquidation_activated_at: Option<i64>,
    pub congestion_activated_at: Option<i64>,
    pub oi_rate_activated_at: Option<i64>,
    
    /// Statistics
    pub total_triggers: u64,
    pub false_positives: u32,
    
    /// Additional fields for test compatibility
    pub is_active: bool,
    pub breaker_type: Option<CircuitBreakerType>,
    pub triggered_at: Option<u64>,
    pub reason: Option<String>,
    pub triggered_by: Option<Pubkey>,
    pub resolved_at: Option<i64>,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::CIRCUIT_BREAKER,
            coverage_threshold: 5000,          // 50% coverage
            price_movement_threshold: 1000,    // 10% movement
            volume_spike_threshold: 300,       // 3x normal volume
            liquidation_cascade_threshold: 10, // 10 liquidations
            congestion_threshold: 2000,        // 20% failed tx
            oi_rate_threshold: 800,            // 8% = 800 basis points
            cooldown_period: 150,              // ~1 minute
            last_trigger_slot: 0,
            coverage_halt_duration: 900,       // ~6 minutes
            price_halt_duration: 300,          // ~2 minutes
            volume_halt_duration: 450,         // ~3 minutes
            liquidation_halt_duration: 600,    // ~4 minutes
            congestion_halt_duration: 150,     // ~1 minute
            oi_rate_halt_duration: 450,        // ~3 minutes
            coverage_breaker_active: false,
            price_breaker_active: false,
            volume_breaker_active: false,
            liquidation_breaker_active: false,
            congestion_breaker_active: false,
            oi_rate_breaker_active: false,
            coverage_activated_at: None,
            price_activated_at: None,
            volume_activated_at: None,
            liquidation_activated_at: None,
            congestion_activated_at: None,
            oi_rate_activated_at: None,
            total_triggers: 0,
            false_positives: 0,
            is_active: false,
            breaker_type: None,
            triggered_at: None,
            reason: None,
            triggered_by: None,
            resolved_at: None,
        }
    }
    
    pub fn check_and_trigger(
        &mut self,
        coverage: u64,
        liquidation_count: u64,
        liquidation_volume: u64,
        total_oi: u64,
        failed_tx: u64,
        current_slot: u64,
        current_time: i64,
    ) -> Result<Vec<BreakerType>, ProgramError> {
        self.check_and_trigger_with_oi_rate(
            coverage,
            liquidation_count,
            liquidation_volume,
            total_oi,
            failed_tx,
            0, // OI rate not provided - maintain backward compatibility
            current_slot,
            current_time,
        )
    }
    
    pub fn check_and_trigger_with_oi_rate(
        &mut self,
        coverage: u64,
        liquidation_count: u64,
        liquidation_volume: u64,
        total_oi: u64,
        failed_tx: u64,
        oi_rate_per_slot: u64, // Open interest change rate in basis points per slot
        current_slot: u64,
        current_time: i64,
    ) -> Result<Vec<BreakerType>, ProgramError> {
        let mut triggered = Vec::new();
        
        // Check cooldown
        if current_slot < self.last_trigger_slot + self.cooldown_period {
            return Ok(triggered);
        }
        
        // Check coverage
        if coverage < self.coverage_threshold as u64 && !self.coverage_breaker_active {
            self.coverage_breaker_active = true;
            self.coverage_activated_at = Some(current_time);
            triggered.push(BreakerType::Coverage);
        }
        
        // Check liquidation cascade
        if liquidation_count > self.liquidation_cascade_threshold as u64 && !self.liquidation_breaker_active {
            self.liquidation_breaker_active = true;
            self.liquidation_activated_at = Some(current_time);
            triggered.push(BreakerType::Liquidation);
        }
        
        // Check congestion
        let congestion_rate = if total_oi > 0 {
            (failed_tx * 10000) / total_oi
        } else {
            0
        };
        
        if congestion_rate > self.congestion_threshold as u64 && !self.congestion_breaker_active {
            self.congestion_breaker_active = true;
            self.congestion_activated_at = Some(current_time);
            triggered.push(BreakerType::Congestion);
        }
        
        // Check OI rate (8% per slot)
        if oi_rate_per_slot > self.oi_rate_threshold as u64 && !self.oi_rate_breaker_active {
            self.oi_rate_breaker_active = true;
            self.oi_rate_activated_at = Some(current_time);
            triggered.push(BreakerType::OIRate);
        }
        
        if !triggered.is_empty() {
            self.last_trigger_slot = current_slot;
            self.total_triggers += triggered.len() as u64;
            
            // Update the new fields
            self.is_active = true;
            self.triggered_at = Some(current_slot);
            
            // Set the breaker type to the first triggered (for simplicity)
            if let Some(first_breaker) = triggered.first() {
                self.breaker_type = Some(match first_breaker {
                    BreakerType::Coverage => CircuitBreakerType::Coverage,
                    BreakerType::Price => CircuitBreakerType::Price,
                    BreakerType::Volume => CircuitBreakerType::Volume,
                    BreakerType::Liquidation => CircuitBreakerType::Liquidation,
                    BreakerType::Congestion => CircuitBreakerType::Congestion,
                    BreakerType::OracleFailure => CircuitBreakerType::Price, // Map oracle failure to price breaker
                    BreakerType::OIRate => CircuitBreakerType::OIRate,
                });
                
                self.reason = Some(match first_breaker {
                    BreakerType::Coverage => "Low coverage ratio detected".to_string(),
                    BreakerType::Price => "Excessive price movement detected".to_string(),
                    BreakerType::Volume => "Volume spike detected".to_string(),
                    BreakerType::Liquidation => "Liquidation cascade detected".to_string(),
                    BreakerType::Congestion => "Network congestion detected".to_string(),
                    BreakerType::OracleFailure => "Oracle failure detected".to_string(),
                    BreakerType::OIRate => "Open interest rate spike detected (>8% per slot)".to_string(),
                });
            }
        }
        
        Ok(triggered)
    }
    
    pub fn check_expired_breakers(&mut self, current_time: i64) -> Vec<BreakerType> {
        let mut expired = Vec::new();
        
        if self.coverage_breaker_active {
            if let Some(activated) = self.coverage_activated_at {
                if current_time > activated + self.coverage_halt_duration as i64 {
                    self.coverage_breaker_active = false;
                    self.coverage_activated_at = None;
                    expired.push(BreakerType::Coverage);
                }
            }
        }
        
        if self.liquidation_breaker_active {
            if let Some(activated) = self.liquidation_activated_at {
                if current_time > activated + self.liquidation_halt_duration as i64 {
                    self.liquidation_breaker_active = false;
                    self.liquidation_activated_at = None;
                    expired.push(BreakerType::Liquidation);
                }
            }
        }
        
        if self.congestion_breaker_active {
            if let Some(activated) = self.congestion_activated_at {
                if current_time > activated + self.congestion_halt_duration as i64 {
                    self.congestion_breaker_active = false;
                    self.congestion_activated_at = None;
                    expired.push(BreakerType::Congestion);
                }
            }
        }
        
        if self.oi_rate_breaker_active {
            if let Some(activated) = self.oi_rate_activated_at {
                if current_time > activated + self.oi_rate_halt_duration as i64 {
                    self.oi_rate_breaker_active = false;
                    self.oi_rate_activated_at = None;
                    expired.push(BreakerType::OIRate);
                }
            }
        }
        
        // Update is_active state after checking all breakers
        if !expired.is_empty() {
            self.update_active_state();
            
            // If no breakers are active anymore, clear the fields
            if !self.is_active {
                self.breaker_type = None;
                self.triggered_at = None;
                self.reason = None;
            }
        }
        
        expired
    }
    
    pub fn is_halted(&self) -> bool {
        self.coverage_breaker_active ||
        self.price_breaker_active ||
        self.volume_breaker_active ||
        self.liquidation_breaker_active ||
        self.congestion_breaker_active ||
        self.oi_rate_breaker_active
    }
    
    /// Update overall is_active state based on individual breakers
    pub fn update_active_state(&mut self) {
        self.is_active = self.is_halted();
    }
    
    /// Activate a specific breaker with reason
    pub fn activate_breaker(&mut self, breaker_type: BreakerType, slot: u64, reason: String) {
        match breaker_type {
            BreakerType::Coverage => self.coverage_breaker_active = true,
            BreakerType::Price => self.price_breaker_active = true,
            BreakerType::Volume => self.volume_breaker_active = true,
            BreakerType::Liquidation => self.liquidation_breaker_active = true,
            BreakerType::Congestion => self.congestion_breaker_active = true,
            BreakerType::OIRate => self.oi_rate_breaker_active = true,
            BreakerType::OracleFailure => {
                // Oracle failure affects all operations - activate all breakers
                self.coverage_breaker_active = true;
                self.price_breaker_active = true;
                self.volume_breaker_active = true;
                self.liquidation_breaker_active = true;
                self.congestion_breaker_active = true;
                self.oi_rate_breaker_active = true;
            }
        }
        
        self.breaker_type = Some(match breaker_type {
            BreakerType::Coverage => CircuitBreakerType::Coverage,
            BreakerType::Price => CircuitBreakerType::Price,
            BreakerType::Volume => CircuitBreakerType::Volume,
            BreakerType::Liquidation => CircuitBreakerType::Liquidation,
            BreakerType::Congestion => CircuitBreakerType::Congestion,
            BreakerType::OracleFailure => CircuitBreakerType::OracleFailure,
            BreakerType::OIRate => CircuitBreakerType::OIRate,
        });
        
        self.triggered_at = Some(slot);
        self.reason = Some(reason);
        self.update_active_state();
    }
    
    /// Get list of active breakers
    pub fn get_active_breakers(&self) -> Vec<BreakerType> {
        let mut active = Vec::new();
        if self.coverage_breaker_active { active.push(BreakerType::Coverage); }
        if self.price_breaker_active { active.push(BreakerType::Price); }
        if self.volume_breaker_active { active.push(BreakerType::Volume); }
        if self.liquidation_breaker_active { active.push(BreakerType::Liquidation); }
        if self.congestion_breaker_active { active.push(BreakerType::Congestion); }
        if self.oi_rate_breaker_active { active.push(BreakerType::OIRate); }
        active
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::CIRCUIT_BREAKER {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Breaker type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum BreakerType {
    Coverage,
    Price,
    Volume,
    Liquidation,
    Congestion,
    OracleFailure,
    OIRate,
}

/// Circuit breaker type (for test compatibility)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum CircuitBreakerType {
    Coverage,
    Price,
    Volume,
    Liquidation,
    Congestion,
    OracleFailure,
    OIRate,
}

/// Attack detector
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct AttackDetector {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Pattern detection window (slots)
    pub detection_window: u64,
    
    /// Suspicious pattern threshold
    pub pattern_threshold: u32,
    
    /// Volume baseline (7-day average)
    pub avg_volume_baseline: u64,
    
    /// Volume standard deviation
    pub volume_std_dev: u64,
    
    /// Flash loan detection threshold
    pub flash_loan_threshold: u64,
    
    /// Minimum blocks between borrow and trade (flash loan protection)
    pub min_blocks_between_borrow_trade: u64,
    
    /// Track recent borrows by user (user -> slot)
    pub recent_borrows: Vec<(Pubkey, u64)>,
    
    /// Wash trading indicators
    pub wash_trade_threshold: u16,
    
    /// Recent trades buffer (circular)
    pub recent_trades: Vec<TradeRecord>,
    pub trade_buffer_index: u32,
    
    /// Attack statistics
    pub attacks_detected: u32,
    pub false_positives: u32,
    pub last_attack_slot: u64,
    
    /// Current alert level
    pub alert_level: AlertLevel,
    
    /// Suspicious addresses
    pub suspicious_addresses: Vec<Pubkey>,
}

impl AttackDetector {
    pub const TRADE_BUFFER_SIZE: usize = 100;
    
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::ATTACK_DETECTOR,
            detection_window: 150,        // ~1 minute
            pattern_threshold: 5,         // 5 suspicious patterns
            avg_volume_baseline: 0,
            volume_std_dev: 0,
            flash_loan_threshold: 10_000_000_000, // 10k USDC
            min_blocks_between_borrow_trade: 2, // Must wait 2 slots after borrowing
            recent_borrows: Vec::with_capacity(100),
            wash_trade_threshold: 500,    // 5% price impact
            recent_trades: Vec::with_capacity(Self::TRADE_BUFFER_SIZE),
            trade_buffer_index: 0,
            attacks_detected: 0,
            false_positives: 0,
            last_attack_slot: 0,
            alert_level: AlertLevel::Normal,
            suspicious_addresses: Vec::with_capacity(10),
        }
    }
    
    pub fn process_trade(
        &mut self,
        market_id: [u8; 32],
        trader: Pubkey,
        size: u64,
        price: u64,
        leverage: u64,
        is_buy: bool,
        slot: u64,
    ) -> Result<AttackType, ProgramError> {
        let trade = TradeRecord {
            market_id,
            trader,
            size,
            price,
            leverage,
            is_buy,
            slot,
        };
        
        // Add to circular buffer
        if self.recent_trades.len() < Self::TRADE_BUFFER_SIZE {
            self.recent_trades.push(trade.clone());
        } else {
            self.recent_trades[self.trade_buffer_index as usize] = trade.clone();
        }
        self.trade_buffer_index = (self.trade_buffer_index + 1) % Self::TRADE_BUFFER_SIZE as u32;
        
        // Check for attacks
        let mut suspicious_patterns = 0;
        
        // 1. Check for flash loan attack
        if size > self.flash_loan_threshold && leverage > 10 {
            suspicious_patterns += 2;
        }
        
        // Enhanced flash loan protection: Check if user borrowed recently
        if let Some((_, borrow_slot)) = self.recent_borrows.iter().find(|(pubkey, _)| pubkey == &trader) {
            if slot < borrow_slot + self.min_blocks_between_borrow_trade {
                // Trade too soon after borrow - likely flash loan attack
                self.attacks_detected += 1;
                self.last_attack_slot = slot;
                return Err(ProgramError::Custom(6094)); // AttackDetected
            }
        }
        
        // 2. Check for wash trading
        let same_trader_trades = self.recent_trades.iter()
            .filter(|t| t.trader == trader && t.slot > slot - self.detection_window)
            .count();
        
        if same_trader_trades > 5 {
            suspicious_patterns += 1;
        }
        
        // 3. Check for price manipulation
        let price_impact = self.calculate_price_impact(&trade);
        if price_impact > self.wash_trade_threshold as u64 {
            suspicious_patterns += 1;
        }
        
        // 4. Check for volume spike
        if self.avg_volume_baseline > 0 {
            let volume_deviation = if size > self.avg_volume_baseline {
                (size - self.avg_volume_baseline) * 10000 / self.avg_volume_baseline
            } else {
                0
            };
            
            if volume_deviation > 20000 { // 200% above average
                suspicious_patterns += 1;
            }
        }
        
        // Determine attack type
        if suspicious_patterns >= self.pattern_threshold {
            self.attacks_detected += 1;
            self.last_attack_slot = slot;
            
            if !self.suspicious_addresses.contains(&trader) && self.suspicious_addresses.len() < 10 {
                self.suspicious_addresses.push(trader);
            }
            
            return Err(ProgramError::Custom(6094)); // AttackDetected
        }
        
        Ok(AttackType::None)
    }
    
    fn calculate_price_impact(&self, trade: &TradeRecord) -> u64 {
        // Simplified price impact calculation
        let recent_prices: Vec<u64> = self.recent_trades.iter()
            .filter(|t| t.market_id == trade.market_id)
            .map(|t| t.price)
            .collect();
        
        if recent_prices.is_empty() {
            return 0;
        }
        
        let avg_price = recent_prices.iter().sum::<u64>() / recent_prices.len() as u64;
        
        if trade.price > avg_price {
            ((trade.price - avg_price) * 10000) / avg_price
        } else {
            ((avg_price - trade.price) * 10000) / avg_price
        }
    }
    
    pub fn record_borrow(&mut self, borrower: Pubkey, slot: u64) {
        // Clean up old borrows (older than detection window)
        self.recent_borrows.retain(|(_, borrow_slot)| *borrow_slot + self.detection_window > slot);
        
        // Add new borrow record
        if self.recent_borrows.len() >= 100 {
            self.recent_borrows.remove(0);
        }
        self.recent_borrows.push((borrower, slot));
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::ATTACK_DETECTOR {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Trade record for pattern detection
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TradeRecord {
    pub market_id: [u8; 32],
    pub trader: Pubkey,
    pub size: u64,
    pub price: u64,
    pub leverage: u64,
    pub is_buy: bool,
    pub slot: u64,
}

/// Alert level
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum AlertLevel {
    Normal,
    Elevated,
    High,
    Critical,
}

/// Attack type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum AttackType {
    None,
    FlashLoan,
    WashTrading,
    PriceManipulation,
    VolumeSpike,
}

/// Liquidation queue
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct LiquidationQueue {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Priority queue of at-risk positions
    pub queue: Vec<AtRiskPosition>,
    
    /// Maximum queue size
    pub max_size: u32,
    
    /// Total positions tracked
    pub total_positions: u64,
    
    /// Total liquidated
    pub total_liquidated: u64,
    
    /// Total keeper rewards paid
    pub total_rewards_paid: u64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Active liquidations in progress
    pub active_liquidations: u32,
}

impl LiquidationQueue {
    pub const MAX_QUEUE_SIZE: u32 = 100;
    
    pub fn new() -> Self {
        Self {
            discriminator: discriminators::LIQUIDATION_QUEUE,
            queue: Vec::with_capacity(Self::MAX_QUEUE_SIZE as usize),
            max_size: Self::MAX_QUEUE_SIZE,
            total_positions: 0,
            total_liquidated: 0,
            total_rewards_paid: 0,
            last_update_slot: 0,
            active_liquidations: 0,
        }
    }
    
    pub fn add_position(&mut self, position: AtRiskPosition) -> Result<(), ProgramError> {
        if self.queue.len() >= self.max_size as usize {
            // Remove lowest risk position if queue is full
            if let Some(min_idx) = self.queue.iter()
                .enumerate()
                .min_by_key(|(_, p)| p.risk_score)
                .map(|(i, _)| i) {
                
                if self.queue[min_idx].risk_score < position.risk_score {
                    self.queue.remove(min_idx);
                } else {
                    return Ok(()); // Don't add if risk is lower than all in queue
                }
            }
        }
        
        // Insert sorted by risk score
        let insert_idx = self.queue.binary_search_by(|p| position.risk_score.cmp(&p.risk_score))
            .unwrap_or_else(|i| i);
        
        self.queue.insert(insert_idx, position);
        self.total_positions += 1;
        
        Ok(())
    }
    
    pub fn get_next_liquidation(&mut self) -> Option<AtRiskPosition> {
        if self.queue.is_empty() {
            return None;
        }
        
        // Always take highest risk position (last in sorted queue)
        self.queue.pop()
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::LIQUIDATION_QUEUE {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.queue.len() > self.max_size as usize {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// At-risk position
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct AtRiskPosition {
    /// Position ID
    pub position_id: [u8; 32],
    
    /// Position account
    pub account: Pubkey,
    
    /// Risk score (0-100)
    pub risk_score: u8,
    
    /// Distance to liquidation price
    pub distance_to_liquidation: u64,
    
    /// Position notional value
    pub notional: u64,
    
    /// Leverage
    pub leverage: u64,
    
    /// Last update timestamp
    pub last_update: i64,
}

/// Price cache for efficient lookups
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct PriceCache {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Verse ID
    pub verse_id: u128,
    
    /// Cached price
    pub price: u64,
    
    /// Last update slot
    pub last_update_slot: u64,
    
    /// Update count
    pub update_count: u64,
    
    /// Is stale
    pub is_stale: bool,
}

impl PriceCache {
    pub fn new(verse_id: u128) -> Self {
        Self {
            discriminator: discriminators::PRICE_CACHE,
            verse_id,
            price: 0,
            last_update_slot: 0,
            update_count: 0,
            is_stale: true,
        }
    }
    
    pub fn update(&mut self, new_price: u64, slot: u64) {
        self.price = new_price;
        self.last_update_slot = slot;
        self.update_count += 1;
        self.is_stale = false;
    }
    
    pub fn check_staleness(&mut self, current_slot: u64, max_age: u64) {
        if current_slot > self.last_update_slot + max_age {
            self.is_stale = true;
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::PRICE_CACHE {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}