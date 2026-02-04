//! CDP Liquidation Engine
//!
//! Handles liquidation of under-collateralized CDPs

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{clock::Clock, Sysvar},
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    oracle::OraclePDA,
    constants::*,
};

use super::state::{CDPAccount, CDPStatus, CollateralType};

/// Liquidation status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum LiquidationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Liquidation parameters
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct LiquidationParams {
    /// Liquidation penalty (percentage of collateral)
    pub liquidation_penalty: f64,
    
    /// Close factor (max percentage of debt that can be liquidated)
    pub close_factor: f64,
    
    /// Liquidator incentive (bonus percentage)
    pub liquidator_incentive: f64,
    
    /// Minimum liquidation amount
    pub min_liquidation_amount: u128,
    
    /// Auction duration (slots)
    pub auction_duration: u64,
    
    /// Grace period before liquidation (slots)
    pub grace_period: u64,
    
    /// Auto-liquidation threshold (health factor)
    pub auto_liquidation_threshold: f64,
    
    /// Max liquidations per slot
    pub max_liquidations_per_slot: u32,
    
    /// Cascade protection enabled
    pub cascade_protection: bool,
}

impl LiquidationParams {
    pub fn new() -> Self {
        Self {
            liquidation_penalty: 0.1, // 10%
            close_factor: 0.5, // 50% max liquidation
            liquidator_incentive: 0.05, // 5% bonus
            min_liquidation_amount: 100 * 10u128.pow(6), // 100 USDC
            auction_duration: 432, // ~3 minutes
            grace_period: 10, // ~4 seconds
            auto_liquidation_threshold: 1.0,
            max_liquidations_per_slot: 10,
            cascade_protection: true,
        }
    }
    
    /// Calculate liquidation amounts
    pub fn calculate_liquidation_amounts(
        &self,
        debt_amount: u128,
        collateral_amount: u128,
        oracle_price: f64,
    ) -> (u128, u128, u128) {
        // Maximum debt that can be liquidated
        let max_liquidatable_debt = ((debt_amount as f64) * self.close_factor) as u128;
        
        // Collateral needed to cover debt (with penalty)
        let collateral_per_debt = 1.0 / oracle_price * (1.0 + self.liquidation_penalty);
        let collateral_to_seize = ((max_liquidatable_debt as f64) * collateral_per_debt) as u128;
        
        // Liquidator bonus
        let liquidator_bonus = ((collateral_to_seize as f64) * self.liquidator_incentive) as u128;
        
        // Ensure we don't seize more than available
        let actual_collateral_seized = collateral_to_seize.min(collateral_amount);
        let actual_debt_repaid = ((actual_collateral_seized as f64) / collateral_per_debt) as u128;
        
        (actual_debt_repaid, actual_collateral_seized, liquidator_bonus)
    }
}

/// Liquidation auction
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct LiquidationAuction {
    /// Auction ID
    pub auction_id: u128,
    
    /// CDP being liquidated
    pub cdp_id: u128,
    
    /// Owner of the CDP
    pub cdp_owner: Pubkey,
    
    /// Start slot
    pub start_slot: u64,
    
    /// End slot
    pub end_slot: u64,
    
    /// Collateral for sale
    pub collateral_amount: u128,
    
    /// Debt to cover
    pub debt_amount: u128,
    
    /// Current best bid
    pub best_bid: u128,
    
    /// Current best bidder
    pub best_bidder: Option<Pubkey>,
    
    /// Reserve price (minimum bid)
    pub reserve_price: u128,
    
    /// Status
    pub status: LiquidationStatus,
    
    /// Total bids received
    pub bid_count: u32,
}

impl LiquidationAuction {
    pub fn new(
        auction_id: u128,
        cdp_id: u128,
        cdp_owner: Pubkey,
        collateral_amount: u128,
        debt_amount: u128,
        duration: u64,
        current_slot: u64,
    ) -> Self {
        Self {
            auction_id,
            cdp_id,
            cdp_owner,
            start_slot: current_slot,
            end_slot: current_slot + duration,
            collateral_amount,
            debt_amount,
            best_bid: 0,
            best_bidder: None,
            reserve_price: debt_amount, // Must at least cover debt
            status: LiquidationStatus::Pending,
            bid_count: 0,
        }
    }
    
