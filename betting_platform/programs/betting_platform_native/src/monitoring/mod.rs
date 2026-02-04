//! Monitoring module
//!
//! System health, performance tracking, and alerts

pub mod health;
pub mod performance;
pub mod alerts;
pub mod performance_display;
pub mod network_latency;

pub use health::{SystemHealth, SystemStatus, ServiceStatus, HealthMonitor};
pub use performance::{PerformanceMetrics, OperationMetrics, PerformanceAlert};
pub use alerts::{AlertConfiguration, Alert, AlertType, AlertSeverity, AlertManager};
pub use performance_display::{
    PerformanceSnapshot, DisplayFormat, DashboardData,
    process_get_performance_snapshot, process_get_dashboard_data,
};
pub use network_latency::{
    NetworkLatencyMonitor, LatencyConfig, LatencyStatus,
    measure_network_latency, process_network_latency_update,
};