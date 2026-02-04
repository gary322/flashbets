use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::{HashMap, VecDeque};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::synthetics::SyntheticWrapper;

/// Derivation engine for calculating synthetic probabilities
pub struct DerivationEngine {
    pub recency_decay_factor: U64F64, // How much to weight recent vs old data
}

#[derive(Debug, Clone)]
pub struct MarketData {
    pub market_id: Pubkey,
    pub probability: U64F64,
    pub volume_7d: u64,
    pub liquidity_depth: u64,
    pub last_trade_time: i64,
    pub category: String,
    pub title: String,
    pub yes_price: u64,
    pub volume_24h: u64,
    pub liquidity: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct PriceSnapshot {
    pub slot: u64,
    pub price: U64F64,
    pub volume: u64,
    pub liquidity: u64,
    pub timestamp: i64,
}

pub struct ProbabilityDeriver {
    pub price_history: HashMap<Pubkey, VecDeque<PriceSnapshot>>,
    pub volume_window: u64,  // 7 days in slots (assuming 2 slots/sec)
    pub update_frequency: u64, // Update every 1000 slots
}

impl Default for DerivationEngine {
    fn default() -> Self {
        Self {
            recency_decay_factor: U64F64::from_num(900_000), // 90% weight for recency (0.9 * 1e6)
        }
    }
}

impl DerivationEngine {
    pub fn new(recency_decay_factor: U64F64) -> Self {
        Self { recency_decay_factor }
    }

    /// Derive synthetic probability from child markets
    pub fn derive_synthetic_probability(
        &self,
        wrapper: &SyntheticWrapper,
        market_data: Vec<MarketData>,
    ) -> Result<U64F64, ProgramError> {
        if market_data.len() != wrapper.polymarket_markets.len() {
            return Err(BettingPlatformError::DataMismatch.into());
        }

        let mut weighted_sum = U64F64::from_num(0);
        let mut total_weight = U64F64::from_num(0);

        for (i, data) in market_data.iter().enumerate() {
            // Calculate dynamic weight based on volume, liquidity, and recency
            let volume_weight = U64F64::from_num(data.volume_7d);
            let liquidity_weight = U64F64::from_num(data.liquidity_depth);

            // Recency factor: 1 - (age_days / 7)
            let current_time = Clock::get()?.unix_timestamp;
            let age_seconds = current_time - data.last_trade_time;
            let age_days = U64F64::from_num(age_seconds as u64) / U64F64::from_num(86400);
            let recency_factor = (U64F64::from_num(1) - (age_days / U64F64::from_num(7)))
                .max(U64F64::from_num(0));

            // Combined weight
            let weight = volume_weight
                .checked_mul(liquidity_weight)?
                .sqrt()?
                .checked_mul(recency_factor)?
                .checked_mul(wrapper.weights[i])?;

            weighted_sum = weighted_sum.checked_add(
                data.probability.checked_mul(weight)?
            )?;
            total_weight = total_weight.checked_add(weight)?;
        }

        // Avoid division by zero
        if total_weight.is_zero() {
            return Ok(U64F64::from_num(500_000)); // Default to 50% (0.5 * 1e6)
        }

        weighted_sum.checked_div(total_weight)
    }

    /// Calculate divergence between markets and synthetic
    pub fn calculate_divergence(
        &self,
        wrapper: &SyntheticWrapper,
        market_data: &Vec<MarketData>,
    ) -> Result<Vec<(Pubkey, U64F64)>, ProgramError> {
        let synthetic_prob = self.derive_synthetic_probability(wrapper, market_data.clone())?;
        let mut divergences = Vec::new();

        for data in market_data {
            let diff = if data.probability > synthetic_prob {
                data.probability.checked_sub(synthetic_prob)?
            } else {
                synthetic_prob.checked_sub(data.probability)?
            };

            if diff > U64F64::from_num(50_000) { // 5% threshold (0.05 * 1e6)
                divergences.push((data.market_id, diff));
            }
        }

        Ok(divergences)
    }
}

impl Default for ProbabilityDeriver {
    fn default() -> Self {
        Self {
            price_history: HashMap::new(),
            volume_window: 604800, // 7 days * 86400 seconds / 2 slots per second
            update_frequency: 1000,
        }
    }
}

impl ProbabilityDeriver {
    pub fn new(volume_window: u64, update_frequency: u64) -> Self {
        Self {
            price_history: HashMap::new(),
            volume_window,
            update_frequency,
        }
    }

