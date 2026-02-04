use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::error::BettingPlatformError;
use crate::math::U64F64;
use crate::synthetics::{SyntheticWrapper, DerivationEngine, MarketData};

/// Arbitrage detector for identifying profit opportunities
pub struct ArbitrageDetector {
    pub min_profit_threshold: U64F64, // Minimum profit to trigger
    pub max_size_per_arb: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub synthetic_id: u128,
    pub market_id: Pubkey,
    pub direction: ArbDirection,
    pub price_diff: U64F64,
    pub potential_profit: u64,
    pub recommended_size: u64,
    pub confidence_score: u8, // 0-100
    pub timestamp: i64,
    pub expected_profit_bps: u16,
    pub synthetic_price: U64F64,
    pub market_price: U64F64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum ArbDirection {
    BuySyntheticSellMarket,
    BuyMarketSellSynthetic,
}

impl Default for ArbitrageDetector {
    fn default() -> Self {
        Self {
            min_profit_threshold: U64F64::from_num(90_000), // 9% minimum edge from spec
            max_size_per_arb: 10_000,
        }
    }
}

impl ArbitrageDetector {
    pub fn new(min_profit_threshold: U64F64, max_size_per_arb: u64) -> Self {
        Self {
            min_profit_threshold,
            max_size_per_arb,
        }
    }

    /// Detect arbitrage opportunities with dynamic threshold
    pub fn detect_opportunities(
        &self,
        wrapper: &SyntheticWrapper,
        market_data: &Vec<MarketData>,
        derivation_engine: &DerivationEngine,
        clock: &Clock,
    ) -> Result<Vec<ArbitrageOpportunity>, ProgramError> {
        let synthetic_prob = derivation_engine
            .derive_synthetic_probability(wrapper, market_data.clone())?;
        let synthetic_price = synthetic_prob;

        let mut opportunities = Vec::new();

        for data in market_data {
            let diff = if data.probability > synthetic_prob {
                data.probability.checked_sub(synthetic_prob)?
            } else {
                synthetic_prob.checked_sub(data.probability)?
            };

            // Dynamic threshold: 9% for verse vs child, lower for other opportunities
            let threshold = if wrapper.is_verse_level {
                U64F64::from_num(90_000) // 9% edge for verse avg 55% vs child 60%
            } else {
                self.min_profit_threshold
            };

            if diff > threshold {
                let direction = if data.probability > synthetic_prob {
                    ArbDirection::BuyMarketSellSynthetic
                } else {
                    ArbDirection::BuySyntheticSellMarket
                };

                let potential_profit = self.calculate_profit(
                    diff,
                    data.liquidity_depth,
                    self.max_size_per_arb,
                )?;

                let recommended_size = self.calculate_optimal_size(
                    diff,
                    data.liquidity_depth,
                )?;

                let confidence_score = self.calculate_confidence(
                    diff,
                    data.volume_7d,
                    data.liquidity_depth,
                )?;

                opportunities.push(ArbitrageOpportunity {
                    synthetic_id: wrapper.synthetic_id,
                    market_id: data.market_id,
                    direction,
                    price_diff: diff,
                    potential_profit,
                    recommended_size,
                    confidence_score,
                    timestamp: clock.unix_timestamp,
                    expected_profit_bps: (potential_profit * 10000 / recommended_size) as u16,
                    synthetic_price,
                    market_price: data.probability,
                });
            }
        }

        // Sort by potential profit (highest first)
        opportunities.sort_by(|a, b| b.potential_profit.cmp(&a.potential_profit));

        Ok(opportunities)
    }

    /// Calculate potential profit from arbitrage
    fn calculate_profit(
        &self,
        price_diff: U64F64,
        liquidity: u64,
        max_size: u64,
    ) -> Result<u64, ProgramError> {
        // Size limited by liquidity and max size
        let effective_size = liquidity.min(max_size);

        // Profit = size * price_diff * (1 - fees)
        let gross_profit = U64F64::from_num(effective_size)
            .checked_mul(price_diff)?;

        // Account for fees (0.3% round trip)
        let fee_factor = U64F64::from_num(3_000); // 0.3% (0.003 * 1e6)
        let fees = U64F64::from_num(effective_size)
            .checked_mul(fee_factor)?;

        let net_profit = gross_profit
            .checked_sub(fees)?
            .to_num();

        Ok(net_profit)
    }

