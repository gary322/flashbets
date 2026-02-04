use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use crate::fixed_types::{U64F64, I64F64};
use std::collections::VecDeque;
use crate::errors::*;
use crate::state::*;

#[account]
pub struct AttackDetector {
    /// Detector identifier
    pub detector_id: [u8; 32],
    /// Current attack risk level (0-100)
    pub risk_level: u8,
    /// Attack patterns detected
    pub detected_patterns: Vec<AttackPattern>,
    /// Recent trade history for analysis
    pub recent_trades: VecDeque<TradeSnapshot>,
    /// Price movement tracking
    pub price_tracker: PriceMovementTracker,
    /// Volume anomaly detector
    pub volume_detector: VolumeAnomalyDetector,
    /// Flash loan detection
    pub flash_loan_detector: FlashLoanDetector,
    /// Cross-verse correlation tracker
    pub correlation_tracker: CrossVerseTracker,
    /// Wash trading detector
    pub wash_trade_detector: WashTradeDetector,
    /// Last update slot
    pub last_update_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AttackPattern {
    pub pattern_type: AttackType,
    pub severity: AttackSeverity,
    pub first_detected: u64,
    pub occurrences: u64,
    pub affected_markets: Vec<[u8; 32]>,
    pub estimated_impact: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum AttackType {
    /// Price manipulation through large orders
    PriceManipulation,
    /// Flash loan attack attempt
    FlashLoan,
    /// Wash trading to manipulate volume
    WashTrading,
    /// Cross-verse correlation attack
    CrossVerseManipulation,
    /// Sandwich attack on user trades
    SandwichAttack,
    /// Oracle manipulation attempt
    OracleManipulation,
    /// Cascading liquidation attack
    LiquidationCascade,
    /// High-frequency manipulation
    HighFrequencyManipulation,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum AttackSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TradeSnapshot {
    pub trader: Pubkey,
    pub market_id: [u8; 32],
    pub size: u64,
    pub price: U64F64,
    pub leverage: u64,
    pub slot: u64,
    pub is_buy: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct PriceMovementTracker {
    /// Price changes per slot (circular buffer)
    pub price_changes: VecDeque<PriceChange>,
    /// Maximum allowed change per slot (2% from CLAUDE.md)
    pub max_change_per_slot: U64F64,
    /// Cumulative change over window
    pub cumulative_change: U64F64,
    /// Slots with excessive movement
    pub violation_count: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct PriceChange {
    pub slot: u64,
    pub market_id: [u8; 32],
    pub old_price: U64F64,
    pub new_price: U64F64,
    pub change_percent: U64F64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct VolumeAnomalyDetector {
    /// Historical volume averages
    pub volume_history: VecDeque<VolumeData>,
    /// Current period volume
    pub current_volume: u64,
    /// Average volume (7-day rolling)
    pub avg_volume_7d: u64,
    /// Standard deviation
    pub volume_std_dev: U64F64,
    /// Anomaly threshold (e.g., 3 std devs)
    pub anomaly_threshold: U64F64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct VolumeData {
    pub slot: u64,
    pub volume: u64,
    pub unique_traders: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct FlashLoanDetector {
    /// Tracks large position changes within same slot
    pub slot_positions: Vec<PositionChange>,
    /// Minimum size to track (e.g., 10% of vault)
    pub min_track_size: u64,
    /// Detected flash loan attempts
    pub detected_attempts: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PositionChange {
    pub trader: Pubkey,
    pub slot: u64,
    pub open_size: u64,
    pub close_size: u64,
    pub profit_loss: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CrossVerseTracker {
    /// Tracks correlations between supposedly unrelated verses
    pub correlation_matrix: Vec<VerseCorrelation>,
    /// Threshold for suspicious correlation
    pub suspicion_threshold: U64F64,
    /// Recent cross-verse trades
    pub cross_trades: VecDeque<CrossVerseTrade>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct VerseCorrelation {
    pub verse_a: [u8; 32],
    pub verse_b: [u8; 32],
    pub correlation: U64F64,
    pub trade_count: u64,
    pub last_update: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrossVerseTrade {
    pub trader: Pubkey,
    pub verse_trades: Vec<([u8; 32], u64, bool)>, // (verse_id, size, is_buy)
    pub slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct WashTradeDetector {
    /// Tracks same-trader opposite trades
    pub trader_activity: Vec<TraderActivity>,
    /// Minimum time between trades to not be wash
    pub min_time_between: u64, // slots
    /// Detected wash trades
    pub wash_trades_detected: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TraderActivity {
    pub trader: Pubkey,
    pub market_id: [u8; 32],
    pub buy_volume: u64,
    pub sell_volume: u64,
    pub net_position: i64,
    pub trade_count: u32,
    pub first_trade_slot: u64,
    pub last_trade_slot: u64,
}

impl AttackDetector {
    pub const LEN: usize = 8 + // discriminator
        32 + // detector_id
        1 + // risk_level
        4 + 64 * 10 + // detected_patterns (max 10)
        4 + 96 * 100 + // recent_trades (max 100)
        256 + // price_tracker
        256 + // volume_detector
        256 + // flash_loan_detector
        512 + // correlation_tracker
        512 + // wash_trade_detector
        8; // last_update_slot

    /// Initialize new attack detector
    pub fn init(&mut self, clock: &Clock) -> Result<()> {
        self.detector_id = Pubkey::new_unique().to_bytes();
        self.risk_level = 0;
        self.detected_patterns = Vec::new();
        self.recent_trades = VecDeque::with_capacity(100);

        self.price_tracker.max_change_per_slot = U64F64::from_num(0.02); // 2%
        self.volume_detector.anomaly_threshold = U64F64::from_num(3.0); // 3 std devs
        self.flash_loan_detector.min_track_size = 0; // Set based on vault size
        self.correlation_tracker.suspicion_threshold = U64F64::from_num(0.8); // 80% correlation
        self.wash_trade_detector.min_time_between = 10; // 10 slots

        self.last_update_slot = clock.slot;
        Ok(())
    }

    /// Process new trade for attack detection
    pub fn process_trade(
        &mut self,
        trade: &TradeSnapshot,
        vault_size: u64,
        clock: &Clock,
    ) -> Result<Vec<SecurityAlert>> {
        let mut alerts = Vec::new();

        // Update recent trades
        self.recent_trades.push_back(trade.clone());
        if self.recent_trades.len() > 100 {
            self.recent_trades.pop_front();
        }

        // Check price manipulation
        if let Some(alert) = self.check_price_manipulation(trade, clock)? {
            alerts.push(alert);
        }

        // Check volume anomalies
        if let Some(alert) = self.check_volume_anomaly(trade)? {
            alerts.push(alert);
        }

        // Check flash loan
        if let Some(alert) = self.check_flash_loan(trade, vault_size, clock)? {
            alerts.push(alert);
        }

        // Check wash trading
        if let Some(alert) = self.check_wash_trading(trade, clock)? {
            alerts.push(alert);
        }

        // Update risk level
        self.update_risk_level(&alerts);

        self.last_update_slot = clock.slot;

        Ok(alerts)
    }

    /// Check for price manipulation
    fn check_price_manipulation(
        &mut self,
        trade: &TradeSnapshot,
        clock: &Clock,
    ) -> Result<Option<SecurityAlert>> {
        // Calculate price change
        let last_price = self.get_last_price(trade.market_id);
        if let Some(last) = last_price {
            let change = if trade.price > last {
                (trade.price - last) / last
            } else {
                (last - trade.price) / last
            };

            // Record price change
            self.price_tracker.price_changes.push_back(PriceChange {
                slot: clock.slot,
                market_id: trade.market_id,
                old_price: last,
                new_price: trade.price,
                change_percent: change,
            });

            // Keep only recent changes (last 100 slots)
            while self.price_tracker.price_changes.len() > 100 {
                self.price_tracker.price_changes.pop_front();
            }

            // Check if exceeds 2% per slot limit
            if change > self.price_tracker.max_change_per_slot {
                self.price_tracker.violation_count += 1;

                return Ok(Some(SecurityAlert {
                    alert_type: AlertType::PriceManipulation,
                    severity: AttackSeverity::High,
                    message: format!("Price change {}% exceeds 2% limit",
                        (change * U64F64::from_num(100)).to_num::<u16>()),
                    action: SecurityAction::ClampPrice,
                    data: AlertData::PriceData {
                        old_price: last,
                        new_price: trade.price,
                        max_allowed: last * (U64F64::one() + self.price_tracker.max_change_per_slot),
                    },
                }));
            }

            // Check cumulative change over 4 slots (5% limit from CLAUDE.md)
            let recent_changes: U64F64 = self.price_tracker.price_changes
                .iter()
                .rev()
                .take(4)
                .filter(|pc| pc.market_id == trade.market_id)
                .map(|pc| pc.change_percent)
                .sum();

            if recent_changes > U64F64::from_num(0.05) {
                return Ok(Some(SecurityAlert {
                    alert_type: AlertType::PriceManipulation,
                    severity: AttackSeverity::Critical,
                    message: format!("Cumulative price change {}% over 4 slots exceeds 5% limit",
                        (recent_changes * U64F64::from_num(100)).to_num::<u16>()),
                    action: SecurityAction::HaltTrading,
                    data: AlertData::CumulativeChange(recent_changes),
                }));
            }
        }

        Ok(None)
    }

    /// Check for volume anomalies
    fn check_volume_anomaly(&mut self, trade: &TradeSnapshot) -> Result<Option<SecurityAlert>> {
        self.volume_detector.current_volume += trade.size;

        if self.volume_detector.avg_volume_7d > 0 {
            let volume_ratio = U64F64::from_num(self.volume_detector.current_volume) /
                              U64F64::from_num(self.volume_detector.avg_volume_7d);

            // Check if volume exceeds threshold (e.g., 3x average)
            if volume_ratio > self.volume_detector.anomaly_threshold {
                return Ok(Some(SecurityAlert {
                    alert_type: AlertType::VolumeAnomaly,
                    severity: AttackSeverity::Medium,
                    message: format!("Volume {}x above 7-day average", volume_ratio.to_num::<u8>()),
                    action: SecurityAction::IncreaseMonitoring,
                    data: AlertData::VolumeData {
                        current: self.volume_detector.current_volume,
                        average: self.volume_detector.avg_volume_7d,
                    },
                }));
            }
        }

        Ok(None)
    }

    /// Check for flash loan attacks
    fn check_flash_loan(
        &mut self,
        trade: &TradeSnapshot,
        vault_size: u64,
        clock: &Clock,
    ) -> Result<Option<SecurityAlert>> {
        // Track large positions relative to vault
        let size_ratio = U64F64::from_num(trade.size) / U64F64::from_num(vault_size);

        if size_ratio > U64F64::from_num(0.1) { // >10% of vault
            // Check if same trader has opposite position in same slot
            let same_slot_trades: Vec<_> = self.recent_trades
                .iter()
                .filter(|t| t.trader == trade.trader && t.slot == clock.slot)
                .collect();

            if same_slot_trades.len() > 1 {
                // Check for opposite trades
                let has_opposite = same_slot_trades.iter()
                    .any(|t| t.is_buy != trade.is_buy && t.market_id == trade.market_id);

                if has_opposite {
                    self.flash_loan_detector.detected_attempts += 1;

                    return Ok(Some(SecurityAlert {
                        alert_type: AlertType::FlashLoan,
                        severity: AttackSeverity::Critical,
                        message: "Flash loan attack detected - opposite trades in same slot".to_string(),
                        action: SecurityAction::RevertTrades,
                        data: AlertData::FlashLoanData {
                            trader: trade.trader,
                            slot: clock.slot,
                            size: trade.size,
                        },
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Check for wash trading
    fn check_wash_trading(
        &mut self,
        trade: &TradeSnapshot,
        clock: &Clock,
    ) -> Result<Option<SecurityAlert>> {
        // Find or create trader activity
        let activity = self.wash_trade_detector.trader_activity
            .iter_mut()
            .find(|a| a.trader == trade.trader && a.market_id == trade.market_id);

        if let Some(activity) = activity {
            // Check time between trades
            if clock.slot - activity.last_trade_slot < self.wash_trade_detector.min_time_between {
                // Check for opposite trade
                let is_wash = (trade.is_buy && activity.sell_volume > 0) ||
                             (!trade.is_buy && activity.buy_volume > 0);

                if is_wash {
                    self.wash_trade_detector.wash_trades_detected += 1;

                    return Ok(Some(SecurityAlert {
                        alert_type: AlertType::WashTrading,
                        severity: AttackSeverity::High,
                        message: "Wash trading detected - opposite trades too close".to_string(),
                        action: SecurityAction::PenalizeFees,
                        data: AlertData::WashTradeData {
                            trader: trade.trader,
                            volume: trade.size,
                            time_between: clock.slot - activity.last_trade_slot,
                        },
                    }));
                }
            }

            // Update activity
            if trade.is_buy {
                activity.buy_volume += trade.size;
            } else {
                activity.sell_volume += trade.size;
            }
            activity.last_trade_slot = clock.slot;
            activity.trade_count += 1;
        } else {
            // Create new activity
            self.wash_trade_detector.trader_activity.push(TraderActivity {
                trader: trade.trader,
                market_id: trade.market_id,
                buy_volume: if trade.is_buy { trade.size } else { 0 },
                sell_volume: if !trade.is_buy { trade.size } else { 0 },
                net_position: if trade.is_buy {
                    trade.size as i64
                } else {
                    -(trade.size as i64)
                },
                trade_count: 1,
                first_trade_slot: clock.slot,
                last_trade_slot: clock.slot,
            });
        }

        Ok(None)
    }

    /// Get last price for market
    fn get_last_price(&self, market_id: [u8; 32]) -> Option<U64F64> {
        self.recent_trades
            .iter()
            .rev()
            .find(|t| t.market_id == market_id)
            .map(|t| t.price)
    }

    /// Update overall risk level
    fn update_risk_level(&mut self, alerts: &[SecurityAlert]) {
        let mut risk_score = 0u8;

        for alert in alerts {
            risk_score += match alert.severity {
                AttackSeverity::Low => 10,
                AttackSeverity::Medium => 25,
                AttackSeverity::High => 50,
                AttackSeverity::Critical => 100,
            };
        }

        // Add base risk from patterns
        risk_score += (self.detected_patterns.len() as u8 * 5).min(30);

        self.risk_level = risk_score.min(100);
    }
}

#[derive(Debug, Clone)]
pub struct SecurityAlert {
    pub alert_type: AlertType,
    pub severity: AttackSeverity,
    pub message: String,
    pub action: SecurityAction,
    pub data: AlertData,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum AlertType {
    PriceManipulation,
    VolumeAnomaly,
    FlashLoan,
    WashTrading,
    CrossVerseManipulation,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum SecurityAction {
    /// Just monitor, no action
    Monitor,
    /// Increase monitoring frequency
    IncreaseMonitoring,
    /// Clamp price to allowed range
    ClampPrice,
    /// Apply penalty fees
    PenalizeFees,
    /// Revert suspicious trades
    RevertTrades,
    /// Halt trading temporarily
    HaltTrading,
}

#[derive(Debug, Clone)]
pub enum AlertData {
    PriceData {
        old_price: U64F64,
        new_price: U64F64,
        max_allowed: U64F64,
    },
    VolumeData {
        current: u64,
        average: u64,
    },
    FlashLoanData {
        trader: Pubkey,
        slot: u64,
        size: u64,
    },
    WashTradeData {
        trader: Pubkey,
        volume: u64,
        time_between: u64,
    },
    CumulativeChange(U64F64),
}