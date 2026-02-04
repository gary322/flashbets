# Health Check System Documentation

## Overview

The Health Check System provides comprehensive monitoring of all system components in the betting platform. It offers real-time health status, metrics, and detailed component diagnostics for production monitoring.

## Architecture

### Core Components

1. **HealthCheckService** (`health_check_service.rs`)
   - Central service managing all health checks
   - Component registration and monitoring
   - Background health check scheduling
   - Health status aggregation

2. **Health Check Endpoints** (`health_check_endpoints.rs`)
   - REST API endpoints for health monitoring
   - Kubernetes-compatible probes
   - Prometheus metrics export
   - Admin diagnostic endpoints

3. **HealthCheckable Trait**
   - Interface for components to implement health checks
   - Async health check support
   - Detailed component metadata

## Health Status Levels

```rust
pub enum HealthStatus {
    Healthy,    // All components functioning normally
    Degraded,   // Some components degraded but service operational
    Unhealthy,  // Critical components failing
}
```

## Component Health Checks

### Database Health Check
- Checks connection pool status
- Verifies query execution
- Reports connection metrics
- Detects fallback mode

### Solana RPC Health Check
- Monitors RPC endpoint availability
- Tracks latest slot number
- Reports failed endpoints
- Checks endpoint rotation

### Trading Engine Health Check
- Monitors active order count
- Tracks market statistics
- Reports processing capacity
- Detects overload conditions

### WebSocket Health Check
- Counts active connections
- Monitors message throughput
- Detects connection limits
- Reports broadcast statistics

### Circuit Breaker Health Check
- Lists open circuit breakers
- Reports failure patterns
- Tracks recovery status
- Monitors service degradation

### External API Health Check
- Checks each platform status
- Reports API availability
- Tracks response times
- Monitors rate limits

## API Endpoints

### Liveness Probe
```http
GET /api/health/live

Response:
{
  "status": "success",
  "data": {
    "status": "Healthy",
    "timestamp": "2024-01-15T10:00:00Z",
    "uptime_seconds": 3600
  }
}
```

### Readiness Probe
```http
GET /api/health/ready

Response:
{
  "status": "success",
  "message": "Service is ready",
  "data": {
    "status": "Healthy",
    "timestamp": "2024-01-15T10:00:00Z",
    "uptime_seconds": 3600
  }
}
```

### Comprehensive Health Check
```http
GET /api/health/check?detailed=true&force_refresh=true

Response:
{
  "status": "success",
  "data": {
    "overall_status": "Healthy",
    "timestamp": "2024-01-15T10:00:00Z",
    "components": [
      {
        "name": "database",
        "status": "Healthy",
        "message": "Database connection healthy",
        "last_check": "2024-01-15T10:00:00Z",
        "response_time_ms": 5,
        "metadata": {
          "connections": {
            "active": 10,
            "idle": 90,
            "max": 100
          }
        }
      }
    ],
    "uptime_seconds": 3600,
    "version": "0.1.0",
    "environment": "production"
  }
}
```

### Component Health
```http
GET /api/health/component/database

Response:
{
  "status": "success",
  "data": {
    "name": "database",
    "status": "Healthy",
    "message": "Database connection healthy",
    "last_check": "2024-01-15T10:00:00Z",
    "response_time_ms": 5,
    "metadata": {
      "connections": {
        "active": 10,
        "idle": 90,
        "max": 100
      }
    }
  }
}
```

### Prometheus Metrics
```http
GET /api/health/metrics

Response:
# HELP health_status Overall health status (0=healthy, 1=degraded, 2=unhealthy)
# TYPE health_status gauge
health_status 0
# HELP uptime_seconds Service uptime in seconds
# TYPE uptime_seconds counter
uptime_seconds 3600
# HELP component_database_healthy Health status for database
# TYPE component_database_healthy gauge
component_database_healthy 1
# HELP component_database_response_time_ms Response time for database health check
# TYPE component_database_response_time_ms gauge
component_database_response_time_ms 5
```

### Admin Endpoints

#### Trigger Health Check
```http
POST /api/health/trigger
Authorization: Bearer <admin-token>

Response:
{
  "status": "success",
  "message": "Health check triggered",
  "data": {
    "overall_status": "Healthy",
    "components": [...]
  }
}
```

#### Health History
```http
GET /api/health/history?limit=100&offset=0
Authorization: Bearer <admin-token>

Response:
{
  "status": "success",
  "data": {
    "checks": [
      {
        "timestamp": "2024-01-15T10:00:00Z",
        "status": "Healthy",
        "unhealthy_components": []
      }
    ]
  }
}
```

