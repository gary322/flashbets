# Feature Flag System Documentation

## Overview

The feature flag system provides runtime control over features, allowing gradual rollouts, A/B testing, and emergency feature toggling without code deployment.

## Architecture

### Components

1. **FeatureFlagService** - Core service for flag evaluation and management
2. **FeatureFlagProvider** - Trait for different flag storage backends
3. **InMemoryProvider** - Default in-memory storage provider
4. **FeatureFlagMiddleware** - Axum middleware for protecting endpoints
5. **REST API Endpoints** - Management and evaluation endpoints

### Key Features

- **Percentage-based rollouts** - Gradually enable features for a percentage of users
- **Target rules** - Enable features for specific users, groups, or conditions
- **Expiration dates** - Automatically disable features after a certain time
- **Cache layer** - Fast flag evaluation with configurable TTL
- **Multiple providers** - Support for different flag sources (memory, database, external)

## Configuration

### Default Flags

The system comes with these default feature flags:

```rust
- new_trading_ui (50% rollout) - New trading interface
- quantum_trading (disabled, beta_testers only) - Quantum position features
- advanced_analytics (enabled) - Analytics dashboard
- maintenance_mode (disabled) - System maintenance toggle
```

### Flag Structure

```json
{
  "name": "feature_name",
  "description": "Feature description",
  "status": "enabled|disabled|percentage",
  "target_rules": [
    {
      "target_type": "user|group|ip_range|market|custom",
      "values": ["value1", "value2"],
      "enabled": true
    }
  ],
  "metadata": {},
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z",
  "expires_at": null
}
```

## API Endpoints

### Public Endpoints

#### Evaluate Feature Flags
```bash
POST /api/feature-flags/evaluate
Authorization: Bearer <token>

{
  "flags": ["new_trading_ui", "quantum_trading"],
  "context": {
    "group_ids": ["beta_testers"],
    "market_id": 12345
  }
}

Response:
{
  "flags": {
    "new_trading_ui": true,
    "quantum_trading": true
  },
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### Get All Flags
```bash
GET /api/feature-flags?active_only=true&search=trading
```

### Admin Endpoints

#### Create Feature Flag
```bash
POST /api/feature-flags
Authorization: Bearer <admin-token>

{
  "name": "new_feature",
  "description": "New experimental feature",
  "status": "percentage",
  "percentage": 10,
  "target_rules": []
}
```

#### Update Feature Flag
```bash
PUT /api/feature-flags/:name
Authorization: Bearer <admin-token>

{
  "status": "enabled",
  "target_rules": [
    {
      "target_type": "group",
      "values": ["premium_users"],
      "enabled": true
    }
  ]
}
```

#### Delete Feature Flag
```bash
DELETE /api/feature-flags/:name
Authorization: Bearer <admin-token>
```

#### Get Statistics
```bash
GET /api/feature-flags/stats
Authorization: Bearer <admin-token>

Response:
{
  "total_flags": 10,
  "enabled": 3,
  "disabled": 4,
  "percentage_rollout": 2,
  "with_targeting": 1,
  "expiring_soon": 0
}
```

## Usage Examples

### 1. Protecting Endpoints with Middleware

```rust
use crate::feature_flag_middleware::require_feature;

// Protect entire route group
let protected_routes = Router::new()
    .route("/new-ui", get(handler))
    .layer(require_feature("new_trading_ui"));

// Or protect individual routes
app.route("/quantum/positions", 
    get(handler).layer(require_feature("quantum_trading")))
```

### 2. Checking Flags in Handlers

```rust
use crate::feature_flags::{FeatureFlagService, EvaluationContext};

