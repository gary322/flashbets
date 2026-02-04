//! Automatic Priority Fee System
//!
//! Implements dynamic priority fees based on network congestion
//! Formula: priority_fee = base_fee + congestion_factor * dynamic_fee

use solana_program::{
    msg,
    program_error::ProgramError,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::error::BettingPlatformError;

/// Priority fee configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriorityFeeConfig {
    /// Base priority fee in microlamports per compute unit
    pub base_fee_micro_lamports: u64,
    /// Maximum priority fee in microlamports per compute unit
    pub max_fee_micro_lamports: u64,
    /// Congestion threshold (TPS)
    pub congestion_threshold_tps: u32,
    /// High congestion threshold (TPS)
    pub high_congestion_threshold_tps: u32,
    /// Update interval in slots
    pub update_interval_slots: u64,
    /// Smoothing factor for congestion calculation (0-100)
    pub smoothing_factor: u8,
}

impl Default for PriorityFeeConfig {
    fn default() -> Self {
        Self {
            base_fee_micro_lamports: 1_000,        // 0.001 SOL per 1M CU
            max_fee_micro_lamports: 50_000,       // 0.05 SOL per 1M CU max
            congestion_threshold_tps: 2_000,       // Start increasing at 2k TPS
            high_congestion_threshold_tps: 4_000,  // Max fee at 4k TPS
            update_interval_slots: 10,             // Update every 10 slots (~4s)
            smoothing_factor: 80,                  // 80% weight to previous value
        }
    }
}

/// Network congestion metrics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CongestionMetrics {
    /// Current slot
    pub current_slot: u64,
    /// Last update slot
    pub last_update_slot: u64,
    /// Current transactions per second
    pub current_tps: u32,
    /// Average TPS over last minute
    pub avg_tps_1min: u32,
    /// Average TPS over last 5 minutes
    pub avg_tps_5min: u32,
    /// Current congestion factor (0-100)
    pub congestion_factor: u8,
    /// Historical TPS samples
    pub tps_history: Vec<u32>,
}

impl Default for CongestionMetrics {
    fn default() -> Self {
        Self {
            current_slot: 0,
            last_update_slot: 0,
            current_tps: 0,
            avg_tps_1min: 0,
            avg_tps_5min: 0,
            congestion_factor: 0,
            tps_history: Vec::with_capacity(300), // 5 minutes of data at 1 sample/sec
        }
    }
}

/// Priority fee calculator
pub struct PriorityFeeCalculator {
    config: PriorityFeeConfig,
    metrics: CongestionMetrics,
}

