//! Comprehensive Data Validation Framework
//! 
//! Provides a flexible, extensible validation system for all data types
//! used throughout the betting platform.

use std::{
    collections::HashMap,
    sync::Arc,
    fmt::Debug,
    str::FromStr,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError, ValidationErrors};
use regex::Regex;
use once_cell::sync::Lazy;
use tracing::{debug, warn};

use crate::{
    typed_errors::{AppError, ErrorKind, ErrorContext},
    platform::Timestamp,
};

/// Validation result type
pub type ValidationResult<T> = Result<T, ValidationErrors>;

/// Custom validation context
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub user_id: Option<String>,
    pub request_id: Option<String>,
    pub source: String,
    pub metadata: HashMap<String, String>,
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self {
            user_id: None,
            request_id: None,
            source: "unknown".to_string(),
            metadata: HashMap::new(),
        }
    }
}

/// Trait for custom validators
#[async_trait]
pub trait CustomValidator: Send + Sync {
    /// Validate the data
    async fn validate(&self, data: &serde_json::Value, context: &ValidationContext) -> ValidationResult<()>;
    
    /// Get validator name
    fn name(&self) -> &str;
    
    /// Get validator description
    fn description(&self) -> &str;
}

/// Validation rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub field: String,
    pub rule_type: ValidationRuleType,
    pub message: Option<String>,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationRuleType {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    Range { min: Option<f64>, max: Option<f64> },
    Email,
    Url,
    Custom(String),
    OneOf(Vec<String>),
    Unique,
    Reference { collection: String, field: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

/// Validation schema
#[derive(Clone)]
pub struct ValidationSchema {
    pub name: String,
    pub rules: Vec<ValidationRule>,
    pub custom_validators: Vec<Arc<dyn CustomValidator>>,
}

impl std::fmt::Debug for ValidationSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidationSchema")
            .field("name", &self.name)
            .field("rules", &self.rules)
            .field("custom_validators_count", &self.custom_validators.len())
            .finish()
    }
}

/// Validation service
pub struct ValidationService {
    schemas: Arc<tokio::sync::RwLock<HashMap<String, ValidationSchema>>>,
    pub validators: Arc<tokio::sync::RwLock<HashMap<String, Arc<dyn CustomValidator>>>>,
    cache_enabled: bool,
    validation_cache: Arc<tokio::sync::RwLock<HashMap<String, (bool, Timestamp)>>>,
    cache_ttl: std::time::Duration,
}

