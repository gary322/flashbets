//! Secure JWT secret generation and management

use rand::{thread_rng, Rng};
use base64::{Engine as _, engine::general_purpose};
use std::fs;
use std::path::Path;

/// Generate a cryptographically secure JWT secret
pub fn generate_jwt_secret() -> String {
    let mut rng = thread_rng();
    let mut bytes = [0u8; 64]; // 512-bit secret
    rng.fill(&mut bytes);
    general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Load or generate JWT secret
pub fn get_or_create_jwt_secret() -> anyhow::Result<String> {
    // First check environment variable
    if let Ok(secret) = std::env::var("JWT_SECRET") {
        if secret.len() >= 32 && !secret.starts_with("your-") && !secret.contains("test") {
            return Ok(secret);
        }
    }
    
    // Check for secret file (for production)
    let secret_file = Path::new(".jwt_secret");
    if secret_file.exists() {
        let secret = fs::read_to_string(secret_file)?;
        let secret = secret.trim().to_string();
        if secret.len() >= 32 {
            return Ok(secret);
        }
    }
    
    // Generate new secret for production
    let new_secret = generate_jwt_secret();
    
    // In production, save to file with restricted permissions
    #[cfg(not(debug_assertions))]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(secret_file, &new_secret)?;
        fs::set_permissions(secret_file, fs::Permissions::from_mode(0o600))?;
    }
    
    Ok(new_secret)
}

/// Validate JWT secret strength
pub fn validate_jwt_secret(secret: &str) -> Result<(), String> {
    if secret.len() < 32 {
        return Err("JWT secret must be at least 32 characters long".to_string());
    }
    
    if secret.starts_with("your-") || secret.contains("test") || secret.contains("example") {
        return Err("JWT secret appears to be a placeholder value".to_string());
    }
    
    // Check for common weak patterns
    let weak_patterns = ["password", "secret", "123456", "admin", "default"];
    for pattern in weak_patterns {
        if secret.to_lowercase().contains(pattern) {
            return Err(format!("JWT secret contains weak pattern: {}", pattern));
        }
    }
    
    Ok(())
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_jwt_secret() {
        let secret1 = generate_jwt_secret();
        let secret2 = generate_jwt_secret();
        
        // Should be different
        assert_ne!(secret1, secret2);
        
        // Should be long enough
        assert!(secret1.len() >= 64);
        assert!(secret2.len() >= 64);
    }
    
    #[test]
    fn test_validate_jwt_secret() {
        // Valid secrets
        assert!(validate_jwt_secret(&generate_jwt_secret()).is_ok());
        assert!(validate_jwt_secret("a-very-secure-random-string-that-is-long-enough").is_ok());
        
        // Invalid secrets
        assert!(validate_jwt_secret("short").is_err());
        assert!(validate_jwt_secret("your-secret-key-must-be-at-least-32-characters-long").is_err());
        assert!(validate_jwt_secret("test-secret-key-that-is-long-enough-but-weak").is_err());
        assert!(validate_jwt_secret("password123456789012345678901234567890").is_err());
    }
}