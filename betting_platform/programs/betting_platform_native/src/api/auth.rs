//! Authentication and Authorization Module
//!
//! Production-grade auth for REST API

#[cfg(all(not(target_arch = "bpf"), not(target_os = "solana")))]
pub mod api_auth {
    use solana_program::pubkey::Pubkey;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose};

/// API key structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Key ID
    pub key_id: String,
    /// Secret key hash
    pub secret_hash: String,
    /// Associated user
    pub user: Pubkey,
    /// Permissions
    pub permissions: ApiPermissions,
    /// Created timestamp
    pub created_at: u64,
    /// Expiry timestamp (0 = never)
    pub expires_at: u64,
    /// Is active
    pub is_active: bool,
    /// Rate limit override
    pub rate_limit_override: Option<u32>,
}

/// API permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPermissions {
    /// Can read market data
    pub read_markets: bool,
    /// Can place orders
    pub place_orders: bool,
    /// Can cancel orders
    pub cancel_orders: bool,
    /// Can read portfolio
    pub read_portfolio: bool,
    /// Can modify portfolio
    pub modify_portfolio: bool,
    /// Can access private data
    pub access_private: bool,
    /// Can access admin endpoints
    pub admin_access: bool,
}

impl Default for ApiPermissions {
    fn default() -> Self {
        Self {
            read_markets: true,
            place_orders: true,
            cancel_orders: true,
            read_portfolio: true,
            modify_portfolio: true,
            access_private: false,
            admin_access: false,
        }
    }
}

/// JWT claims
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user pubkey)
    pub sub: String,
    /// Issued at
    pub iat: u64,
    /// Expiration
    pub exp: u64,
    /// Permissions
    pub permissions: ApiPermissions,
    /// Session ID
    pub session_id: String,
}

/// Authentication manager
pub struct AuthManager {
    /// API keys storage
    api_keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    /// JWT secret
    jwt_secret: Vec<u8>,
    /// Session timeout
    session_timeout: Duration,
}

impl AuthManager {
    /// Create new auth manager
    pub fn new(jwt_secret: Vec<u8>) -> Self {
        Self {
            api_keys: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            jwt_secret,
            session_timeout: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Generate new API key
    pub fn generate_api_key(
        &self,
        user: Pubkey,
        permissions: ApiPermissions,
        expires_in: Option<Duration>,
    ) -> Result<(String, String), AuthError> {
        let key_id = generate_random_string(16);
        let secret = generate_random_string(32);
        let secret_hash = hash_secret(&secret);
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let expires_at = expires_in
            .map(|d| now + d.as_secs())
            .unwrap_or(0);
        
        let api_key = ApiKey {
            key_id: key_id.clone(),
            secret_hash,
            user,
            permissions,
            created_at: now,
            expires_at,
            is_active: true,
            rate_limit_override: None,
        };
        
        self.api_keys.write().unwrap()
            .insert(key_id.clone(), api_key);
        
        Ok((key_id, secret))
    }

    /// Authenticate with API key
    pub fn authenticate_api_key(
        &self,
        key_id: &str,
        secret: &str,
    ) -> Result<AuthResult, AuthError> {
        let keys = self.api_keys.read().unwrap();
        
        let api_key = keys.get(key_id)
            .ok_or(AuthError::InvalidCredentials)?;
        
        // Verify secret
        if !verify_secret(secret, &api_key.secret_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        
        // Check if active
        if !api_key.is_active {
            return Err(AuthError::KeyInactive);
        }
        
        // Check expiry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if api_key.expires_at > 0 && api_key.expires_at < now {
            return Err(AuthError::KeyExpired);
        }
        
        Ok(AuthResult {
            user: api_key.user,
            permissions: api_key.permissions.clone(),
            rate_limit_override: api_key.rate_limit_override,
        })
    }

    /// Create JWT token
    pub fn create_jwt_token(
        &self,
        user: Pubkey,
        permissions: ApiPermissions,
    ) -> Result<String, AuthError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let claims = JwtClaims {
            sub: user.to_string(),
            iat: now,
            exp: now + self.session_timeout.as_secs(),
            permissions,
            session_id: generate_random_string(16),
        };
        
        // Simple JWT implementation (in production, use proper JWT library)
        let header = r#"{"alg":"HS256","typ":"JWT"}"#;
        let payload = serde_json::to_string(&claims)
            .map_err(|_| AuthError::InternalError)?;
        
        let message = format!(
            "{}.{}",
            general_purpose::URL_SAFE_NO_PAD.encode(header),
            general_purpose::URL_SAFE_NO_PAD.encode(&payload)
        );
        
        let signature = sign_hmac(&message, &self.jwt_secret);
        let token = format!("{}.{}", message, signature);
        
        // Store session
        let session = Session {
            user,
            permissions: claims.permissions.clone(),
            created_at: now,
            last_activity: now,
            session_id: claims.session_id.clone(),
        };
        
        self.sessions.write().unwrap()
            .insert(claims.session_id, session);
        
        Ok(token)
    }

    /// Verify JWT token
    pub fn verify_jwt_token(&self, token: &str) -> Result<AuthResult, AuthError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(AuthError::InvalidToken);
        }
        