impl ValidationService {
    pub fn new(cache_enabled: bool, cache_ttl: std::time::Duration) -> Self {
        Self {
            schemas: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            validators: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            cache_enabled,
            validation_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }
    
    /// Register a validation schema
    pub async fn register_schema(&self, schema: ValidationSchema) {
        let mut schemas = self.schemas.write().await;
        schemas.insert(schema.name.clone(), schema);
    }
    
    /// Register a custom validator
    pub async fn register_validator(&self, name: String, validator: Arc<dyn CustomValidator>) {
        let mut validators = self.validators.write().await;
        validators.insert(name, validator);
    }
    
    /// Validate data against a schema
    pub async fn validate_with_schema(
        &self,
        schema_name: &str,
        data: &serde_json::Value,
        context: &ValidationContext,
    ) -> Result<ValidationReport, AppError> {
        let ctx = ErrorContext::new("validation_framework", "validate_with_schema");
        
        // Check cache if enabled
        if self.cache_enabled {
            let cache_key = format!("{}:{}", schema_name, serde_json::to_string(data).unwrap_or_default());
            let cache = self.validation_cache.read().await;
            
            if let Some((is_valid, timestamp)) = cache.get(&cache_key) {
                if timestamp.as_unix() > (Timestamp::now().as_unix() - self.cache_ttl.as_secs() as i64) {
                    debug!("Validation cache hit for schema: {}", schema_name);
                    return Ok(ValidationReport {
                        is_valid: *is_valid,
                        errors: Vec::new(),
                        warnings: Vec::new(),
                        info: Vec::new(),
                        schema_name: schema_name.to_string(),
                        validated_at: Timestamp::now(),
                    });
                }
            }
        }
        
        // Get schema
        let schemas = self.schemas.read().await;
        let schema = schemas.get(schema_name)
            .ok_or_else(|| AppError::new(
                ErrorKind::NotFound,
                format!("Validation schema not found: {}", schema_name),
                ctx.clone(),
            ))?;
        
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut info = Vec::new();
        
        // Apply rules
        for rule in &schema.rules {
            match self.apply_rule(rule, data).await {
                Ok(_) => {},
                Err(e) => {
                    let violation = ValidationViolation {
                        field: rule.field.clone(),
                        rule: format!("{:?}", rule.rule_type),
                        message: e.to_string(),
                        severity: rule.severity,
                    };
                    
                    match rule.severity {
                        ValidationSeverity::Error => errors.push(violation),
                        ValidationSeverity::Warning => warnings.push(violation),
                        ValidationSeverity::Info => info.push(violation),
                    }
                }
            }
        }
        
        // Apply custom validators
        for validator in &schema.custom_validators {
            if let Err(validation_errors) = validator.validate(data, context).await {
                for (field, field_errors) in validation_errors.field_errors() {
                    for error in field_errors {
                        errors.push(ValidationViolation {
                            field: field.to_string(),
                            rule: validator.name().to_string(),
                            message: error.message.as_ref().unwrap_or(&"Validation failed".into()).to_string(),
                            severity: ValidationSeverity::Error,
                        });
                    }
                }
            }
        }
        
        let is_valid = errors.is_empty();
        
        // Update cache
        if self.cache_enabled {
            let cache_key = format!("{}:{}", schema_name, serde_json::to_string(data).unwrap_or_default());
            let mut cache = self.validation_cache.write().await;
            cache.insert(cache_key, (is_valid, Timestamp::now()));
        }
        
        Ok(ValidationReport {
            is_valid,
            errors,
            warnings,
            info,
            schema_name: schema_name.to_string(),
            validated_at: Timestamp::now(),
        })
    }
    
    /// Apply a single validation rule
    async fn apply_rule(&self, rule: &ValidationRule, data: &serde_json::Value) -> Result<(), String> {
        let field_value = self.get_field_value(data, &rule.field);
        
        match &rule.rule_type {
            ValidationRuleType::Required => {
                if field_value.is_none() || field_value == Some(&serde_json::Value::Null) {
                    return Err(rule.message.clone().unwrap_or_else(|| format!("{} is required", rule.field)));
                }
            }
            
            ValidationRuleType::MinLength(min) => {
                if let Some(value) = field_value {
                    let length = match value {
                        serde_json::Value::String(s) => s.len(),
                        serde_json::Value::Array(a) => a.len(),
                        _ => return Ok(()),
                    };
                    
                    if length < *min {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} must be at least {} characters long", rule.field, min)
                        ));
                    }
                }
            }
            