    /// Calculate optimal trade size
    fn calculate_optimal_size(
        &self,
        price_diff: U64F64,
        liquidity: u64,
    ) -> Result<u64, ProgramError> {
        // Optimal size = min(sqrt(liquidity * price_diff * constant), max_size)
        // This balances profit vs market impact

        let constant = U64F64::from_num(1000000); // Tuning parameter
        
        let optimal = U64F64::from_num(liquidity)
            .checked_mul(price_diff)?
            .checked_mul(constant)?
            .sqrt()?
            .to_num();

        Ok(optimal.min(self.max_size_per_arb))
    }

    /// Calculate confidence score for opportunity
    fn calculate_confidence(
        &self,
        price_diff: U64F64,
        volume_7d: u64,
        liquidity: u64,
    ) -> Result<u8, ProgramError> {
        // Factors: price difference, volume, liquidity
        let price_score = (price_diff.to_num() as f64 * 100.0).min(40.0);
        
        let volume_score = if volume_7d > 1_000_000 {
            30.0
        } else if volume_7d > 100_000 {
            20.0
        } else if volume_7d > 10_000 {
            10.0
        } else {
            5.0
        };

        let liquidity_score = if liquidity > 500_000 {
            30.0
        } else if liquidity > 100_000 {
            20.0
        } else if liquidity > 20_000 {
            10.0
        } else {
            5.0
        };

        let total_score = (price_score + volume_score + liquidity_score).min(100.0) as u8;
        Ok(total_score)
    }
}

/// Advanced arbitrage strategies
pub struct ArbitrageStrategy {
    pub detector: ArbitrageDetector,
    pub risk_parameters: RiskParameters,
}

#[derive(Debug, Clone)]
pub struct RiskParameters {
    pub max_exposure: u64,          // Maximum total exposure
    pub max_correlation: U64F64,    // Maximum correlation between positions
    pub min_liquidity_ratio: U64F64, // Min liquidity/trade size ratio
    pub max_positions: u8,          // Maximum concurrent positions
}

impl Default for RiskParameters {
    fn default() -> Self {
        Self {
            max_exposure: 100_000,
            max_correlation: U64F64::from_num(700_000), // 0.7 * 1e6
            min_liquidity_ratio: U64F64::from_num(10),
            max_positions: 5,
        }
    }
}

impl ArbitrageStrategy {
    pub fn new(detector: ArbitrageDetector, risk_parameters: RiskParameters) -> Self {
        Self {
            detector,
            risk_parameters,
        }
    }

    /// Filter opportunities based on risk parameters
    pub fn filter_opportunities(
        &self,
        opportunities: Vec<ArbitrageOpportunity>,
        current_exposure: u64,
    ) -> Vec<ArbitrageOpportunity> {
        opportunities
            .into_iter()
            .filter(|opp| {
                // Check exposure limit
                if current_exposure + opp.recommended_size > self.risk_parameters.max_exposure {
                    return false;
                }

                // Check confidence threshold
                if opp.confidence_score < 60 {
                    return false;
                }

                true
            })
            .take(self.risk_parameters.max_positions as usize)
            .collect()
    }

    /// Calculate optimal portfolio of arbitrage trades
    pub fn optimize_portfolio(
        &self,
        opportunities: Vec<ArbitrageOpportunity>,
        available_capital: u64,
    ) -> Result<Vec<(ArbitrageOpportunity, u64)>, ProgramError> {
        let mut portfolio = Vec::new();
        let mut remaining_capital = available_capital;
        let mut total_exposure = 0u64;

        // Sort by profit/size ratio (efficiency)
        let mut sorted_opps = opportunities;
        sorted_opps.sort_by(|a, b| {
            let a_efficiency = a.potential_profit.saturating_mul(1000) / a.recommended_size.max(1);
            let b_efficiency = b.potential_profit.saturating_mul(1000) / b.recommended_size.max(1);
            b_efficiency.cmp(&a_efficiency)
        });

        for opp in sorted_opps {
            if total_exposure >= self.risk_parameters.max_exposure {
                break;
            }

            let allocation = opp.recommended_size
                .min(remaining_capital)
                .min(self.risk_parameters.max_exposure - total_exposure);

            if allocation > 0 {
                portfolio.push((opp, allocation));
                remaining_capital = remaining_capital.saturating_sub(allocation);
                total_exposure += allocation;
            }

            if portfolio.len() >= self.risk_parameters.max_positions as usize {
                break;
            }
        }

        Ok(portfolio)
    }
}

/// Arbitrage execution tracker
pub struct ArbitrageTracker {
    pub active_positions: Vec<ActiveArbitrage>,
    pub completed_trades: Vec<CompletedArbitrage>,
    pub total_profit: i64,
    pub total_volume: u64,
}

#[derive(Debug, Clone)]
pub struct ActiveArbitrage {
    pub opportunity: ArbitrageOpportunity,
    pub entry_price: U64F64,
    pub entry_time: i64,
    pub size: u64,
    pub unrealized_pnl: i64,
}

#[derive(Debug, Clone)]
pub struct CompletedArbitrage {
    pub opportunity: ArbitrageOpportunity,
    pub entry_price: U64F64,
    pub exit_price: U64F64,
    pub size: u64,
    pub profit: i64,
    pub duration: i64,
}

impl ArbitrageTracker {
    pub fn new() -> Self {
        Self {
            active_positions: Vec::new(),
            completed_trades: Vec::new(),
            total_profit: 0,
            total_volume: 0,
        }
    }

