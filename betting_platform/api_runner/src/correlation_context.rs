//! Correlation context for distributed tracing

use std::sync::Arc;
use tokio::task_local;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use crate::tracing_logger::CorrelationId;

// Task-local storage for correlation context
task_local! {
    static CORRELATION_CONTEXT: Arc<CorrelationContext>;
}

/// Correlation context for tracking requests across services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationContext {
    /// Unique correlation ID for the request
    pub correlation_id: CorrelationId,
    /// Parent span ID for distributed tracing
    pub parent_span_id: Option<String>,
    /// Current span ID
    pub span_id: String,
    /// User information
    pub user_context: Option<UserContext>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Request start time
    pub start_time: i64,
}

/// User context within correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub user_id: String,
    pub wallet_address: String,
    pub role: String,
    pub ip_address: Option<String>,
}

impl CorrelationContext {
    /// Create new correlation context
    pub fn new(correlation_id: CorrelationId) -> Self {
        Self {
            correlation_id,
            parent_span_id: None,
            span_id: uuid::Uuid::new_v4().to_string(),
            user_context: None,
            metadata: HashMap::new(),
            start_time: chrono::Utc::now().timestamp_millis(),
        }
    }
    
    /// Create child context
    pub fn child(&self) -> Self {
        Self {
            correlation_id: self.correlation_id.clone(),
            parent_span_id: Some(self.span_id.clone()),
            span_id: uuid::Uuid::new_v4().to_string(),
            user_context: self.user_context.clone(),
            metadata: self.metadata.clone(),
            start_time: chrono::Utc::now().timestamp_millis(),
        }
    }
    
    /// Set user context
    pub fn with_user(mut self, user: UserContext) -> Self {
        self.user_context = Some(user);
        self
    }
    
    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis() - self.start_time
    }
}

/// Run async function with correlation context
pub async fn with_correlation_context<F, T>(
    context: Arc<CorrelationContext>,
    f: F,
) -> T
where
    F: std::future::Future<Output = T>,
{
    CORRELATION_CONTEXT.scope(context, f).await
}

/// Get current correlation context
pub fn current_context() -> Option<Arc<CorrelationContext>> {
    CORRELATION_CONTEXT.try_with(|ctx| ctx.clone()).ok()
}

/// Get current correlation ID
pub fn current_correlation_id() -> Option<CorrelationId> {
    current_context().map(|ctx| ctx.correlation_id.clone())
}

/// Helper macro to log with current correlation context
#[macro_export]
macro_rules! log_with_current_context {
    ($level:expr, $($arg:tt)*) => {
        if let Some(ctx) = $crate::correlation_context::current_context() {
            match $level {
                tracing::Level::ERROR => {
                    tracing::error!(
                        correlation_id = %ctx.correlation_id,
                        span_id = %ctx.span_id,
                        parent_span_id = ?ctx.parent_span_id,
                        user_id = ?ctx.user_context.as_ref().map(|u| &u.user_id),
                        $($arg)*
                    );
                }
                tracing::Level::WARN => {
                    tracing::warn!(
                        correlation_id = %ctx.correlation_id,
                        span_id = %ctx.span_id,
                        parent_span_id = ?ctx.parent_span_id,
                        user_id = ?ctx.user_context.as_ref().map(|u| &u.user_id),
                        $($arg)*
                    );
                }
                tracing::Level::INFO => {
                    tracing::info!(
                        correlation_id = %ctx.correlation_id,
                        span_id = %ctx.span_id,
                        parent_span_id = ?ctx.parent_span_id,
                        user_id = ?ctx.user_context.as_ref().map(|u| &u.user_id),
                        $($arg)*
                    );
                }
                tracing::Level::DEBUG => {
                    tracing::debug!(
                        correlation_id = %ctx.correlation_id,
                        span_id = %ctx.span_id,
                        parent_span_id = ?ctx.parent_span_id,
                        user_id = ?ctx.user_context.as_ref().map(|u| &u.user_id),
                        $($arg)*
                    );
                }
                tracing::Level::TRACE => {
                    tracing::trace!(
                        correlation_id = %ctx.correlation_id,
                        span_id = %ctx.span_id,
                        parent_span_id = ?ctx.parent_span_id,
                        user_id = ?ctx.user_context.as_ref().map(|u| &u.user_id),
                        $($arg)*
                    );
                }
            }
        } else {
            match $level {
                tracing::Level::ERROR => tracing::error!($($arg)*),
                tracing::Level::WARN => tracing::warn!($($arg)*),
                tracing::Level::INFO => tracing::info!($($arg)*),
                tracing::Level::DEBUG => tracing::debug!($($arg)*),
                tracing::Level::TRACE => tracing::trace!($($arg)*),
            }
        }
    };
}

/// Propagate correlation context to external services
pub fn propagate_context(context: &CorrelationContext) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    
    headers.insert(
        "x-correlation-id".to_string(),
        context.correlation_id.0.clone(),
    );
    
    headers.insert(
        "x-span-id".to_string(),
        context.span_id.clone(),
    );
    
    if let Some(parent) = &context.parent_span_id {
        headers.insert(
            "x-parent-span-id".to_string(),
            parent.clone(),
        );
    }
    
    if let Some(user) = &context.user_context {
        headers.insert(
            "x-user-id".to_string(),
            user.user_id.clone(),
        );
    }
    
    headers
}

/// Extract correlation context from headers
pub fn extract_context(headers: &HashMap<String, String>) -> Option<CorrelationContext> {
    let correlation_id = headers.get("x-correlation-id")
        .map(|id| CorrelationId::from_string(id.clone()))?;
    
    let mut context = CorrelationContext::new(correlation_id);
    
    if let Some(parent_span) = headers.get("x-parent-span-id") {
        context.parent_span_id = Some(parent_span.clone());
    }
    
    if let Some(span_id) = headers.get("x-span-id") {
        context.span_id = span_id.clone();
    }
    
    Some(context)
}

/// Correlation context builder
pub struct ContextBuilder {
    context: CorrelationContext,
}

impl ContextBuilder {
    /// Create new builder
    pub fn new(correlation_id: CorrelationId) -> Self {
        Self {
            context: CorrelationContext::new(correlation_id),
        }
    }
    
    /// Set parent span
    pub fn parent_span(mut self, span_id: String) -> Self {
        self.context.parent_span_id = Some(span_id);
        self
    }
    
    /// Set user context
    pub fn user(mut self, user: UserContext) -> Self {
        self.context.user_context = Some(user);
        self
    }
    
    /// Add metadata
    pub fn metadata(mut self, key: String, value: String) -> Self {
        self.context.metadata.insert(key, value);
        self
    }
    
    /// Build context
    pub fn build(self) -> Arc<CorrelationContext> {
        Arc::new(self.context)
    }
}