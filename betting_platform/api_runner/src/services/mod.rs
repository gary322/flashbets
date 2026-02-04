//! Services module for business logic

pub mod polymarket_order_service;

pub use polymarket_order_service::{
    PolymarketOrderService,
    OrderServiceConfig,
    CreateOrderParams,
    OrderSubmissionResult,
    OrderTracking,
    OrderLifecycle,
    BatchOrderManager,
};