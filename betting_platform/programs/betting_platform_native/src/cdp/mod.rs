//! Collateralized Debt Position (CDP) Module
//!
//! Implements synthetic leverage through CDP mechanism with oracle validation

pub mod state;
pub mod vault;
pub mod borrowing;
pub mod liquidation;
pub mod interest;
pub mod oracle_feed;
pub mod instructions;

pub use state::{
    CDPAccount,
    CDPState,
    DebtPosition,
    CollateralType,
    CDPStatus,
    CDP_ACCOUNT_SEED,
    CDP_VAULT_SEED,
};

pub use vault::{
    CDPVault,
    VaultState,
    VaultStats,
    CollateralPool,
    calculate_vault_health,
    execute_vault_deposit,
    execute_vault_withdraw,
};

pub use borrowing::{
    BorrowRequest,
    BorrowLimits,
    BorrowPosition,
    calculate_borrow_capacity,
    calculate_max_borrow,
    execute_borrow,
    execute_repay,
};

pub use liquidation::{
    LiquidationEngine,
    LiquidationParams,
    LiquidationAuction,
    LiquidationStatus,
    check_liquidation_threshold,
    execute_liquidation,
    distribute_liquidation_proceeds,
};

pub use interest::{
    InterestModel,
    InterestRate,
    calculate_interest,
    accrue_interest,
    compound_interest,
    get_current_rate,
};

pub use oracle_feed::{
    CDPOracleFeed,
    PriceFeed,
    validate_oracle_price,
    get_collateral_value,
    calculate_ltv_ratio,
};

pub use instructions::{
    create_cdp,
    deposit_collateral,
    borrow_synthetic,
    repay_debt,
    withdraw_collateral,
    liquidate_cdp,
    update_oracle_price,
    emergency_shutdown,
};