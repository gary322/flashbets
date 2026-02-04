//! Anomaly Detection System
//!
//! Implements comprehensive anomaly detection:
//! - Statistical analysis with z-scores
//! - Pattern recognition for unusual activity
//! - Configurable alert thresholds
//! - Real-time monitoring
//!
//! Per specification: Production-grade anomaly detection

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use std::collections::{HashMap, VecDeque};

use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
};

/// Statistical thresholds
pub const Z_SCORE_THRESHOLD: f64 = 3.0; // 3 standard deviations
pub const MIN_SAMPLE_SIZE: usize = 30;
pub const CONFIDENCE_INTERVAL: f64 = 0.95;
pub const ANOMALY_WINDOW_SECONDS: i64 = 3600; // 1 hour

/// Anomaly types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum AnomalyType {
    PriceSpike { market_id: [u8; 16], deviation: f64 },
    VolumeSpike { market_id: [u8; 16], multiplier: f64 },
    RapidTrading { user: Pubkey, trades_per_minute: u32 },
    UnusualPattern { pattern_id: String, confidence: f64 },
    MarketManipulation { market_id: [u8; 16], evidence_score: u64 },
    FlashCrash { market_id: [u8; 16], drop_percentage: f64 },
    LiquidityDrain { pool_id: [u8; 16], drain_percentage: f64 },
    ConcentratedActivity { market_id: [u8; 16], concentration: f64 },
}

impl AnomalyType {
    /// Get severity of anomaly
    pub fn severity(&self) -> AnomalySeverity {
        match self {
            AnomalyType::MarketManipulation { evidence_score, .. } => {
                if *evidence_score > 80 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                }
            }
            AnomalyType::FlashCrash { drop_percentage, .. } => {
                if *drop_percentage > 50.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                }
            }
            AnomalyType::LiquidityDrain { drain_percentage, .. } => {
                if *drain_percentage > 80.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::High
                }
            }
            AnomalyType::PriceSpike { deviation, .. } => {
                if *deviation > 5.0 {
                    AnomalySeverity::High
                } else {
                    AnomalySeverity::Medium
                }
            }
            AnomalyType::VolumeSpike { multiplier, .. } => {
                if *multiplier > 10.0 {
                    AnomalySeverity::High
                } else {
                    AnomalySeverity::Medium
                }
            }
            AnomalyType::RapidTrading { trades_per_minute, .. } => {
                if *trades_per_minute > 100 {
                    AnomalySeverity::High
                } else {
                    AnomalySeverity::Medium
                }
            }
            AnomalyType::ConcentratedActivity { concentration, .. } => {
                if *concentration > 0.8 {
                    AnomalySeverity::High
                } else {
                    AnomalySeverity::Medium
                }
            }
            AnomalyType::UnusualPattern { .. } => AnomalySeverity::Low,
        }
    }
}

