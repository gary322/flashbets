use anchor_lang::prelude::*;
use fixed::types::U64F64;
use crate::state::*;
use crate::errors::ErrorCode;
use crate::bootstrap::trade_result::BootstrapTradeResult;

pub struct BootstrapIncentiveEngine;

impl BootstrapIncentiveEngine {
    /// Process a trade during bootstrap phase
    pub fn process_bootstrap_trade(
        bootstrap_state: &mut BootstrapState,
        trader_state: &mut BootstrapTrader,
        trade_volume: u64,
        fee_paid: u64,
        leverage_used: U64F64,
        clock: &Clock,
    ) -> Result<BootstrapTradeResult> {
        // Check if trader is early (first 100)
        let is_early = bootstrap_state.early_traders_count < bootstrap_state.max_early_traders;
        
        // Update trader state
        if trader_state.trade_count == 0 {
            trader_state.first_trade_slot = clock.slot;
            if is_early {
                trader_state.is_early_trader = true;
                bootstrap_state.early_traders_count += 1;
            }
            bootstrap_state.unique_traders += 1;
        }
        
        trader_state.volume_traded = trader_state.volume_traded
            .checked_add(trade_volume)
            .ok_or(ErrorCode::MathOverflow)?;
        trader_state.trade_count += 1;
        
        // Update average leverage
        let total_leverage = trader_state.avg_leverage * U64F64::from_num(trader_state.trade_count - 1);
        trader_state.avg_leverage = (total_leverage + leverage_used) / U64F64::from_num(trader_state.trade_count);
        
        // Calculate trader tier
        let tier = Self::get_trader_tier(trader_state.volume_traded);
        
        // Calculate MMT rewards
        let mmt_reward = bootstrap_state.calculate_mmt_reward(
            trade_volume,
            trader_state.is_early_trader,
            &tier,
        );
        
        // Apply fee rebate
        let rebate = (fee_paid as u128 * tier.fee_rebate_bps as u128) / 10_000;
        let net_fee = fee_paid.saturating_sub(rebate as u64);
        
        // Update bootstrap state
        bootstrap_state.total_volume = bootstrap_state.total_volume
            .checked_add(trade_volume)
            .ok_or(ErrorCode::MathOverflow)?;
        bootstrap_state.current_vault_balance = bootstrap_state.current_vault_balance
            .checked_add(net_fee)
            .ok_or(ErrorCode::MathOverflow)?;
        bootstrap_state.mmt_distributed = bootstrap_state.mmt_distributed
            .checked_add(mmt_reward)
            .ok_or(ErrorCode::MathOverflow)?;
        
        // Update trader rewards
        trader_state.mmt_earned = trader_state.mmt_earned
            .checked_add(mmt_reward)
            .ok_or(ErrorCode::MathOverflow)?;
        trader_state.vault_contribution = trader_state.vault_contribution
            .checked_add(net_fee)
            .ok_or(ErrorCode::MathOverflow)?;
        
        Ok(BootstrapTradeResult {
            mmt_reward,
            fee_rebate: rebate as u64,
            net_fee,
            new_coverage: bootstrap_state.current_coverage,
            is_early_trader: trader_state.is_early_trader,
            tier,
        })
    }
    
    /// Get trader tier based on volume
    pub fn get_trader_tier(volume: u64) -> IncentiveTier {
        match volume {
            v if v >= 1_000_000 * 10u64.pow(6) => IncentiveTier {
                min_volume: 1_000_000 * 10u64.pow(6),
                reward_multiplier: U64F64::from_num(3),
                fee_rebate_bps: 15,
                liquidation_priority: 1,
                advanced_features: true,
            },
            v if v >= 100_000 * 10u64.pow(6) => IncentiveTier {
                min_volume: 100_000 * 10u64.pow(6),
                reward_multiplier: U64F64::from_num(2),
                fee_rebate_bps: 10,
                liquidation_priority: 2,
                advanced_features: true,
            },
            v if v >= 10_000 * 10u64.pow(6) => IncentiveTier {
                min_volume: 10_000 * 10u64.pow(6),
                reward_multiplier: U64F64::from_num(1.5),
                fee_rebate_bps: 5,
                liquidation_priority: 3,
                advanced_features: false,
            },
            _ => IncentiveTier {
                min_volume: 0,
                reward_multiplier: U64F64::from_num(1),
                fee_rebate_bps: 0,
                liquidation_priority: 4,
                advanced_features: false,
            },
        }
    }
    
    /// Calculate coverage ratio with bootstrap adjustments
    pub fn calculate_bootstrap_coverage(
        vault_balance: u64,
        total_open_interest: u64,
        bootstrap_phase: bool,
    ) -> U64F64 {
        if total_open_interest == 0 {
            return U64F64::from_num(0);
        }
        
        // During bootstrap, use more conservative tail loss (0.5 -> 0.7)
        let tail_loss = if bootstrap_phase {
            U64F64::from_num(0.7)
        } else {
            U64F64::from_num(0.5)
        };
        
        let coverage = U64F64::from_num(vault_balance) /
                      (tail_loss * U64F64::from_num(total_open_interest));
        
        coverage
    }
    
    /// Process referral bonus during bootstrap
    pub fn process_referral(
        referrer_state: &mut BootstrapTrader,
        referred_volume: u64,
        bootstrap_state: &BootstrapState,
    ) -> Result<u64> {
        // 5% of referred trader's rewards go to referrer
        let referral_rate = U64F64::from_num(0.05);
        
        let tier = Self::get_trader_tier(referred_volume);
        let base_reward = bootstrap_state.calculate_mmt_reward(
            referred_volume,
            false, // Referral doesn't get early bonus
            &tier,
        );
        
        let referral_bonus = (U64F64::from_num(base_reward) * referral_rate)
            .to_num::<u64>();
        
        referrer_state.referral_bonus = referrer_state.referral_bonus
            .checked_add(referral_bonus)
            .ok_or(ErrorCode::MathOverflow)?;
        referrer_state.referred_count += 1;
        
        Ok(referral_bonus)
    }
}