## Configuration

```rust
pub struct HealthCheckConfig {
    pub check_interval: Duration,      // How often to run checks (default: 30s)
    pub timeout: Duration,             // Check timeout (default: 5s)
    pub failure_threshold: u32,        // Failures before marking unhealthy (default: 3)
    pub recovery_threshold: u32,       // Successes before marking healthy (default: 2)
    pub detailed_checks: bool,         // Include detailed metadata (default: true)
}
```

### Environment Variables

```bash
# Health check configuration
HEALTH_CHECK_INTERVAL=30          # Seconds between checks
HEALTH_CHECK_TIMEOUT=5            # Timeout for individual checks
HEALTH_FAILURE_THRESHOLD=3        # Consecutive failures before unhealthy
HEALTH_RECOVERY_THRESHOLD=2       # Consecutive successes before healthy
HEALTH_DETAILED_CHECKS=true       # Include detailed metadata
```

## Integration

### Kubernetes Integration

#### Liveness Probe
```yaml
livenessProbe:
  httpGet:
    path: /api/health/live
    port: 8081
  initialDelaySeconds: 30
  periodSeconds: 10
  timeoutSeconds: 5
  failureThreshold: 3
```

#### Readiness Probe
```yaml
readinessProbe:
  httpGet:
    path: /api/health/ready
    port: 8081
  initialDelaySeconds: 10
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 3
```

### Prometheus Integration

```yaml
scrape_configs:
  - job_name: 'betting-platform'
    scrape_interval: 30s
    metrics_path: '/api/health/metrics'
    static_configs:
      - targets: ['api:8081']
```

### Grafana Dashboard

Example queries:

```promql
# Overall health status
health_status

# Component health
component_database_healthy
component_trading_engine_healthy
component_websocket_healthy

# Response times
component_database_response_time_ms
component_solana_rpc_response_time_ms

# Service uptime
uptime_seconds / 3600  # Hours
```

## Custom Component Health Checks

To add health checks for new components:

1. Implement the `HealthCheckable` trait:

```rust
#[async_trait]
impl HealthCheckable for MyComponent {
    async fn check_health(&self) -> Result<ComponentHealth> {
        let start = Instant::now();
        let mut metadata = HashMap::new();
        
        // Perform health checks
        let (status, message) = if self.is_healthy() {
            (HealthStatus::Healthy, "Component healthy".to_string())
        } else {
            (HealthStatus::Unhealthy, "Component unhealthy".to_string())
        };
        
        // Add metadata
        metadata.insert("metric".to_string(), serde_json::json!(42));
        
        Ok(ComponentHealth {
            name: "my_component".to_string(),
            status,
            message,
            last_check: Utc::now(),
            response_time_ms: start.elapsed().as_millis() as u64,
            metadata,
        })
    }
}
```

2. Register with health service:

```rust
health_service.register_component(
    "my_component".to_string(),
    Arc::new(my_component)
).await;
```

## Monitoring Best Practices

1. **Set Appropriate Thresholds**
   - Adjust failure/recovery thresholds based on component stability
   - Consider network latency for external services

2. **Use Detailed Checks Wisely**
   - Enable detailed checks in development/staging
   - Consider disabling in production for performance

3. **Monitor Health Metrics**
   - Set up alerts for status changes
   - Track response time trends
   - Monitor component-specific metrics

4. **Regular Health Reviews**
   - Review health history for patterns
   - Identify frequently failing components
   - Optimize health check queries

## Troubleshooting

### Common Issues

1. **Health checks timing out**
   - Increase timeout configuration
   - Optimize health check queries
   - Check network connectivity

2. **False positives**
   - Adjust failure threshold
   - Review health check logic
   - Check for transient issues

3. **Performance impact**
   - Increase check interval
   - Disable detailed checks
   - Use cached results

### Debug Mode

Enable debug logging for health checks:

```bash
RUST_LOG=betting_platform_api::health_check=debug cargo run
```

## Performance Considerations

1. **Caching**
   - Health results are cached between checks
   - Use `force_refresh=true` to bypass cache

2. **Concurrent Checks**
   - All component checks run in parallel
   - Individual timeouts prevent blocking

3. **Resource Usage**
   - Minimal CPU/memory overhead
   - Database queries optimized for speed
   - Network calls use connection pooling