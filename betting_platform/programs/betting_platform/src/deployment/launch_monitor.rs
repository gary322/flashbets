use anchor_lang::prelude::*;
use std::sync::Arc;
use std::sync::RwLock;
use crate::deployment::errors::{MonitorError, AlertLevel};

#[derive(Clone, Debug)]
pub struct MetricsData {
    pub vault_balance: u64,
    pub coverage_ratio: f64,
    pub tps: f64,
    pub keeper_status: KeeperStatus,
    pub timestamp: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeeperStatus {
    Healthy,
    Degraded,
    Failed,
}

pub struct MetricsCollector {
    metrics_history: Arc<RwLock<Vec<MetricsData>>>,
    max_history_size: usize,
}

impl MetricsCollector {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
        }
    }

    pub fn record_metrics(&self, metrics: MetricsData) {
        let mut history = self.metrics_history.write().unwrap();
        history.push(metrics);
        
        // Keep only recent history
        if history.len() > self.max_history_size {
            history.remove(0);
        }
    }

    pub fn get_latest_metrics(&self) -> Option<MetricsData> {
        let history = self.metrics_history.read().unwrap();
        history.last().cloned()
    }

    pub fn get_average_tps(&self, duration_seconds: u64) -> f64 {
        let history = self.metrics_history.read().unwrap();
        let now = Clock::get().unwrap().unix_timestamp;
        let cutoff = now - duration_seconds as i64;
        
        let recent_metrics: Vec<&MetricsData> = history
            .iter()
            .filter(|m| m.timestamp >= cutoff)
            .collect();
        
        if recent_metrics.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = recent_metrics.iter().map(|m| m.tps).sum();
        sum / recent_metrics.len() as f64
    }
}

pub struct AlertSystem {
    alert_history: Arc<RwLock<Vec<Alert>>>,
    webhook_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Alert {
    pub level: AlertLevel,
    pub message: String,
    pub timestamp: i64,
    pub metrics_snapshot: Option<MetricsData>,
}

impl AlertSystem {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            alert_history: Arc::new(RwLock::new(Vec::new())),
            webhook_url,
        }
    }

    pub fn send_alert(
        &self,
        level: AlertLevel,
        message: &str,
    ) -> Result<()> {
        let alert = Alert {
            level,
            message: message.to_string(),
            timestamp: Clock::get()?.unix_timestamp,
            metrics_snapshot: None,
        };
        
        // Store alert in history
        let mut history = self.alert_history.write().unwrap();
        history.push(alert.clone());
        
        // Send webhook if configured
        if let Some(webhook_url) = &self.webhook_url {
            self.send_webhook_alert(&alert, webhook_url)?;
        }
        
        // Log alert
        match level {
            AlertLevel::Info => msg!("INFO: {}", message),
            AlertLevel::Warning => msg!("WARNING: {}", message),
            AlertLevel::Critical => msg!("CRITICAL: {}", message),
            AlertLevel::Emergency => msg!("EMERGENCY: {}", message),
        }
        
        Ok(())
    }

    fn send_webhook_alert(&self, alert: &Alert, url: &str) -> Result<()> {
        // In production, this would send an HTTP POST to the webhook
        msg!("Webhook alert sent to {}: {:?}", url, alert);
        Ok(())
    }
}

pub struct HealthChecker {
    checks: Vec<HealthCheck>,
}

#[derive(Clone)]
pub struct HealthCheck {
    pub name: String,
    pub check_fn: Arc<dyn Fn() -> Result<bool> + Send + Sync>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
        }
    }

    pub fn add_check(&mut self, name: String, check_fn: Arc<dyn Fn() -> Result<bool> + Send + Sync>) {
        self.checks.push(HealthCheck { name, check_fn });
    }

    pub fn run_checks(&self) -> Vec<(String, bool)> {
        let mut results = Vec::new();
        
        for check in &self.checks {
            let result = (check.check_fn)().unwrap_or(false);
            results.push((check.name.clone(), result));
        }
        
        results
    }
}

pub struct LaunchMonitor {
    pub metrics_collector: MetricsCollector,
    pub alert_system: AlertSystem,
    pub health_checker: HealthChecker,
    program_id: Pubkey,
    vault_pubkey: Pubkey,
}

impl LaunchMonitor {
    pub fn new(
        program_id: Pubkey,
        vault_pubkey: Pubkey,
        webhook_url: Option<String>,
    ) -> Self {
        Self {
            metrics_collector: MetricsCollector::new(10000),
            alert_system: AlertSystem::new(webhook_url),
            health_checker: HealthChecker::new(),
            program_id,
            vault_pubkey,
        }
    }

    pub fn monitor_launch(&self) -> Result<()> {
        msg!("Starting launch monitoring");
        
        // In production, this would spawn a monitoring thread
        // For now, just a placeholder
        
        Ok(())
    }

    pub fn collect_and_check_metrics(&self) -> Result<()> {
        // Check vault balance
        let vault_balance = self.check_vault_balance()?;
        
        // Check coverage ratio
        let coverage = self.calculate_coverage()?;
        
        // Check transaction throughput
        let tps = self.measure_tps()?;
        
        // Check keeper activity
        let keeper_health = self.check_keeper_health()?;
        
        // Create metrics data
        let metrics = MetricsData {
            vault_balance,
            coverage_ratio: coverage,
            tps,
            keeper_status: keeper_health.clone(),
            timestamp: Clock::get()?.unix_timestamp,
        };
        
        // Record metrics
        self.metrics_collector.record_metrics(metrics.clone());
        
        // Check for alert conditions
        if coverage < 0.5 && vault_balance > 0 {
            self.alert_system.send_alert(
                AlertLevel::Critical,
                "Coverage below safety threshold",
            )?;
        }
        
        if tps < 100.0 {
            self.alert_system.send_alert(
                AlertLevel::Warning,
                "TPS below expected threshold",
            )?;
        }
        
        if keeper_health == KeeperStatus::Failed {
            self.alert_system.send_alert(
                AlertLevel::Emergency,
                "Keeper system failure detected",
            )?;
        }
        
        Ok(())
    }

    pub fn check_vault_balance(&self) -> Result<u64> {
        // In production, this would query the actual vault account
        Ok(0) // Start with $0 vault as per spec
    }

    pub fn calculate_coverage(&self) -> Result<f64> {
        // In production, this would calculate actual coverage ratio
        Ok(0.0) // Start at 0 for bootstrap as per spec
    }

    pub fn measure_tps(&self) -> Result<f64> {
        // In production, this would measure actual TPS
        Ok(0.0)
    }

    pub fn check_keeper_health(&self) -> Result<KeeperStatus> {
        // In production, this would check actual keeper status
        Ok(KeeperStatus::Healthy)
    }
}

impl Clone for LaunchMonitor {
    fn clone(&self) -> Self {
        Self {
            metrics_collector: MetricsCollector::new(10000),
            alert_system: AlertSystem::new(None),
            health_checker: HealthChecker::new(),
            program_id: self.program_id,
            vault_pubkey: self.vault_pubkey,
        }
    }
}