        // Verify signature
        let message = format!("{}.{}", parts[0], parts[1]);
        let expected_signature = sign_hmac(&message, &self.jwt_secret);
        
        if parts[2] != expected_signature {
            return Err(AuthError::InvalidToken);
        }
        
        // Decode claims
        let payload = general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| AuthError::InvalidToken)?;
        
        let claims: JwtClaims = serde_json::from_slice(&payload)
            .map_err(|_| AuthError::InvalidToken)?;
        
        // Check expiry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if claims.exp < now {
            return Err(AuthError::TokenExpired);
        }
        
        // Check session
        let mut sessions = self.sessions.write().unwrap();
        let session = sessions.get_mut(&claims.session_id)
            .ok_or(AuthError::SessionNotFound)?;
        
        // Update last activity
        session.last_activity = now;
        
        let user = Pubkey::from_string(&claims.sub)
            .map_err(|_| AuthError::InvalidToken)?;
        
        Ok(AuthResult {
            user,
            permissions: claims.permissions,
            rate_limit_override: None,
        })
    }

    /// Revoke API key
    pub fn revoke_api_key(&self, key_id: &str) -> Result<(), AuthError> {
        let mut keys = self.api_keys.write().unwrap();
        
        match keys.get_mut(key_id) {
            Some(key) => {
                key.is_active = false;
                Ok(())
            }
            None => Err(AuthError::KeyNotFound),
        }
    }

    /// Clean expired sessions
    pub fn cleanup_sessions(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut sessions = self.sessions.write().unwrap();
        sessions.retain(|_, session| {
            now - session.last_activity < self.session_timeout.as_secs()
        });
    }

    /// List active sessions for user
    pub fn get_user_sessions(&self, user: &Pubkey) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions.values()
            .filter(|s| &s.user == user)
            .cloned()
            .collect()
    }
}

/// Session information
#[derive(Debug, Clone)]
pub struct Session {
    pub user: Pubkey,
    pub permissions: ApiPermissions,
    pub created_at: u64,
    pub last_activity: u64,
    pub session_id: String,
}

/// Authentication result
#[derive(Debug)]
pub struct AuthResult {
    pub user: Pubkey,
    pub permissions: ApiPermissions,
    pub rate_limit_override: Option<u32>,
}

/// Auth errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthError {
    InvalidCredentials,
    InvalidToken,
    TokenExpired,
    KeyExpired,
    KeyInactive,
    KeyNotFound,
    SessionNotFound,
    PermissionDenied,
    InternalError,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthError::InvalidToken => write!(f, "Invalid token"),
            AuthError::TokenExpired => write!(f, "Token expired"),
            AuthError::KeyExpired => write!(f, "API key expired"),
            AuthError::KeyInactive => write!(f, "API key inactive"),
            AuthError::KeyNotFound => write!(f, "API key not found"),
            AuthError::SessionNotFound => write!(f, "Session not found"),
            AuthError::PermissionDenied => write!(f, "Permission denied"),
            AuthError::InternalError => write!(f, "Internal error"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Hash API secret
fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    general_purpose::STANDARD.encode(hasher.finalize())
}

