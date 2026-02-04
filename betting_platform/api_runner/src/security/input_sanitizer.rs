//! Input sanitization middleware for production use

use axum::{
    body::Body,
    http::{header, Method, StatusCode, Request},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Sanitization rules configuration
#[derive(Clone)]
pub struct SanitizationConfig {
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    
    /// Maximum string field length
    pub max_string_length: usize,
    
    /// Maximum array size
    pub max_array_size: usize,
    
    /// Maximum JSON depth
    pub max_json_depth: usize,
    
    /// Enable SQL injection protection
    pub sql_injection_protection: bool,
    
    /// Enable XSS protection
    pub xss_protection: bool,
    
    /// Enable path traversal protection
    pub path_traversal_protection: bool,
    
    /// Allowed content types
    pub allowed_content_types: Vec<String>,
    
    /// Custom field validators
    pub field_validators: HashMap<String, FieldValidator>,
}

impl Default for SanitizationConfig {
    fn default() -> Self {
        let mut field_validators = HashMap::new();
        
        // Add specific field validators
        field_validators.insert("email".to_string(), FieldValidator::Email);
        field_validators.insert("wallet".to_string(), FieldValidator::SolanaAddress);
        field_validators.insert("amount".to_string(), FieldValidator::PositiveNumber);
        field_validators.insert("market_id".to_string(), FieldValidator::NumericId);
        
        Self {
            max_body_size: 1024 * 1024, // 1MB
            max_string_length: 10000,
            max_array_size: 1000,
            max_json_depth: 10,
            sql_injection_protection: true,
            xss_protection: true,
            path_traversal_protection: true,
            allowed_content_types: vec![
                "application/json".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            ],
            field_validators,
        }
    }
}

/// Field-specific validators
#[derive(Clone)]
pub enum FieldValidator {
    Email,
    SolanaAddress,
    PositiveNumber,
    NumericId,
    CustomRegex(String),
}

// Precompiled regex patterns for performance
static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
static SOLANA_ADDRESS_REGEX: OnceLock<Regex> = OnceLock::new();
static SQL_INJECTION_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
static XSS_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
static PATH_TRAVERSAL_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

fn get_email_regex() -> &'static Regex {
    EMAIL_REGEX.get_or_init(|| {
        Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
    })
}

fn get_solana_address_regex() -> &'static Regex {
    SOLANA_ADDRESS_REGEX.get_or_init(|| {
        Regex::new(r"^[1-9A-HJ-NP-Za-km-z]{32,44}$").unwrap()
    })
}

fn get_sql_injection_patterns() -> &'static Vec<Regex> {
    SQL_INJECTION_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"(?i)(union\s+select|select\s+\*|drop\s+table|insert\s+into|delete\s+from|update\s+set)").unwrap(),
            Regex::new(r"(?i)(;|\s+or\s+|--|\*/|/\*|xp_|sp_|exec|execute)").unwrap(),
            Regex::new(r"(?i)(script|javascript|vbscript|onload|onerror|onclick)").unwrap(),
        ]
    })
}

fn get_xss_patterns() -> &'static Vec<Regex> {
    XSS_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"<\s*script[^>]*>.*?<\s*/\s*script\s*>").unwrap(),
            Regex::new(r"<\s*iframe[^>]*>.*?<\s*/\s*iframe\s*>").unwrap(),
            Regex::new(r"javascript\s*:").unwrap(),
            Regex::new(r"on\w+\s*=").unwrap(),
        ]
    })
}

fn get_path_traversal_patterns() -> &'static Vec<Regex> {
    PATH_TRAVERSAL_PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"\.\.[\\/]").unwrap(),
            Regex::new(r"^/etc/").unwrap(),
            Regex::new(r"^/proc/").unwrap(),
            Regex::new(r"\x00").unwrap(), // Null byte injection
        ]
    })
}

/// Input sanitizer service
pub struct InputSanitizer {
    config: SanitizationConfig,
}

impl InputSanitizer {
    pub fn new(config: SanitizationConfig) -> Self {
        Self { config }
    }
    
    /// Sanitize JSON value recursively
    pub fn sanitize_json(&self, value: &mut Value, depth: usize) -> Result<(), SanitizationError> {
        if depth > self.config.max_json_depth {
            return Err(SanitizationError::MaxDepthExceeded);
        }
        
        match value {
            Value::String(s) => {
                *s = self.sanitize_string(s)?;
            }
            Value::Array(arr) => {
                if arr.len() > self.config.max_array_size {
                    return Err(SanitizationError::ArrayTooLarge);
                }
                for item in arr.iter_mut() {
                    self.sanitize_json(item, depth + 1)?;
                }
            }
            Value::Object(obj) => {
                for (key, val) in obj.iter_mut() {
                    // Validate specific fields
                    if let Some(validator) = self.config.field_validators.get(key) {
                        self.validate_field(key, val, validator)?;
                    }
                    self.sanitize_json(val, depth + 1)?;
                }
            }
            _ => {} // Numbers, bools, null are safe
        }
        
        Ok(())
    }
    