    /// Track new arbitrage position
    pub fn open_position(
        &mut self,
        opportunity: ArbitrageOpportunity,
        entry_price: U64F64,
        size: u64,
    ) -> ProgramResult {
        let active = ActiveArbitrage {
            opportunity,
            entry_price,
            entry_time: Clock::get()?.unix_timestamp,
            size,
            unrealized_pnl: 0,
        };

        self.active_positions.push(active);
        self.total_volume += size;

        Ok(())
    }

    /// Close arbitrage position
    pub fn close_position(
        &mut self,
        market_id: &Pubkey,
        exit_price: U64F64,
    ) -> Result<i64, ProgramError> {
        let pos_index = self.active_positions
            .iter()
            .position(|p| p.opportunity.market_id == *market_id)
            .ok_or(ProgramError::InvalidAccountData)?;

        let position = self.active_positions.remove(pos_index);

        let profit = match position.opportunity.direction {
            ArbDirection::BuySyntheticSellMarket => {
                // Bought synthetic low, selling market high
                let price_diff = exit_price.checked_sub(position.entry_price)?;
                (U64F64::from_num(position.size).checked_mul(price_diff)?.to_num() as i64)
            }
            ArbDirection::BuyMarketSellSynthetic => {
                // Bought market low, selling synthetic high
                let price_diff = position.entry_price.checked_sub(exit_price)?;
                (U64F64::from_num(position.size).checked_mul(price_diff)?.to_num() as i64)
            }
        };

        let completed = CompletedArbitrage {
            opportunity: position.opportunity,
            entry_price: position.entry_price,
            exit_price,
            size: position.size,
            profit,
            duration: Clock::get()?.unix_timestamp - position.entry_time,
        };

        self.completed_trades.push(completed);
        self.total_profit += profit;

        Ok(profit)
    }

