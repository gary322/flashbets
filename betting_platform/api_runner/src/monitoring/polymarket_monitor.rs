//! Polymarket Integration Monitoring
//! Real-time monitoring and alerting for Polymarket operations

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, error};
use prometheus::{
    register_counter_vec, register_histogram_vec, register_gauge_vec,
    CounterVec, HistogramVec, GaugeVec, Encoder, TextEncoder,
};

/// Monitoring metrics for Polymarket integration
pub struct PolymarketMonitor {
    // Metrics
    order_counter: CounterVec,
    order_latency: HistogramVec,
    order_success_rate: GaugeVec,
    api_errors: CounterVec,
    websocket_status: GaugeVec,
    ctf_operations: CounterVec,
    position_values: GaugeVec,
    
    // State tracking
    order_stats: Arc<RwLock<OrderStats>>,
    api_health: Arc<RwLock<ApiHealth>>,
    alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone)]
struct OrderStats {
    total_orders: u64,
    successful_orders: u64,
    failed_orders: u64,
    cancelled_orders: u64,
    total_volume: f64,
    last_order_time: Option<DateTime<Utc>>,
    average_fill_time: Duration,
    order_history: Vec<OrderEvent>,
}

#[derive(Debug, Clone)]
struct OrderEvent {
    order_id: String,
    timestamp: DateTime<Utc>,
    status: String,
    size: f64,
    price: f64,
    latency_ms: u64,
}

#[derive(Debug, Clone)]
struct ApiHealth {
    clob_status: ServiceStatus,
    websocket_status: ServiceStatus,
    ctf_status: ServiceStatus,
    database_status: ServiceStatus,
    last_sync_time: Option<DateTime<Utc>>,
    error_count: HashMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq)]
enum ServiceStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone)]
struct AlertThresholds {
    max_order_latency_ms: u64,
    min_success_rate: f64,
    max_error_rate: f64,
    websocket_disconnect_timeout: Duration,
    order_volume_threshold: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_order_latency_ms: 5000,
            min_success_rate: 0.95,
            max_error_rate: 0.05,
            websocket_disconnect_timeout: Duration::minutes(5),
            order_volume_threshold: 100000.0, // $100k
        }
    }
}

impl PolymarketMonitor {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let order_counter = register_counter_vec!(
            "polymarket_orders_total",
            "Total number of Polymarket orders",
            &["status", "side", "order_type"]
        )?;
        
        let order_latency = register_histogram_vec!(
            "polymarket_order_latency_seconds",
            "Order submission latency",
            &["operation"]
        )?;
        
        let order_success_rate = register_gauge_vec!(
            "polymarket_order_success_rate",
            "Order success rate",
            &["timeframe"]
        )?;
        
        let api_errors = register_counter_vec!(
            "polymarket_api_errors_total",
            "Total API errors",
            &["endpoint", "error_type"]
        )?;
        
        let websocket_status = register_gauge_vec!(
            "polymarket_websocket_status",
            "WebSocket connection status (1=connected, 0=disconnected)",
            &["channel"]
        )?;
        
        let ctf_operations = register_counter_vec!(
            "polymarket_ctf_operations_total",
            "CTF operations count",
            &["operation", "status"]
        )?;
        
        let position_values = register_gauge_vec!(
            "polymarket_position_values",
            "Current position values in USD",
            &["user", "market"]
        )?;
        
