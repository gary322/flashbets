#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! fixed = "1.24"
//! ```

// Phase 11 Implementation Demo
// This demonstrates the attack detection, circuit breaker, and liquidation priority systems

use std::collections::VecDeque;
use fixed::types::{U64F64, I64F64};

// Simplified types for demo
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttackSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlertType {
    PriceManipulation,
    VolumeAnomaly,
    FlashLoan,
    WashTrading,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SecurityAction {
    Monitor,
    ClampPrice,
    PenalizeFees,
    RevertTrades,
    HaltTrading,
}

#[derive(Clone, Debug)]
pub struct TradeSnapshot {
    pub trader: String,
    pub market_id: [u8; 32],
    pub size: u64,
    pub price: U64F64,
    pub leverage: u64,
    pub slot: u64,
    pub is_buy: bool,
}

#[derive(Clone, Debug)]
pub struct SecurityAlert {
    pub alert_type: AlertType,
    pub severity: AttackSeverity,
    pub message: String,
    pub action: SecurityAction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HaltReason {
    LowCoverage,
    PriceVolatility,
    VolumeSurge,
    LiquidationCascade,
    NetworkCongestion,
    EmergencyHalt,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BreakerAction {
    Continue,
    Halt { reason: HaltReason, duration: u64, severity: AttackSeverity },
    RemainHalted,
    Resume,
    InCooldown,
    EmergencyShutdown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StakingTier {
    None,
    Bronze,
    Silver,
    Gold,
    Platinum,
}

// Simple attack detector
pub struct AttackDetector {
    pub recent_trades: VecDeque<TradeSnapshot>,
    pub max_price_change_per_slot: U64F64,
    pub wash_trade_min_time: u64,
}

impl AttackDetector {
    pub fn new() -> Self {
        Self {
            recent_trades: VecDeque::new(),
            max_price_change_per_slot: U64F64::from_num(0.02), // 2%
            wash_trade_min_time: 10, // 10 slots
        }
    }
    
    pub fn process_trade(&mut self, trade: &TradeSnapshot) -> Vec<SecurityAlert> {
        let mut alerts = Vec::new();
        
        // Check price manipulation
        if let Some(last_trade) = self.recent_trades.iter()
            .rev()
            .find(|t| t.market_id == trade.market_id) {
            
            let price_change = if trade.price > last_trade.price {
                (trade.price - last_trade.price) / last_trade.price
            } else {
                (last_trade.price - trade.price) / last_trade.price
            };
            
            if price_change > self.max_price_change_per_slot {
                alerts.push(SecurityAlert {
                    alert_type: AlertType::PriceManipulation,
                    severity: AttackSeverity::High,
                    message: format!("Price change {}% exceeds 2% limit", 
                        (price_change * U64F64::from_num(100)).to_num::<u16>()),
                    action: SecurityAction::ClampPrice,
                });
            }
        }
        
        // Check wash trading
        for recent in &self.recent_trades {
            if recent.trader == trade.trader && 
               recent.market_id == trade.market_id &&
               recent.is_buy != trade.is_buy &&
               trade.slot - recent.slot < self.wash_trade_min_time {
                
                alerts.push(SecurityAlert {
                    alert_type: AlertType::WashTrading,
                    severity: AttackSeverity::High,
                    message: "Wash trading detected - opposite trades too close".to_string(),
                    action: SecurityAction::PenalizeFees,
                });
                break;
            }
        }
        
        self.recent_trades.push_back(trade.clone());
        if self.recent_trades.len() > 100 {
            self.recent_trades.pop_front();
        }
        
        alerts
    }
}

// Simple circuit breaker
pub struct CircuitBreaker {
    pub min_coverage: U64F64,
    pub max_liquidations_per_slot: u64,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            min_coverage: U64F64::from_num(0.5), // 0.5 coverage minimum
            max_liquidations_per_slot: 50,
        }
    }
    
    pub fn check_breakers(
        &self,
        coverage: U64F64,
        liquidation_count: u64,
    ) -> BreakerAction {
        // Check coverage breaker
        if coverage < self.min_coverage {
            return BreakerAction::Halt {
                reason: HaltReason::LowCoverage,
                duration: 8640, // 1 hour
                severity: AttackSeverity::Critical,
            };
        }
        
        // Check liquidation cascade
        if liquidation_count > self.max_liquidations_per_slot {
            return BreakerAction::Halt {
                reason: HaltReason::LiquidationCascade,
                duration: 8640, // 1 hour
                severity: AttackSeverity::Critical,
            };
        }
        
        BreakerAction::Continue
    }
}

// Liquidation priority calculation
pub fn calculate_priority_score(
    risk_score: u8,
    distance_to_liquidation: f64,
    staking_tier: StakingTier,
    is_chained: bool,
    chain_depth: u8,
) -> u64 {
    // Base score from risk (0-100)
    let mut score = risk_score as u64 * 1_000_000;
    
    // Distance factor
    let distance_factor = if distance_to_liquidation < 0.01 {
        1_000_000 // <1% from liquidation
    } else if distance_to_liquidation < 0.05 {
        500_000 // <5% from liquidation
    } else {
        100_000 // >5% from liquidation
    };
    score += distance_factor;
    
    // Staking protection (subtracted)
    let staking_protection = match staking_tier {
        StakingTier::None => 0,
        StakingTier::Bronze => 100_000,
        StakingTier::Silver => 200_000,
        StakingTier::Gold => 300_000,
        StakingTier::Platinum => 500_000,
    };
    score = score.saturating_sub(staking_protection);
    
    // Chain risk factor (added)
    if is_chained {
        score += chain_depth as u64 * 100_000;
    }
    
    score
}

pub fn get_staking_tier(mmt_staked: u64) -> StakingTier {
    match mmt_staked {
        0..=99_999_999 => StakingTier::None,
        100_000_000..=999_999_999 => StakingTier::Bronze,
        1_000_000_000..=9_999_999_999 => StakingTier::Silver,
        10_000_000_000..=99_999_999_999 => StakingTier::Gold,
        _ => StakingTier::Platinum,
    }
}

fn main() {
    println!("🚀 Phase 11 Implementation Demo\n");
    
    // Demo 1: Attack Detection
    println!("=== Attack Detection Demo ===");
    let mut detector = AttackDetector::new();
    
    // Normal trade
    let trade1 = TradeSnapshot {
        trader: "Alice".to_string(),
        market_id: [1; 32],
        size: 1000,
        price: U64F64::from_num(100.0),
        leverage: 10,
        slot: 1000,
        is_buy: true,
    };
    
    let alerts1 = detector.process_trade(&trade1);
    println!("Trade 1 (normal): {} alerts", alerts1.len());
    
    // Price manipulation attempt
    let trade2 = TradeSnapshot {
        trader: "Bob".to_string(),
        market_id: [1; 32],
        size: 5000,
        price: U64F64::from_num(103.0), // 3% increase
        leverage: 20,
        slot: 1001,
        is_buy: true,
    };
    
    let alerts2 = detector.process_trade(&trade2);
    println!("Trade 2 (price manipulation): {} alerts", alerts2.len());
    if !alerts2.is_empty() {
        println!("  Alert: {:?}", alerts2[0]);
    }
    
    // Wash trading attempt
    let trade3 = TradeSnapshot {
        trader: "Alice".to_string(),
        market_id: [1; 32],
        size: 1000,
        price: U64F64::from_num(102.0),
        leverage: 10,
        slot: 1005,
        is_buy: false, // Opposite of trade1
    };
    
    let alerts3 = detector.process_trade(&trade3);
    println!("Trade 3 (wash trading): {} alerts", alerts3.len());
    if !alerts3.is_empty() {
        println!("  Alert: {:?}", alerts3[0]);
    }
    
    // Demo 2: Circuit Breaker
    println!("\n=== Circuit Breaker Demo ===");
    let breaker = CircuitBreaker::new();
    
    // Normal conditions
    let action1 = breaker.check_breakers(U64F64::from_num(0.8), 10);
    println!("Normal conditions: {:?}", action1);
    
    // Low coverage
    let action2 = breaker.check_breakers(U64F64::from_num(0.4), 10);
    println!("Low coverage (0.4): {:?}", action2);
    
    // Liquidation cascade
    let action3 = breaker.check_breakers(U64F64::from_num(0.8), 60);
    println!("High liquidations (60): {:?}", action3);
    
    // Demo 3: Liquidation Priority
    println!("\n=== Liquidation Priority Demo ===");
    
    // Position 1: High risk, no protection
    let priority1 = calculate_priority_score(
        90, // risk score
        0.005, // 0.5% from liquidation
        StakingTier::None,
        false,
        0,
    );
    println!("Position 1 (high risk, no protection): Priority = {}", priority1);
    
    // Position 2: High risk, Gold staking
    let priority2 = calculate_priority_score(
        90, // risk score
        0.005, // 0.5% from liquidation
        StakingTier::Gold,
        false,
        0,
    );
    println!("Position 2 (high risk, Gold staking): Priority = {}", priority2);
    
    // Position 3: Moderate risk, chained position
    let priority3 = calculate_priority_score(
        70, // risk score
        0.02, // 2% from liquidation
        StakingTier::None,
        true, // chained
        3, // chain depth
    );
    println!("Position 3 (moderate risk, chained): Priority = {}", priority3);
    
    // Demo 4: Staking Tiers
    println!("\n=== Staking Tier Demo ===");
    println!("0 MMT: {:?}", get_staking_tier(0));
    println!("100M MMT: {:?}", get_staking_tier(100_000_000));
    println!("1B MMT: {:?}", get_staking_tier(1_000_000_000));
    println!("10B MMT: {:?}", get_staking_tier(10_000_000_000));
    println!("100B MMT: {:?}", get_staking_tier(100_000_000_000));
    
    println!("\n✅ Phase 11 Implementation Demo Complete!");
    println!("\nKey Features Demonstrated:");
    println!("  - Price manipulation detection (2% per slot limit)");
    println!("  - Wash trading detection");
    println!("  - Circuit breaker coverage halt (<0.5)");
    println!("  - Liquidation cascade prevention (>50 liquidations)");
    println!("  - Priority-based liquidation with staking protection");
    println!("  - Chained position risk adjustment");
}