    /// Calculate current portfolio metrics
    pub fn calculate_metrics(&self) -> ArbitrageMetrics {
        let active_exposure: u64 = self.active_positions
            .iter()
            .map(|p| p.size)
            .sum();

        let win_rate = if self.completed_trades.is_empty() {
            0.0
        } else {
            let wins = self.completed_trades
                .iter()
                .filter(|t| t.profit > 0)
                .count();
            (wins as f64 / self.completed_trades.len() as f64) * 100.0
        };

        let avg_profit = if self.completed_trades.is_empty() {
            0
        } else {
            self.total_profit / self.completed_trades.len() as i64
        };

        ArbitrageMetrics {
            active_positions: self.active_positions.len() as u8,
            completed_trades: self.completed_trades.len() as u64,
            total_profit: self.total_profit,
            total_volume: self.total_volume,
            active_exposure,
            win_rate,
            avg_profit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageMetrics {
    pub active_positions: u8,
    pub completed_trades: u64,
    pub total_profit: i64,
    pub total_volume: u64,
    pub active_exposure: u64,
    pub win_rate: f64,
    pub avg_profit: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetics::{SyntheticType, WrapperStatus};

    #[test]
    fn test_arbitrage_detection() {
        let detector = ArbitrageDetector::default();
        let derivation_engine = DerivationEngine::default();

        let wrapper = SyntheticWrapper {
            is_initialized: true,
            synthetic_id: 1,
            synthetic_type: SyntheticType::Verse,
            polymarket_markets: vec![Pubkey::new_unique(), Pubkey::new_unique()],
            weights: vec![U64F64::from_num(1) / U64F64::from_num(2), U64F64::from_num(1) / U64F64::from_num(2)], // 0.5, 0.5
            derived_probability: U64F64::from_num(1) / U64F64::from_num(2), // 0.5
            total_volume_7d: 0,
            last_update_slot: 0,
            status: WrapperStatus::Active,
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
                probability: U64F64::from_num(4) / U64F64::from_num(5), // 0.8 - Large divergence
                volume_7d: 150_000,
                liquidity_depth: 75_000,
                last_trade_time: 0,
                category: "Politics".to_string(),
                title: "Test Market 2".to_string(),
                yes_price: 8000,
                volume_24h: 25_000,
                liquidity: 75_000,
                created_at: 0,
            },
        ];

        let clock = Clock {
            slot: 0,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: 1234567890,
        };
        
        let opportunities = detector.detect_opportunities(
            &wrapper,
            &market_data,
            &derivation_engine,
            &clock,
        ).unwrap();

        assert!(!opportunities.is_empty());
        assert!(opportunities[0].price_diff > U64F64::from_num(5) / U64F64::from_num(100)); // > 0.05
    }

    #[test]
    fn test_profit_calculation() {
        let detector = ArbitrageDetector::default();

        let profit = detector.calculate_profit(
            U64F64::from_num(5) / U64F64::from_num(100), // 0.05 - 5% price difference
            50_000,                  // Liquidity
            10_000,                  // Max size
        ).unwrap();

        // Profit should be approximately 5% * 10,000 - fees
        assert!(profit > 450); // ~5% minus fees
        assert!(profit < 500); // Not more than gross profit
    }

    #[test]
    fn test_portfolio_optimization() {
        let detector = ArbitrageDetector::default();
        let risk_params = RiskParameters::default();
        let strategy = ArbitrageStrategy::new(detector, risk_params);

        let opportunities = vec![
            ArbitrageOpportunity {
                synthetic_id: 1,
                market_id: Pubkey::new_unique(),
                direction: ArbDirection::BuySyntheticSellMarket,
                price_diff: U64F64::from_num(50_000), // 5% as fixed point
                potential_profit: 500,
                recommended_size: 10_000,
                confidence_score: 80,
                timestamp: 1234567890,
                expected_profit_bps: 50, // 0.5%
                synthetic_price: U64F64::from_num(550_000), // 55%
                market_price: U64F64::from_num(500_000), // 50%
            },
            ArbitrageOpportunity {
                synthetic_id: 2,
                market_id: Pubkey::new_unique(),
                direction: ArbDirection::BuyMarketSellSynthetic,
                price_diff: U64F64::from_num(30_000), // 3% as fixed point
                potential_profit: 300,
                recommended_size: 10_000,
                confidence_score: 70,
                timestamp: 1234567890,
                expected_profit_bps: 30, // 0.3%
                synthetic_price: U64F64::from_num(470_000), // 47%
                market_price: U64F64::from_num(500_000), // 50%
            },
        ];

        let portfolio = strategy.optimize_portfolio(
            opportunities,
            50_000, // Available capital
        ).unwrap();

        assert_eq!(portfolio.len(), 2);
        assert_eq!(portfolio[0].1, 10_000); // Full allocation to best opportunity
    }

    #[test]
    fn test_arbitrage_tracking() {
        let mut tracker = ArbitrageTracker::new();
        let market_id = Pubkey::new_unique();

        let opportunity = ArbitrageOpportunity {
            synthetic_id: 1,
            market_id,
            direction: ArbDirection::BuySyntheticSellMarket,
            price_diff: U64F64::from_num(50_000), // 5% as fixed point
            potential_profit: 500,
            recommended_size: 10_000,
            confidence_score: 80,
            timestamp: 1234567890,
            expected_profit_bps: 50, // 0.5%
            synthetic_price: U64F64::from_num(550_000), // 55%
            market_price: U64F64::from_num(500_000), // 50%
        };

        // Open position
        tracker.open_position(
            opportunity,
            U64F64::from_num(500_000), // 50%
            10_000,
        ).unwrap();

        assert_eq!(tracker.active_positions.len(), 1);
        assert_eq!(tracker.total_volume, 10_000);

        // Close with profit
        let profit = tracker.close_position(
            &market_id,
            U64F64::from_num(55) / U64F64::from_num(100), // 0.55
        ).unwrap();

        assert_eq!(profit, 500); // 5% profit on 10,000
        assert_eq!(tracker.completed_trades.len(), 1);
        assert_eq!(tracker.total_profit, 500);
    }
}