    /// Add price snapshot for a market
    pub fn add_price_snapshot(
        &mut self,
        market_id: Pubkey,
        snapshot: PriceSnapshot,
    ) {
        let history = self.price_history
            .entry(market_id)
            .or_insert_with(VecDeque::new);
        
        history.push_back(snapshot);
        
        // Keep only recent history
        let cutoff_slot = Clock::get()
            .map(|c| c.slot.saturating_sub(self.volume_window))
            .unwrap_or(0);
        
        while let Some(front) = history.front() {
            if front.slot < cutoff_slot {
                history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Derive verse probability from child markets
    pub fn derive_verse_probability(
        &self,
        verse: &SyntheticWrapper,
        current_slot: u64,
    ) -> Result<U64F64, ProgramError> {
        let mut weighted_sum = U64F64::from_num(0);
        let mut total_weight = U64F64::from_num(0);

        // Calculate time window
        let cutoff_slot = current_slot.saturating_sub(self.volume_window);

        for child in &verse.polymarket_markets {
            // Get price history for child
            let history = self.price_history.get(child)
                .ok_or(BettingPlatformError::NoPriceHistory)?;

            // Calculate 7-day volume-weighted average price
            let (vwap, total_volume) = self.calculate_vwap(history, cutoff_slot)?;

            // Get latest liquidity depth
            let liquidity_depth = history.back()
                .map(|s| s.liquidity)
                .unwrap_or(0);

            // Weight based on volume and liquidity
            let volume_weight = U64F64::from_num(total_volume);
            let liquidity_weight = U64F64::from_num(liquidity_depth);

            // Combined weight = sqrt(volume * liquidity)
            let combined_weight = volume_weight
                .checked_mul(liquidity_weight)?
                .sqrt()?;

            // Add to weighted sum
            let weighted_price = vwap.checked_mul(combined_weight)?;
            weighted_sum = weighted_sum.checked_add(weighted_price)?;
            total_weight = total_weight.checked_add(combined_weight)?;
        }

        // Return weighted average
        if total_weight.is_zero() {
            Ok(U64F64::from_num(500_000)) // Default to 50% (0.5 * 1e6)
        } else {
            weighted_sum.checked_div(total_weight)
        }
    }

    /// Calculate volume-weighted average price
    fn calculate_vwap(
        &self,
        history: &VecDeque<PriceSnapshot>,
        cutoff_slot: u64,
    ) -> Result<(U64F64, u64), ProgramError> {
        let mut price_volume_sum = U64F64::from_num(0);
        let mut total_volume = 0u64;

        for snapshot in history.iter() {
            if snapshot.slot < cutoff_slot {
                continue;
            }

            let weighted = snapshot.price
                .checked_mul(U64F64::from_num(snapshot.volume))?;
            price_volume_sum = price_volume_sum.checked_add(weighted)?;
            total_volume = total_volume.checked_add(snapshot.volume)
                .ok_or(ProgramError::InvalidAccountData)?;
        }

        if total_volume == 0 {
            Ok((U64F64::from_num(500_000), 0)) // 0.5 * 1e6
        } else {
            let vwap = price_volume_sum
                .checked_div(U64F64::from_num(total_volume))?;
            Ok((vwap, total_volume))
        }
    }

    /// Calculate historical volatility
    pub fn calculate_volatility(
        &self,
        market_id: &Pubkey,
        window_slots: u64,
    ) -> Result<U64F64, ProgramError> {
        let history = self.price_history.get(market_id)
            .ok_or(BettingPlatformError::NoPriceHistory)?;

        if history.len() < 2 {
            return Ok(U64F64::from_num(0));
        }

        let current_slot = Clock::get()?.slot;
        let cutoff_slot = current_slot.saturating_sub(window_slots);

        let mut returns = Vec::new();
        let mut prev_price = None;

        for snapshot in history.iter() {
            if snapshot.slot < cutoff_slot {
                continue;
            }

            if let Some(prev) = prev_price {
                if prev > U64F64::from_num(0) {
                    let return_val = snapshot.price.checked_div(prev)?
                        .checked_sub(U64F64::from_num(1))?;
                    returns.push(return_val);
                }
            }
            prev_price = Some(snapshot.price);
        }

        if returns.is_empty() {
            return Ok(U64F64::from_num(0));
        }

        // Calculate mean return
        let mut sum = U64F64::from_num(0);
        for &ret in &returns {
            sum = sum.checked_add(ret)?;
        }
        let mean = sum.checked_div(U64F64::from_num(returns.len() as u64))?;

        // Calculate variance
        let mut variance_sum = U64F64::from_num(0);
        for &ret in &returns {
            let diff = if ret > mean {
                ret.checked_sub(mean)?
            } else {
                mean.checked_sub(ret)?
            };
            let squared = diff.checked_mul(diff)?;
            variance_sum = variance_sum.checked_add(squared)?;
        }

        let variance = variance_sum.checked_div(U64F64::from_num(returns.len() as u64))?;
        
        // Return standard deviation (sqrt of variance)
        variance.sqrt()
    }
}

/// Advanced probability adjustment based on market conditions
pub struct ProbabilityAdjuster {
    pub momentum_weight: U64F64,
    pub mean_reversion_weight: U64F64,
}

impl Default for ProbabilityAdjuster {
    fn default() -> Self {
        Self {
            momentum_weight: U64F64::from_num(300_000), // 0.3 * 1e6
            mean_reversion_weight: U64F64::from_num(700_000), // 0.7 * 1e6
        }
    }
}

impl ProbabilityAdjuster {
    /// Adjust probability based on momentum and mean reversion
    pub fn adjust_probability(
        &self,
        base_probability: U64F64,
        momentum: U64F64,
        long_term_average: U64F64,
    ) -> Result<U64F64, ProgramError> {
        // Momentum component
        let momentum_adjustment = base_probability
            .checked_add(momentum.checked_mul(self.momentum_weight)?)?;

        // Mean reversion component
        let mean_reversion = long_term_average
            .checked_sub(base_probability)?
            .checked_mul(self.mean_reversion_weight)?;

        let adjusted = momentum_adjustment
            .checked_add(mean_reversion)?
            .max(U64F64::from_num(10_000))  // Min 1% (0.01 * 1e6)
            .min(U64F64::from_num(990_000)); // Max 99% (0.99 * 1e6)

        Ok(adjusted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Clock;

    #[test]
    fn test_probability_derivation() {
        let engine = DerivationEngine::default();

        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 1,
            synthetic_type: crate::synthetics::SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(1) / U64F64::from_num(2), U64F64::from_num(1) / U64F64::from_num(2)], // 0.5, 0.5
            derived_probability: U64F64::from_num(1) / U64F64::from_num(2), // 0.5
            total_volume_7d: 0,
            last_update_slot: 0,
            status: crate::synthetics::WrapperStatus::Active,
            is_verse_level: true,
            bump: 0,
        };

        let market_data = vec![
            MarketData {
                market_id: wrapper.polymarket_markets[0],
                probability: U64F64::from_num(3) / U64F64::from_num(5), // 0.6
                volume_7d: 100_000,
                liquidity_depth: 50_000,
                last_trade_time: 0,
                category: "Politics".to_string(),
                title: "Test Market 1".to_string(),
                yes_price: 6000,
                volume_24h: 15_000,
                liquidity: 50_000,
                created_at: 0,
            },
            MarketData {
                market_id: wrapper.polymarket_markets[1],
                probability: U64F64::from_num(7) / U64F64::from_num(10), // 0.7
                volume_7d: 150_000,
                liquidity_depth: 75_000,
                last_trade_time: -86400, // 1 day ago
                category: "Politics".to_string(),
                title: "Test Market 2".to_string(),
                yes_price: 7000,
                volume_24h: 25_000,
                liquidity: 75_000,
                created_at: -86400,
            },
        ];

        let derived_prob = engine.derive_synthetic_probability(&wrapper, market_data).unwrap();

        // Should be weighted towards second market due to higher volume/liquidity
        assert!(derived_prob > U64F64::from_num(65) / U64F64::from_num(100)); // > 0.65
        assert!(derived_prob < U64F64::from_num(7) / U64F64::from_num(10)); // < 0.7
    }

    #[test]
    fn test_divergence_calculation() {
        let engine = DerivationEngine::default();

        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 1,
            synthetic_type: crate::synthetics::SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(1) / U64F64::from_num(2), U64F64::from_num(1) / U64F64::from_num(2)], // 0.5, 0.5
            derived_probability: U64F64::from_num(1) / U64F64::from_num(2), // 0.5
            total_volume_7d: 0,
            last_update_slot: 0,
            status: crate::synthetics::WrapperStatus::Active,
            is_verse_level: true,
            bump: 0,
        };

        let market_data = vec![
            MarketData {
                market_id: wrapper.polymarket_markets[0],
                probability: U64F64::from_num(1) / U64F64::from_num(2), // 0.5
                volume_7d: 100_000,
                liquidity_depth: 50_000,
                last_trade_time: 0,
                category: "Politics".to_string(),
                title: "Test Market 1".to_string(),
                yes_price: 5000,
                volume_24h: 15_000,
                liquidity: 50_000,
                created_at: 0,
            },
            MarketData {
                market_id: wrapper.polymarket_markets[1],
                probability: U64F64::from_num(4) / U64F64::from_num(5), // 0.8 - Large divergence
                volume_7d: 100_000,
                liquidity_depth: 50_000,
                last_trade_time: 0,
                category: "Politics".to_string(),
                title: "Test Market 2".to_string(),
                yes_price: 8000,
                volume_24h: 15_000,
                liquidity: 50_000,
                created_at: 0,
            },
        ];

        let divergences = engine.calculate_divergence(&wrapper, &market_data).unwrap();

        // Should detect divergence in second market
        assert!(divergences.len() > 0);
        assert!(divergences[0].1 > U64F64::from_num(5) / U64F64::from_num(100)); // > 0.05
    }

    #[test]
    fn test_vwap_calculation() {
        let mut deriver = ProbabilityDeriver::default();
        let market_id = Pubkey::new_unique();

        // Add price snapshots
        for i in 0..10 {
            deriver.add_price_snapshot(
                market_id,
                PriceSnapshot {
                    slot: 1000 + i * 100,
                    price: U64F64::from_num(1) / U64F64::from_num(2) + U64F64::from_num(i) / U64F64::from_num(100), // 0.5 + i * 0.01
                    volume: 1000 * (i + 1),
                    liquidity: 50_000,
                    timestamp: i as i64 * 3600,
                },
            );
        }

        let history = deriver.price_history.get(&market_id).unwrap();
        let (vwap, total_volume) = deriver.calculate_vwap(history, 0).unwrap();

        // VWAP should be weighted towards later prices due to higher volume
        assert!(vwap > U64F64::from_num(1) / U64F64::from_num(2)); // > 0.5
        assert_eq!(total_volume, 55_000); // Sum of 1000 * (1+2+...+10)
    }
}