/// Anomaly severity levels
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnomalySeverity {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Anomaly detection result
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct AnomalyResult {
    pub anomaly_id: [u8; 16],
    pub timestamp: i64,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub confidence_score: f64,
    pub affected_entities: Vec<AffectedEntity>,
    pub recommended_action: RecommendedAction,
    pub details: String,
}

impl AnomalyResult {
    pub const SIZE: usize = 512;

    /// Create new anomaly result
    pub fn new(
        anomaly_type: AnomalyType,
        confidence_score: f64,
        timestamp: i64,
    ) -> Self {
        let anomaly_id = Self::generate_id(&anomaly_type, timestamp);
        let severity = anomaly_type.severity();
        
        Self {
            anomaly_id,
            timestamp,
            anomaly_type,
            severity,
            confidence_score,
            affected_entities: Vec::new(),
            recommended_action: RecommendedAction::Monitor,
            details: String::new(),
        }
    }

    /// Generate unique anomaly ID
    fn generate_id(anomaly_type: &AnomalyType, timestamp: i64) -> [u8; 16] {
        use solana_program::keccak;
        
        let type_bytes = match anomaly_type {
            AnomalyType::PriceSpike { market_id, .. } => market_id.to_vec(),
            AnomalyType::VolumeSpike { market_id, .. } => market_id.to_vec(),
            AnomalyType::RapidTrading { user, .. } => user.to_bytes().to_vec(),
            _ => vec![0u8; 16],
        };
        
        let hash = keccak::hashv(&[
            &type_bytes,
            &timestamp.to_le_bytes(),
        ]);
        
        let mut id = [0u8; 16];
        id.copy_from_slice(&hash.0[..16]);
        id
    }
}

/// Affected entity
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum AffectedEntity {
    Market { id: [u8; 16] },
    User { pubkey: Pubkey },
    Pool { id: [u8; 16] },
    Verse { id: u128 },
}

/// Recommended action
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub enum RecommendedAction {
    Monitor,
    Alert,
    InvestigateManually,
    FreezeTradingTemporarily,
    HaltMarket,
    RollbackTransactions,
}

/// Statistical analyzer for anomaly detection
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct StatisticalAnalyzer {
    pub price_history: HashMap<[u8; 16], PriceStatistics>,
    pub volume_history: HashMap<[u8; 16], VolumeStatistics>,
    pub user_activity: HashMap<Pubkey, UserActivityStats>,
    pub pattern_detector: PatternDetector,
}

impl StatisticalAnalyzer {
    pub const SIZE: usize = 1024 * 64; // 64KB

    pub fn new() -> Self {
        Self {
            price_history: HashMap::new(),
            volume_history: HashMap::new(),
            user_activity: HashMap::new(),
            pattern_detector: PatternDetector::new(),
        }
    }

    /// Update price statistics
    pub fn update_price(
        &mut self,
        market_id: [u8; 16],
        price: f64,
        timestamp: i64,
    ) -> Option<AnomalyResult> {
        let stats = self.price_history
            .entry(market_id)
            .or_insert_with(|| PriceStatistics::new());
        
        // Calculate z-score
        let z_score = stats.calculate_z_score(price);
        
        // Update statistics
        stats.add_price(price, timestamp);
        
        // Check for anomaly
        if z_score.abs() > Z_SCORE_THRESHOLD && stats.sample_count >= MIN_SAMPLE_SIZE {
            let anomaly = AnomalyType::PriceSpike {
                market_id,
                deviation: z_score,
            };
            
            let mut result = AnomalyResult::new(anomaly, z_score.abs() / 5.0, timestamp);
            result.affected_entities.push(AffectedEntity::Market { id: market_id });
            result.details = format!("Price deviation of {:.2} standard deviations detected", z_score);
            
            if z_score.abs() > 5.0 {
                result.recommended_action = RecommendedAction::FreezeTradingTemporarily;
            } else {
                result.recommended_action = RecommendedAction::Alert;
            }
            
            return Some(result);
        }
        
        None
    }

    /// Update volume statistics
    pub fn update_volume(
        &mut self,
        market_id: [u8; 16],
        volume: u64,
        timestamp: i64,
    ) -> Option<AnomalyResult> {
        let stats = self.volume_history
            .entry(market_id)
            .or_insert_with(|| VolumeStatistics::new());
        
        // Calculate volume multiplier
        let avg_volume = stats.get_average();
        let multiplier = if avg_volume > 0 {
            volume as f64 / avg_volume as f64
        } else {
            1.0
        };
        
        // Update statistics
        stats.add_volume(volume, timestamp);
        
        // Check for anomaly
        if multiplier > 5.0 && stats.sample_count >= MIN_SAMPLE_SIZE {
            let anomaly = AnomalyType::VolumeSpike {
                market_id,
                multiplier,
            };
            
            let mut result = AnomalyResult::new(
                anomaly,
                (multiplier / 10.0).min(1.0),
                timestamp,
            );
            result.affected_entities.push(AffectedEntity::Market { id: market_id });
            result.details = format!("Volume spike of {:.1}x average detected", multiplier);
            result.recommended_action = RecommendedAction::InvestigateManually;
            
            return Some(result);
        }
        
        None
    }

    /// Track user activity
    pub fn track_user_activity(
        &mut self,
        user: Pubkey,
        activity_type: ActivityType,
        timestamp: i64,
    ) -> Option<AnomalyResult> {
        let stats = self.user_activity
            .entry(user)
            .or_insert_with(|| UserActivityStats::new());
        
        stats.add_activity(activity_type, timestamp);
        
        // Check for rapid trading
        let trades_per_minute = stats.get_trades_per_minute(timestamp);
        if trades_per_minute > 30 {
            let anomaly = AnomalyType::RapidTrading {
                user,
                trades_per_minute,
            };
            
            let mut result = AnomalyResult::new(
                anomaly,
                (trades_per_minute as f64 / 100.0).min(1.0),
                timestamp,
            );
            result.affected_entities.push(AffectedEntity::User { pubkey: user });
            result.details = format!("{} trades per minute detected", trades_per_minute);
            result.recommended_action = RecommendedAction::InvestigateManually;
            
            return Some(result);
        }
        
        None
    }

    /// Detect flash crash
    pub fn detect_flash_crash(
        &mut self,
        market_id: [u8; 16],
        current_price: f64,
        timestamp: i64,
    ) -> Option<AnomalyResult> {
        if let Some(stats) = self.price_history.get(&market_id) {
            let recent_high = stats.get_recent_high(timestamp - 300); // 5 minutes
            
            if recent_high > 0.0 {
                let drop_percentage = ((recent_high - current_price) / recent_high) * 100.0;
                
                if drop_percentage > 30.0 {
                    let anomaly = AnomalyType::FlashCrash {
                        market_id,
                        drop_percentage,
                    };
                    
                    let mut result = AnomalyResult::new(
                        anomaly,
                        (drop_percentage / 50.0).min(1.0),
                        timestamp,
                    );
                    result.affected_entities.push(AffectedEntity::Market { id: market_id });
                    result.details = format!("Flash crash: {:.1}% drop in 5 minutes", drop_percentage);
                    result.recommended_action = RecommendedAction::HaltMarket;
                    
                    return Some(result);
                }
            }
        }
        
        None
    }

    /// Detect market manipulation patterns
    pub fn detect_manipulation(
        &mut self,
        market_id: [u8; 16],
        trading_data: &TradingData,
        timestamp: i64,
    ) -> Option<AnomalyResult> {
        let evidence_score = self.pattern_detector.calculate_manipulation_score(trading_data);
        
        if evidence_score > 60 {
            let anomaly = AnomalyType::MarketManipulation {
                market_id,
                evidence_score,
            };
            
            let mut result = AnomalyResult::new(
                anomaly,
                evidence_score as f64 / 100.0,
                timestamp,
            );
            result.affected_entities.push(AffectedEntity::Market { id: market_id });
            result.details = format!("Manipulation evidence score: {}/100", evidence_score);
            
            if evidence_score > 80 {
                result.recommended_action = RecommendedAction::FreezeTradingTemporarily;
            } else {
                result.recommended_action = RecommendedAction::InvestigateManually;
            }
            
            return Some(result);
        }
        
        None
    }
}

/// Price statistics
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PriceStatistics {
    pub prices: VecDeque<(f64, i64)>, // (price, timestamp)
    pub sum: f64,
    pub sum_squared: f64,
    pub sample_count: usize,
    pub mean: f64,
    pub std_dev: f64,
}

impl PriceStatistics {
    pub const MAX_SAMPLES: usize = 1000;

    pub fn new() -> Self {
        Self {
            prices: VecDeque::new(),
            sum: 0.0,
            sum_squared: 0.0,
            sample_count: 0,
            mean: 0.0,
            std_dev: 0.0,
        }
    }

    pub fn add_price(&mut self, price: f64, timestamp: i64) {
        self.prices.push_back((price, timestamp));
        self.sum += price;
        self.sum_squared += price * price;
        self.sample_count += 1;

        // Maintain size limit
        while self.prices.len() > Self::MAX_SAMPLES {
            if let Some((old_price, _)) = self.prices.pop_front() {
                self.sum -= old_price;
                self.sum_squared -= old_price * old_price;
                self.sample_count -= 1;
            }
        }

        // Update statistics
        self.update_statistics();
    }

    fn update_statistics(&mut self) {
        if self.sample_count > 0 {
            self.mean = self.sum / self.sample_count as f64;
            
            if self.sample_count > 1 {
                let variance = (self.sum_squared / self.sample_count as f64) - (self.mean * self.mean);
                self.std_dev = variance.max(0.0).sqrt();
            }
        }
    }

    pub fn calculate_z_score(&self, value: f64) -> f64 {
        if self.std_dev > 0.0 {
            (value - self.mean) / self.std_dev
        } else {
            0.0
        }
    }

    pub fn get_recent_high(&self, since_timestamp: i64) -> f64 {
        self.prices
            .iter()
            .filter(|(_, ts)| *ts >= since_timestamp)
            .map(|(price, _)| *price)
            .fold(0.0, f64::max)
    }
}

/// Volume statistics
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct VolumeStatistics {
    pub volumes: VecDeque<(u64, i64)>, // (volume, timestamp)
    pub total_volume: u64,
    pub sample_count: usize,
}

impl VolumeStatistics {
    pub const MAX_SAMPLES: usize = 100;

    pub fn new() -> Self {
        Self {
            volumes: VecDeque::new(),
            total_volume: 0,
            sample_count: 0,
        }
    }

    pub fn add_volume(&mut self, volume: u64, timestamp: i64) {
        self.volumes.push_back((volume, timestamp));
        self.total_volume = self.total_volume.saturating_add(volume);
        self.sample_count += 1;

        // Maintain size limit
        while self.volumes.len() > Self::MAX_SAMPLES {
            if let Some((old_volume, _)) = self.volumes.pop_front() {
                self.total_volume = self.total_volume.saturating_sub(old_volume);
                self.sample_count -= 1;
            }
        }
    }

    pub fn get_average(&self) -> u64 {
        if self.sample_count > 0 {
            self.total_volume / self.sample_count as u64
        } else {
            0
        }
    }
}

/// User activity statistics
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct UserActivityStats {
    pub activities: VecDeque<(ActivityType, i64)>,
    pub trade_count: u32,
    pub last_activity: i64,
}

impl UserActivityStats {
    pub const MAX_ACTIVITIES: usize = 1000;

    pub fn new() -> Self {
        Self {
            activities: VecDeque::new(),
            trade_count: 0,
            last_activity: 0,
        }
    }

    pub fn add_activity(&mut self, activity: ActivityType, timestamp: i64) {
        // Check if it's a trade before moving
        let is_trade = matches!(activity, ActivityType::Trade);
        
        self.activities.push_back((activity, timestamp));
        
        if is_trade {
            self.trade_count += 1;
        }
        
        self.last_activity = timestamp;

        // Maintain size limit
        while self.activities.len() > Self::MAX_ACTIVITIES {
            if let Some((old_activity, _)) = self.activities.pop_front() {
                if matches!(old_activity, ActivityType::Trade) {
                    self.trade_count = self.trade_count.saturating_sub(1);
                }
            }
        }
    }

    pub fn get_trades_per_minute(&self, current_timestamp: i64) -> u32 {
        let one_minute_ago = current_timestamp - 60;
        
        self.activities
            .iter()
            .filter(|(activity, ts)| {
                *ts >= one_minute_ago && matches!(activity, ActivityType::Trade)
            })
            .count() as u32
    }
}

/// Activity types
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub enum ActivityType {
    Trade,
    Deposit,
    Withdrawal,
    CreateMarket,
    AddLiquidity,
    RemoveLiquidity,
}

/// Pattern detector for complex anomalies
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PatternDetector {
    pub known_patterns: HashMap<String, Pattern>,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self {
            known_patterns: Self::init_patterns(),
        }
    }

    fn init_patterns() -> HashMap<String, Pattern> {
        let mut patterns = HashMap::new();
        
        // Pump and dump pattern
        patterns.insert(
            "pump_and_dump".to_string(),
            Pattern {
                id: "pump_and_dump".to_string(),
                features: vec![
                    PatternFeature::RapidPriceIncrease { threshold: 50.0 },
                    PatternFeature::HighVolumeSpike { multiplier: 10.0 },
                    PatternFeature::ConcentratedBuying { concentration: 0.7 },
                ],
                min_confidence: 0.7,
            },
        );
        
        // Wash trading pattern
        patterns.insert(
            "wash_trading".to_string(),
            Pattern {
                id: "wash_trading".to_string(),
                features: vec![
                    PatternFeature::RepeatedTrades { threshold: 20 },
                    PatternFeature::MinimalPriceMovement { threshold: 1.0 },
                    PatternFeature::SameUserPattern { threshold: 0.8 },
                ],
                min_confidence: 0.8,
            },
        );
        
        patterns
    }

    pub fn calculate_manipulation_score(&self, trading_data: &TradingData) -> u64 {
        let mut max_score = 0u64;
        
        for pattern in self.known_patterns.values() {
            let score = pattern.match_score(trading_data);
            max_score = max_score.max(score);
        }
        
        max_score
    }
}

