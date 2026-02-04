use anchor_lang::prelude::*;

#[error_code]
pub enum ProfileError {
    #[msg("Failed to get current CU")]
    CuTrackingError,
    
    #[msg("Failed to measure memory usage")]
    MemoryMeasurementError,
    
    #[msg("Operation execution failed")]
    OperationFailed,
    
    #[msg("Performance metric collection failed")]
    MetricCollectionError,
    
    #[msg("Bottleneck detection failed")]
    BottleneckDetectionError,
}

#[error_code]
pub enum OptimizationError {
    #[msg("Arithmetic overflow occurred")]
    Overflow,
    
    #[msg("Division by zero")]
    DivisionByZero,
    
    #[msg("Invalid parameters for optimization")]
    InvalidParameters,
    
    #[msg("Cache lookup failed")]
    CacheLookupError,
    
    #[msg("Precomputed table not found")]
    TableNotFound,
    
    #[msg("Newton-Raphson did not converge")]
    ConvergenceFailure,
    
    #[msg("Fixed point conversion error")]
    FixedPointError,
}

#[error_code]
pub enum StressTestError {
    #[msg("Stress test setup failed")]
    SetupFailed,
    
    #[msg("Load generation failed")]
    LoadGenerationFailed,
    
    #[msg("Scenario execution failed")]
    ScenarioFailed,
    
    #[msg("Metrics collection failed during test")]
    MetricsCollectionFailed,
    
    #[msg("Test timeout exceeded")]
    TimeoutExceeded,
    
    #[msg("Resource exhaustion")]
    ResourceExhaustion,
}

#[error_code]
pub enum CompressionError {
    #[msg("State compression failed")]
    CompressionFailed,
    
    #[msg("State decompression failed")]
    DecompressionFailed,
    
    #[msg("ZK proof generation failed")]
    ProofGenerationFailed,
    
    #[msg("ZK proof verification failed")]
    ProofVerificationFailed,
    
    #[msg("Delta encoding failed")]
    DeltaEncodingFailed,
}

// Performance constants
pub const TARGET_CU_PER_TRADE: u64 = 20_000;
pub const TARGET_CU_PER_LEVERAGE_CALC: u64 = 1_000;
pub const TARGET_CU_PER_CHAIN_STEP: u64 = 10_000;
pub const MAX_CHAIN_CU: u64 = 50_000;
pub const TARGET_TPS: f64 = 5_000.0;
pub const MAX_LATENCY_MS: f64 = 20.0;
pub const CONVERGENCE_THRESHOLD: i64 = 100; // 0.0001 in fixed point
pub const FIXED_POINT_SCALE: i64 = 1_000_000;
pub const NEWTON_RAPHSON_MAX_ITERATIONS: u8 = 5;