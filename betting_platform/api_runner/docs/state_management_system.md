# State Management System Documentation

## Overview

The betting platform includes a comprehensive state management system that provides centralized state storage, synchronization, and persistence across the application. This system enables real-time state sharing between components, request tracking, session management, and distributed state synchronization.

## Architecture

### Core Components

1. **StateManager** (`state_manager.rs`)
   - Centralized state storage with atomic operations
   - Version tracking for optimistic locking
   - Event broadcasting for state changes
   - Automatic persistence with snapshots
   - In-memory storage with optional persistence backends

2. **State Management Endpoints** (`state_management_endpoints.rs`)
   - REST API for state operations
   - Admin-only write operations
   - WebSocket endpoint for real-time state events
   - Bulk operations support

3. **State Synchronization Middleware** (`state_sync_middleware.rs`)
   - Request tracking and correlation
   - Session state management
   - Automatic state propagation
   - Background cleanup tasks

## Features

### 1. Atomic State Operations

```rust
// Set state value
state_manager.set("key", value, "source").await?;

// Get state value
let value: Option<MyType> = state_manager.get("key").await?;

// Remove state value
state_manager.remove("key", "source").await?;

// Compare and swap
let success = state_manager.compare_and_swap(
    "key",
    Some(expected_value),
    Some(new_value),
    "source"
).await?;
```

### 2. State Metadata

Each state entry can have associated metadata:

```rust
// Set metadata
let metadata = HashMap::from([
    ("created_by".to_string(), "user123".to_string()),
    ("purpose".to_string(), "session_data".to_string()),
]);
state_manager.set_metadata("key", metadata).await;

// Get metadata
let metadata = state_manager.get_metadata("key").await;
```

### 3. Event Broadcasting

Subscribe to state changes:

```rust
let mut receiver = state_manager.subscribe();

while let Ok(event) = receiver.recv().await {
    println!("State changed: {} -> {:?}", event.key, event.new_value);
}
```

### 4. State Persistence

Automatic snapshots for recovery:

```rust
// Manual snapshot
let snapshot = state_manager.create_snapshot().await?;

// Restore from snapshot
state_manager.restore_snapshot(snapshot).await?;
```

## REST API Endpoints

### Get State Value
```
GET /api/v1/state/:key
```

Query parameters:
- `include_metadata` (optional): Include metadata in response

Response:
```json
{
  "success": true,
  "data": {
    "key": "session:user123",
    "value": { ... },
    "metadata": { ... },
    "version": 42
  }
}
```

### Set State Value
```
PUT /api/v1/state/:key
```

Request body:
```json
{
  "key": "session:user123",
  "value": { ... },
  "metadata": { ... }
}
```

### Remove State Value
```
DELETE /api/v1/state/:key
```

### List Keys by Prefix
```
GET /api/v1/state/keys?prefix=session:
```

Response:
```json
{
  "success": true,
  "data": {
    "keys": ["session:user123", "session:user456"],
    "total": 2
  }
}
```

### Compare and Swap
```
POST /api/v1/state/cas
```

Request body:
```json
{
  "key": "counter",
  "expected": 10,
  "new_value": 11
}
```

### Get Statistics
```
GET /api/v1/state/stats
```

Response:
```json
{
  "success": true,
  "data": {
    "total_keys": 150,
    "total_size": 45678,
    "version": 1234,
    "metadata_keys": 50
  }
}
```

### Create Snapshot
```
POST /api/v1/state/snapshot
```

### WebSocket Events
```
GET /api/v1/state/events
```

WebSocket messages:
```json
{
  "type": "state_change",
  "data": {
    "key": "market:BTC-USD",
    "old_value": { ... },
    "new_value": { ... },
    "timestamp": 1234567890,
    "source": "price_feed"
  }
}
```

## State Synchronization

### Request Tracking

The middleware automatically tracks all requests:

```
request:<correlation_id>
├── method: "POST"
├── uri: "/api/trade/place"
├── started_at: "2024-01-01T00:00:00Z"
└── headers: { ... }

request:<correlation_id>:completion
├── completed_at: "2024-01-01T00:00:01Z"
├── status: 200
└── duration_ms: 150
```

### Session Management

User sessions are automatically tracked:

```
session:<user_id>
├── last_request: "2024-01-01T00:00:00Z"
├── correlation_id: "abc-123"
├── active: true
└── last_heartbeat: "2024-01-01T00:00:30Z"
```

### State Propagation

WebSocket connections receive relevant state updates:

1. User-specific state (keys containing user ID)
2. Global state (keys with `global:` prefix)
3. Market updates (keys with `market:` prefix)

## Configuration

### Environment Variables

```bash
# State snapshot interval (seconds)
STATE_SNAPSHOT_INTERVAL_SECS=300

# Maximum number of snapshots to retain
STATE_MAX_SNAPSHOTS=100

# Enable state persistence
STATE_PERSISTENCE_ENABLED=true

# Broadcast state changes
STATE_BROADCAST_CHANGES=true
```

