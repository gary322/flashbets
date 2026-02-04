//! Middleware modules for the API server

pub mod auth;

pub use auth::{AuthenticatedUser, OptionalAuth, RequireRole, ApiKeyAuth};