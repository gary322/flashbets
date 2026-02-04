use anchor_lang::prelude::*;
use crate::fixed_types::U64F64;
use std::collections::HashMap;
use crate::errors::*;
use crate::attack_detection::*;

#[account]
pub struct CircuitBreaker {
    /// Circuit breaker identifier
    pub breaker_id: [u8; 32],
    /// Current circuit breaker state
    pub state: BreakerState,
    /// Coverage-based breaker
    pub coverage_breaker: CoverageBreaker,
    /// Price movement breaker
    pub price_breaker: PriceBreaker,
    /// Volume surge breaker
    pub volume_breaker: VolumeBreaker,
    /// Liquidation cascade breaker
    pub liquidation_breaker: LiquidationBreaker,
    /// Network congestion breaker
    pub congestion_breaker: CongestionBreaker,
    /// Last trigger information
    pub last_trigger: Option<BreakerTrigger>,
    /// Total triggers count
    pub total_triggers: u64,
    /// Cooldown period after trigger (slots)
    pub cooldown_period: u64,
    /// Emergency shutdown authority (burned after init)
    pub emergency_authority: Option<Pubkey>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum BreakerState {
    /// Normal operation
    Active,
    /// Temporarily halted
    Halted {
        start_slot: u64,
        expected_resume: u64,
        reason: HaltReason,
    },
    /// In cooldown after halt
    Cooldown {
        end_slot: u64,
    },
    /// Emergency shutdown
    EmergencyShutdown,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum HaltReason {
    /// Coverage dropped below threshold
    LowCoverage,
    /// Excessive price movement
    PriceVolatility,
    /// Volume surge detected
    VolumeSurge,
    /// Cascading liquidations
    LiquidationCascade,
    /// Network congestion
    NetworkCongestion,
    /// Manual emergency
    EmergencyHalt,
    /// Attack detected
    SecurityThreat,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BreakerTrigger {
    pub trigger_type: HaltReason,
    pub trigger_slot: u64,
    pub severity: AttackSeverity,
    pub details: String,
    pub automatic_resume: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CoverageBreaker {
    /// Minimum coverage before halt (0.5 from CLAUDE.md)
    pub min_coverage: U64F64,
    /// Current coverage ratio
    pub current_coverage: U64F64,
    /// Halt duration for low coverage (1 hour = 8640 slots)
    pub halt_duration: u64,
    /// Times triggered
    pub trigger_count: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct PriceBreaker {
    /// Max cumulative change over window (5% over 4 slots from CLAUDE.md)
    pub max_cumulative_change: U64F64,
    /// Window size in slots
    pub window_size: u64,
    /// Current cumulative change
    pub current_change: U64F64,
    /// Halt duration for price volatility
    pub halt_duration: u64,
    /// Recent price changes
    pub recent_changes: Vec<PriceChange>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct VolumeBreaker {
    /// Max volume multiplier vs average
    pub max_volume_multiplier: U64F64,
    /// Current volume in window
    pub current_volume: u64,
    /// Average volume baseline
    pub avg_volume_baseline: u64,
    /// Halt duration for volume surge
    pub halt_duration: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct LiquidationBreaker {
    /// Max liquidations per slot
    pub max_liquidations_per_slot: u64,
    /// Max liquidation volume per slot (% of OI)
    pub max_liquidation_volume_percent: U64F64,
    /// Current slot liquidations
    pub current_liquidations: u64,
    /// Current liquidation volume
    pub current_liquidation_volume: u64,
    /// Halt duration for cascade
    pub halt_duration: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CongestionBreaker {
    /// Max slot time deviation (ms)
    pub max_slot_deviation_ms: u64,
    /// Max failed transactions per slot
    pub max_failed_tx_per_slot: u64,
    /// Current failed transactions
    pub current_failed_tx: u64,
    /// Halt duration for congestion
    pub halt_duration: u64,
}

impl CircuitBreaker {
    pub const LEN: usize = 8 + // discriminator
        32 + // breaker_id
        20 + // state (worst case)
        64 + // coverage_breaker
        128 + // price_breaker
        64 + // volume_breaker
        64 + // liquidation_breaker
        64 + // congestion_breaker
        1 + 64 + // optional last_trigger
        8 + // total_triggers
        8 + // cooldown_period
        1 + 32; // optional emergency_authority

    /// Initialize circuit breaker with CLAUDE.md parameters
    pub fn init(&mut self, clock: &Clock) -> Result<()> {
        self.breaker_id = Pubkey::new_unique().to_bytes();
        self.state = BreakerState::Active;

        // Coverage breaker: halt at <0.5 coverage
        self.coverage_breaker = CoverageBreaker {
            min_coverage: U64F64::from_num(0.5),
            current_coverage: U64F64::zero(),
            halt_duration: 8640, // 1 hour
            trigger_count: 0,
        };

        // Price breaker: halt at >5% over 4 slots
        self.price_breaker = PriceBreaker {
            max_cumulative_change: U64F64::from_num(0.05),
            window_size: 4,
            current_change: U64F64::zero(),
            halt_duration: 8640, // 1 hour
            recent_changes: Vec::new(),
        };

        // Volume breaker: halt at >10x average
        self.volume_breaker = VolumeBreaker {
            max_volume_multiplier: U64F64::from_num(10),
            current_volume: 0,
            avg_volume_baseline: 0,
            halt_duration: 4320, // 30 minutes
        };

        // Liquidation breaker: halt at >50 liquidations or >10% OI
        self.liquidation_breaker = LiquidationBreaker {
            max_liquidations_per_slot: 50,
            max_liquidation_volume_percent: U64F64::from_num(0.1),
            current_liquidations: 0,
            current_liquidation_volume: 0,
            halt_duration: 8640, // 1 hour
        };

        // Congestion breaker: halt at >1.5ms slot time or >100 failed tx
        self.congestion_breaker = CongestionBreaker {
            max_slot_deviation_ms: 1500,
            max_failed_tx_per_slot: 100,
            current_failed_tx: 0,
            halt_duration: 2160, // 15 minutes
        };

        self.last_trigger = None;
        self.total_triggers = 0;
        self.cooldown_period = 720; // 5 minutes
        self.emergency_authority = Some(Pubkey::default()); // Will be burned

        Ok(())
    }

    /// Check all breakers and return action
    pub fn check_breakers(
        &mut self,
        coverage: U64F64,
        recent_trades: &[TradeSnapshot],
        liquidation_count: u64,
        liquidation_volume: u64,
        total_oi: u64,
        failed_tx: u64,
        clock: &Clock,
    ) -> Result<BreakerAction> {
        // Check if currently halted
        match self.state {
            BreakerState::Halted { expected_resume, .. } => {
                if clock.slot >= expected_resume {
                    self.state = BreakerState::Cooldown {
                        end_slot: clock.slot + self.cooldown_period,
                    };
                    return Ok(BreakerAction::Resume);
                }
                return Ok(BreakerAction::RemainHalted);
            },
            BreakerState::Cooldown { end_slot } => {
                if clock.slot >= end_slot {
                    self.state = BreakerState::Active;
                }
                return Ok(BreakerAction::InCooldown);
            },
            BreakerState::EmergencyShutdown => {
                return Ok(BreakerAction::EmergencyShutdown);
            },
            _ => {}
        }

        // Check coverage breaker
        self.coverage_breaker.current_coverage = coverage;
        if coverage < self.coverage_breaker.min_coverage {
            return self.trigger_halt(
                HaltReason::LowCoverage,
                AttackSeverity::Critical,
                format!("Coverage {} below minimum 0.5", coverage),
                self.coverage_breaker.halt_duration,
                clock,
            );
        }

        // Check price breaker
        if self.check_price_breaker(recent_trades, clock)? {
            return self.trigger_halt(
                HaltReason::PriceVolatility,
                AttackSeverity::High,
                format!("Price movement {}% exceeds 5% limit over 4 slots",
                    (self.price_breaker.current_change * U64F64::from_num(100)).to_num::<u16>()),
                self.price_breaker.halt_duration,
                clock,
            );
        }

        // Check liquidation breaker
        self.liquidation_breaker.current_liquidations = liquidation_count;
        self.liquidation_breaker.current_liquidation_volume = liquidation_volume;

        if liquidation_count > self.liquidation_breaker.max_liquidations_per_slot {
            return self.trigger_halt(
                HaltReason::LiquidationCascade,
                AttackSeverity::Critical,
                format!("{} liquidations exceeds max 50 per slot", liquidation_count),
                self.liquidation_breaker.halt_duration,
                clock,
            );
        }

        if total_oi > 0 {
            let liq_percent = U64F64::from_num(liquidation_volume) / U64F64::from_num(total_oi);
            if liq_percent > self.liquidation_breaker.max_liquidation_volume_percent {
                return self.trigger_halt(
                    HaltReason::LiquidationCascade,
                    AttackSeverity::Critical,
                    format!("Liquidation volume {}% of OI exceeds 10% limit",
                        (liq_percent * U64F64::from_num(100)).to_num::<u16>()),
                    self.liquidation_breaker.halt_duration,
                    clock,
                );
            }
        }

        // Check congestion breaker
        self.congestion_breaker.current_failed_tx = failed_tx;
        if failed_tx > self.congestion_breaker.max_failed_tx_per_slot {
            return self.trigger_halt(
                HaltReason::NetworkCongestion,
                AttackSeverity::High,
                format!("{} failed transactions exceeds max 100", failed_tx),
                self.congestion_breaker.halt_duration,
                clock,
            );
        }

        Ok(BreakerAction::Continue)
    }

    /// Check price breaker logic
    fn check_price_breaker(
        &mut self,
        recent_trades: &[TradeSnapshot],
        clock: &Clock,
    ) -> Result<bool> {
        // Group trades by market and calculate cumulative changes
        let mut market_changes: HashMap<[u8; 32], Vec<&TradeSnapshot>> = HashMap::new();

        for trade in recent_trades.iter().rev().take(100) {
            if clock.slot - trade.slot <= self.price_breaker.window_size {
                market_changes.entry(trade.market_id).or_default().push(trade);
            }
        }

        for (market_id, trades) in market_changes {
            if trades.len() < 2 {
                continue;
            }

            let first_price = trades.last().unwrap().price;
            let last_price = trades.first().unwrap().price;

            let change = if last_price > first_price {
                (last_price - first_price) / first_price
            } else {
                (first_price - last_price) / first_price
            };

            if change > self.price_breaker.max_cumulative_change {
                self.price_breaker.current_change = change;
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Trigger a halt
    fn trigger_halt(
        &mut self,
        reason: HaltReason,
        severity: AttackSeverity,
        details: String,
        duration: u64,
        clock: &Clock,
    ) -> Result<BreakerAction> {
        self.state = BreakerState::Halted {
            start_slot: clock.slot,
            expected_resume: clock.slot + duration,
            reason,
        };

        self.last_trigger = Some(BreakerTrigger {
            trigger_type: reason,
            trigger_slot: clock.slot,
            severity,
            details: details.clone(),
            automatic_resume: true,
        });

        self.total_triggers += 1;

        // Update specific breaker trigger count
        match reason {
            HaltReason::LowCoverage => self.coverage_breaker.trigger_count += 1,
            _ => {}
        }

        msg!("Circuit breaker triggered: {:?}", reason);
        msg!("Details: {}", details);
        msg!("Halt duration: {} slots", duration);

        Ok(BreakerAction::Halt {
            reason,
            duration,
            severity,
        })
    }

    /// Emergency shutdown (one-time use)
    pub fn emergency_shutdown(&mut self, authority: &Pubkey) -> Result<()> {
        require!(
            self.emergency_authority == Some(*authority),
            crate::errors::ErrorCode::UnauthorizedEmergency
        );
        
        self.state = BreakerState::EmergencyShutdown;
        self.emergency_authority = None; // Burn authority after use
        
        msg!("EMERGENCY SHUTDOWN ACTIVATED");
        
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakerAction {
    /// Continue normal operation
    Continue,
    /// Halt trading
    Halt {
        reason: HaltReason,
        duration: u64,
        severity: AttackSeverity,
    },
    /// Remain in halted state
    RemainHalted,
    /// Resume from halt
    Resume,
    /// In cooldown period
    InCooldown,
    /// Emergency shutdown
    EmergencyShutdown,
}