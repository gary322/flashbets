//! Production-ready RBAC authorization framework

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;

use crate::jwt_validation::{AuthenticatedUser, JwtClaims};

/// User roles in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,        // Regular user
    Trader,      // Can place trades
    MarketMaker, // Can create markets and provide liquidity
    Admin,       // Full system access
    Support,     // Customer support access
    Auditor,     // Read-only access to all data
}

impl Role {
    /// Get permissions for this role
    pub fn permissions(&self) -> HashSet<Permission> {
        match self {
            Role::User => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ViewMarkets);
                perms.insert(Permission::ViewOwnPositions);
                perms.insert(Permission::ViewOwnBalance);
                perms
            }
            Role::Trader => {
                let mut perms = Role::User.permissions();
                perms.insert(Permission::PlaceTrades);
                perms.insert(Permission::CloseTrades);
                perms.insert(Permission::ViewTradeHistory);
                perms
            }
            Role::MarketMaker => {
                let mut perms = Role::Trader.permissions();
                perms.insert(Permission::CreateMarkets);
                perms.insert(Permission::ProvideLiquidity);
                perms.insert(Permission::RemoveLiquidity);
                perms.insert(Permission::SetMarketFees);
                perms
            }
            Role::Admin => {
                // Admin has all permissions
                Permission::all()
            }
            Role::Support => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ViewMarkets);
                perms.insert(Permission::ViewAllPositions);
                perms.insert(Permission::ViewAllBalances);
                perms.insert(Permission::ViewUserDetails);
                perms.insert(Permission::HandleDisputes);
                perms
            }
            Role::Auditor => {
                let mut perms = HashSet::new();
                perms.insert(Permission::ViewMarkets);
                perms.insert(Permission::ViewAllPositions);
                perms.insert(Permission::ViewAllBalances);
                perms.insert(Permission::ViewTradeHistory);
                perms.insert(Permission::ViewSystemMetrics);
                perms.insert(Permission::ExportData);
                perms
            }
        }
    }
    
    /// Check if role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions().contains(permission)
    }
    
    /// Parse role from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "user" => Some(Role::User),
            "trader" => Some(Role::Trader),
            "marketmaker" | "market_maker" => Some(Role::MarketMaker),
            "admin" => Some(Role::Admin),
            "support" => Some(Role::Support),
            "auditor" => Some(Role::Auditor),
            _ => None,
        }
    }
}

/// System permissions
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    // Market permissions
    ViewMarkets,
    CreateMarkets,
    UpdateMarkets,
    DeleteMarkets,
    
    // Trading permissions
    PlaceTrades,
    CloseTrades,
    ViewOwnPositions,
    ViewAllPositions,
    
    // Balance permissions
    ViewOwnBalance,
    ViewAllBalances,
    
    // Liquidity permissions
    ProvideLiquidity,
    RemoveLiquidity,
    SetMarketFees,
    
    // User management
    ViewUserDetails,
    UpdateUserRoles,
    BanUsers,
    
    // System permissions
    ViewSystemMetrics,
    UpdateSystemConfig,
    EmergencyShutdown,
    
    // Support permissions
    HandleDisputes,
    ViewTradeHistory,
    
    // Data permissions
    ExportData,
    ImportData,
}

impl Permission {
    /// Get all permissions
    pub fn all() -> HashSet<Permission> {
        let mut perms = HashSet::new();
        perms.insert(Permission::ViewMarkets);
        perms.insert(Permission::CreateMarkets);
        perms.insert(Permission::UpdateMarkets);
        perms.insert(Permission::DeleteMarkets);
        perms.insert(Permission::PlaceTrades);
        perms.insert(Permission::CloseTrades);
        perms.insert(Permission::ViewOwnPositions);
        perms.insert(Permission::ViewAllPositions);
        perms.insert(Permission::ViewOwnBalance);
        perms.insert(Permission::ViewAllBalances);
        perms.insert(Permission::ProvideLiquidity);
        perms.insert(Permission::RemoveLiquidity);
        perms.insert(Permission::SetMarketFees);
        perms.insert(Permission::ViewUserDetails);
        perms.insert(Permission::UpdateUserRoles);
        perms.insert(Permission::BanUsers);
        perms.insert(Permission::ViewSystemMetrics);
        perms.insert(Permission::UpdateSystemConfig);
        perms.insert(Permission::EmergencyShutdown);
        perms.insert(Permission::HandleDisputes);
        perms.insert(Permission::ViewTradeHistory);
        perms.insert(Permission::ExportData);
        perms.insert(Permission::ImportData);
        perms
    }
}

