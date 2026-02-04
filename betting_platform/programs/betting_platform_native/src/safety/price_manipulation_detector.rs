//! Advanced Price Manipulation Detection
//!
//! Implements statistical anomaly detection and pattern recognition
//! for identifying price manipulation attempts

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    math::{U64F64, U128F128},
    state::ProposalPDA,
};

/// Price history window for analysis
pub const PRICE_HISTORY_WINDOW: usize = 100;
pub const VOLUME_HISTORY_WINDOW: usize = 50;

/// Detection thresholds
pub const Z_SCORE_THRESHOLD: f64 = 3.0; // 3 standard deviations
pub const VOLUME_SPIKE_THRESHOLD: u64 = 500; // 5x average volume
pub const PRICE_VELOCITY_THRESHOLD: u64 = 1000; // 10% per slot max

/// Price manipulation detector state
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceManipulationDetector {
    /// Proposal ID being monitored
    pub proposal_id: [u8; 32],
    
    /// Rolling price history (circular buffer)
    pub price_history: Vec<PricePoint>,
    pub history_index: usize,
    
    /// Statistical metrics
    pub price_mean: U64F64,
    pub price_variance: U64F64,
    pub volume_mean: u64,
    
    /// Detection state
    pub anomaly_count: u32,
    pub last_alert_slot: u64,
    pub manipulation_score: u8, // 0-100
    
    /// Pattern detection
    pub wash_trade_patterns: u32,
    pub pump_dump_signals: u32,
    pub spoofing_attempts: u32,
}

/// Price point with metadata
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct PricePoint {
    pub slot: u64,
    pub price: u64,
    pub volume: u64,
    pub trader: Pubkey,
    pub outcome: u8,
}

impl PriceManipulationDetector {
    /// Create new detector for a proposal
    pub fn new(proposal_id: [u8; 32]) -> Self {
        Self {
            proposal_id,
            price_history: Vec::with_capacity(PRICE_HISTORY_WINDOW),
            history_index: 0,
            price_mean: U64F64::from_num(0),
            price_variance: U64F64::from_num(0),
            volume_mean: 0,
            anomaly_count: 0,
            last_alert_slot: 0,
            manipulation_score: 0,
            wash_trade_patterns: 0,
            pump_dump_signals: 0,
            spoofing_attempts: 0,
        }
    }
    
    /// Add new price point and check for manipulation
    pub fn add_price_point(
        &mut self,
        price: u64,
        volume: u64,
        trader: Pubkey,
        outcome: u8,
    ) -> Result<ManipulationCheck, ProgramError> {
        let clock = Clock::get()?;
        let current_slot = clock.slot;
        
        let point = PricePoint {
            slot: current_slot,
            price,
            volume,
            trader,
            outcome,
        };
        
        // Add to circular buffer
        if self.price_history.len() < PRICE_HISTORY_WINDOW {
            self.price_history.push(point);
        } else {
            self.price_history[self.history_index] = point;
            self.history_index = (self.history_index + 1) % PRICE_HISTORY_WINDOW;
        }
        
        // Update statistics
        self.update_statistics()?;
        
        // Run detection algorithms
        let z_score = self.calculate_z_score(price)?;
        let volume_spike = self.detect_volume_spike(volume);
        let velocity = self.calculate_price_velocity()?;
        let wash_trade = self.detect_wash_trading(&trader)?;
        let pump_dump = self.detect_pump_dump()?;
        let spoofing = self.detect_spoofing()?;
        
        // Calculate manipulation score
        let mut score = 0u8;
        
        if z_score.abs() > Z_SCORE_THRESHOLD {
            score += 25;
            self.anomaly_count += 1;
        }
        
        if volume_spike {
            score += 20;
        }
        
        if velocity > PRICE_VELOCITY_THRESHOLD {
            score += 20;
        }
        
        if wash_trade {
            score += 15;
            self.wash_trade_patterns += 1;
        }
        
        if pump_dump {
            score += 15;
            self.pump_dump_signals += 1;
        }
        
        if spoofing {
            score += 5;
            self.spoofing_attempts += 1;
        }
        
        self.manipulation_score = score.min(100);
        
        // Determine action
        let action = if score >= 80 {
            ManipulationAction::HaltTrading
        } else if score >= 60 {
            ManipulationAction::IncreaseMonitoring
        } else if score >= 40 {
            ManipulationAction::Alert
        } else {
            ManipulationAction::Continue
        };
        
        Ok(ManipulationCheck {
            manipulation_score: score,
            z_score,
            volume_spike,
            price_velocity: velocity,
            wash_trade_detected: wash_trade,
            pump_dump_detected: pump_dump,
            spoofing_detected: spoofing,
            recommended_action: action,
        })
    }
    