impl PriorityFeeCalculator {
    /// Create new calculator with default config
    pub fn new() -> Self {
        Self {
            config: PriorityFeeConfig::default(),
            metrics: CongestionMetrics::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: PriorityFeeConfig) -> Self {
        Self {
            config,
            metrics: CongestionMetrics::default(),
        }
    }

    /// Calculate current priority fee based on network conditions
    pub fn calculate_priority_fee(
        &self,
        compute_units: u64,
    ) -> Result<u64, ProgramError> {
        // Base calculation: fee = base + (congestion_factor / 100) * (max - base)
        let base_fee = self.config.base_fee_micro_lamports;
        let max_fee = self.config.max_fee_micro_lamports;
        let congestion_factor = self.metrics.congestion_factor as u64;
        
        // Calculate dynamic component
        let dynamic_range = max_fee.saturating_sub(base_fee);
        let dynamic_fee = (dynamic_range * congestion_factor) / 100;
        
        // Total fee per compute unit
        let fee_per_cu = base_fee + dynamic_fee;
        
        // Calculate total fee for requested compute units
        let total_fee = (fee_per_cu * compute_units) / 1_000_000; // Convert to lamports
        
        msg!("Priority fee calculation: {} CU @ {} microlamports/CU = {} lamports",
             compute_units, fee_per_cu, total_fee);
        
        Ok(total_fee)
    }

    /// Update congestion metrics based on current network state
    pub fn update_metrics(
        &mut self,
        current_tps: u32,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check if update is needed
        if current_slot < self.metrics.last_update_slot + self.config.update_interval_slots {
            return Ok(());
        }

        // Update TPS history
        self.metrics.tps_history.push(current_tps);
        if self.metrics.tps_history.len() > 300 {
            self.metrics.tps_history.remove(0);
        }

        // Calculate averages
        self.update_tps_averages();

        // Update congestion factor
        self.update_congestion_factor(current_tps);

        // Update timestamps
        self.metrics.current_tps = current_tps;
        self.metrics.current_slot = current_slot;
        self.metrics.last_update_slot = current_slot;

        msg!("Updated congestion metrics: TPS={}, factor={}%",
             current_tps, self.metrics.congestion_factor);

        Ok(())
    }

    /// Calculate TPS averages
    fn update_tps_averages(&mut self) {
        let history_len = self.metrics.tps_history.len();
        
        if history_len == 0 {
            return;
        }

        // 1-minute average (last 60 samples)
        let one_min_samples = history_len.min(60);
        let one_min_sum: u32 = self.metrics.tps_history
            .iter()
            .rev()
            .take(one_min_samples)
            .sum();
        self.metrics.avg_tps_1min = one_min_sum / one_min_samples as u32;

        // 5-minute average (all samples)
        let five_min_sum: u32 = self.metrics.tps_history.iter().sum();
        self.metrics.avg_tps_5min = five_min_sum / history_len as u32;
    }

    /// Update congestion factor based on current TPS
    fn update_congestion_factor(&mut self, current_tps: u32) {
        let threshold = self.config.congestion_threshold_tps;
        let high_threshold = self.config.high_congestion_threshold_tps;
        
        // Calculate raw congestion factor
        let raw_factor = if current_tps <= threshold {
            0
        } else if current_tps >= high_threshold {
            100
        } else {
            // Linear interpolation between thresholds
            let range = high_threshold - threshold;
            let excess = current_tps - threshold;
            (excess * 100 / range) as u8
        };

        // Apply smoothing
        let smoothing = self.config.smoothing_factor as u32;
        let new_factor = (self.metrics.congestion_factor as u32 * smoothing + 
                         raw_factor as u32 * (100 - smoothing)) / 100;
        
        self.metrics.congestion_factor = new_factor as u8;
    }

    /// Get recommended priority fee for a transaction
    pub fn recommend_priority_fee(
        &self,
        transaction_type: TransactionType,
    ) -> Result<u64, ProgramError> {
        let base_cu = match transaction_type {
            TransactionType::Trade => 20_000,
            TransactionType::BatchTrade => 180_000,
            TransactionType::Liquidation => 50_000,
            TransactionType::Settlement => 30_000,
            TransactionType::UpdatePrice => 10_000,
            TransactionType::Withdrawal => 15_000,
        };

        // Add buffer for safety
        let cu_with_buffer = (base_cu * 120) / 100; // 20% buffer
        
        self.calculate_priority_fee(cu_with_buffer)
    }

    /// Get current fee tier
    pub fn get_fee_tier(&self) -> FeeTier {
        match self.metrics.congestion_factor {
            0..=20 => FeeTier::Low,
            21..=50 => FeeTier::Medium,
            51..=80 => FeeTier::High,
            _ => FeeTier::Critical,
        }
    }

    /// Estimate transaction confirmation time based on priority fee
    pub fn estimate_confirmation_time(&self, priority_fee: u64) -> u64 {
        let base_time_ms = 400; // Base block time
        let congestion_delay = (self.metrics.congestion_factor as u64 * 20); // Up to 2s delay
        
        // Higher fees reduce wait time
        let fee_reduction = priority_fee.saturating_sub(self.config.base_fee_micro_lamports) / 1000;
        
        base_time_ms + congestion_delay.saturating_sub(fee_reduction.min(congestion_delay))
    }
}

/// Transaction types for fee calculation
#[derive(Debug, Clone, Copy)]
pub enum TransactionType {
    Trade,
    BatchTrade,
    Liquidation,
    Settlement,
    UpdatePrice,
    Withdrawal,
}

/// Fee tiers based on congestion
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeeTier {
    Low,      // 0-20% congestion
    Medium,   // 21-50% congestion
    High,     // 51-80% congestion
    Critical, // 81-100% congestion
}

/// Priority fee statistics
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriorityFeeStats {
    /// Total fees collected
    pub total_fees_collected: u64,
    /// Number of transactions
    pub transaction_count: u64,
    /// Average fee per transaction
    pub avg_fee_per_tx: u64,
    /// Highest fee paid
    pub max_fee_paid: u64,
    /// Last update timestamp
    pub last_update: i64,
}

impl PriorityFeeStats {
    /// Update statistics with new transaction
    pub fn record_transaction(&mut self, fee_paid: u64) {
        self.total_fees_collected += fee_paid;
        self.transaction_count += 1;
        self.avg_fee_per_tx = self.total_fees_collected / self.transaction_count;
        self.max_fee_paid = self.max_fee_paid.max(fee_paid);
        self.last_update = Clock::get().unwrap().unix_timestamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_fee_calculation() {
        let calculator = PriorityFeeCalculator::new();
        
        // Test with no congestion
        let fee = calculator.calculate_priority_fee(100_000).unwrap();
        assert_eq!(fee, 100); // Base fee only
        
        // Test with custom calculator with congestion
        let mut calc_with_congestion = PriorityFeeCalculator::new();
        calc_with_congestion.metrics.congestion_factor = 50;
        
        let fee = calc_with_congestion.calculate_priority_fee(100_000).unwrap();
        assert!(fee > 100); // Should be higher than base
    }

    #[test]
    fn test_congestion_factor_update() {
        let mut calculator = PriorityFeeCalculator::new();
        
        // Below threshold
        calculator.update_congestion_factor(1000);
        assert_eq!(calculator.metrics.congestion_factor, 0);
        
        // At threshold
        calculator.update_congestion_factor(2000);
        assert_eq!(calculator.metrics.congestion_factor, 0);
        
        // Between thresholds
        calculator.update_congestion_factor(3000);
        assert!(calculator.metrics.congestion_factor > 0);
        assert!(calculator.metrics.congestion_factor < 100);
        
        // Above high threshold
        calculator.metrics.congestion_factor = 0; // Reset for clean test
        calculator.update_congestion_factor(5000);
        assert!(calculator.metrics.congestion_factor > 50);
    }

    #[test]
    fn test_fee_tiers() {
        let mut calculator = PriorityFeeCalculator::new();
        
        calculator.metrics.congestion_factor = 10;
        assert_eq!(calculator.get_fee_tier(), FeeTier::Low);
        
        calculator.metrics.congestion_factor = 40;
        assert_eq!(calculator.get_fee_tier(), FeeTier::Medium);
        
        calculator.metrics.congestion_factor = 70;
        assert_eq!(calculator.get_fee_tier(), FeeTier::High);
        
        calculator.metrics.congestion_factor = 90;
        assert_eq!(calculator.get_fee_tier(), FeeTier::Critical);
    }

    #[test]
    fn test_transaction_type_fees() {
        let calculator = PriorityFeeCalculator::new();
        
        let trade_fee = calculator.recommend_priority_fee(TransactionType::Trade).unwrap();
        let batch_fee = calculator.recommend_priority_fee(TransactionType::BatchTrade).unwrap();
        
        // Batch trades should cost more due to higher CU usage
        assert!(batch_fee > trade_fee);
    }
}