    /// Place a bid
    pub fn place_bid(
        &mut self,
        bidder: Pubkey,
        bid_amount: u128,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check auction is active
        if self.status != LiquidationStatus::InProgress {
            msg!("Auction not active");
            return Err(BettingPlatformError::AuctionNotActive.into());
        }
        
        // Check auction hasn't ended
        if current_slot > self.end_slot {
            msg!("Auction has ended");
            return Err(BettingPlatformError::AuctionEnded.into());
        }
        
        // Check bid meets reserve
        if bid_amount < self.reserve_price {
            msg!("Bid below reserve price");
            return Err(BettingPlatformError::BidBelowReserve.into());
        }
        
        // Check bid is higher than current best
        if bid_amount <= self.best_bid {
            msg!("Bid not high enough");
            return Err(BettingPlatformError::BidTooLow.into());
        }
        
        // Update auction
        self.best_bid = bid_amount;
        self.best_bidder = Some(bidder);
        self.bid_count += 1;
        
        // Extend auction if bid near end (within 10 slots)
        if self.end_slot - current_slot < 10 {
            self.end_slot += 10;
        }
        
        Ok(())
    }
    
    /// Finalize auction
    pub fn finalize(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        if current_slot <= self.end_slot {
            msg!("Auction still active");
            return Err(BettingPlatformError::AuctionStillActive.into());
        }
        
        if self.best_bidder.is_none() {
            self.status = LiquidationStatus::Failed;
            msg!("Auction failed - no bids");
        } else {
            self.status = LiquidationStatus::Completed;
            msg!("Auction completed with bid: {}", self.best_bid);
        }
        
        Ok(())
    }
}

/// Liquidation engine
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct LiquidationEngine {
    /// Liquidation parameters
    pub params: LiquidationParams,
    
    /// Active liquidations
    pub active_liquidations: Vec<u128>, // CDP IDs
    
    /// Completed liquidations
    pub completed_liquidations: u64,
    
    /// Failed liquidations
    pub failed_liquidations: u64,
    
    /// Total collateral liquidated
    pub total_collateral_liquidated: u128,
    
    /// Total debt repaid
    pub total_debt_repaid: u128,
    
    /// Current slot liquidation count
    pub current_slot_liquidations: u32,
    
    /// Last liquidation slot
    pub last_liquidation_slot: u64,
    
    /// Cascade protection active
    pub cascade_protection_active: bool,
    
    /// Emergency pause
    pub emergency_pause: bool,
}

impl LiquidationEngine {
    pub fn new() -> Self {
        Self {
            params: LiquidationParams::new(),
            active_liquidations: Vec::new(),
            completed_liquidations: 0,
            failed_liquidations: 0,
            total_collateral_liquidated: 0,
            total_debt_repaid: 0,
            current_slot_liquidations: 0,
            last_liquidation_slot: 0,
            cascade_protection_active: false,
            emergency_pause: false,
        }
    }
    
