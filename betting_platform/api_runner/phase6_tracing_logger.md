# Phase 6.3: Enhanced Logging with Tracing and Correlation IDs

## Overview
Implemented a production-ready distributed tracing system with correlation IDs for comprehensive request tracking across the platform.

## Components

### 1. Tracing Logger (`tracing_logger.rs`)
- **Correlation IDs**: Unique identifiers for tracking requests
- **Request Context**: Captures user, path, method, timing
- **Operation Metrics**: Performance tracking for all operations
- **Structured Logging**: JSON format with rich metadata

### 2. Tracing Middleware (`tracing_middleware.rs`)
- **Automatic Correlation ID**: Generation and propagation
- **Request/Response Logging**: Complete lifecycle tracking
- **Header Propagation**: X-Correlation-ID headers
- **Performance Monitoring**: Automatic timing of requests

### 3. Correlation Context (`correlation_context.rs`)
- **Task-Local Storage**: Thread-safe context propagation
- **Distributed Tracing**: Parent/child span relationships
- **User Context**: Track user information across operations
- **Metadata Support**: Additional context for debugging

## Key Features

### Request Tracking
```rust
// Every request gets a unique correlation ID
X-Correlation-ID: 550e8400-e29b-41d4-a716-446655440000

// Propagated through all operations
correlation_id = "550e8400-e29b-41d4-a716-446655440000"
path = "/api/markets"
method = "GET"
user_id = "user123"
duration_ms = 145
```

### Operation Logging
```rust
// Database operations
logger.log_query(
    "SELECT",
    "markets",
    correlation_id,
    duration,
    success,
    error
).await;

// External API calls
logger.log_external_api_call(
    "polymarket",
    "/markets",
    "GET",
    correlation_id,
    duration,
    status_code,
    error
).await;

// Solana transactions
logger.log_solana_transaction(
    "place_bet",
    signature,
    correlation_id,
    duration,
    success,
    error
).await;
```

### Performance Metrics
- Operation duration tracking
- Success/failure rates
- Slow query detection
- External API latency

## Integration Points

### 1. HTTP Middleware
All HTTP requests automatically get:
- Correlation ID generation
- Request/response logging
- Performance tracking
- Error correlation

### 2. Database Operations
```rust
log_db_operation!(
    logger,
    correlation_id,
    "SELECT",
    "markets",
    db.query(&query).await
)
```

### 3. External APIs
```rust
log_api_call!(
    logger,
    correlation_id,
    "polymarket",
    "/markets",
    "GET",
    client.get(url).send().await
)
```

### 4. WebSocket Events
```rust
logger.log_websocket_event(
    "trade_execution",
    connection_id,
    Some(correlation_id),
    metadata
).await;
```

## Logging Levels

### Structured Output
```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "level": "INFO",
  "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
  "span_id": "a1b2c3d4",
  "parent_span_id": "parent123",
  "user_id": "user123",
  "path": "/api/markets",
  "method": "GET",
  "duration_ms": 145,
  "status": 200,
  "message": "Request completed"
}
```

### Log Levels
- **ERROR**: System errors, failures
- **WARN**: Slow queries, high latency, security events
- **INFO**: Request lifecycle, important operations
- **DEBUG**: Detailed operation tracking
- **TRACE**: Fine-grained debugging

## Benefits

### 1. Request Traceability
- Track requests across all services
- Correlate errors with specific requests
- Debug complex distributed operations

### 2. Performance Monitoring
- Identify slow operations
- Track external API latency
- Monitor database query performance

### 3. Security Auditing
- Track user actions
- Correlate security events
- Audit trail with full context

### 4. Debugging
- Rich context for error investigation
- Complete request lifecycle visibility
- Cross-service correlation

## Usage Examples

### Basic Request Tracking
```rust
// Automatic in HTTP handlers
async fn get_markets(
    req: Request<Body>,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    // Correlation ID automatically available
    let correlation_id = get_correlation_id(&req).await;
    
    // All operations logged with correlation
    let markets = fetch_markets(&correlation_id).await?;
    
    Ok(Json(markets))
}
```

### Complex Operations
```rust
// Track multi-step operations
let context = logger.create_request_context(
    "/api/trades".to_string(),
    "POST".to_string()
).await;

// Step 1: Validate user
logger.log_operation(
    "validate_user",
    &context.correlation_id.0,
    metadata,
    validate_user(user_id)
).await?;

// Step 2: Execute trade
logger.log_operation(
    "execute_trade",
    &context.correlation_id.0,
    metadata,
    execute_trade(trade_request)
).await?;

// Complete context
logger.complete_request_context(
    &context.correlation_id.0,
    200,
    None
).await;
```

### Cross-Service Tracing
```rust
// Propagate to external services
let headers = propagate_context(&correlation_context);
client.post(url)
    .headers(headers)
    .json(&request)
    .send()
    .await?;
```

## Configuration

### Environment Variables
```bash
# Log level
RUST_LOG=info,betting_platform_api=debug

# Enable JSON logging
LOG_FORMAT=json

# Performance thresholds
SLOW_QUERY_THRESHOLD_MS=1000
SLOW_API_THRESHOLD_MS=5000
```

### Initialization
```rust
// Initialize tracing
TracingLogger::init_subscriber();

// Create logger instance
let logger = Arc::new(TracingLogger::new(Level::INFO));
```

## Monitoring Integration

The tracing system provides data for:
- Application Performance Monitoring (APM)
- Log aggregation systems
- Metrics dashboards
- Alert systems

## Future Enhancements

1. **OpenTelemetry Integration**: Export traces to Jaeger/Zipkin
2. **Sampling**: Intelligent trace sampling for high-volume
3. **Trace Storage**: Long-term storage for audit trails
4. **Real-time Dashboards**: Live monitoring of traces
5. **AI-powered Analysis**: Anomaly detection in traces

## Code Quality

- Thread-safe implementation
- Zero-allocation where possible
- Minimal performance overhead
- Production-ready with proper error handling
- Comprehensive test coverage