        Ok(Self {
            order_counter,
            order_latency,
            order_success_rate,
            api_errors,
            websocket_status,
            ctf_operations,
            position_values,
            order_stats: Arc::new(RwLock::new(OrderStats {
                total_orders: 0,
                successful_orders: 0,
                failed_orders: 0,
                cancelled_orders: 0,
                total_volume: 0.0,
                last_order_time: None,
                average_fill_time: Duration::seconds(0),
                order_history: Vec::new(),
            })),
            api_health: Arc::new(RwLock::new(ApiHealth {
                clob_status: ServiceStatus::Unknown,
                websocket_status: ServiceStatus::Unknown,
                ctf_status: ServiceStatus::Unknown,
                database_status: ServiceStatus::Unknown,
                last_sync_time: None,
                error_count: HashMap::new(),
            })),
            alert_thresholds: AlertThresholds::default(),
        })
    }
    
    /// Record order submission
    pub async fn record_order_submission(
        &self,
        order_id: &str,
        side: &str,
        order_type: &str,
        size: f64,
        price: f64,
        latency_ms: u64,
        success: bool,
    ) {
        let status = if success { "success" } else { "failed" };
        
        self.order_counter
            .with_label_values(&[status, side, order_type])
            .inc();
        
        self.order_latency
            .with_label_values(&["submit"])
            .observe(latency_ms as f64 / 1000.0);
        
        let mut stats = self.order_stats.write().await;
        stats.total_orders += 1;
        
        if success {
            stats.successful_orders += 1;
        } else {
            stats.failed_orders += 1;
        }
        
        stats.total_volume += size * price;
        stats.last_order_time = Some(Utc::now());
        
        // Keep last 1000 orders
        if stats.order_history.len() >= 1000 {
            stats.order_history.remove(0);
        }
        
        stats.order_history.push(OrderEvent {
            order_id: order_id.to_string(),
            timestamp: Utc::now(),
            status: status.to_string(),
            size,
            price,
            latency_ms,
        });
        
        // Update success rate
        let success_rate = stats.successful_orders as f64 / stats.total_orders as f64;
        self.order_success_rate
            .with_label_values(&["all_time"])
            .set(success_rate);
        
        // Check alert thresholds
        if latency_ms > self.alert_thresholds.max_order_latency_ms {
            warn!(
                "Order latency exceeded threshold: {}ms > {}ms",
                latency_ms, self.alert_thresholds.max_order_latency_ms
            );
        }
        
        if success_rate < self.alert_thresholds.min_success_rate {
            error!(
                "Order success rate below threshold: {:.2}% < {:.2}%",
                success_rate * 100.0,
                self.alert_thresholds.min_success_rate * 100.0
            );
        }
    }
    
    /// Record order cancellation
    pub async fn record_order_cancellation(&self, order_id: &str, latency_ms: u64) {
        self.order_counter
            .with_label_values(&["cancelled", "n/a", "n/a"])
            .inc();
        
        self.order_latency
            .with_label_values(&["cancel"])
            .observe(latency_ms as f64 / 1000.0);
        
        let mut stats = self.order_stats.write().await;
        stats.cancelled_orders += 1;
        
        info!("Order cancelled: {} (latency: {}ms)", order_id, latency_ms);
    }
    
    /// Record API error
    pub async fn record_api_error(&self, endpoint: &str, error_type: &str) {
        self.api_errors
            .with_label_values(&[endpoint, error_type])
            .inc();
        
        let mut health = self.api_health.write().await;
        *health.error_count.entry(endpoint.to_string()).or_insert(0) += 1;
        
        // Update service status based on error count
        let error_count = health.error_count[endpoint];
        if error_count > 10 {
            match endpoint {
                "clob" => health.clob_status = ServiceStatus::Unhealthy,
                "ctf" => health.ctf_status = ServiceStatus::Unhealthy,
                _ => {}
            }
        } else if error_count > 5 {
            match endpoint {
                "clob" => health.clob_status = ServiceStatus::Degraded,
                "ctf" => health.ctf_status = ServiceStatus::Degraded,
                _ => {}
            }
        }
        
        error!("API error on {}: {}", endpoint, error_type);
    }
    
    /// Update WebSocket status
    pub async fn update_websocket_status(&self, channel: &str, connected: bool) {
        let status = if connected { 1.0 } else { 0.0 };
        self.websocket_status
            .with_label_values(&[channel])
            .set(status);
        
        let mut health = self.api_health.write().await;
        health.websocket_status = if connected {
            ServiceStatus::Healthy
        } else {
            ServiceStatus::Unhealthy
        };
        
        if connected {
            info!("WebSocket connected: {}", channel);
        } else {
            warn!("WebSocket disconnected: {}", channel);
        }
    }
    
    /// Record CTF operation
    pub async fn record_ctf_operation(
        &self,
        operation: &str,
        success: bool,
        gas_used: Option<u64>,
    ) {
        let status = if success { "success" } else { "failed" };
        
        self.ctf_operations
            .with_label_values(&[operation, status])
            .inc();
        
        if let Some(gas) = gas_used {
            info!("CTF {} {}: gas used {}", operation, status, gas);
        }
    }
    
    /// Update position value
    pub async fn update_position_value(&self, user: &str, market: &str, value_usd: f64) {
        self.position_values
            .with_label_values(&[user, market])
            .set(value_usd);
        
        if value_usd > self.alert_thresholds.order_volume_threshold {
            warn!(
                "Large position detected: {} has ${:.2} in {}",
                user, value_usd, market
            );
        }
    }
    
    /// Get current health status
    pub async fn get_health_status(&self) -> HealthReport {
        let stats = self.order_stats.read().await;
        let health = self.api_health.read().await;
        
        let success_rate = if stats.total_orders > 0 {
            stats.successful_orders as f64 / stats.total_orders as f64
        } else {
            1.0
        };
        
        let error_rate = if stats.total_orders > 0 {
            stats.failed_orders as f64 / stats.total_orders as f64
        } else {
            0.0
        };
        
        let overall_status = if health.clob_status == ServiceStatus::Unhealthy
            || health.websocket_status == ServiceStatus::Unhealthy
            || health.database_status == ServiceStatus::Unhealthy
            || success_rate < self.alert_thresholds.min_success_rate
            || error_rate > self.alert_thresholds.max_error_rate
        {
            ServiceStatus::Unhealthy
        } else if health.clob_status == ServiceStatus::Degraded
            || health.websocket_status == ServiceStatus::Degraded
            || success_rate < 0.98
        {
            ServiceStatus::Degraded
        } else {
            ServiceStatus::Healthy
        };
        
        HealthReport {
            overall_status,
            clob_status: health.clob_status.clone(),
            websocket_status: health.websocket_status.clone(),
            ctf_status: health.ctf_status.clone(),
            database_status: health.database_status.clone(),
            total_orders: stats.total_orders,
            success_rate,
            error_rate,
            total_volume: stats.total_volume,
            last_order_time: stats.last_order_time,
            last_sync_time: health.last_sync_time,
            recent_errors: health.error_count.clone(),
        }
    }
    
    /// Export Prometheus metrics
    pub fn export_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
    
    /// Check and trigger alerts
    pub async fn check_alerts(&self) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let stats = self.order_stats.read().await;
        let health = self.api_health.read().await;
        
        // Check success rate
        if stats.total_orders > 100 {
            let success_rate = stats.successful_orders as f64 / stats.total_orders as f64;
            if success_rate < self.alert_thresholds.min_success_rate {
                alerts.push(Alert {
                    severity: AlertSeverity::Critical,
                    message: format!(
                        "Low order success rate: {:.1}%",
                        success_rate * 100.0
                    ),
                    timestamp: Utc::now(),
                });
            }
        }
        
        // Check WebSocket connection
        if health.websocket_status == ServiceStatus::Unhealthy {
            if let Some(last_order) = stats.last_order_time {
                if Utc::now() - last_order > self.alert_thresholds.websocket_disconnect_timeout {
                    alerts.push(Alert {
                        severity: AlertSeverity::High,
                        message: "WebSocket disconnected for extended period".to_string(),
                        timestamp: Utc::now(),
                    });
                }
            }
        }
        
        // Check service health
        if health.clob_status == ServiceStatus::Unhealthy {
            alerts.push(Alert {
                severity: AlertSeverity::Critical,
                message: "CLOB service unhealthy".to_string(),
                timestamp: Utc::now(),
            });
        }
        
        // Check error rates
        for (endpoint, count) in &health.error_count {
            if *count > 20 {
                alerts.push(Alert {
                    severity: AlertSeverity::High,
                    message: format!("High error count on {}: {}", endpoint, count),
                    timestamp: Utc::now(),
                });
            }
        }
        
        alerts
    }
}