    /// Update rolling statistics
    fn update_statistics(&mut self) -> Result<(), ProgramError> {
        if self.price_history.is_empty() {
            return Ok(());
        }
        
        let n = self.price_history.len() as u64;
        
        // Calculate mean
        let sum: u64 = self.price_history.iter().map(|p| p.price).sum();
        self.price_mean = U64F64::from_num(sum) / U64F64::from_num(n);
        
        // Calculate variance
        let mut variance_sum = U64F64::from_num(0);
        for point in &self.price_history {
            let diff = U64F64::from_num(point.price) - self.price_mean;
            let diff_squared = diff * diff;
            variance_sum = variance_sum + diff_squared;
        }
        self.price_variance = variance_sum / U64F64::from_num(n);
        
        // Calculate volume mean
        let volume_sum: u64 = self.price_history.iter().map(|p| p.volume).sum();
        self.volume_mean = volume_sum / n;
        
        Ok(())
    }
    
    /// Calculate z-score for price
    fn calculate_z_score(&self, price: u64) -> Result<f64, ProgramError> {
        if self.price_variance == U64F64::from_num(0) {
            return Ok(0.0);
        }
        
        let price_fp = U64F64::from_num(price);
        let diff = price_fp - self.price_mean;
        
        // Calculate standard deviation
        let std_dev = self.price_variance.sqrt()?;
        
        // Z-score = (x - μ) / σ
        let z_score = diff / std_dev;
        
        // Convert to f64 for comparison
        // For now, return z_score as integer approximation
        Ok(z_score.to_num() as f64)
    }
    
    /// Detect volume spikes
    fn detect_volume_spike(&self, volume: u64) -> bool {
        if self.volume_mean == 0 {
            return false;
        }
        
        volume > self.volume_mean * VOLUME_SPIKE_THRESHOLD / 100
    }
    
    /// Calculate price velocity (rate of change)
    fn calculate_price_velocity(&self) -> Result<u64, ProgramError> {
        if self.price_history.len() < 2 {
            return Ok(0);
        }
        
        let recent_prices: Vec<&PricePoint> = self.price_history
            .iter()
            .rev()
            .take(5)
            .collect();
        
        if recent_prices.len() < 2 {
            return Ok(0);
        }
        
        let price_change = recent_prices[0].price.abs_diff(recent_prices[recent_prices.len() - 1].price);
        let slot_diff = recent_prices[0].slot - recent_prices[recent_prices.len() - 1].slot;
        
        if slot_diff == 0 {
            return Ok(0);
        }
        
        // Velocity in basis points per slot
        Ok((price_change * 10000) / (recent_prices[recent_prices.len() - 1].price * slot_diff))
    }
    
    /// Detect wash trading patterns
    fn detect_wash_trading(&self, trader: &Pubkey) -> Result<bool, ProgramError> {
        let mut same_trader_trades = 0;
        let mut alternating_trades = 0;
        
        for i in 0..self.price_history.len().saturating_sub(4) {
            if self.price_history[i].trader == *trader {
                same_trader_trades += 1;
            }
            
            // Check for alternating buy/sell pattern
            if i + 1 < self.price_history.len() {
                let price_diff = self.price_history[i].price as i64 - self.price_history[i + 1].price as i64;
                if price_diff.abs() < 100 { // Small price movements
                    alternating_trades += 1;
                }
            }
        }
        
        // Wash trading if same trader appears frequently with small price moves
        Ok(same_trader_trades >= 3 && alternating_trades >= 2)
    }
    