            ValidationRuleType::MaxLength(max) => {
                if let Some(value) = field_value {
                    let length = match value {
                        serde_json::Value::String(s) => s.len(),
                        serde_json::Value::Array(a) => a.len(),
                        _ => return Ok(()),
                    };
                    
                    if length > *max {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} must be at most {} characters long", rule.field, max)
                        ));
                    }
                }
            }
            
            ValidationRuleType::Pattern(pattern) => {
                if let Some(serde_json::Value::String(s)) = field_value {
                    let regex = Regex::new(pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;
                    if !regex.is_match(s) {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} does not match required pattern", rule.field)
                        ));
                    }
                }
            }
            
            ValidationRuleType::Range { min, max } => {
                if let Some(value) = field_value {
                    let num = match value {
                        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
                        _ => return Ok(()),
                    };
                    
                    if let Some(min_val) = min {
                        if num < *min_val {
                            return Err(rule.message.clone().unwrap_or_else(|| 
                                format!("{} must be at least {}", rule.field, min_val)
                            ));
                        }
                    }
                    
                    if let Some(max_val) = max {
                        if num > *max_val {
                            return Err(rule.message.clone().unwrap_or_else(|| 
                                format!("{} must be at most {}", rule.field, max_val)
                            ));
                        }
                    }
                }
            }
            
            ValidationRuleType::Email => {
                if let Some(serde_json::Value::String(s)) = field_value {
                    static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
                        Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
                    });
                    
                    if !EMAIL_REGEX.is_match(s) {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} must be a valid email address", rule.field)
                        ));
                    }
                }
            }
            
            ValidationRuleType::Url => {
                if let Some(serde_json::Value::String(s)) = field_value {
                    if url::Url::parse(s).is_err() {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} must be a valid URL", rule.field)
                        ));
                    }
                }
            }
            
            ValidationRuleType::OneOf(allowed) => {
                if let Some(value) = field_value {
                    let value_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        _ => value.to_string(),
                    };
                    
                    if !allowed.contains(&value_str) {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} must be one of: {}", rule.field, allowed.join(", "))
                        ));
                    }
                }
            }
            
            ValidationRuleType::Custom(validator_name) => {
                let validators = self.validators.read().await;
                if let Some(validator) = validators.get(validator_name) {
                    let context = ValidationContext::default();
                    if let Err(_) = validator.validate(data, &context).await {
                        return Err(rule.message.clone().unwrap_or_else(|| 
                            format!("{} failed custom validation", rule.field)
                        ));
                    }
                }
            }
            
            ValidationRuleType::Unique => {
                // This would require database access
                warn!("Unique validation not implemented in this example");
            }
            
            ValidationRuleType::Reference { .. } => {
                // This would require database access
                warn!("Reference validation not implemented in this example");
            }
        }
        
        Ok(())
    }
    
    /// Get field value from JSON using dot notation
    fn get_field_value<'a>(&self, data: &'a serde_json::Value, field: &str) -> Option<&'a serde_json::Value> {
        let parts: Vec<&str> = field.split('.').collect();
        let mut current = data;
        
        for part in parts {
            current = current.get(part)?;
        }
        
        Some(current)
    }
}

/// Validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<ValidationViolation>,
    pub warnings: Vec<ValidationViolation>,
    pub info: Vec<ValidationViolation>,
    pub schema_name: String,
    pub validated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationViolation {
    pub field: String,
    pub rule: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

/// Common validators
pub mod validators {
    use super::*;
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;
    
    /// Solana address validator
    pub struct SolanaAddressValidator;
    
    #[async_trait]
    impl CustomValidator for SolanaAddressValidator {
        async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
            let mut errors = ValidationErrors::new();
            
            if let Some(address) = data.get("wallet_address").and_then(|v| v.as_str()) {
                if Pubkey::from_str(address).is_err() {
                    let mut error = ValidationError::new("invalid_solana_address");
                    error.message = Some("Invalid Solana wallet address".into());
                    errors.add("wallet_address", error);
                }
            }
            
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        }
        
        fn name(&self) -> &str {
            "solana_address"
        }
        
        fn description(&self) -> &str {
            "Validates Solana wallet addresses"
        }
    }
    
    /// Market data validator
    pub struct MarketDataValidator;
    
    #[async_trait]
    impl CustomValidator for MarketDataValidator {
        async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
            let mut errors = ValidationErrors::new();
            
            // Validate market ID format
            if let Some(market_id) = data.get("market_id").and_then(|v| v.as_str()) {
                if !market_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                    let mut error = ValidationError::new("invalid_market_id");
                    error.message = Some("Market ID must contain only alphanumeric characters, hyphens, and underscores".into());
                    errors.add("market_id", error);
                }
            }
            
            // Validate resolution date
            if let Some(resolution_date) = data.get("resolution_date").and_then(|v| v.as_i64()) {
                let now = Timestamp::now().as_unix();
                if resolution_date <= now {
                    let mut error = ValidationError::new("invalid_resolution_date");
                    error.message = Some("Resolution date must be in the future".into());
                    errors.add("resolution_date", error);
                }
            }
            
            // Validate outcomes
            if let Some(outcomes) = data.get("outcomes").and_then(|v| v.as_array()) {
                if outcomes.len() < 2 {
                    let mut error = ValidationError::new("insufficient_outcomes");
                    error.message = Some("Market must have at least 2 outcomes".into());
                    errors.add("outcomes", error);
                }
                
                if outcomes.len() > 10 {
                    let mut error = ValidationError::new("too_many_outcomes");
                    error.message = Some("Market cannot have more than 10 outcomes".into());
                    errors.add("outcomes", error);
                }
            }
            
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        }
        
        fn name(&self) -> &str {
            "market_data"
        }
        
        fn description(&self) -> &str {
            "Validates market creation data"
        }
    }
    
    /// Trade data validator
    pub struct TradeDataValidator;
    
    #[async_trait]
    impl CustomValidator for TradeDataValidator {
        async fn validate(&self, data: &serde_json::Value, _context: &ValidationContext) -> ValidationResult<()> {
            let mut errors = ValidationErrors::new();
            
            // Validate amount
            if let Some(amount) = data.get("amount").and_then(|v| v.as_f64()) {
                if amount <= 0.0 {
                    let mut error = ValidationError::new("invalid_amount");
                    error.message = Some("Trade amount must be positive".into());
                    errors.add("amount", error);
                }
                
                if amount > 1_000_000.0 {
                    let mut error = ValidationError::new("amount_too_large");
                    error.message = Some("Trade amount exceeds maximum limit".into());
                    errors.add("amount", error);
                }
            }
            
            // Validate price
            if let Some(price) = data.get("price").and_then(|v| v.as_f64()) {
                if price < 0.0 || price > 1.0 {
                    let mut error = ValidationError::new("invalid_price");
                    error.message = Some("Price must be between 0 and 1".into());
                    errors.add("price", error);
                }
            }
            
            // Validate slippage
            if let Some(slippage) = data.get("max_slippage").and_then(|v| v.as_f64()) {
                if slippage < 0.0 || slippage > 0.5 {
                    let mut error = ValidationError::new("invalid_slippage");
                    error.message = Some("Slippage must be between 0% and 50%".into());
                    errors.add("max_slippage", error);
                }
            }
            
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        }
        
        fn name(&self) -> &str {
            "trade_data"
        }
        
        fn description(&self) -> &str {
            "Validates trade execution data"
        }
    }
    
    /// Security validator
    pub struct SecurityValidator;
    
    #[async_trait]
    impl CustomValidator for SecurityValidator {
        async fn validate(&self, data: &serde_json::Value, context: &ValidationContext) -> ValidationResult<()> {
            let mut errors = ValidationErrors::new();
            
            // Check for SQL injection patterns
            let sql_patterns = vec![
                "'; DROP TABLE",
                "' OR '1'='1",
                "UNION SELECT",
                "'; --",
            ];
            
            fn check_value_for_patterns(value: &serde_json::Value, patterns: &[&str]) -> bool {
                match value {
                    serde_json::Value::String(s) => {
                        let s_upper = s.to_uppercase();
                        patterns.iter().any(|p| s_upper.contains(&p.to_uppercase()))
                    }
                    serde_json::Value::Object(map) => {
                        map.values().any(|v| check_value_for_patterns(v, patterns))
                    }
                    serde_json::Value::Array(arr) => {
                        arr.iter().any(|v| check_value_for_patterns(v, patterns))
                    }
                    _ => false,
                }
            }
            
            if check_value_for_patterns(data, &sql_patterns) {
                let mut error = ValidationError::new("potential_sql_injection");
                error.message = Some("Input contains potentially malicious patterns".into());
                errors.add("_security", error);
                
                warn!("Potential SQL injection attempt from user: {:?}", context.user_id);
            }
            
            // Check for script injection
            let script_patterns = vec![
                "<script",
                "javascript:",
                "onerror=",
                "onload=",
            ];
            
            if check_value_for_patterns(data, &script_patterns) {
                let mut error = ValidationError::new("potential_xss");
                error.message = Some("Input contains potentially malicious scripts".into());
                errors.add("_security", error);
                
                warn!("Potential XSS attempt from user: {:?}", context.user_id);
            }
            
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            }
        }
        
        fn name(&self) -> &str {
            "security"
        }
        
        fn description(&self) -> &str {
            "Validates input for security threats"
        }
    }
}

