//! Optimization module for betting platform
//!
//! Contains compute unit optimization and performance improvements

pub mod cu_optimizer;
pub mod batch_optimizer;
pub mod rent_optimizer;
pub mod compute_units;
pub mod data_compression;
pub mod batch_processing;
pub mod cache_layer;
pub mod benchmarks;

pub use cu_optimizer::{CUOptimizer, OptimizationResult, optimized_math};
pub use batch_optimizer::{BatchOptimizer, BatchOperationResult, BatchOperationType};
pub use rent_optimizer::{RentCalculator, RentOptimizer, RentOptimizationConfig};
pub use compute_units::*;
pub use data_compression::*;
pub use batch_processing::*;
pub use cache_layer::*;
pub use benchmarks::*;