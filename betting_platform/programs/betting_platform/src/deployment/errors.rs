use anchor_lang::prelude::*;

#[error_code]
pub enum DeploymentError {
    #[msg("Failed to deploy program")]
    DeploymentFailed,
    
    #[msg("Failed to burn upgrade authority")]
    BurnAuthorityFailed,
    
    #[msg("Program is still upgradeable")]
    ProgramStillUpgradeable,
    
    #[msg("Failed to get recent blockhash")]
    BlockhashFailed,
    
    #[msg("Transaction failed")]
    TransactionFailed,
    
    #[msg("Invalid program bytes")]
    InvalidProgramBytes,
    
    #[msg("Authority mismatch")]
    AuthorityMismatch,
    
    #[msg("RPC client error")]
    RpcError,
    
    #[msg("Program account not found")]
    ProgramAccountNotFound,
    
    #[msg("Upgrade authority still exists")]
    UpgradeAuthorityExists,
}

#[error_code]
pub enum GenesisError {
    #[msg("Genesis initialization failed")]
    InitializationFailed,
    
    #[msg("MMT token creation failed")]
    MmtCreationFailed,
    
    #[msg("Failed to lock tokens in entropy sink")]
    TokenLockFailed,
    
    #[msg("Invalid genesis configuration")]
    InvalidConfiguration,
    
    #[msg("Vault creation failed")]
    VaultCreationFailed,
    
    #[msg("Global config already initialized")]
    AlreadyInitialized,
}

#[error_code]
pub enum MonitorError {
    #[msg("Monitoring task failed")]
    MonitoringFailed,
    
    #[msg("Failed to get vault balance")]
    VaultBalanceError,
    
    #[msg("Failed to calculate coverage")]
    CoverageCalculationError,
    
    #[msg("Failed to measure TPS")]
    TpsMeasurementError,
    
    #[msg("Keeper health check failed")]
    KeeperHealthError,
    
    #[msg("Alert system error")]
    AlertSystemError,
}

#[error_code]
pub enum IncentiveError {
    #[msg("Failed to activate incentives")]
    ActivationFailed,
    
    #[msg("Invalid incentive configuration")]
    InvalidConfiguration,
    
    #[msg("Bootstrap mode already active")]
    BootstrapAlreadyActive,
    
    #[msg("Invalid bonus multiplier")]
    InvalidBonusMultiplier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
    Emergency,
}