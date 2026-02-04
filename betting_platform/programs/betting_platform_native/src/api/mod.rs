//! API Module
//!
//! Production-grade API types and structures for REST/WebSocket interfaces
//! These types are designed to work both on-chain (for serialization) and off-chain (for API servers)

// Feature-gate types module as it uses serde
#[cfg(feature = "api")]
pub mod types;

// Feature-gated modules that require async runtime
#[cfg(feature = "api")]
pub mod rest_server;
#[cfg(feature = "api")]
pub mod rate_limiter;
#[cfg(feature = "api")]
pub mod endpoints;
#[cfg(feature = "api")]
pub mod auth;
#[cfg(feature = "api")]
pub mod websocket;

#[cfg(feature = "api")]
pub use types::*;

#[cfg(feature = "api")]
pub use rest_server::*;
#[cfg(feature = "api")]
pub use rate_limiter::*;
#[cfg(feature = "api")]
pub use endpoints::*;
#[cfg(feature = "api")]
pub use auth::*;
#[cfg(feature = "api")]
pub use websocket::*;