async fn handler(
    State(state): State<Arc<AppState>>,
    auth: AuthenticatedUser,
) -> Result<Json<Response>, AppError> {
    let feature_service = state.feature_flags.as_ref()
        .ok_or_else(|| /* error */)?;
    
    let context = EvaluationContext {
        user_id: Some(auth.claims.wallet),
        ..Default::default()
    };
    
    if feature_service.is_enabled("new_feature", &context).await? {
        // New feature code
    } else {
        // Legacy code
    }
}
```

### 3. Conditional Middleware

```rust
// Apply middleware only if feature is enabled
app.route("/api/trades", post(create_trade))
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        |state, req, next| {
            feature_flag_middleware::conditional_middleware(
                state, req, next, "enhanced_validation"
            )
        }
    ));
```

## Target Rules

### User Targeting
```json
{
  "target_type": "user",
  "values": ["wallet123", "wallet456"],
  "enabled": true
}
```

### Group Targeting
```json
{
  "target_type": "group",
  "values": ["beta_testers", "premium_users"],
  "enabled": true
}
```

### IP Range Targeting
```json
{
  "target_type": "ip_range",
  "values": ["192.168.1.0/24", "10.0.0.0/8"],
  "enabled": true
}
```

### Market Targeting
```json
{
  "target_type": "market",
  "values": ["12345", "67890"],
  "enabled": true
}
```

### Custom Attribute Targeting
```json
{
  "target_type": "custom",
  "custom_type": "region",
  "values": ["us-east", "eu-west"],
  "enabled": true
}
```

## Percentage Rollouts

The system uses consistent hashing based on user ID or IP address to ensure users always get the same flag state:

```rust
// 20% rollout
{
  "status": "percentage",
  "percentage": 20
}
```

## Best Practices

1. **Gradual Rollouts**: Start with small percentages and increase gradually
2. **Target Beta Users**: Use group targeting for beta testing
3. **Set Expiration**: Use expiration dates for temporary features
4. **Monitor Impact**: Track metrics before and after enabling features
5. **Emergency Kill Switch**: Keep maintenance_mode flag for emergencies
6. **Cache Wisely**: Balance cache TTL with flag update frequency

## Integration with Environment Config

Feature flags can be configured via environment configuration:

```toml
[feature_flags]
cache_ttl_minutes = 5
default_provider = "memory"

[feature_flags.defaults]
new_trading_ui = { status = "percentage", percentage = 50 }
quantum_trading = { status = "disabled" }
```

## Monitoring

### Metrics to Track
- Flag evaluation rate
- Cache hit rate
- Flag change frequency
- User coverage per flag
- Error rates

### Logging
All flag evaluations and changes are logged with correlation IDs:

```
INFO feature_flags{correlation_id=abc123}: Feature flag 'new_trading_ui' evaluated: true
WARN feature_flags{correlation_id=def456}: Feature flag 'unknown_flag' not found
```

## Error Handling

The system fails open by default - if flag evaluation fails, the request proceeds:

```rust
match feature_service.is_enabled(&flag_name, &context).await {
    Ok(true) => // Feature enabled
    Ok(false) => // Feature disabled
    Err(_) => // Fail open - allow request
}
```

## Testing

### Unit Tests
```bash
cargo test feature_flags
```

### Integration Tests
See `tests/test_feature_flags.sh` for comprehensive testing scripts.

## Security Considerations

1. **Admin-only Management**: Flag creation/update/delete requires admin role
2. **No PII in Flags**: Don't store sensitive data in flag metadata
3. **Audit Trail**: All flag changes are logged with user info
4. **Rate Limiting**: Evaluation endpoint is rate-limited
5. **Input Validation**: All flag names and values are validated

## Future Enhancements

1. **Database Provider**: Store flags in PostgreSQL
2. **Redis Provider**: Use Redis for distributed caching
3. **WebSocket Updates**: Real-time flag updates
4. **Flag Dependencies**: Support dependent flags
5. **Analytics Integration**: Track feature usage metrics
6. **A/B Testing**: Built-in experiment framework
7. **Flag Templates**: Reusable flag configurations
8. **Scheduled Changes**: Schedule flag changes in advance