#[derive(Debug, Clone)]
pub struct HealthReport {
    pub overall_status: ServiceStatus,
    pub clob_status: ServiceStatus,
    pub websocket_status: ServiceStatus,
    pub ctf_status: ServiceStatus,
    pub database_status: ServiceStatus,
    pub total_orders: u64,
    pub success_rate: f64,
    pub error_rate: f64,
    pub total_volume: f64,
    pub last_order_time: Option<DateTime<Utc>>,
    pub last_sync_time: Option<DateTime<Utc>>,
    pub recent_errors: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Background monitoring task
pub async fn start_monitoring(monitor: Arc<PolymarketMonitor>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        // Check alerts
        let alerts = monitor.check_alerts().await;
        for alert in alerts {
            match alert.severity {
                AlertSeverity::Critical => error!("CRITICAL ALERT: {}", alert.message),
                AlertSeverity::High => error!("HIGH ALERT: {}", alert.message),
                AlertSeverity::Medium => warn!("MEDIUM ALERT: {}", alert.message),
                AlertSeverity::Low => info!("LOW ALERT: {}", alert.message),
            }
        }
        
        // Get health report
        let health = monitor.get_health_status().await;
        info!(
            "Polymarket Health: {:?} | Orders: {} | Success: {:.1}% | Volume: ${:.2}",
            health.overall_status,
            health.total_orders,
            health.success_rate * 100.0,
            health.total_volume
        );
    }
}