/// Pattern definition
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct Pattern {
    pub id: String,
    pub features: Vec<PatternFeature>,
    pub min_confidence: f64,
}

impl Pattern {
    pub fn match_score(&self, data: &TradingData) -> u64 {
        let mut matched_features = 0;
        
        for feature in &self.features {
            if feature.matches(data) {
                matched_features += 1;
            }
        }
        
        let confidence = matched_features as f64 / self.features.len() as f64;
        
        if confidence >= self.min_confidence {
            (confidence * 100.0) as u64
        } else {
            0
        }
    }
}

/// Pattern features
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum PatternFeature {
    RapidPriceIncrease { threshold: f64 },
    HighVolumeSpike { multiplier: f64 },
    ConcentratedBuying { concentration: f64 },
    RepeatedTrades { threshold: u32 },
    MinimalPriceMovement { threshold: f64 },
    SameUserPattern { threshold: f64 },
}

impl PatternFeature {
    pub fn matches(&self, data: &TradingData) -> bool {
        match self {
            PatternFeature::RapidPriceIncrease { threshold } => {
                data.price_change_percentage > *threshold
            }
            PatternFeature::HighVolumeSpike { multiplier } => {
                data.volume_multiplier > *multiplier
            }
            PatternFeature::ConcentratedBuying { concentration } => {
                data.buyer_concentration > *concentration
            }
            PatternFeature::RepeatedTrades { threshold } => {
                data.repeated_trades > *threshold
            }
            PatternFeature::MinimalPriceMovement { threshold } => {
                data.price_volatility < *threshold
            }
            PatternFeature::SameUserPattern { threshold } => {
                data.same_user_ratio > *threshold
            }
        }
    }
}