/// Verify secret against hash
fn verify_secret(secret: &str, hash: &str) -> bool {
    hash_secret(secret) == hash
}

/// Sign message with HMAC-SHA256
fn sign_hmac(message: &str, key: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<Sha256>;
    
    let mut mac = HmacSha256::new_from_slice(key)
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    
    general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Generate random string
fn generate_random_string(len: usize) -> String {
    use rand::Rng;
    use rand::distributions::Alphanumeric;
    
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

/// Auth middleware
pub struct AuthMiddleware {
    auth_manager: Arc<AuthManager>,
}

impl AuthMiddleware {
    pub fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self { auth_manager }
    }

    /// Verify request authentication
    pub fn verify_request(
        &self,
        auth_header: Option<&str>,
    ) -> Result<AuthResult, AuthError> {
        let auth_header = auth_header
            .ok_or(AuthError::InvalidCredentials)?;
        
        if auth_header.starts_with("Bearer ") {
            // JWT token
            let token = &auth_header[7..];
            self.auth_manager.verify_jwt_token(token)
        } else if auth_header.starts_with("ApiKey ") {
            // API key
            let parts: Vec<&str> = auth_header[7..].split(':').collect();
            if parts.len() != 2 {
                return Err(AuthError::InvalidCredentials);
            }
            self.auth_manager.authenticate_api_key(parts[0], parts[1])
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }

    /// Check permission
    pub fn check_permission(
        &self,
        auth: &AuthResult,
        required: impl Fn(&ApiPermissions) -> bool,
    ) -> Result<(), AuthError> {
        if required(&auth.permissions) {
            Ok(())
        } else {
            Err(AuthError::PermissionDenied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_generation() {
        let auth_manager = AuthManager::new(b"test_secret".to_vec());
        let user = Pubkey::new_unique();
        
        let (key_id, secret) = auth_manager
            .generate_api_key(user, ApiPermissions::default(), None)
            .unwrap();
        
        assert!(!key_id.is_empty());
        assert!(!secret.is_empty());
        
        // Should authenticate successfully
        let result = auth_manager
            .authenticate_api_key(&key_id, &secret)
            .unwrap();
        
        assert_eq!(result.user, user);
    }

    #[test]
    fn test_jwt_token() {
        let auth_manager = AuthManager::new(b"test_secret".to_vec());
        let user = Pubkey::new_unique();
        
        let token = auth_manager
            .create_jwt_token(user, ApiPermissions::default())
            .unwrap();
        
        assert!(!token.is_empty());
        
        // Should verify successfully
        let result = auth_manager.verify_jwt_token(&token).unwrap();
        assert_eq!(result.user, user);
    }

    #[test]
    fn test_invalid_credentials() {
        let auth_manager = AuthManager::new(b"test_secret".to_vec());
        
        // Invalid API key
        let result = auth_manager.authenticate_api_key("invalid", "wrong");
        assert!(matches!(result, Err(AuthError::InvalidCredentials)));
        
        // Invalid JWT
        let result = auth_manager.verify_jwt_token("invalid.token.here");
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }

    #[test]
    fn test_permissions() {
        let auth_manager = AuthManager::new(b"test_secret".to_vec());
        let user = Pubkey::new_unique();
        
        let mut permissions = ApiPermissions::default();
        permissions.admin_access = false;
        
        let (key_id, secret) = auth_manager
            .generate_api_key(user, permissions, None)
            .unwrap();
        
        let auth_result = auth_manager
            .authenticate_api_key(&key_id, &secret)
            .unwrap();
        
        let middleware = AuthMiddleware::new(Arc::new(auth_manager));
        
        // Should fail admin check
        let result = middleware.check_permission(&auth_result, |p| p.admin_access);
        assert!(matches!(result, Err(AuthError::PermissionDenied)));
        
        // Should pass read check
        let result = middleware.check_permission(&auth_result, |p| p.read_markets);
        assert!(result.is_ok());
    }
}}
