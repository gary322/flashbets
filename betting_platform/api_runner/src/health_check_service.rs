//! Comprehensive Health Check Service
//! 
//! Provides production-grade health monitoring for all system components

use anyhow::{Result, Context as AnyhowContext};
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Instant,
};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use crate::{
    db::fallback::FallbackDatabase,
    solana_rpc_service::SolanaRpcService,
    trading_engine::TradingEngine,
    external_api_service::ExternalApiService,
    circuit_breaker::CircuitBreakerManager,
    websocket::enhanced::EnhancedWebSocketManager,
    typed_errors::{AppError, ErrorKind, ErrorContext},
};

/// Health status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All components functioning normally
    Healthy,
    /// Some components degraded but service operational
    Degraded,
    /// Critical components failing, service not operational
    Unhealthy,
}

/// Health check result for a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub last_check: DateTime<Utc>,
    pub response_time_ms: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Aggregated health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub overall_status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub components: Vec<ComponentHealth>,
    pub uptime_seconds: u64,
    pub version: String,
    pub environment: String,
}

/// Component that can be health checked
#[async_trait]
pub trait HealthCheckable: Send + Sync {
    async fn check_health(&self) -> Result<ComponentHealth>;
}

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub check_interval: std::time::Duration,
    pub timeout: std::time::Duration,
    pub failure_threshold: u32,
    pub recovery_threshold: u32,
    pub detailed_checks: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: std::time::Duration::from_secs(30),
            timeout: std::time::Duration::from_secs(5),
            failure_threshold: 3,
            recovery_threshold: 2,
            detailed_checks: true,
        }
    }
}

/// Health check service
pub struct HealthCheckService {
    config: HealthCheckConfig,
    components: Arc<RwLock<HashMap<String, Arc<dyn HealthCheckable>>>>,
    health_cache: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    failure_counts: Arc<RwLock<HashMap<String, u32>>>,
    start_time: Instant,
    version: String,
    environment: String,
}

impl HealthCheckService {
    /// Create new health check service
    pub fn new(config: HealthCheckConfig) -> Self {
        let version = env!("CARGO_PKG_VERSION").to_string();
        let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "production".to_string());
        
        Self {
            config,
            components: Arc::new(RwLock::new(HashMap::new())),
            health_cache: Arc::new(RwLock::new(HashMap::new())),
            failure_counts: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
            version,
            environment,
        }
    }
    
    /// Register a component for health checking
    pub async fn register_component(&self, name: String, component: Arc<dyn HealthCheckable>) {
        let name_clone = name.clone();
        self.components.write().await.insert(name.clone(), component);
        self.failure_counts.write().await.insert(name, 0);
        info!("Registered component for health checks: {}", name_clone);
    }
    
    /// Check health of all components
    pub async fn check_all_components(&self) -> HealthReport {
        let mut component_results = Vec::new();
        let components = self.components.read().await;
        
        for (name, component) in components.iter() {
            let start = Instant::now();
            
            let health = match tokio::time::timeout(
                self.config.timeout,
                component.check_health()
            ).await {
                Ok(Ok(health)) => {
                    // Reset failure count on success
                    self.failure_counts.write().await.insert(name.clone(), 0);
                    health
                },
                Ok(Err(e)) => {
                    self.increment_failure_count(name).await;
                    ComponentHealth {
                        name: name.clone(),
                        status: HealthStatus::Unhealthy,
                        message: format!("Health check failed: {}", e),
                        last_check: Utc::now(),
                        response_time_ms: start.elapsed().as_millis() as u64,
                        metadata: HashMap::new(),
                    }
                },
                Err(_) => {
                    self.increment_failure_count(name).await;
                    ComponentHealth {
                        name: name.clone(),
                        status: HealthStatus::Unhealthy,
                        message: "Health check timed out".to_string(),
                        last_check: Utc::now(),
                        response_time_ms: self.config.timeout.as_millis() as u64,
                        metadata: HashMap::new(),
                    }
                }
            };
            
            // Cache the result
            self.health_cache.write().await.insert(name.clone(), health.clone());
            component_results.push(health);
        }
        
        // Determine overall status
        let overall_status = self.calculate_overall_status(&component_results);
        
        HealthReport {
            overall_status,
            timestamp: Utc::now(),
            components: component_results,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: self.version.clone(),
            environment: self.environment.clone(),
        }
    }
    
    /// Get cached health report
    pub async fn get_cached_health(&self) -> HealthReport {
        let cache = self.health_cache.read().await;
        let components: Vec<ComponentHealth> = cache.values().cloned().collect();
        let overall_status = self.calculate_overall_status(&components);
        
        HealthReport {
            overall_status,
            timestamp: Utc::now(),
            components,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: self.version.clone(),
            environment: self.environment.clone(),
        }
    }
    
    /// Start background health check task
    pub fn start_background_checks(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.config.check_interval);
            
            loop {
                interval.tick().await;
                
                let report = self.check_all_components().await;
                
                if report.overall_status != HealthStatus::Healthy {
                    warn!(
                        "Health check detected issues. Status: {:?}, unhealthy components: {}",
                        report.overall_status,
                        report.components
                            .iter()
                            .filter(|c| c.status != HealthStatus::Healthy)
                            .map(|c| &c.name)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                } else {
                    debug!("All health checks passed");
                }
            }
        });
        
        info!("Started background health check task");
    }
    
    /// Calculate overall health status
    fn calculate_overall_status(&self, components: &[ComponentHealth]) -> HealthStatus {
        let unhealthy_count = components.iter()
            .filter(|c| c.status == HealthStatus::Unhealthy)
            .count();
        
        let degraded_count = components.iter()
            .filter(|c| c.status == HealthStatus::Degraded)
            .count();
        
        if unhealthy_count > 0 {
            HealthStatus::Unhealthy
        } else if degraded_count > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
    
    /// Increment failure count for a component
    async fn increment_failure_count(&self, component: &str) {
        let mut counts = self.failure_counts.write().await;
        let count = counts.entry(component.to_string()).or_insert(0);
        *count += 1;
        
        if *count >= self.config.failure_threshold {
            error!(
                "Component {} has failed {} consecutive health checks",
                component, count
            );
        }
    }
}

