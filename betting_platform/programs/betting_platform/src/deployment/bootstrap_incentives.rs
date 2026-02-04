use anchor_lang::prelude::*;
use crate::deployment::types::GlobalConfig;
use crate::deployment::errors::IncentiveError;

#[derive(Clone, Debug)]
pub struct BootstrapIncentives {
    pub double_mmt_duration: u64, // First 100 trades
    pub early_maker_bonus: f64, // 2x rewards
    pub liquidity_mining_rate: f64,
}

impl Default for BootstrapIncentives {
    fn default() -> Self {
        Self {
            double_mmt_duration: 100, // First 100 trades get double MMT
            early_maker_bonus: 2.0, // 2x rewards for early makers
            liquidity_mining_rate: 0.001, // 0.1% per epoch
        }
    }
}

impl BootstrapIncentives {
    pub fn new(
        double_mmt_duration: u64,
        early_maker_bonus: f64,
        liquidity_mining_rate: f64,
    ) -> Result<Self> {
        // Validate parameters
        if early_maker_bonus < 1.0 || early_maker_bonus > 10.0 {
            return Err(IncentiveError::InvalidBonusMultiplier.into());
        }
        
        if liquidity_mining_rate < 0.0 || liquidity_mining_rate > 0.1 {
            return Err(IncentiveError::InvalidConfiguration.into());
        }
        
        Ok(Self {
            double_mmt_duration,
            early_maker_bonus,
            liquidity_mining_rate,
        })
    }

    pub fn activate_launch_incentives(
        &self,
        global_config: &mut GlobalConfig,
    ) -> Result<()> {
        msg!("Activating bootstrap incentives");
        
        // Check if already activated
        if global_config.bootstrap_mode {
            return Err(IncentiveError::BootstrapAlreadyActive.into());
        }
        
        // Enable double MMT for first traders
        global_config.bootstrap_mode = true;
        global_config.bootstrap_trade_count = 0;
        global_config.bootstrap_max_trades = self.double_mmt_duration;
        
        msg!("Double MMT enabled for first {} trades", self.double_mmt_duration);
        
        // Set early maker bonus
        global_config.maker_bonus_multiplier = self.early_maker_bonus;
        
        msg!("Early maker bonus set to {}x", self.early_maker_bonus);
        
        // Initialize liquidity mining
        global_config.liquidity_mining_active = true;
        global_config.liquidity_mining_rate = self.liquidity_mining_rate;
        
        msg!("Liquidity mining activated at {} rate", self.liquidity_mining_rate);
        
        Ok(())
    }

    pub fn deactivate_bootstrap_incentives(
        &self,
        global_config: &mut GlobalConfig,
    ) -> Result<()> {
        msg!("Deactivating bootstrap incentives");
        
        // Disable bootstrap mode
        global_config.bootstrap_mode = false;
        
        // Reset maker bonus to normal (1x)
        global_config.maker_bonus_multiplier = 1.0;
        
        // Keep liquidity mining active but could adjust rate
        global_config.liquidity_mining_rate = self.liquidity_mining_rate / 2.0;
        
        msg!("Bootstrap incentives deactivated");
        
        Ok(())
    }

    pub fn should_apply_double_mmt(
        &self,
        global_config: &GlobalConfig,
    ) -> bool {
        global_config.bootstrap_mode && 
        global_config.bootstrap_trade_count < global_config.bootstrap_max_trades
    }

    pub fn calculate_mmt_reward(
        &self,
        base_reward: u64,
        global_config: &GlobalConfig,
    ) -> u64 {
        if self.should_apply_double_mmt(global_config) {
            base_reward.saturating_mul(2)
        } else {
            base_reward
        }
    }

    pub fn calculate_maker_reward(
        &self,
        base_reward: u64,
        global_config: &GlobalConfig,
    ) -> u64 {
        let multiplier = global_config.maker_bonus_multiplier;
        (base_reward as f64 * multiplier) as u64
    }

    pub fn increment_bootstrap_counter(
        &self,
        global_config: &mut GlobalConfig,
    ) -> Result<()> {
        if global_config.bootstrap_mode {
            global_config.bootstrap_trade_count = 
                global_config.bootstrap_trade_count.saturating_add(1);
            
            // Check if bootstrap period is complete
            if global_config.bootstrap_trade_count >= global_config.bootstrap_max_trades {
                msg!("Bootstrap period complete after {} trades", 
                     global_config.bootstrap_trade_count);
                
                // Could trigger automatic deactivation or adjustment
                // For now, just log it
            }
        }
        
        Ok(())
    }

    pub fn calculate_liquidity_mining_reward(
        &self,
        liquidity_provided: u64,
        duration_slots: u64,
        global_config: &GlobalConfig,
    ) -> u64 {
        if !global_config.liquidity_mining_active {
            return 0;
        }
        
        let rate = global_config.liquidity_mining_rate;
        let base_reward = (liquidity_provided as f64 * rate * duration_slots as f64 / 432000.0) as u64;
        
        // Apply early provider bonus during bootstrap
        if global_config.bootstrap_mode {
            (base_reward as f64 * global_config.maker_bonus_multiplier) as u64
        } else {
            base_reward
        }
    }

    pub fn get_bootstrap_stats(&self, global_config: &GlobalConfig) -> BootstrapStats {
        BootstrapStats {
            is_active: global_config.bootstrap_mode,
            trades_completed: global_config.bootstrap_trade_count,
            trades_remaining: global_config.bootstrap_max_trades
                .saturating_sub(global_config.bootstrap_trade_count),
            current_maker_bonus: global_config.maker_bonus_multiplier,
            liquidity_mining_rate: global_config.liquidity_mining_rate,
            double_mmt_active: self.should_apply_double_mmt(global_config),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BootstrapStats {
    pub is_active: bool,
    pub trades_completed: u64,
    pub trades_remaining: u64,
    pub current_maker_bonus: f64,
    pub liquidity_mining_rate: f64,
    pub double_mmt_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_activation() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        assert!(!config.bootstrap_mode);
        
        incentives.activate_launch_incentives(&mut config).unwrap();
        
        assert!(config.bootstrap_mode);
        assert_eq!(config.bootstrap_trade_count, 0);
        assert_eq!(config.bootstrap_max_trades, 100);
        assert_eq!(config.maker_bonus_multiplier, 2.0);
        assert!(config.liquidity_mining_active);
    }

    #[test]
    fn test_double_mmt_calculation() {
        let incentives = BootstrapIncentives::default();
        let mut config = GlobalConfig::default();
        
        incentives.activate_launch_incentives(&mut config).unwrap();
        
        let base_reward = 1000;
        let boosted_reward = incentives.calculate_mmt_reward(base_reward, &config);
        
        assert_eq!(boosted_reward, 2000);
        
        // After 100 trades, no more double MMT
        config.bootstrap_trade_count = 100;
        let normal_reward = incentives.calculate_mmt_reward(base_reward, &config);
        
        assert_eq!(normal_reward, 1000);
    }
}