/// Trading data for pattern matching
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct TradingData {
    pub price_change_percentage: f64,
    pub volume_multiplier: f64,
    pub buyer_concentration: f64,
    pub repeated_trades: u32,
    pub price_volatility: f64,
    pub same_user_ratio: f64,
}

/// Anomaly detector main engine
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AnomalyDetector {
    pub analyzer: StatisticalAnalyzer,
    pub alert_manager: AlertManager,
    pub detection_history: VecDeque<AnomalyResult>,
}

impl AnomalyDetector {
    pub const SIZE: usize = 1024 * 128; // 128KB

    pub fn new() -> Self {
        Self {
            analyzer: StatisticalAnalyzer::new(),
            alert_manager: AlertManager::new(),
            detection_history: VecDeque::new(),
        }
    }

    /// Process market update
    pub fn process_market_update(
        &mut self,
        market_id: [u8; 16],
        price: f64,
        volume: u64,
        timestamp: i64,
    ) -> Vec<AnomalyResult> {
        let mut anomalies = Vec::new();

        // Check price anomaly
        if let Some(anomaly) = self.analyzer.update_price(market_id, price, timestamp) {
            anomalies.push(anomaly);
        }

        // Check volume anomaly
        if let Some(anomaly) = self.analyzer.update_volume(market_id, volume, timestamp) {
            anomalies.push(anomaly);
        }

        // Check flash crash
        if let Some(anomaly) = self.analyzer.detect_flash_crash(market_id, price, timestamp) {
            anomalies.push(anomaly);
        }

        // Process alerts
        for anomaly in &anomalies {
            self.alert_manager.process_anomaly(anomaly);
            self.add_to_history(anomaly.clone());
        }

        anomalies
    }