/// Authorization error types
#[derive(Debug)]
pub enum AuthorizationError {
    InsufficientPermissions(Permission),
    InvalidRole,
    Forbidden,
}

impl IntoResponse for AuthorizationError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthorizationError::InsufficientPermissions(perm) => {
                (StatusCode::FORBIDDEN, format!("Missing permission: {:?}", perm))
            }
            AuthorizationError::InvalidRole => {
                (StatusCode::BAD_REQUEST, "Invalid role specified".to_string())
            }
            AuthorizationError::Forbidden => {
                (StatusCode::FORBIDDEN, "Access forbidden".to_string())
            }
        };
        
        (status, Json(serde_json::json!({
            "error": message,
            "code": "AUTHORIZATION_ERROR"
        }))).into_response()
    }
}

/// Extractor that requires specific permission
pub struct RequirePermission {
    pub user: AuthenticatedUser,
    pub permission: Permission,
}

/// Extractor that requires specific role
pub struct RequireRole {
    pub user: AuthenticatedUser,
    pub role: Role,
}

/// Extractor that requires any of the specified roles
pub struct RequireAnyRole {
    pub user: AuthenticatedUser,
    pub roles: Vec<Role>,
}

// Helper macro for creating permission extractors
macro_rules! impl_permission_extractor {
    ($name:ident, $permission:expr) => {
        pub struct $name {
            pub user: AuthenticatedUser,
        }
        
        #[axum::async_trait]
        impl<S> FromRequestParts<S> for $name
        where
            S: Send + Sync,
            AuthenticatedUser: FromRequestParts<S, Rejection = Response>,
        {
            type Rejection = Response;
            
            async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
                let user = AuthenticatedUser::from_request_parts(parts, state).await?;
                
                let role = Role::from_str(&user.claims.role)
                    .ok_or_else(|| AuthorizationError::InvalidRole.into_response())?;
                
                if !role.has_permission(&$permission) {
                    return Err(AuthorizationError::InsufficientPermissions($permission).into_response());
                }
                
                Ok($name { user })
            }
        }
    };
}

// Implement common permission extractors
impl_permission_extractor!(CanViewMarkets, Permission::ViewMarkets);
impl_permission_extractor!(CanCreateMarkets, Permission::CreateMarkets);
impl_permission_extractor!(CanPlaceTrades, Permission::PlaceTrades);
impl_permission_extractor!(CanViewAllPositions, Permission::ViewAllPositions);
impl_permission_extractor!(CanUpdateSystemConfig, Permission::UpdateSystemConfig);

/// Authorization service for runtime checks
pub struct AuthorizationService {
    /// Optional custom permission overrides
    custom_permissions: Arc<std::sync::RwLock<std::collections::HashMap<String, HashSet<Permission>>>>,
}

impl AuthorizationService {
    pub fn new() -> Self {
        Self {
            custom_permissions: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Check if user has permission
    pub fn check_permission(&self, claims: &JwtClaims, permission: &Permission) -> bool {
        // Check custom permissions first
        if let Ok(custom) = self.custom_permissions.read() {
            if let Some(perms) = custom.get(&claims.wallet) {
                if perms.contains(permission) {
                    return true;
                }
            }
        }
        
        // Check role-based permissions
        Role::from_str(&claims.role)
            .map(|role| role.has_permission(permission))
            .unwrap_or(false)
    }
    
    /// Check if a role has permission (for compatibility)
    pub fn has_permission(&self, role: &crate::auth::UserRole, permission: &Permission) -> bool {
        Role::from_str(&role.to_string())
            .map(|r| r.has_permission(permission))
            .unwrap_or(false)
    }
    
    /// Grant custom permission to user
    pub fn grant_permission(&self, wallet: &str, permission: Permission) {
        if let Ok(mut custom) = self.custom_permissions.write() {
            custom.entry(wallet.to_string())
                .or_insert_with(HashSet::new)
                .insert(permission);
        }
    }
    
    /// Revoke custom permission from user
    pub fn revoke_permission(&self, wallet: &str, permission: &Permission) {
        if let Ok(mut custom) = self.custom_permissions.write() {
            if let Some(perms) = custom.get_mut(wallet) {
                perms.remove(permission);
            }
        }
    }
}