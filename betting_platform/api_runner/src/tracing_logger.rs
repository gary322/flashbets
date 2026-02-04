//! Enhanced logging with tracing and correlation IDs

use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn, error, debug, trace, span, Level, Span};
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
    Layer,
};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Correlation ID for request tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationId(pub String);

impl CorrelationId {
    /// Generate new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
    
    /// Create from existing ID
    pub fn from_string(id: String) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Request context for tracing
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub correlation_id: CorrelationId,
    pub user_id: Option<String>,
    pub wallet_address: Option<String>,
    pub request_path: String,
    pub request_method: String,
    pub start_time: Instant,
    pub span: Span,
}

impl RequestContext {
    /// Create new request context
    pub fn new(path: String, method: String) -> Self {
        let correlation_id = CorrelationId::new();
        let span = span!(
            Level::INFO,
            "request",
            correlation_id = %correlation_id,
            path = %path,
            method = %method,
        );
        
        Self {
            correlation_id,
            user_id: None,
            wallet_address: None,
            request_path: path,
            request_method: method,
            start_time: Instant::now(),
            span,
        }
    }
    
    /// Set user information
    pub fn set_user(&mut self, user_id: String, wallet_address: String) {
        self.user_id = Some(user_id);
        self.wallet_address = Some(wallet_address);
        self.span.record("user_id", &self.user_id.as_ref().unwrap().as_str());
        self.span.record("wallet", &self.wallet_address.as_ref().unwrap().as_str());
    }
    
    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Performance metrics for operations
#[derive(Debug, Clone, Serialize)]
pub struct OperationMetrics {
    pub operation: String,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Tracing logger manager
pub struct TracingLogger {
    /// Active request contexts
    contexts: Arc<RwLock<HashMap<String, RequestContext>>>,
    /// Performance metrics
    metrics: Arc<RwLock<Vec<OperationMetrics>>>,
    /// Log level
    log_level: Level,
}

impl TracingLogger {
    /// Initialize new tracing logger
    pub fn new(log_level: Level) -> Self {
        Self {
            contexts: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(Vec::new())),
            log_level,
        }
    }
    
    /// Initialize global tracing subscriber
    pub fn init_subscriber() {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));
        
        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true);
        
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
        
        info!("Tracing logger initialized");
    }
    
    /// Create request context
    pub async fn create_request_context(
        &self,
        path: String,
        method: String,
    ) -> RequestContext {
        let context = RequestContext::new(path, method);
        let correlation_id = context.correlation_id.0.clone();
        
        self.contexts.write().await.insert(correlation_id.clone(), context.clone());
        
        info!(
            correlation_id = %correlation_id,
            path = %context.request_path,
            method = %context.request_method,
            "Request started"
        );
        
        context
    }
    
    /// Complete request context
    pub async fn complete_request_context(
        &self,
        correlation_id: &str,
        status_code: u16,
        error: Option<String>,
    ) {
        if let Some(context) = self.contexts.write().await.remove(correlation_id) {
            let duration = context.elapsed();
            
            if let Some(err) = &error {
                error!(
                    correlation_id = %correlation_id,
                    user_id = ?context.user_id,
                    wallet = ?context.wallet_address,
                    path = %context.request_path,
                    method = %context.request_method,
                    status = status_code,
                    duration_ms = duration.as_millis(),
                    error = %err,
                    "Request failed"
                );
            } else {
                info!(
                    correlation_id = %correlation_id,
                    user_id = ?context.user_id,
                    wallet = ?context.wallet_address,
                    path = %context.request_path,
                    method = %context.request_method,
                    status = status_code,
                    duration_ms = duration.as_millis(),
                    "Request completed"
                );
            }
        }
    }
    
    /// Log operation with metrics
    pub async fn log_operation<F, T, E>(
        &self,
        operation: &str,
        correlation_id: &str,
        metadata: HashMap<String, serde_json::Value>,
        f: F,
    ) -> Result<T, E>
    where
        F: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let start = Instant::now();
        let op_span = span!(
            Level::DEBUG,
            "operation",
            name = %operation,
            correlation_id = %correlation_id,
        );
        
        let _enter = op_span.enter();
        
        debug!(
            operation = %operation,
            correlation_id = %correlation_id,
            metadata = ?metadata,
            "Operation started"
        );
        
        let result = f.await;
        let duration = start.elapsed();
        
        let metrics = OperationMetrics {
            operation: operation.to_string(),
            duration_ms: duration.as_millis() as u64,
            success: result.is_ok(),
            error: result.as_ref().err().map(|e| e.to_string()),
            metadata,
        };
        
        self.metrics.write().await.push(metrics.clone());
        
        match &result {
            Ok(_) => {
                debug!(
                    operation = %operation,
                    correlation_id = %correlation_id,
                    duration_ms = duration.as_millis(),
                    "Operation completed successfully"
                );
            }
            Err(e) => {
                error!(
                    operation = %operation,
                    correlation_id = %correlation_id,
                    duration_ms = duration.as_millis(),
                    error = %e,
                    "Operation failed"
                );
            }
        }
        
        result
    }
    
    /// Log database query
    pub async fn log_query(
        &self,
        query_type: &str,
        table: &str,
        correlation_id: &str,
        duration: Duration,
        success: bool,
        error: Option<String>,
    ) {
        let level = if duration.as_millis() > 1000 {
            Level::WARN
        } else {
            Level::DEBUG
        };
        
        match level {
            Level::WARN => {
                warn!(
                    query_type = %query_type,
                    table = %table,
                    correlation_id = %correlation_id,
                    duration_ms = duration.as_millis(),
                    success = success,
                    error = ?error,
                    "Slow database query"
                );
            }
            _ => {
                debug!(
                    query_type = %query_type,
                    table = %table,
                    correlation_id = %correlation_id,
                    duration_ms = duration.as_millis(),
                    success = success,
                    error = ?error,
                    "Database query executed"
                );
            }
        }
    }
    
    /// Log external API call
    pub async fn log_external_api_call(
        &self,
        service: &str,
        endpoint: &str,
        method: &str,
        correlation_id: &str,
        duration: Duration,
        status_code: Option<u16>,
        error: Option<String>,
    ) {
        if let Some(err) = &error {
            error!(
                service = %service,
                endpoint = %endpoint,
                method = %method,
                correlation_id = %correlation_id,
                duration_ms = duration.as_millis(),
                status_code = ?status_code,
                error = %err,
                "External API call failed"
            );
        } else {
            info!(
                service = %service,
                endpoint = %endpoint,
                method = %method,
                correlation_id = %correlation_id,
                duration_ms = duration.as_millis(),
                status_code = ?status_code,
                "External API call completed"
            );
        }
    }
    
    /// Log Solana transaction
    pub async fn log_solana_transaction(
        &self,
        transaction_type: &str,
        signature: Option<&str>,
        correlation_id: &str,
        duration: Duration,
        success: bool,
        error: Option<String>,
    ) {
        if success {
            info!(
                transaction_type = %transaction_type,
                signature = ?signature,
                correlation_id = %correlation_id,
                duration_ms = duration.as_millis(),
                "Solana transaction successful"
            );
        } else {
            error!(
                transaction_type = %transaction_type,
                correlation_id = %correlation_id,
                duration_ms = duration.as_millis(),
                error = ?error,
                "Solana transaction failed"
            );
        }
    }
    
    /// Log WebSocket event
    pub async fn log_websocket_event(
        &self,
        event_type: &str,
        connection_id: &str,
        correlation_id: Option<&str>,
        metadata: HashMap<String, serde_json::Value>,
    ) {
        debug!(
            event_type = %event_type,
            connection_id = %connection_id,
            correlation_id = ?correlation_id,
            metadata = ?metadata,
            "WebSocket event"
        );
    }
    
    /// Log security event
    pub async fn log_security_event(
        &self,
        event_type: &str,
        severity: &str,
        correlation_id: &str,
        user_id: Option<&str>,
        ip_address: Option<&str>,
        details: &str,
    ) {
        warn!(
            event_type = %event_type,
            severity = %severity,
            correlation_id = %correlation_id,
            user_id = ?user_id,
            ip_address = ?ip_address,
            details = %details,
            "Security event detected"
        );
    }
    
    /// Get performance metrics
    pub async fn get_metrics(&self) -> Vec<OperationMetrics> {
        self.metrics.read().await.clone()
    }
    
    /// Clear old metrics
    pub async fn clear_old_metrics(&self, older_than: Duration) {
        let cutoff = Instant::now() - older_than;
        // In production, we'd track creation time for each metric
        // For now, just clear if metrics list is too large
        let mut metrics = self.metrics.write().await;
        if metrics.len() > 10000 {
            metrics.clear();
        }
    }
}