    /// Add anomaly to history
    fn add_to_history(&mut self, anomaly: AnomalyResult) {
        self.detection_history.push_back(anomaly);

        // Maintain size limit
        while self.detection_history.len() > 10000 {
            self.detection_history.pop_front();
        }
    }

    /// Get recent anomalies
    pub fn get_recent_anomalies(&self, window_seconds: i64) -> Vec<&AnomalyResult> {
        let cutoff = Clock::get().unwrap().unix_timestamp - window_seconds;
        
        self.detection_history
            .iter()
            .filter(|a| a.timestamp >= cutoff)
            .collect()
    }
}

/// Alert manager
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct AlertManager {
    pub alert_thresholds: HashMap<AnomalySeverity, u32>,
    pub alert_counts: HashMap<AnomalySeverity, u32>,
    pub last_alert_time: HashMap<AnomalySeverity, i64>,
}

impl AlertManager {
    pub fn new() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert(AnomalySeverity::Low, 10);
        thresholds.insert(AnomalySeverity::Medium, 5);
        thresholds.insert(AnomalySeverity::High, 2);
        thresholds.insert(AnomalySeverity::Critical, 1);

        Self {
            alert_thresholds: thresholds,
            alert_counts: HashMap::new(),
            last_alert_time: HashMap::new(),
        }
    }

    pub fn process_anomaly(&mut self, anomaly: &AnomalyResult) {
        // Get or insert count and increment it
        let current_count = {
            let count = self.alert_counts
                .entry(anomaly.severity.clone())
                .or_insert(0);
            *count += 1;
            *count
        };

        let threshold = self.alert_thresholds
            .get(&anomaly.severity)
            .copied()
            .unwrap_or(1);

        if current_count >= threshold {
            self.trigger_alert(anomaly);
            // Reset counter after triggering alert
            if let Some(count) = self.alert_counts.get_mut(&anomaly.severity) {
                *count = 0;
            }
        }
    }

    fn trigger_alert(&mut self, anomaly: &AnomalyResult) {
        msg!("ANOMALY ALERT: {:?} - Severity: {:?}", 
            anomaly.anomaly_type, anomaly.severity);
        
        let current_time = Clock::get().unwrap().unix_timestamp;
        self.last_alert_time.insert(anomaly.severity.clone(), current_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z_score_calculation() {
        let mut stats = PriceStatistics::new();
        
        // Add normal prices
        for i in 95..=105 {
            stats.add_price(i as f64, i as i64);
        }

        // Test normal price
        let z_score = stats.calculate_z_score(100.0);
        assert!(z_score.abs() < 1.0);

        // Test anomalous price
        let z_score = stats.calculate_z_score(150.0);
        assert!(z_score > 3.0);
    }

    #[test]
    fn test_anomaly_detection() {
        let mut analyzer = StatisticalAnalyzer::new();
        let market_id = [0u8; 16];

        // Build up history
        for i in 0..50 {
            analyzer.update_price(market_id, 100.0 + (i % 5) as f64, i as i64);
        }

        // Detect price spike
        let anomaly = analyzer.update_price(market_id, 200.0, 51);
        assert!(anomaly.is_some());
        
        if let Some(result) = anomaly {
            assert!(matches!(result.anomaly_type, AnomalyType::PriceSpike { .. }));
            assert!(result.severity >= AnomalySeverity::Medium);
        }
    }

    #[test]
    fn test_pattern_matching() {
        let detector = PatternDetector::new();
        
        let trading_data = TradingData {
            price_change_percentage: 60.0,
            volume_multiplier: 12.0,
            buyer_concentration: 0.75,
            repeated_trades: 5,
            price_volatility: 0.5,
            same_user_ratio: 0.3,
        };

        let score = detector.calculate_manipulation_score(&trading_data);
        assert!(score > 50); // Should detect pump and dump pattern
    }
}