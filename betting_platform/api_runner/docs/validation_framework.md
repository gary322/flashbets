# Data Validation Framework Documentation

## Overview

The Data Validation Framework provides a comprehensive, extensible validation system for all data types used throughout the betting platform. It supports both generic validation rules and domain-specific validators, with caching and middleware integration for automatic request validation.

## Architecture

### Core Components

1. **ValidationService**: Central validation service that manages schemas and validators
2. **ValidationSchema**: Defines validation rules for specific data types
3. **CustomValidator**: Trait for implementing domain-specific validators
4. **ValidationMiddleware**: Automatic request validation based on endpoints
5. **DomainValidators**: Pre-built validators for betting platform domain objects

### Key Features

- **Flexible Rule System**: Support for common validation rules (required, min/max length, patterns, etc.)
- **Custom Validators**: Extensible trait-based system for complex validations
- **Caching**: Optional caching of validation results for performance
- **Middleware Integration**: Automatic validation of incoming requests
- **Domain-Specific**: Pre-built validators for positions, orders, settlements, etc.
- **Security Validation**: Built-in SQL injection and XSS detection

## Usage

### 1. Initialize Validation Service

```rust
// In main.rs
let validation_service = validation_middleware::initialize_validation_service(
    true,  // Enable caching
    std::time::Duration::from_secs(300),  // Cache TTL
).await;
```

### 2. Define Validation Schemas

```rust
use crate::validation_framework::{ValidationSchema, ValidationRule, ValidationRuleType, ValidationSeverity};

let market_schema = ValidationSchema {
    name: "market_creation".to_string(),
    rules: vec![
        ValidationRule {
            field: "title".to_string(),
            rule_type: ValidationRuleType::Required,
            message: None,
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            field: "title".to_string(),
            rule_type: ValidationRuleType::MinLength(10),
            message: Some("Market title must be at least 10 characters".into()),
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            field: "category".to_string(),
            rule_type: ValidationRuleType::OneOf(vec![
                "sports".to_string(),
                "politics".to_string(),
                "crypto".to_string(),
            ]),
            message: None,
            severity: ValidationSeverity::Error,
        },
    ],
    custom_validators: vec![
        Arc::new(validators::MarketDataValidator),
        Arc::new(validators::SecurityValidator),
    ],
};

validation_service.register_schema(market_schema).await;
```

### 3. Create Custom Validators

```rust
use async_trait::async_trait;
use validator::{ValidationError, ValidationErrors};

pub struct PositionValidator {
    pub max_position_size: u64,
    pub max_leverage: u32,
    pub min_collateral: u64,
}

#[async_trait]
impl CustomValidator for PositionValidator {
    async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
        let mut errors = ValidationErrors::new();
        
        // Validate position size
        if let Some(size) = data.get("size").and_then(|v| v.as_u64()) {
            if size > self.max_position_size {
                let mut error = ValidationError::new("position_too_large");
                error.message = Some(format!("Position size {} exceeds maximum {}", size, self.max_position_size).into());
                errors.add("size", error);
            }
        }
        
        // Validate leverage
        if let Some(leverage) = data.get("leverage").and_then(|v| v.as_u64()) {
            if leverage as u32 > self.max_leverage {
                let mut error = ValidationError::new("leverage_too_high");
                error.message = Some(format!("Leverage {} exceeds maximum {}", leverage, self.max_leverage).into());
                errors.add("leverage", error);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    fn name(&self) -> &str {
        "position_validator"
    }
    
    fn description(&self) -> &str {
        "Validates trading position parameters"
    }
}
```

### 4. Configure Endpoint Validation

```rust
// Configure validation for specific endpoints
let config = ValidationMiddlewareConfig {
    enabled: true,
    log_violations: true,
    fail_on_warning: false,
    endpoints: vec![
        EndpointValidation {
            method: "POST".to_string(),
            path_pattern: "/api/markets/create".to_string(),
            schema_name: "market_creation".to_string(),
            extract_params: false,
            validate_query: false,
        },
        EndpointValidation {
            method: "POST".to_string(),
            path_pattern: "/api/trade/place".to_string(),
            schema_name: "trade_execution".to_string(),
            extract_params: false,
            validate_query: false,
        },
    ],
};
```

### 5. Manual Validation

```rust
// Validate data manually
let context = ValidationContext {
    user_id: Some("user123".to_string()),
    request_id: Some("req456".to_string()),
    source: "api".to_string(),
    metadata: HashMap::new(),
};

let report = validation_service
    .validate_with_schema("market_creation", &market_data, &context)
    .await?;

if !report.is_valid {
    // Handle validation errors
    for error in &report.errors {
        println!("Error in {}: {}", error.field, error.message);
    }
}
```

## Validation Rule Types

### Built-in Rules

- **Required**: Field must be present and not null
- **MinLength(usize)**: String/array minimum length
- **MaxLength(usize)**: String/array maximum length
- **Pattern(String)**: Regex pattern matching
- **Range { min, max }**: Numeric range validation
- **Email**: Valid email format
- **Url**: Valid URL format
- **OneOf(Vec<String>)**: Value must be one of allowed values
- **Unique**: Value must be unique (requires DB access)
- **Reference**: Foreign key validation
- **Custom(String)**: Use a named custom validator

