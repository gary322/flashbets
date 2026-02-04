use anchor_lang::prelude::*;
use std::time::Instant;
use crate::performance::errors::*;

pub const TARGET_CU_PER_TRADE: u64 = 20_000;
pub const CU_PER_8_OUTCOME_BATCH: u64 = 180_000;
pub const CU_PER_CHAIN_TRADE: u64 = 45_000;
pub const TARGET_CU_PER_LEVERAGE_CALC: u64 = 5_000;
pub const TARGET_CU_PER_CHAIN_STEP: u64 = 15_000;

#[derive(Clone, Debug)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub compute_units: u64,
    pub latency_ms: f64,
    pub memory_usage: u64,
    pub bottlenecks: Vec<Bottleneck>,
}

#[derive(Clone, Debug)]
pub struct Bottleneck {
    pub component: String,
    pub severity: BottleneckSeverity,
    pub cu_consumed: u64,
    pub percentage_of_total: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BottleneckSeverity {
    Low,
    Medium,
    High,
    Critical,
}

pub struct ComputeUnitTracker {
    current_cu: u64,
    operation_stack: Vec<(String, u64)>,
}

impl ComputeUnitTracker {
    pub fn new() -> Self {
        Self {
            current_cu: 0,
            operation_stack: Vec::new(),
        }
    }

    pub fn start_operation(&mut self, name: String) {
        self.operation_stack.push((name, self.current_cu));
    }

    pub fn end_operation(&mut self) -> Option<(String, u64)> {
        if let Some((name, start_cu)) = self.operation_stack.pop() {
            let cu_used = self.current_cu.saturating_sub(start_cu);
            Some((name, cu_used))
        } else {
            None
        }
    }

    pub fn add_cu(&mut self, cu: u64) {
        self.current_cu = self.current_cu.saturating_add(cu);
    }

    pub fn get_current_cu(&self) -> u64 {
        self.current_cu
    }
}

pub struct LatencyMonitor {
    operation_timings: Vec<(String, f64)>,
}

impl LatencyMonitor {
    pub fn new() -> Self {
        Self {
            operation_timings: Vec::new(),
        }
    }

    pub fn record_timing(&mut self, operation: String, duration_ms: f64) {
        self.operation_timings.push((operation, duration_ms));
    }

    pub fn get_average_latency(&self) -> f64 {
        if self.operation_timings.is_empty() {
            return 0.0;
        }
        
        let total: f64 = self.operation_timings.iter().map(|(_, ms)| ms).sum();
        total / self.operation_timings.len() as f64
    }

    pub fn get_p99_latency(&self) -> f64 {
        if self.operation_timings.is_empty() {
            return 0.0;
        }
        
        let mut timings: Vec<f64> = self.operation_timings.iter().map(|(_, ms)| *ms).collect();
        timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let p99_index = (timings.len() as f64 * 0.99) as usize;
        timings[p99_index.min(timings.len() - 1)]
    }
}

pub struct BottleneckDetector {
    thresholds: BottleneckThresholds,
}

#[derive(Clone)]
pub struct BottleneckThresholds {
    pub high_cu_threshold: u64,
    pub critical_cu_threshold: u64,
    pub high_latency_threshold: f64,
    pub critical_latency_threshold: f64,
}

impl Default for BottleneckThresholds {
    fn default() -> Self {
        Self {
            high_cu_threshold: 15_000,
            critical_cu_threshold: 20_000,
            high_latency_threshold: 15.0,
            critical_latency_threshold: 20.0,
        }
    }
}

impl BottleneckDetector {
    pub fn new() -> Self {
        Self {
            thresholds: BottleneckThresholds::default(),
        }
    }