/// Span builder for complex operations
pub struct SpanBuilder {
    name: String,
    level: Level,
    fields: HashMap<String, String>,
}

impl SpanBuilder {
    /// Create new span builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            level: Level::INFO,
            fields: HashMap::new(),
        }
    }
    
    /// Set span level
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
    
    /// Add field to span
    pub fn field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
    
    /// Build span
    pub fn build(self) -> Span {
        let mut span = match self.level {
            Level::ERROR => span!(Level::ERROR, "operation", name = %self.name),
            Level::WARN => span!(Level::WARN, "operation", name = %self.name),
            Level::INFO => span!(Level::INFO, "operation", name = %self.name),
            Level::DEBUG => span!(Level::DEBUG, "operation", name = %self.name),
            Level::TRACE => span!(Level::TRACE, "operation", name = %self.name),
        };
        for (key, value) in self.fields {
            span.record(key.as_str(), &value.as_str());
        }
        span
    }
}

/// Structured logging macros
#[macro_export]
macro_rules! log_with_context {
    ($level:expr, $correlation_id:expr, $($key:tt = $value:expr),* $(,)?) => {
        match $level {
            tracing::Level::ERROR => {
                tracing::error!(
                    correlation_id = %$correlation_id,
                    $($key = $value),*
                );
            }
            tracing::Level::WARN => {
                tracing::warn!(
                    correlation_id = %$correlation_id,
                    $($key = $value),*
                );
            }
            tracing::Level::INFO => {
                tracing::info!(
                    correlation_id = %$correlation_id,
                    $($key = $value),*
                );
            }
            tracing::Level::DEBUG => {
                tracing::debug!(
                    correlation_id = %$correlation_id,
                    $($key = $value),*
                );
            }
            tracing::Level::TRACE => {
                tracing::trace!(
                    correlation_id = %$correlation_id,
                    $($key = $value),*
                );
            }
        }
    };
}

/// Log timing for operations
#[macro_export]
macro_rules! time_operation {
    ($name:expr, $correlation_id:expr, $op:expr) => {{
        let start = std::time::Instant::now();
        let result = $op;
        let duration = start.elapsed();
        
        tracing::debug!(
            operation = $name,
            correlation_id = %$correlation_id,
            duration_ms = duration.as_millis(),
            "Operation completed"
        );
        
        result
    }};
}