### Severity Levels

- **Error**: Validation failure, request rejected
- **Warning**: Issue logged but request allowed
- **Info**: Informational only

## Domain Validators

### PositionValidator
- Max position size
- Max leverage
- Min collateral
- Risk parameter validation

### OrderValidator
- Order types (market, limit, stop, stop_limit)
- Order sides (buy, sell)
- Size limits
- Price validation
- Time in force validation

### SettlementValidator
- Settlement time windows
- Oracle validation
- Outcome validation
- Proof validation

### LiquidityValidator
- Liquidity amounts
- Lock periods
- Slippage tolerance
- Pool ratio validation

### QuantumPositionValidator
- Entanglement degree
- Coherence levels
- Superposition states
- Probability amplitudes

### TransactionValidator
- Account limits
- Data size limits
- Compute unit limits
- Program authorization

## API Endpoints

### Validation Management

- `POST /api/validation/schemas` - Register new validation schema
- `GET /api/validation/schemas/:name` - Get schema details
- `POST /api/validation/validate/:schema` - Validate data against schema
- `PUT /api/validation/config` - Update middleware configuration
- `GET /api/validation/stats` - Get validation statistics
- `POST /api/validation/cache/clear` - Clear validation cache
- `POST /api/validation/endpoints` - Configure endpoint validation

## Configuration

### Environment Variables

- `VALIDATION_CACHE_ENABLED` - Enable/disable validation caching (default: true)
- `VALIDATION_CACHE_TTL_SECS` - Cache TTL in seconds (default: 300)

### Middleware Configuration

```rust
ValidationMiddlewareConfig {
    enabled: bool,              // Enable/disable validation
    log_violations: bool,       // Log validation failures
    fail_on_warning: bool,      // Reject requests with warnings
    endpoints: Vec<EndpointValidation>, // Endpoint configurations
}
```

## Security Features

### SQL Injection Detection
- Detects common SQL injection patterns
- Logs potential attempts with user context

### XSS Prevention
- Detects script injection attempts
- Validates against common XSS patterns

### Input Sanitization
- Automatic trimming and normalization
- Safe character validation

## Performance Considerations

### Caching
- Validation results cached by schema and data hash
- Configurable TTL
- Automatic cache invalidation

### Optimization
- Lazy compilation of regex patterns
- Efficient field traversal
- Minimal allocations

## Error Handling

### Validation Reports
```rust
ValidationReport {
    is_valid: bool,
    errors: Vec<ValidationViolation>,
    warnings: Vec<ValidationViolation>,
    info: Vec<ValidationViolation>,
    schema_name: String,
    validated_at: Timestamp,
}
```

### Error Response Format
```json
{
    "error": "Validation failed: title: Market title must be at least 10 characters; category: category must be one of: sports, politics, crypto",
    "status": 400
}
```

## Best Practices

1. **Schema Design**
   - Keep schemas focused and cohesive
   - Use clear, descriptive field names
   - Provide helpful error messages
   - Use appropriate severity levels

2. **Custom Validators**
   - Implement async validators for I/O operations
   - Return specific error codes
   - Include context in error messages
   - Test validators thoroughly

3. **Performance**
   - Enable caching for frequently validated data
   - Use specific schemas rather than generic ones
   - Avoid expensive validations in hot paths
   - Monitor validation performance metrics

4. **Security**
   - Always validate user input
   - Use the security validator for public endpoints
   - Log validation failures for security monitoring
   - Implement rate limiting for validation endpoints

## Integration Examples

### Market Creation
```rust
// Endpoint automatically validates against "market_creation" schema
POST /api/markets/create
{
    "title": "Will BTC reach $100k by end of 2024?",
    "description": "Resolution based on CoinGecko price",
    "category": "crypto",
    "outcomes": ["Yes", "No"],
    "resolution_date": 1735689600
}
```

### Trade Execution
```rust
// Endpoint automatically validates against "trade_execution" schema
POST /api/trade/place
{
    "market_id": "btc-100k-2024",
    "side": "buy",
    "outcome": 0,
    "amount": 100.0,
    "price": 0.65,
    "wallet_address": "11111111111111111111111111111111"
}
```

## Troubleshooting

### Common Issues

1. **Schema Not Found**
   - Ensure schema is registered before use
   - Check schema name spelling
   - Verify initialization order

2. **Validation Always Passes**
   - Check if validation is enabled
   - Verify endpoint configuration
   - Ensure middleware is in the stack

3. **Performance Issues**
   - Enable caching
   - Reduce validation complexity
   - Use async validators for I/O

4. **False Positives**
   - Review validation rules
   - Check regex patterns
   - Adjust security thresholds

## Future Enhancements

1. **Dynamic Schema Loading**: Load schemas from configuration files
2. **Schema Versioning**: Support multiple schema versions
3. **Validation Metrics**: Detailed performance metrics
4. **Schema Builder UI**: Web interface for schema creation
5. **Cross-Field Validation**: Support for field dependencies
6. **Async Rule Evaluation**: Parallel rule processing
7. **Custom Error Codes**: Standardized error code system
8. **Validation Pipelines**: Chain multiple validators