    pub fn detect_bottlenecks(
        &self,
        cu_breakdown: &[(String, u64)],
        total_cu: u64,
    ) -> Vec<Bottleneck> {
        let mut bottlenecks = Vec::new();
        
        for (component, cu) in cu_breakdown {
            let percentage = (*cu as f64 / total_cu as f64) * 100.0;
            
            let severity = if *cu >= self.thresholds.critical_cu_threshold {
                BottleneckSeverity::Critical
            } else if *cu >= self.thresholds.high_cu_threshold {
                BottleneckSeverity::High
            } else if percentage > 50.0 {
                BottleneckSeverity::Medium
            } else if percentage > 25.0 {
                BottleneckSeverity::Low
            } else {
                continue;
            };
            
            bottlenecks.push(Bottleneck {
                component: component.clone(),
                severity,
                cu_consumed: *cu,
                percentage_of_total: percentage,
            });
        }
        
        bottlenecks.sort_by(|a, b| b.cu_consumed.cmp(&a.cu_consumed));
        bottlenecks
    }
}

pub struct PerformanceProfiler {
    pub cu_tracker: ComputeUnitTracker,
    pub latency_monitor: LatencyMonitor,
    pub bottleneck_detector: BottleneckDetector,
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            cu_tracker: ComputeUnitTracker::new(),
            latency_monitor: LatencyMonitor::new(),
            bottleneck_detector: BottleneckDetector::new(),
        }
    }

    pub fn profile_transaction<F, R>(
        &mut self,
        operation: &str,
        f: F,
    ) -> Result<(R, PerformanceMetrics)>
    where
        F: FnOnce() -> Result<R>,
    {
        let start_cu = self.get_current_cu();
        let start_time = Instant::now();
        
        self.cu_tracker.start_operation(operation.to_string());
        
        // Execute operation
        let result = f()?;
        
        let end_time = Instant::now();
        let end_cu = self.get_current_cu();
        
        let operation_data = self.cu_tracker.end_operation();
        let cu_used = end_cu.saturating_sub(start_cu);
        let latency_ms = end_time.duration_since(start_time).as_secs_f64() * 1000.0;
        
        self.latency_monitor.record_timing(operation.to_string(), latency_ms);
        
        let metrics = PerformanceMetrics {
            operation: operation.to_string(),
            compute_units: cu_used,
            latency_ms,
            memory_usage: self.measure_memory_usage(),
            bottlenecks: self.detect_bottlenecks(),
        };
        
        // Alert if CU exceeds target
        if metrics.compute_units > TARGET_CU_PER_TRADE {
            self.alert_high_cu(operation, metrics.compute_units);
        }
        
        Ok((result, metrics))
    }

    pub fn get_current_cu(&self) -> u64 {
        self.cu_tracker.get_current_cu()
    }

    pub fn measure_memory_usage(&self) -> u64 {
        // In production, this would measure actual memory usage
        // For now, return a placeholder
        0
    }

    pub fn detect_bottlenecks(&self) -> Vec<Bottleneck> {
        // In production, this would analyze the operation stack
        // For now, return empty vec
        Vec::new()
    }

    pub fn alert_high_cu(&self, operation: &str, cu: u64) {
        msg!(
            "WARNING: High CU usage in {}: {} CU (target: {} CU)",
            operation,
            cu,
            TARGET_CU_PER_TRADE
        );
    }

    pub fn get_performance_summary(&self) -> PerformanceSummary {
        PerformanceSummary {
            average_latency_ms: self.latency_monitor.get_average_latency(),
            p99_latency_ms: self.latency_monitor.get_p99_latency(),
            total_cu_consumed: self.cu_tracker.get_current_cu(),
            operations_profiled: self.latency_monitor.operation_timings.len(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PerformanceSummary {
    pub average_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub total_cu_consumed: u64,
    pub operations_profiled: usize,
}

// Helper functions for profiling specific operations
pub fn profile_leverage_calculation<F>(
    profiler: &mut PerformanceProfiler,
    f: F,
) -> Result<u64>
where
    F: FnOnce() -> Result<u64>,
{
    let (result, metrics) = profiler.profile_transaction("leverage_calculation", f)?;
    
    if metrics.compute_units > TARGET_CU_PER_LEVERAGE_CALC {
        msg!(
            "Leverage calculation exceeded target: {} CU (target: {} CU)",
            metrics.compute_units,
            TARGET_CU_PER_LEVERAGE_CALC
        );
    }
    
    Ok(result)
}

pub fn profile_chain_step<F>(
    profiler: &mut PerformanceProfiler,
    step_number: u8,
    f: F,
) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let operation = format!("chain_step_{}", step_number);
    let (_, metrics) = profiler.profile_transaction(&operation, f)?;
    
    if metrics.compute_units > TARGET_CU_PER_CHAIN_STEP {
        msg!(
            "Chain step {} exceeded target: {} CU (target: {} CU)",
            step_number,
            metrics.compute_units,
            TARGET_CU_PER_CHAIN_STEP
        );
    }
    
    Ok(())
}

pub fn profile_chain_trade<F, R>(
    profiler: &mut PerformanceProfiler,
    num_steps: u8,
    f: F,
) -> Result<(R, u64)>
where
    F: FnOnce() -> Result<R>,
{
    let operation = format!("chain_trade_{}_steps", num_steps);
    let (result, metrics) = profiler.profile_transaction(&operation, f)?;
    
    // For 3-step chain, should be ~45k CU total
    let target_cu = if num_steps == 3 { CU_PER_CHAIN_TRADE } else { TARGET_CU_PER_CHAIN_STEP * num_steps as u64 };
    
    if metrics.compute_units > target_cu {
        msg!(
            "Chain trade ({} steps) exceeded target: {} CU (target: {} CU)",
            num_steps,
            metrics.compute_units,
            target_cu
        );
    }
    
    Ok((result, metrics.compute_units))
}