//! Throughput optimization for high-performance API endpoints

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::time::Duration;
use tower::ServiceBuilder;
// use tower_http::compression::CompressionLayer;
// use tower_http::timeout::TimeoutLayer;

/// Configuration for high-throughput endpoints
pub struct ThroughputConfig {
    /// Request timeout
    pub request_timeout: Duration,
    /// Enable response compression
    pub enable_compression: bool,
    /// Keep-alive timeout
    pub keep_alive_timeout: Duration,
    /// TCP nodelay (disable Nagle's algorithm)
    pub tcp_nodelay: bool,
}

impl Default for ThroughputConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            enable_compression: true,
            keep_alive_timeout: Duration::from_secs(75),
            tcp_nodelay: true,
        }
    }
}

/// Create optimized service layers for high throughput
pub fn create_optimized_layers() -> ServiceBuilder<tower::layer::util::Identity> {
    ServiceBuilder::new()
        // Future: Add compression and timeout layers when available
        // .layer(TimeoutLayer::new(Duration::from_secs(30)))
        // .layer(CompressionLayer::new())
}

/// Fast JSON response builder that avoids unnecessary allocations
pub struct FastJson<T>(pub T);

impl<T> IntoResponse for FastJson<T>
where
    T: serde::Serialize,
{
    fn into_response(self) -> Response {
        // Pre-allocate buffer with estimated size
        let mut buf = Vec::with_capacity(1024);
        
        match serde_json::to_writer(&mut buf, &self.0) {
            Ok(()) => {
                let headers = [
                    (axum::http::header::CONTENT_TYPE, "application/json"),
                    (axum::http::header::CACHE_CONTROL, "no-cache"),
                ];
                
                (StatusCode::OK, headers, buf).into_response()
            }
            Err(e) => {
                tracing::error!("Failed to serialize response: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// TCP socket optimization settings
pub fn optimize_tcp_socket(socket: &std::net::TcpListener) -> std::io::Result<()> {
    use socket2::{Socket, Domain, Type};
    
    let sock = Socket::from(socket.try_clone()?);
    
    // Enable TCP_NODELAY to disable Nagle's algorithm
    sock.set_nodelay(true)?;
    
    // Set socket buffer sizes for better throughput
    sock.set_send_buffer_size(256 * 1024)?; // 256KB send buffer
    sock.set_recv_buffer_size(256 * 1024)?; // 256KB receive buffer
    
    // Enable SO_REUSEADDR for faster restarts
    sock.set_reuse_address(true)?;
    
    Ok(())
}

/// Batch response helper for multiple items
pub struct BatchResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub has_more: bool,
}

impl<T: serde::Serialize> BatchResponse<T> {
    pub fn new(items: Vec<T>, total: usize) -> Self {
        let has_more = items.len() < total;
        Self {
            items,
            total,
            has_more,
        }
    }
}

/// Performance metrics for monitoring
#[derive(Default)]
pub struct PerformanceMetrics {
    pub request_count: std::sync::atomic::AtomicU64,
    pub total_response_time_ms: std::sync::atomic::AtomicU64,
}

impl PerformanceMetrics {
    pub fn record_request(&self, duration_ms: u64) {
        self.request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(duration_ms, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn average_response_time_ms(&self) -> f64 {
        let count = self.request_count.load(std::sync::atomic::Ordering::Relaxed);
        let total = self.total_response_time_ms.load(std::sync::atomic::Ordering::Relaxed);
        
        if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        }
    }
}