    /// Sanitize string input
    fn sanitize_string(&self, input: &str) -> Result<String, SanitizationError> {
        // Check length
        if input.len() > self.config.max_string_length {
            return Err(SanitizationError::StringTooLong);
        }
        
        let mut sanitized = input.to_string();
        
        // SQL injection protection
        if self.config.sql_injection_protection {
            for pattern in get_sql_injection_patterns().iter() {
                if pattern.is_match(&sanitized) {
                    return Err(SanitizationError::SqlInjectionDetected);
                }
            }
        }
        
        // XSS protection
        if self.config.xss_protection {
            for pattern in get_xss_patterns().iter() {
                if pattern.is_match(&sanitized) {
                    return Err(SanitizationError::XssDetected);
                }
            }
            
            // HTML encode dangerous characters
            sanitized = sanitized
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#x27;");
        }
        
        // Path traversal protection
        if self.config.path_traversal_protection {
            for pattern in get_path_traversal_patterns().iter() {
                if pattern.is_match(&sanitized) {
                    return Err(SanitizationError::PathTraversalDetected);
                }
            }
        }
        
        Ok(sanitized)
    }
    
    /// Validate specific fields
    fn validate_field(
        &self,
        field_name: &str,
        value: &Value,
        validator: &FieldValidator,
    ) -> Result<(), SanitizationError> {
        match validator {
            FieldValidator::Email => {
                if let Value::String(email) = value {
                    if !get_email_regex().is_match(email) {
                        return Err(SanitizationError::InvalidEmail(field_name.to_string()));
                    }
                }
            }
            FieldValidator::SolanaAddress => {
                if let Value::String(address) = value {
                    if !get_solana_address_regex().is_match(address) {
                        return Err(SanitizationError::InvalidSolanaAddress(field_name.to_string()));
                    }
                }
            }
            FieldValidator::PositiveNumber => {
                match value {
                    Value::Number(n) => {
                        if n.as_f64().unwrap_or(0.0) <= 0.0 {
                            return Err(SanitizationError::InvalidNumber(field_name.to_string()));
                        }
                    }
                    _ => return Err(SanitizationError::InvalidNumber(field_name.to_string())),
                }
            }
            FieldValidator::NumericId => {
                match value {
                    Value::Number(n) => {
                        if !n.is_u64() && !n.is_i64() {
                            return Err(SanitizationError::InvalidId(field_name.to_string()));
                        }
                    }
                    _ => return Err(SanitizationError::InvalidId(field_name.to_string())),
                }
            }
            FieldValidator::CustomRegex(pattern) => {
                if let Value::String(s) = value {
                    let regex = Regex::new(pattern).map_err(|_| {
                        SanitizationError::InvalidRegex(pattern.clone())
                    })?;
                    if !regex.is_match(s) {
                        return Err(SanitizationError::ValidationFailed(field_name.to_string()));
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Sanitize query parameters
    pub fn sanitize_query_params(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, SanitizationError> {
        let mut sanitized = HashMap::new();
        
        for (key, value) in params {
            let clean_key = self.sanitize_string(key)?;
            let clean_value = self.sanitize_string(value)?;
            sanitized.insert(clean_key, clean_value);
        }
        
        Ok(sanitized)
    }
    
    /// Sanitize path parameters
    pub fn sanitize_path(&self, path: &str) -> Result<String, SanitizationError> {
        // Basic path sanitization
        if path.contains("..") || path.contains('\0') {
            return Err(SanitizationError::PathTraversalDetected);
        }
        
        Ok(path.to_string())
    }
}

/// Sanitization errors
#[derive(Debug)]
pub enum SanitizationError {
    MaxDepthExceeded,
    ArrayTooLarge,
    StringTooLong,
    SqlInjectionDetected,
    XssDetected,
    PathTraversalDetected,
    InvalidEmail(String),
    InvalidSolanaAddress(String),
    InvalidNumber(String),
    InvalidId(String),
    InvalidRegex(String),
    ValidationFailed(String),
    BodyTooLarge,
    InvalidContentType,
}

impl std::fmt::Display for SanitizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SanitizationError::MaxDepthExceeded => write!(f, "JSON depth exceeds maximum allowed"),
            SanitizationError::ArrayTooLarge => write!(f, "Array size exceeds maximum allowed"),
            SanitizationError::StringTooLong => write!(f, "String length exceeds maximum allowed"),
            SanitizationError::SqlInjectionDetected => write!(f, "Potential SQL injection detected"),
            SanitizationError::XssDetected => write!(f, "Potential XSS attack detected"),
            SanitizationError::PathTraversalDetected => write!(f, "Path traversal attempt detected"),
            SanitizationError::InvalidEmail(field) => write!(f, "Invalid email in field: {}", field),
            SanitizationError::InvalidSolanaAddress(field) => write!(f, "Invalid Solana address in field: {}", field),
            SanitizationError::InvalidNumber(field) => write!(f, "Invalid number in field: {}", field),
            SanitizationError::InvalidId(field) => write!(f, "Invalid ID in field: {}", field),
            SanitizationError::InvalidRegex(pattern) => write!(f, "Invalid regex pattern: {}", pattern),
            SanitizationError::ValidationFailed(field) => write!(f, "Validation failed for field: {}", field),
            SanitizationError::BodyTooLarge => write!(f, "Request body too large"),
            SanitizationError::InvalidContentType => write!(f, "Invalid content type"),
        }
    }
}

/// Input sanitization middleware
pub async fn sanitize_input_middleware(
    request: Request<Body>,
    next: Next<Body>,
) -> Response {
    let sanitizer = InputSanitizer::new(SanitizationConfig::default());
    
    // Check content type
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");
    
    let is_json = content_type.contains("application/json");
    let method = request.method().clone();
    
    // Sanitize path
    let path = request.uri().path();
    if let Err(e) = sanitizer.sanitize_path(path) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid path",
                "details": e.to_string()
            }))
        ).into_response();
    }
    
    // Only sanitize requests with bodies
    if matches!(method, Method::POST | Method::PUT | Method::PATCH) && is_json {
        // Extract body
        let (parts, body) = request.into_parts();
        
        // Convert to bytes
        let body_bytes = match hyper::body::to_bytes(body).await {
            Ok(bytes) => {
                if bytes.len() > sanitizer.config.max_body_size {
                    return (
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(serde_json::json!({
                            "error": "Request body too large"
                        }))
                    ).into_response();
                }
                bytes
            }
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Failed to read request body"
                    }))
                ).into_response();
            }
        };
        
        // Parse JSON
        match serde_json::from_slice::<Value>(&body_bytes) {
            Ok(mut json) => {
                // Sanitize JSON
                match sanitizer.sanitize_json(&mut json, 0) {
                    Ok(()) => {
                        // Reconstruct request with sanitized body
                        let sanitized_bytes = serde_json::to_vec(&json).unwrap();
                        let body = Body::from(sanitized_bytes);
                        let request = Request::from_parts(parts, body);
                        next.run(request).await
                    }
                    Err(e) => {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({
                                "error": "Input validation failed",
                                "details": e.to_string()
                            }))
                        ).into_response()
                    }
                }
            }
            Err(_) => {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Invalid JSON"
                    }))
                ).into_response()
            }
        }
    } else {
        next.run(request).await
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_sql_injection_detection() {
        let sanitizer = InputSanitizer::new(SanitizationConfig::default());
        
        let malicious_inputs = vec![
            "'; DROP TABLE users; --",
            "1' OR '1'='1",
            "admin' --",
            "1; DELETE FROM accounts",
        ];
        
        for input in malicious_inputs {
            assert!(sanitizer.sanitize_string(input).is_err());
        }
    }
    
    #[test]
    fn test_xss_detection() {
        let sanitizer = InputSanitizer::new(SanitizationConfig::default());
        
        let xss_inputs = vec![
            "<script>alert('XSS')</script>",
            "<iframe src='evil.com'></iframe>",
            "javascript:alert(1)",
            "<img src=x onerror=alert(1)>",
        ];
        
        for input in xss_inputs {
            let result = sanitizer.sanitize_string(input);
            assert!(result.is_err() || !result.unwrap().contains('<'));
        }
    }
    
    #[test]
    fn test_email_validation() {
        let sanitizer = InputSanitizer::new(SanitizationConfig::default());
        
        let mut valid_email = json!("user@example.com");
        let mut invalid_email = json!("not-an-email");
        
        assert!(sanitizer
            .validate_field("email", &valid_email, &FieldValidator::Email)
            .is_ok());
        
        assert!(sanitizer
            .validate_field("email", &invalid_email, &FieldValidator::Email)
            .is_err());
    }
    
    #[test]
    fn test_solana_address_validation() {
        let sanitizer = InputSanitizer::new(SanitizationConfig::default());
        
        let valid_address = json!("7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2");
        let invalid_address = json!("invalid-address");
        
        assert!(sanitizer
            .validate_field("wallet", &valid_address, &FieldValidator::SolanaAddress)
            .is_ok());
        
        assert!(sanitizer
            .validate_field("wallet", &invalid_address, &FieldValidator::SolanaAddress)
            .is_err());
    }
    
    #[test]
    fn test_json_sanitization() {
        let mut sanitizer = InputSanitizer::new(SanitizationConfig::default());
        
        let mut json_data = json!({
            "email": "test@example.com",
            "wallet": "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2",
            "amount": 100.5,
            "comment": "Normal comment"
        });
        
        assert!(sanitizer.sanitize_json(&mut json_data, 0).is_ok());
        
        let mut malicious_json = json!({
            "email": "test@example.com",
            "comment": "'; DROP TABLE users; --"
        });
        
        assert!(sanitizer.sanitize_json(&mut malicious_json, 0).is_err());
    }
}