/// Initialize default validation schemas
pub async fn initialize_default_schemas(service: &ValidationService) {
    // Market creation schema
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
                field: "title".to_string(),
                rule_type: ValidationRuleType::MaxLength(200),
                message: Some("Market title cannot exceed 200 characters".into()),
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "description".to_string(),
                rule_type: ValidationRuleType::MaxLength(1000),
                message: None,
                severity: ValidationSeverity::Warning,
            },
            ValidationRule {
                field: "category".to_string(),
                rule_type: ValidationRuleType::OneOf(vec![
                    "sports".to_string(),
                    "politics".to_string(),
                    "crypto".to_string(),
                    "entertainment".to_string(),
                    "other".to_string(),
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
    
    service.register_schema(market_schema).await;
    
    // Trade execution schema
    let trade_schema = ValidationSchema {
        name: "trade_execution".to_string(),
        rules: vec![
            ValidationRule {
                field: "market_id".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "market_id".to_string(),
                rule_type: ValidationRuleType::Pattern(r"^[a-zA-Z0-9\-_]+$".to_string()),
                message: Some("Invalid market ID format".into()),
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "side".to_string(),
                rule_type: ValidationRuleType::OneOf(vec!["buy".to_string(), "sell".to_string()]),
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "amount".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
        ],
        custom_validators: vec![
            Arc::new(validators::TradeDataValidator),
            Arc::new(validators::SolanaAddressValidator),
            Arc::new(validators::SecurityValidator),
        ],
    };
    
    service.register_schema(trade_schema).await;
    
    // User registration schema
    let user_schema = ValidationSchema {
        name: "user_registration".to_string(),
        rules: vec![
            ValidationRule {
                field: "email".to_string(),
                rule_type: ValidationRuleType::Email,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "username".to_string(),
                rule_type: ValidationRuleType::Required,
                message: None,
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "username".to_string(),
                rule_type: ValidationRuleType::Pattern(r"^[a-zA-Z0-9_]{3,20}$".to_string()),
                message: Some("Username must be 3-20 characters, alphanumeric and underscore only".into()),
                severity: ValidationSeverity::Error,
            },
            ValidationRule {
                field: "terms_accepted".to_string(),
                rule_type: ValidationRuleType::Required,
                message: Some("You must accept the terms and conditions".into()),
                severity: ValidationSeverity::Error,
            },
        ],
        custom_validators: vec![
            Arc::new(validators::SecurityValidator),
        ],
    };
    
    service.register_schema(user_schema).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_validation_framework() {
        let service = ValidationService::new(false, std::time::Duration::from_secs(300));
        
        // Register test schema
        let schema = ValidationSchema {
            name: "test".to_string(),
            rules: vec![
                ValidationRule {
                    field: "name".to_string(),
                    rule_type: ValidationRuleType::Required,
                    message: None,
                    severity: ValidationSeverity::Error,
                },
                ValidationRule {
                    field: "age".to_string(),
                    rule_type: ValidationRuleType::Range { min: Some(18.0), max: Some(100.0) },
                    message: None,
                    severity: ValidationSeverity::Error,
                },
            ],
            custom_validators: vec![],
        };
        
        service.register_schema(schema).await;
        
        // Test valid data
        let valid_data = serde_json::json!({
            "name": "John Doe",
            "age": 25
        });
        
        let context = ValidationContext::default();
        let result = service.validate_with_schema("test", &valid_data, &context).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_valid);
        
        // Test invalid data
        let invalid_data = serde_json::json!({
            "age": 15
        });
        
        let result = service.validate_with_schema("test", &invalid_data, &context).await;
        assert!(result.is_ok());
        let report = result.unwrap();
        assert!(!report.is_valid);
        assert_eq!(report.errors.len(), 2); // Missing name and age out of range
    }
}