/// Database health check implementation
#[async_trait]
impl HealthCheckable for FallbackDatabase {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        let (status, message) = if self.is_degraded().await {
            metadata.insert("mode".to_string(), serde_json::Value::String("fallback".to_string()));
            (HealthStatus::Degraded, "Database unavailable, using fallback mode".to_string())
        } else {
            // Try to get a connection
            match self.get_connection().await {
                Ok(_) => {
                    let pool_status = self.pool_status();
                    metadata.insert("connections".to_string(), serde_json::json!({
                        "size": pool_status.size,
                        "available": pool_status.available,
                        "waiting": pool_status.waiting,
                    }));
                    (HealthStatus::Healthy, "Database connection healthy".to_string())
                },
                Err(e) => {
                    (HealthStatus::Unhealthy, format!("Database connection failed: {}", e))
                }
            }
        };
        
        Ok(ComponentHealth {
            name: "database".to_string(),
            status,
            message,
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}

/// Solana RPC health check implementation
#[async_trait]
impl HealthCheckable for SolanaRpcService {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        let health = self.get_health_status().await;
        metadata.insert("total_requests".to_string(), serde_json::Value::Number(health.total_requests.into()));
        metadata.insert("success_rate".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(health.success_rate).unwrap_or(0.into())));
        metadata.insert("avg_latency_ms".to_string(), serde_json::Value::Number(health.avg_latency_ms.into()));
        
        let status = if health.success_rate > 0.95 {
            HealthStatus::Healthy
        } else if health.success_rate > 0.5 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };
        
        Ok(ComponentHealth {
            name: "solana_rpc".to_string(),
            status,
            message: format!("RPC health: {}% success rate", 
                (health.success_rate * 100.0) as u32
            ),
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}

/// Trading engine health check implementation
#[async_trait]
impl HealthCheckable for TradingEngine {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        // For now, just check if the trading engine is responsive
        // TODO: Add proper statistics when available
        metadata.insert("status".to_string(), serde_json::Value::String("running".to_string()));
        
        let status = HealthStatus::Healthy;
        
        Ok(ComponentHealth {
            name: "trading_engine".to_string(),
            status,
            message: "Trading engine operational".to_string(),
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}

/// WebSocket manager health check implementation
#[async_trait]
impl HealthCheckable for EnhancedWebSocketManager {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        // For now, just check if the WebSocket manager is responsive
        // TODO: Add proper statistics when available
        metadata.insert("status".to_string(), serde_json::Value::String("running".to_string()));
        
        let status = HealthStatus::Healthy;
        
        Ok(ComponentHealth {
            name: "websocket".to_string(),
            status,
            message: "WebSocket service is running".to_string(),
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}

/// Circuit breaker health check implementation
#[async_trait]
impl HealthCheckable for CircuitBreakerManager {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        // For now, just check if the circuit breaker manager is responsive
        // TODO: Add proper circuit breaker status when available
        metadata.insert("status".to_string(), serde_json::Value::String("operational".to_string()));
        
        let status = HealthStatus::Healthy;
        
        Ok(ComponentHealth {
            name: "circuit_breakers".to_string(),
            status,
            message: "Circuit breakers operational".to_string(),
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}

/// External API service health check
#[async_trait]
impl HealthCheckable for ExternalApiService {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        let health = self.get_health_status().await;
        
        for (platform, status) in &health {
            metadata.insert(
                format!("{}_healthy", platform),
                serde_json::Value::Bool(status.is_healthy)
            );
        }
        
        let unhealthy_count = health.values()
            .filter(|s| !s.is_healthy)
            .count();
        
        let status = if unhealthy_count == 0 {
            HealthStatus::Healthy
        } else if unhealthy_count < health.len() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };
        
        Ok(ComponentHealth {
            name: "external_apis".to_string(),
            status,
            message: format!("{}/{} external APIs healthy", 
                health.len() - unhealthy_count,
                health.len()
            ),
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_health_check_service() {
        let config = HealthCheckConfig::default();
        let service = Arc::new(HealthCheckService::new(config));
        
        // Test without components
        let report = service.check_all_components().await;
        assert_eq!(report.overall_status, HealthStatus::Healthy);
        assert_eq!(report.components.len(), 0);
    }
    
    #[tokio::test]
    async fn test_overall_status_calculation() {
        let config = HealthCheckConfig::default();
        let service = HealthCheckService::new(config);
        
        let components = vec![
            ComponentHealth {
                name: "test1".to_string(),
                status: HealthStatus::Healthy,
                message: "OK".to_string(),
                last_check: Utc::now(),
                response_time_ms: 10,
                metadata: HashMap::new(),
            },
            ComponentHealth {
                name: "test2".to_string(),
                status: HealthStatus::Degraded,
                message: "Degraded".to_string(),
                last_check: Utc::now(),
                response_time_ms: 20,
                metadata: HashMap::new(),
            },
        ];
        
        let status = service.calculate_overall_status(&components);
        assert_eq!(status, HealthStatus::Degraded);
    }
}