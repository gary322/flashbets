//! Simple rate limiting implementation

use axum::{
    http::{header, HeaderValue, StatusCode, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower::Service;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

/// Rate limit entry for tracking requests
#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: u32,
    reset_at: Instant,
}

/// Simple in-memory rate limiter
pub struct SimpleRateLimiter {
    limits: Arc<Mutex<HashMap<IpAddr, RateLimitEntry>>>,
    max_requests: u32,
    window_duration: Duration,
}

impl SimpleRateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            limits: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_duration: Duration::from_secs(window_seconds),
        }
    }

    async fn check_rate_limit(&self, ip: IpAddr) -> Result<(u32, Instant), StatusCode> {
        let mut limits = self.limits.lock().await;
        let now = Instant::now();

        let entry = limits.entry(ip).or_insert_with(|| RateLimitEntry {
            count: 0,
            reset_at: now + self.window_duration,
        });

        // Reset if window expired
        if now >= entry.reset_at {
            entry.count = 0;
            entry.reset_at = now + self.window_duration;
        }

        // Check limit
        if entry.count >= self.max_requests {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        // Increment counter
        entry.count += 1;
        let remaining = self.max_requests - entry.count;
        
        Ok((remaining, entry.reset_at))
    }
}

// Global rate limiter instance
lazy_static::lazy_static! {
    static ref RATE_LIMITER: SimpleRateLimiter = SimpleRateLimiter::new(600, 60); // 600 requests per minute (10 per second)
}

/// Rate limiting middleware
pub async fn rate_limit_middleware<B>(
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> 
where
    B: Send + 'static,
{
    // Extract IP from X-Forwarded-For or socket address
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .unwrap_or_else(|| IpAddr::from([127, 0, 0, 1]));

    // Check rate limit
    match RATE_LIMITER.check_rate_limit(ip).await {
        Ok((remaining, reset_at)) => {
            let mut response = next.run(req).await;
            
            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Limit",
                HeaderValue::from_str(&RATE_LIMITER.max_requests.to_string()).unwrap(),
            );
            headers.insert(
                "X-RateLimit-Remaining",
                HeaderValue::from_str(&remaining.to_string()).unwrap(),
            );
            let reset_timestamp = reset_at.duration_since(Instant::now()).as_secs();
            headers.insert(
                "X-RateLimit-Reset",
                HeaderValue::from_str(&reset_timestamp.to_string()).unwrap(),
            );
            
            Ok(response)
        }
        Err(status) => {
            let mut response = (
                status,
                [(header::RETRY_AFTER, "60")],
                "Rate limit exceeded. Please try again later.",
            ).into_response();
            
            // Add rate limit headers
            let headers = response.headers_mut();
            headers.insert(
                "X-RateLimit-Limit",
                HeaderValue::from_str(&RATE_LIMITER.max_requests.to_string()).unwrap(),
            );
            headers.insert(
                "X-RateLimit-Remaining",
                HeaderValue::from_static("0"),
            );
            headers.insert(
                "X-RateLimit-Reset",
                HeaderValue::from_static("60"),
            );
            
            Err(status)
        }
    }
}

/// Rate limiting layer that can be cloned
#[derive(Clone)]
pub struct RateLimitLayer;

impl<S> tower::Layer<S> for RateLimitLayer {
    type Service = RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitMiddleware { inner }
    }
}

/// Rate limiting middleware service
#[derive(Clone)]
pub struct RateLimitMiddleware<S> {
    inner: S,
}

impl<S, B> tower::Service<Request<B>> for RateLimitMiddleware<S>
where
    S: tower::Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        
        Box::pin(async move {
            // Extract IP from X-Forwarded-For or socket address
            let ip = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
                .unwrap_or_else(|| IpAddr::from([127, 0, 0, 1]));

            // Check rate limit
            match RATE_LIMITER.check_rate_limit(ip).await {
                Ok((remaining, reset_at)) => {
                    let mut response = inner.call(req).await?;
                    
                    // Add rate limit headers
                    let headers = response.headers_mut();
                    headers.insert(
                        "X-RateLimit-Limit",
                        HeaderValue::from_str(&RATE_LIMITER.max_requests.to_string()).unwrap(),
                    );
                    headers.insert(
                        "X-RateLimit-Remaining",
                        HeaderValue::from_str(&remaining.to_string()).unwrap(),
                    );
                    let reset_timestamp = reset_at.duration_since(Instant::now()).as_secs();
                    headers.insert(
                        "X-RateLimit-Reset",
                        HeaderValue::from_str(&reset_timestamp.to_string()).unwrap(),
                    );
                    
                    Ok(response)
                }
                Err(_) => {
                    let mut response = (
                        StatusCode::TOO_MANY_REQUESTS,
                        [(header::RETRY_AFTER, "60")],
                        "Rate limit exceeded. Please try again later.",
                    ).into_response();
                    
                    // Add rate limit headers
                    let headers = response.headers_mut();
                    headers.insert(
                        "X-RateLimit-Limit",
                        HeaderValue::from_str(&RATE_LIMITER.max_requests.to_string()).unwrap(),
                    );
                    headers.insert(
                        "X-RateLimit-Remaining",
                        HeaderValue::from_static("0"),
                    );
                    headers.insert(
                        "X-RateLimit-Reset",
                        HeaderValue::from_static("60"),
                    );
                    
                    Ok(response)
                }
            }
        })
    }
}

/// Create a simple rate limiting layer
pub fn create_rate_limit_layer() -> RateLimitLayer {
    RateLimitLayer
}