### State Manager Configuration

```rust
let config = StateManagerConfig {
    snapshot_interval: Duration::from_secs(300),
    max_snapshots: 100,
    enable_persistence: true,
    broadcast_changes: true,
};
```

### Middleware Configuration

```rust
let config = StateSyncConfig {
    track_requests: true,
    propagate_state: true,
    request_prefix: "request:".to_string(),
    session_prefix: "session:".to_string(),
    request_state_ttl: Duration::from_secs(300),
    session_state_ttl: Duration::from_secs(3600),
};
```

## Usage Examples

### 1. Storing User Preferences

```rust
// Store user preferences
let preferences = UserPreferences {
    theme: "dark".to_string(),
    notifications: true,
    language: "en".to_string(),
};

state_manager.set(
    &format!("user:{}:preferences", user_id),
    preferences,
    "user_service"
).await?;
```

### 2. Distributed Cache

```rust
// Cache market data
let market_data = fetch_market_data().await?;
state_manager.set(
    &format!("cache:market:{}", market_id),
    market_data,
    "market_service"
).await?;

// Set TTL via metadata
let metadata = HashMap::from([
    ("ttl".to_string(), "300".to_string()),
]);
state_manager.set_metadata(&cache_key, metadata).await;
```

### 3. Rate Limiting

```rust
// Track API usage
let key = format!("rate_limit:{}:{}", user_id, endpoint);
let current: u32 = state_manager.get(&key).await?.unwrap_or(0);

if current >= limit {
    return Err(AppError::rate_limited());
}

// Increment counter atomically
state_manager.compare_and_swap(
    &key,
    Some(current),
    Some(current + 1),
    "rate_limiter"
).await?;
```

### 4. Feature Flags

```rust
// Store feature flag state
let flag = FeatureFlag {
    enabled: true,
    rollout_percentage: 50,
    target_users: vec!["user123".to_string()],
};

state_manager.set(
    &format!("feature_flag:{}", flag_name),
    flag,
    "feature_service"
).await?;
```

## Performance Considerations

### Memory Usage

- Each state entry includes value + metadata + version info
- Large values should be stored externally with references in state
- Use prefixes to organize and query state efficiently

### Concurrency

- All operations are thread-safe
- Read operations use RwLock for concurrent access
- Write operations are serialized per key
- CAS operations provide optimistic concurrency control

### Persistence

- Snapshots are created asynchronously
- Only changed state is persisted (delta snapshots planned)
- Restore operations load entire snapshot into memory

## Monitoring

### Metrics

Track these metrics for state management health:

1. **State size**: Total keys and memory usage
2. **Operation latency**: Get/Set/CAS operation times
3. **Event queue depth**: Broadcast channel backlog
4. **Snapshot frequency**: Time between snapshots
5. **Cache hit rate**: For cached state values

### Health Checks

The state manager provides health status:

```rust
let stats = state_manager.get_stats().await;
if stats.total_keys > 100000 {
    warn!("High state key count: {}", stats.total_keys);
}
```

## Security Considerations

### Access Control

- Write operations require admin role
- Read operations available to authenticated users
- Sensitive keys should use encryption

### Data Validation

- All values must be JSON-serializable
- Size limits prevent memory exhaustion
- Key format validation prevents injection

### Audit Trail

State changes include:
- Source identifier
- Timestamp
- Old and new values
- User/service that made the change

## Troubleshooting

### Common Issues

1. **"State management service not available"**
   - Ensure StateManager is initialized in main.rs
   - Check if state_manager field is Some in AppState

2. **"Failed to deserialize state value"**
   - Ensure type matches stored value
   - Check for schema changes

3. **"Compare-and-swap failed"**
   - Value changed between read and write
   - Retry with exponential backoff

4. **Memory growth**
   - Check for unbounded key creation
   - Implement TTL for temporary state
   - Monitor snapshot sizes

### Debug Commands

```bash
# Get state statistics
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8081/api/v1/state/stats

# List all keys with prefix
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8081/api/v1/state/keys?prefix=session:"

# Watch state changes via WebSocket
wscat -c "ws://localhost:8081/api/v1/state/events" \
  -H "Authorization: Bearer $TOKEN"
```

## Future Enhancements

1. **Distributed State Sync**
   - Redis backend for multi-instance deployments
   - Conflict resolution strategies
   - Eventual consistency guarantees

2. **Advanced Persistence**
   - Delta snapshots for efficiency
   - Compression for large values
   - S3/GCS backend support

3. **State Migrations**
   - Schema versioning
   - Automatic migration on startup
   - Backward compatibility

4. **Analytics**
   - State access patterns
   - Hot key detection
   - Usage forecasting