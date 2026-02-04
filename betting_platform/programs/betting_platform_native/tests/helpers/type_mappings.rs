//! Type mappings for test compatibility
//!
//! Maps test type names to actual implementation types

use betting_platform_native::state::{
    LSMRMarket as LmsrMarketPDA,
    PMAMMPool as PmammPoolPDA,
    L2DistributionState as L2DistributionPDA,
    DarkPool as DarkPoolPDA,
    IcebergOrder as IcebergOrderPDA,
    TwapOrder as TwapOrderPDA,
    StopOrder as StopOrderPDA,
    BootstrapPhase as BootstrapPDA,
};

// Re-export with test names
pub type LmsrMarketPDA = LSMRMarket;
pub type PmammPoolPDA = PMAMMPool;
pub type L2DistributionPDA = L2DistributionState;
pub type DarkPoolPDA = DarkPool;
pub type IcebergOrderPDA = IcebergOrder;
pub type TwapOrderPDA = TwapOrder;
pub type StopOrderPDA = StopOrder;
pub type BootstrapPDA = BootstrapPhase;