//! Performance monitoring and metrics
//!
//! Tracks per-operation metrics, latency, and success rates

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};

use crate::error::BettingPlatformError;
use crate::math::U64F64;

/// Performance metrics account
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PerformanceMetrics {
    pub last_update_slot: u64,
    pub measurement_window_slots: u64,
    
    // Operation metrics
    pub open_position_metrics: OperationMetrics,
    pub close_position_metrics: OperationMetrics,
    pub liquidation_metrics: OperationMetrics,
    pub order_execution_metrics: OperationMetrics,
    pub keeper_task_metrics: OperationMetrics,
    
    // Latency tracking (in milliseconds)
    pub api_latency_p50: u32,
    pub api_latency_p95: u32,
    pub api_latency_p99: u32,
    
    // Success rates
    pub overall_success_rate: u8, // Percentage 0-100
    pub keeper_success_rate: u8,
    pub liquidation_success_rate: u8,
    
    // Resource usage
    pub average_accounts_per_tx: u8,
    pub average_cpi_calls_per_tx: u8,
    pub memory_usage_bytes: u64,
    
    // Alerts
    pub active_alerts: Vec<PerformanceAlert>,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct OperationMetrics {
    pub total_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub average_cu_usage: u32,
    pub max_cu_usage: u32,
    pub average_latency_ms: u32,
    pub p95_latency_ms: u32,
    pub p99_latency_ms: u32,
    pub last_failure_slot: Option<u64>,
    pub consecutive_failures: u16,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PerformanceAlert {
    pub alert_type: PerformanceAlertType,
    pub triggered_slot: u64,
    pub metric_value: u64,
    pub threshold: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum PerformanceAlertType {
    HighCUUsage,        // >20k average
    LowSuccessRate,     // <95%
    HighLatency,        // p95 > 1000ms
    ConsecutiveFailures, // >5 failures
    MemoryPressure,     // >80% usage
}

/// Performance monitor for tracking metrics
pub struct PerformanceMonitor;

impl PerformanceMonitor {
    /// Record operation metric
    pub fn record_operation(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        operation: &str,
        success: bool,
        cu_used: u32,
        latency_ms: u32,
    ) -> ProgramResult {
        // Account layout:
        // 0. Performance metrics account (mut)
        // 1. Clock sysvar
        
        if accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        
        let metrics_account = &accounts[0];
        let clock = Clock::get()?;
        
        // Deserialize metrics
        let mut metrics_data = metrics_account.try_borrow_mut_data()?;
        let mut metrics = PerformanceMetrics::try_from_slice(&metrics_data)?;
        
        // Get the appropriate operation metrics
        let op_metrics = match operation {
            "open_position" => &mut metrics.open_position_metrics,
            "close_position" => &mut metrics.close_position_metrics,
            "liquidation" => &mut metrics.liquidation_metrics,
            "order_execution" => &mut metrics.order_execution_metrics,
            "keeper_task" => &mut metrics.keeper_task_metrics,
            _ => return Err(BettingPlatformError::InvalidOperation.into()),
        };
        
        // Update metrics
        op_metrics.total_count += 1;
        if success {
            op_metrics.success_count += 1;
            op_metrics.consecutive_failures = 0;
        } else {
            op_metrics.failure_count += 1;
            op_metrics.consecutive_failures += 1;
            op_metrics.last_failure_slot = Some(clock.slot);
        }
        
        // Update CU metrics
        Self::update_average(
            &mut op_metrics.average_cu_usage,
            cu_used,
            op_metrics.total_count,
        );
        
        if cu_used > op_metrics.max_cu_usage {
            op_metrics.max_cu_usage = cu_used;
        }
        
        // Update latency metrics
        Self::update_average(
            &mut op_metrics.average_latency_ms,
            latency_ms,
            op_metrics.total_count,
        );
        
        // Update percentiles (simplified - in production would use proper percentile tracking)
        if latency_ms > op_metrics.p95_latency_ms {
            op_metrics.p99_latency_ms = op_metrics.p99_latency_ms.max(latency_ms);
            if op_metrics.total_count % 20 == 0 { // Roughly p95
                op_metrics.p95_latency_ms = latency_ms;
            }
        }
        
        // Check for alerts - copy needed values to avoid borrow conflict
        let op_total_count = op_metrics.total_count;
        let op_success_count = op_metrics.success_count;
        let op_consecutive_failures = op_metrics.consecutive_failures;
        let op_p95_latency = op_metrics.p95_latency_ms;
        
        Self::check_alerts(&mut metrics, operation, op_total_count, op_success_count, op_consecutive_failures, op_p95_latency, cu_used)?;
        
        // Update overall success rate
        let total_ops = metrics.open_position_metrics.total_count +
            metrics.close_position_metrics.total_count +
            metrics.liquidation_metrics.total_count +
            metrics.order_execution_metrics.total_count;
        
        let total_success = metrics.open_position_metrics.success_count +
            metrics.close_position_metrics.success_count +
            metrics.liquidation_metrics.success_count +
            metrics.order_execution_metrics.success_count;
        
        if total_ops > 0 {
            metrics.overall_success_rate = ((total_success * 100) / total_ops) as u8;
        }
        
        metrics.last_update_slot = clock.slot;
        
        // Serialize updated metrics
        metrics.serialize(&mut *metrics_data)?;
        
        Ok(())
    }
    
    /// Update running average
    fn update_average(current: &mut u32, new_value: u32, count: u64) {
        if count == 1 {
            *current = new_value;
        } else {
            // Weighted average giving more weight to recent values
            *current = ((*current as u64 * (count - 1) + new_value as u64) / count) as u32;
        }
    }
    
    /// Check for performance alerts
    fn check_alerts(
        metrics: &mut PerformanceMetrics,
        operation: &str,
        op_total_count: u64,
        op_success_count: u64,
        op_consecutive_failures: u16,
        op_p95_latency: u32,
        cu_used: u32,
    ) -> ProgramResult {
        let clock = Clock::get()?;
        
        // High CU usage alert
        if cu_used > 20_000 {
            let alert = PerformanceAlert {
                alert_type: PerformanceAlertType::HighCUUsage,
                triggered_slot: clock.slot,
                metric_value: cu_used as u64,
                threshold: 20_000,
            };
            
            Self::add_alert(metrics, alert)?;
            
            msg!("PerformanceAlert - type: HighCUUsage, operation: {}, cu_used: {}", operation, cu_used);
        }
        
        // Low success rate alert
        if op_total_count > 100 {
            let success_rate = (op_success_count * 100) / op_total_count;
            if success_rate < 95 {
                let alert = PerformanceAlert {
                    alert_type: PerformanceAlertType::LowSuccessRate,
                    triggered_slot: clock.slot,
                    metric_value: success_rate,
                    threshold: 95,
                };
                
                Self::add_alert(metrics, alert)?;
            }
        }
        
        // Consecutive failures alert
        if op_consecutive_failures > 5 {
            let alert = PerformanceAlert {
                alert_type: PerformanceAlertType::ConsecutiveFailures,
                triggered_slot: clock.slot,
                metric_value: op_consecutive_failures as u64,
                threshold: 5,
            };
            
            Self::add_alert(metrics, alert)?;
        }
        
        // High latency alert
        if op_p95_latency > 1000 {
            let alert = PerformanceAlert {
                alert_type: PerformanceAlertType::HighLatency,
                triggered_slot: clock.slot,
                metric_value: op_p95_latency as u64,
                threshold: 1000,
            };
            
            Self::add_alert(metrics, alert)?;
        }
        
        Ok(())
    }
    
    /// Add alert to list (keep last 10)
    fn add_alert(metrics: &mut PerformanceMetrics, alert: PerformanceAlert) -> ProgramResult {
        metrics.active_alerts.push(alert);
        
        // Keep only last 10 alerts
        if metrics.active_alerts.len() > 10 {
            metrics.active_alerts.remove(0);
        }
        
        Ok(())
    }
    
    /// Get performance report
    pub fn get_performance_report(
        metrics: &PerformanceMetrics,
    ) -> PerformanceReport {
        PerformanceReport {
            overall_health_score: Self::calculate_health_score(metrics),
            success_rate: metrics.overall_success_rate,
            average_cu_usage: Self::calculate_average_cu(metrics),
            high_cu_operations: Self::count_high_cu_operations(metrics),
            active_alert_count: metrics.active_alerts.len() as u8,
            recommendations: Self::generate_recommendations(metrics),
        }
    }
    
    /// Calculate overall health score
    fn calculate_health_score(metrics: &PerformanceMetrics) -> u8 {
        let mut score = 100u8;
        
        // Deduct for low success rate
        if metrics.overall_success_rate < 99 {
            score = score.saturating_sub(10);
        }
        if metrics.overall_success_rate < 95 {
            score = score.saturating_sub(20);
        }
        
        // Deduct for active alerts
        score = score.saturating_sub((metrics.active_alerts.len() * 5) as u8);
        
        // Deduct for high latency
        if metrics.api_latency_p95 > 1000 {
            score = score.saturating_sub(10);
        }
        
        score
    }
    
    /// Calculate average CU across all operations
    fn calculate_average_cu(metrics: &PerformanceMetrics) -> u32 {
        let operations = [
            &metrics.open_position_metrics,
            &metrics.close_position_metrics,
            &metrics.liquidation_metrics,
            &metrics.order_execution_metrics,
            &metrics.keeper_task_metrics,
        ];
        
        let total_cu: u64 = operations.iter()
            .map(|op| op.average_cu_usage as u64 * op.total_count)
            .sum();
        
        let total_count: u64 = operations.iter()
            .map(|op| op.total_count)
            .sum();
        
        if total_count > 0 {
            (total_cu / total_count) as u32
        } else {
            0
        }
    }
    
    /// Count operations with high CU usage
    fn count_high_cu_operations(metrics: &PerformanceMetrics) -> u8 {
        let operations = [
            &metrics.open_position_metrics,
            &metrics.close_position_metrics,
            &metrics.liquidation_metrics,
            &metrics.order_execution_metrics,
            &metrics.keeper_task_metrics,
        ];
        
        operations.iter()
            .filter(|op| op.max_cu_usage > 20_000)
            .count() as u8
    }
    
    /// Generate performance recommendations
    fn generate_recommendations(metrics: &PerformanceMetrics) -> Vec<&'static str> {
        let mut recommendations = Vec::new();
        
        if metrics.overall_success_rate < 95 {
            recommendations.push("Investigate failing operations");
        }
        
        if Self::calculate_average_cu(metrics) > 15_000 {
            recommendations.push("Optimize high CU operations");
        }
        
        if metrics.api_latency_p95 > 1000 {
            recommendations.push("Improve API response times");
        }
        
        if metrics.active_alerts.len() > 5 {
            recommendations.push("Address active performance alerts");
        }
        
        recommendations
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub overall_health_score: u8,
    pub success_rate: u8,
    pub average_cu_usage: u32,
    pub high_cu_operations: u8,
    pub active_alert_count: u8,
    pub recommendations: Vec<&'static str>,
}

impl OperationMetrics {
    pub const SIZE: usize = 8 + // total_count
        8 + // success_count
        8 + // failure_count
        4 + // average_cu_usage
        4 + // max_cu_usage
        4 + // average_latency_ms
        4 + // p95_latency_ms
        4 + // p99_latency_ms
        1 + 8 + // last_failure_slot Option
        2; // consecutive_failures
    
    /// Get success rate percentage
    pub fn success_rate(&self) -> u8 {
        if self.total_count == 0 {
            return 100;
        }
        ((self.success_count * 100) / self.total_count) as u8
    }
    
    /// Check if operation is healthy
    pub fn is_healthy(&self) -> bool {
        self.success_rate() >= 95 &&
        self.average_cu_usage < 20_000 &&
        self.consecutive_failures < 3
    }
}