    /// Detect pump and dump patterns
    fn detect_pump_dump(&self) -> Result<bool, ProgramError> {
        if self.price_history.len() < 10 {
            return Ok(false);
        }
        
        // Look for rapid price increase followed by volume spike and price drop
        let recent_10: Vec<&PricePoint> = self.price_history
            .iter()
            .rev()
            .take(10)
            .collect();
        
        // Phase 1: Price pump (first 5 trades)
        let mut pump_detected = false;
        if recent_10.len() >= 5 {
            let start_price = recent_10[4].price;
            let peak_price = recent_10[2].price;
            
            if peak_price > start_price * 120 / 100 { // 20% increase
                pump_detected = true;
            }
        }
        
        // Phase 2: Volume spike and dump (last 5 trades)
        let mut dump_detected = false;
        if pump_detected && recent_10.len() >= 3 {
            let peak_price = recent_10[2].price;
            let current_price = recent_10[0].price;
            let recent_volume = recent_10[0].volume;
            
            if current_price < peak_price * 85 / 100 && // 15% drop
               recent_volume > self.volume_mean * 2 { // 2x volume
                dump_detected = true;
            }
        }
        
        Ok(pump_detected && dump_detected)
    }
    
    /// Detect spoofing (fake orders)
    fn detect_spoofing(&self) -> Result<bool, ProgramError> {
        // Look for large volume with minimal price impact
        if self.price_history.len() < 3 {
            return Ok(false);
        }
        
        let recent: Vec<&PricePoint> = self.price_history
            .iter()
            .rev()
            .take(3)
            .collect();
        
        for point in &recent {
            if point.volume > self.volume_mean * 3 { // 3x average volume
                // Check if price barely moved despite large volume
                let price_impact = point.price.abs_diff(self.price_mean.to_num());
                let expected_impact = point.volume * 100 / (self.volume_mean + 1); // Basis points
                
                if price_impact < expected_impact / 10 { // Less than 10% of expected impact
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// Get current risk assessment
    pub fn get_risk_assessment(&self) -> RiskAssessment {
        RiskAssessment {
            manipulation_score: self.manipulation_score,
            anomaly_count: self.anomaly_count,
            wash_trade_patterns: self.wash_trade_patterns,
            pump_dump_signals: self.pump_dump_signals,
            spoofing_attempts: self.spoofing_attempts,
            risk_level: match self.manipulation_score {
                0..=20 => RiskLevel::Low,
                21..=40 => RiskLevel::Medium,
                41..=60 => RiskLevel::High,
                61..=80 => RiskLevel::Critical,
                _ => RiskLevel::Emergency,
            },
        }
    }
}

/// Manipulation check result
#[derive(Debug)]
pub struct ManipulationCheck {
    pub manipulation_score: u8,
    pub z_score: f64,
    pub volume_spike: bool,
    pub price_velocity: u64,
    pub wash_trade_detected: bool,
    pub pump_dump_detected: bool,
    pub spoofing_detected: bool,
    pub recommended_action: ManipulationAction,
}

/// Recommended action based on detection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ManipulationAction {
    Continue,           // Normal trading
    Alert,              // Log alert but continue
    IncreaseMonitoring, // Increase monitoring frequency
    HaltTrading,        // Halt trading immediately
}

/// Risk assessment summary
#[derive(Debug)]
pub struct RiskAssessment {
    pub manipulation_score: u8,
    pub anomaly_count: u32,
    pub wash_trade_patterns: u32,
    pub pump_dump_signals: u32,
    pub spoofing_attempts: u32,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
    Emergency,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_z_score_calculation() {
        let mut detector = PriceManipulationDetector::new([0; 32]);
        
        // Add normal prices
        for i in 0..10 {
            detector.add_price_point(
                50000 + i * 100, // Price around 50000
                1000,
                Pubkey::new_unique(),
                0,
            ).unwrap();
        }
        
        // Add anomalous price
        let result = detector.add_price_point(
            70000, // Significant deviation
            1000,
            Pubkey::new_unique(),
            0,
        ).unwrap();
        
        assert!(result.z_score.abs() > 2.0);
        assert!(result.manipulation_score > 0);
    }
    
    #[test]
    fn test_wash_trade_detection() {
        let mut detector = PriceManipulationDetector::new([0; 32]);
        let trader = Pubkey::new_unique();
        
        // Simulate wash trading pattern
        for i in 0..6 {
            let price = if i % 2 == 0 { 50000 } else { 50050 };
            detector.add_price_point(
                price,
                1000,
                trader, // Same trader
                0,
            ).unwrap();
        }
        
        let result = detector.add_price_point(
            50000,
            1000,
            trader,
            0,
        ).unwrap();
        
        assert!(result.wash_trade_detected);
        assert!(detector.wash_trade_patterns > 0);
    }
}