    /// Check if can liquidate
    pub fn can_liquidate(
        &mut self,
        cdp: &CDPAccount,
        current_slot: u64,
    ) -> Result<bool, ProgramError> {
        // Check emergency pause
        if self.emergency_pause {
            return Ok(false);
        }
        
        // Check if CDP is liquidatable
        if !cdp.should_liquidate() {
            return Ok(false);
        }
        
        // Check if already being liquidated
        if self.active_liquidations.contains(&cdp.cdp_id) {
            return Ok(false);
        }
        
        // Check cascade protection
        if self.params.cascade_protection {
            // Reset counter if new slot
            if current_slot > self.last_liquidation_slot {
                self.current_slot_liquidations = 0;
                self.last_liquidation_slot = current_slot;
            }
            
            // Check if at limit
            if self.current_slot_liquidations >= self.params.max_liquidations_per_slot {
                self.cascade_protection_active = true;
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Start liquidation
    pub fn start_liquidation(
        &mut self,
        cdp: &mut CDPAccount,
        current_slot: u64,
    ) -> Result<LiquidationAuction, ProgramError> {
        if !self.can_liquidate(cdp, current_slot)? {
            return Err(BettingPlatformError::NotLiquidatable.into());
        }
        
        // Mark CDP as liquidating
        cdp.liquidate()?;
        
        // Add to active liquidations
        self.active_liquidations.push(cdp.cdp_id);
        self.current_slot_liquidations += 1;
        
        // Create auction
        let auction = LiquidationAuction::new(
            cdp.cdp_id * 10000 + self.completed_liquidations, // Unique auction ID
            cdp.cdp_id,
            cdp.owner,
            cdp.collateral_amount,
            cdp.debt_amount,
            self.params.auction_duration,
            current_slot,
        );
        
        msg!("Started liquidation auction for CDP {}", cdp.cdp_id);
        
        Ok(auction)
    }
    
    /// Complete liquidation
    pub fn complete_liquidation(
        &mut self,
        cdp: &mut CDPAccount,
        auction: &LiquidationAuction,
    ) -> Result<(), ProgramError> {
        if auction.status != LiquidationStatus::Completed {
            return Err(BettingPlatformError::AuctionNotComplete.into());
        }
        
        // Update CDP
        cdp.complete_liquidation();
        
        // Remove from active
        self.active_liquidations.retain(|&id| id != cdp.cdp_id);
        
        // Update stats
        self.completed_liquidations += 1;
        self.total_collateral_liquidated += auction.collateral_amount;
        self.total_debt_repaid += auction.best_bid;
        
        msg!("Completed liquidation of CDP {}", cdp.cdp_id);
        
        Ok(())
    }
}

/// Check if CDP meets liquidation threshold
pub fn check_liquidation_threshold(
    cdp: &CDPAccount,
    oracle_price: f64,
) -> bool {
    // Calculate current health factor
    let mut cdp_mut = cdp.clone();
    let health = cdp_mut.calculate_health_factor(oracle_price);
    
    // Check if below threshold
    health < 1.0
}

/// Execute liquidation
pub fn execute_liquidation(
    program_id: &Pubkey,
    cdp: &mut CDPAccount,
    liquidator: &Pubkey,
    repay_amount: u128,
    oracle_pda: &OraclePDA,
    liquidation_params: &LiquidationParams,
) -> Result<(u128, u128), ProgramError> {
    // Verify CDP is liquidatable
    if !cdp.should_liquidate() {
        msg!("CDP not eligible for liquidation");
        return Err(BettingPlatformError::NotLiquidatable.into());
    }
    
    // Calculate liquidation amounts
    let (debt_repaid, collateral_seized, bonus) = 
        liquidation_params.calculate_liquidation_amounts(
            cdp.debt_amount,
            cdp.collateral_amount,
            oracle_pda.current_prob,
        );
    
    // Ensure repay amount is sufficient
    if repay_amount < debt_repaid {
        msg!("Insufficient repay amount for liquidation");
        return Err(BettingPlatformError::InsufficientRepayment.into());
    }
    
    // Update CDP
    cdp.debt_amount = cdp.debt_amount.saturating_sub(debt_repaid);
    cdp.collateral_amount = cdp.collateral_amount.saturating_sub(collateral_seized);
    
    // Recalculate health
    cdp.calculate_health_factor(oracle_pda.current_prob);
    
    // Check if fully liquidated
    if cdp.debt_amount == 0 || cdp.collateral_amount == 0 {
        cdp.complete_liquidation();
    } else if cdp.health_factor >= 1.0 {
        // CDP is healthy again
        cdp.status = CDPStatus::Active;
    }
    
    msg!("Liquidated {} debt for {} collateral (+ {} bonus)", 
         debt_repaid, collateral_seized, bonus);
    
    Ok((collateral_seized + bonus, debt_repaid))
}

/// Distribute liquidation proceeds
pub fn distribute_liquidation_proceeds(
    collateral_seized: u128,
    debt_repaid: u128,
    liquidator: &Pubkey,
    protocol_treasury: &Pubkey,
    liquidation_params: &LiquidationParams,
) -> Result<(u128, u128, u128), ProgramError> {
    // Calculate distributions
    let liquidator_bonus = ((collateral_seized as f64) * liquidation_params.liquidator_incentive) as u128;
    let protocol_fee = ((debt_repaid as f64) * 0.01) as u128; // 1% protocol fee
    let borrower_return = collateral_seized
        .saturating_sub(debt_repaid)
        .saturating_sub(liquidator_bonus)
        .saturating_sub(protocol_fee);
    
    msg!("Liquidation distribution:");
    msg!("  Liquidator: {} (includes {} bonus)", 
         debt_repaid + liquidator_bonus, liquidator_bonus);
    msg!("  Protocol: {}", protocol_fee);
    msg!("  Borrower: {}", borrower_return);
    
    Ok((debt_repaid + liquidator_bonus, protocol_fee, borrower_return))
}

/// Calculate liquidation price for a CDP
pub fn calculate_liquidation_price(
    cdp: &CDPAccount,
) -> f64 {
    if cdp.collateral_amount == 0 {
        return 0.0;
    }
    
    let liquidation_ratio = cdp.collateral_type.get_liquidation_ratio();
    (cdp.debt_amount as f64 * liquidation_ratio) / cdp.collateral_amount as f64
}

/// Batch liquidation for cascade protection
pub fn batch_liquidate_cdps(
    cdps: &mut [CDPAccount],
    oracle_price: f64,
    max_liquidations: usize,
) -> Vec<u128> {
    let mut liquidated = Vec::new();
    
    for cdp in cdps.iter_mut().take(max_liquidations) {
        if cdp.should_liquidate() {
            let health = cdp.calculate_health_factor(oracle_price);
            if health < 1.0 {
                cdp.status = CDPStatus::Liquidating;
                liquidated.push(cdp.cdp_id);
            }
        }
    }
    
    liquidated
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_liquidation_amounts() {
        let params = LiquidationParams::new();
        
        let (debt_repaid, collateral_seized, bonus) = 
            params.calculate_liquidation_amounts(1000, 1500, 1.0);
        
        // With 50% close factor, should repay 500 debt
        assert_eq!(debt_repaid, 500);
        
        // Collateral seized should include penalty
        assert!(collateral_seized > debt_repaid);
        
        // Should have liquidator bonus
        assert!(bonus > 0);
    }
    
    #[test]
    fn test_auction() {
        let mut auction = LiquidationAuction::new(
            1,
            1,
            Pubkey::default(),
            1500,
            1000,
            100,
            0,
        );
        
        auction.status = LiquidationStatus::InProgress;
        
        // Place bid
        assert!(auction.place_bid(Pubkey::default(), 1100, 10).is_ok());
        assert_eq!(auction.best_bid, 1100);
        
        // Lower bid should fail
        assert!(auction.place_bid(Pubkey::default(), 1050, 20).is_err());
        
        // Higher bid should succeed
        assert!(auction.place_bid(Pubkey::default(), 1200, 30).is_ok());
        assert_eq!(auction.best_bid, 1200);
    }
    
    #[test]
    fn test_cascade_protection() {
        let mut engine = LiquidationEngine::new();
        engine.params.max_liquidations_per_slot = 2;
        
        let mut cdp = CDPAccount::new(
            Pubkey::default(),
            1,
            1,
            CollateralType::USDC,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        );
        cdp.health_factor = 0.5; // Unhealthy
        
        // First two should succeed
        assert!(engine.can_liquidate(&cdp, 100).unwrap());
        engine.current_slot_liquidations = 1;
        
        assert!(engine.can_liquidate(&cdp, 100).unwrap());
        engine.current_slot_liquidations = 2;
        
        // Third should fail (cascade protection)
        assert!(!engine.can_liquidate(&cdp, 100).unwrap());
        
        // New slot should reset
        assert!(engine.can_liquidate(&cdp, 101).unwrap());
    }
}