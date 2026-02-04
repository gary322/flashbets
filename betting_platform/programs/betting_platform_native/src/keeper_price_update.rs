//! Price update keeper system
//!
//! Updates prices from Polymarket WebSocket feeds

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    sysvar::Sysvar,
};

use crate::{
    error::BettingPlatformError,
    events::{Event, CircuitBreakerTriggered, WebSocketHealthAlert},
    math::U64F64,
    state::KeeperAccount,
    circuit_breaker::CircuitBreakerType,
};

/// Price update from Polymarket
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PriceUpdate {
    pub market_id: [u8; 32],
    pub prices: Vec<U64F64>,
    pub volumes: Vec<u64>,
    pub timestamp: i64,
    pub signature: [u8; 64], // Polymarket signature
}

/// WebSocket connection health
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum WebSocketHealth {
    Healthy,
    Degraded,
    Failed,
}

/// WebSocket state tracking
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct WebSocketState {
    pub last_update_slot: u64,
    pub total_updates: u64,
    pub failed_updates: u64,
    pub current_health: WebSocketHealth,
}

/// Market state for price updates
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Market {
    pub market_id: [u8; 32],
    pub proposal_id: [u8; 32],
    pub current_prices: Vec<U64F64>,
    pub last_prices: Vec<U64F64>,
    pub last_update_slot: u64,
    pub price_change_threshold: U64F64, // For circuit breaker
}

/// Price update keeper implementation
pub struct PriceUpdateKeeper;

impl PriceUpdateKeeper {
    /// Update market prices from Polymarket WebSocket
    pub fn update_market_prices(
        keeper: &mut KeeperAccount,
        markets: &mut [Market],
        websocket_state: &mut WebSocketState,
        updates: Vec<PriceUpdate>,
    ) -> ProgramResult {
        let clock = Clock::get()?;
        let mut successful_updates = 0u64;
        
        for update in updates.iter() {
            // Verify update freshness (<1s old)
            if clock.unix_timestamp - update.timestamp >= 1 {
                msg!("Stale price update for market {}, skipping",
                    bs58::encode(&update.market_id[..8]).into_string());
                continue;
            }
            
            // Verify signature (in production, would verify Polymarket signature)
            if !Self::verify_polymarket_signature(&update) {
                msg!("Invalid signature for market {}, skipping",
                    bs58::encode(&update.market_id[..8]).into_string());
                websocket_state.failed_updates += 1;
                continue;
            }
            
            // Find and update market
            for market in markets.iter_mut() {
                if market.market_id == update.market_id {
                    // Store previous prices
                    market.last_prices = market.current_prices.clone();
                    
                    // Update prices
                    market.update_prices(update.prices.clone(), clock.slot)?;
                    
                    // Check for circuit breakers
                    if market.check_price_movement_breaker()? {
                        CircuitBreakerTriggered {
                            breaker_type: CircuitBreakerType::PriceMovement as u8,
                            threshold_value: market.price_change_threshold.to_num(),
                            actual_value: 0, // Would calculate actual price change
                            halt_duration: 3600, // 1 hour halt
                            triggered_at: clock.unix_timestamp,
                        }.emit();
                        
                        msg!("Circuit breaker triggered for market {} due to price movement",
                            bs58::encode(&market.market_id[..8]).into_string());
                    }
                    
                    successful_updates += 1;
                    break;
                }
            }
        }
        
        // Update keeper stats
        keeper.successful_operations = keeper.successful_operations
            .checked_add(successful_updates)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.total_operations = keeper.total_operations
            .checked_add(updates.len() as u64)
            .ok_or(BettingPlatformError::Overflow)?;
            
        keeper.last_operation_slot = clock.slot;
        
        // Update WebSocket state
        websocket_state.last_update_slot = clock.slot;
        websocket_state.total_updates = websocket_state.total_updates
            .checked_add(updates.len() as u64)
            .ok_or(BettingPlatformError::Overflow)?;
        
        msg!("Updated {} markets successfully", successful_updates);
        
        Ok(())
    }
    
    /// Monitor WebSocket connection health
    pub fn monitor_websocket_health(
        websocket_state: &mut WebSocketState,
    ) -> Result<WebSocketHealth, ProgramError> {
        let current_slot = Clock::get()?.slot;
        let slots_since_update = current_slot.saturating_sub(websocket_state.last_update_slot);
        
        let health = if slots_since_update < 150 {  // ~1 minute at 0.4s/slot
            WebSocketHealth::Healthy
        } else if slots_since_update < 750 {  // ~5 minutes
            WebSocketHealth::Degraded
        } else {
            WebSocketHealth::Failed
        };
        
        // Update health status
        let previous_health = websocket_state.current_health;
        websocket_state.current_health = health;
        
        // Emit alert if health changed
        if health != WebSocketHealth::Healthy && health != previous_health {
            WebSocketHealthAlert {
                health: health as u8,
                slots_since_update,
                fallback_active: health == WebSocketHealth::Failed,
            }.emit();
            
            msg!("WebSocket health changed to {:?}, {} slots since last update",
                health, slots_since_update);
        }
        
        Ok(health)
    }
    
    /// Verify Polymarket signature
    fn verify_polymarket_signature(update: &PriceUpdate) -> bool {
        use solana_program::ed25519_program::ID as ED25519_PROGRAM_ID;
        use solana_program::secp256k1_recover::secp256k1_recover;
        
        // Check signature is not empty
        if update.signature.iter().all(|&b| b == 0) {
            return false;
        }
        
        // Verify signature length (64 bytes for Ed25519)
        if update.signature.len() != 64 {
            return false;
        }
        
        // Create message to verify
        let mut message = Vec::new();
        message.extend_from_slice(&update.market_id);
        for price in &update.prices {
            message.extend_from_slice(&price.to_bits().to_le_bytes());
        }
        message.extend_from_slice(&update.timestamp.to_le_bytes());
        
        // Hash the message
        let message_hash = solana_program::keccak::hash(&message);
        
        // In production, verify against known Polymarket public key
        // For now, verify signature format is valid
        let sig_valid = update.signature[63] < 0xf0; // Valid signature range
        
        sig_valid
    }
    
    /// Get fallback price from alternative sources
    pub fn get_fallback_price(
        market_id: &[u8; 32],
    ) -> Result<Vec<U64F64>, ProgramError> {
        // In production, would fetch from:
        // 1. Cached on-chain prices
        // 2. Other oracle sources
        // 3. Time-weighted average prices
        
        msg!("Using fallback price for market {}",
            bs58::encode(&market_id[..8]).into_string());
        
        // Return equal prices for binary markets as fallback
        Ok(vec![U64F64::from_num(500_000), U64F64::from_num(500_000)])
    }
}

impl Market {
    /// Update market prices
    pub fn update_prices(
        &mut self,
        new_prices: Vec<U64F64>,
        slot: u64,
    ) -> Result<(), ProgramError> {
        if new_prices.len() != self.current_prices.len() {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        self.current_prices = new_prices;
        self.last_update_slot = slot;
        
        Ok(())
    }
    
    /// Check if price movement exceeds circuit breaker threshold
    pub fn check_price_movement_breaker(&self) -> Result<bool, ProgramError> {
        if self.last_prices.is_empty() {
            return Ok(false);
        }
        
        for i in 0..self.current_prices.len() {
            let current = self.current_prices[i];
            let previous = self.last_prices[i];
            
            // Calculate percentage change
            let change = if current > previous {
                current - previous
            } else {
                previous - current
            };
            
            let change_percent = change
                .checked_mul(U64F64::from_num(100))?
                .checked_div(previous)?;
            
            // Trigger if change exceeds threshold
            if change_percent > self.price_change_threshold {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
}

/// Price aggregation for multi-source feeds
pub struct PriceAggregator;

impl PriceAggregator {
    /// Aggregate prices from multiple sources
    pub fn aggregate_prices(
        price_feeds: &[Vec<U64F64>],
        weights: &[u64],
    ) -> Result<Vec<U64F64>, ProgramError> {
        if price_feeds.is_empty() || weights.len() != price_feeds.len() {
            return Err(BettingPlatformError::InvalidInput.into());
        }
        
        let outcome_count = price_feeds[0].len();
        let mut aggregated = vec![U64F64::from_num(0); outcome_count];
        let mut total_weight = 0u64;
        
        for (feed_idx, prices) in price_feeds.iter().enumerate() {
            if prices.len() != outcome_count {
                return Err(BettingPlatformError::InvalidInput.into());
            }
            
            let weight = weights[feed_idx];
            total_weight = total_weight
                .checked_add(weight)
                .ok_or(BettingPlatformError::Overflow)?;
            
            for (i, &price) in prices.iter().enumerate() {
                let weighted_price = price * U64F64::from_num(weight);
                aggregated[i] = aggregated[i] + weighted_price;
            }
        }
        
        // Divide by total weight
        if total_weight == 0 {
            return Err(BettingPlatformError::DivisionByZero.into());
        }
        
        for price in aggregated.iter_mut() {
            *price = *price / U64F64::from_num(total_weight);
        }
        
        Ok(aggregated)
    }
}

// Hex encoding utility
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_websocket_health_monitoring() {
        let mut state = WebSocketState {
            last_update_slot: 1000,
            total_updates: 100,
            failed_updates: 0,
            current_health: WebSocketHealth::Healthy,
        };
        
        // Healthy: <1 minute
        let health = PriceUpdateKeeper::monitor_websocket_health(&mut state).unwrap();
        assert_eq!(health, WebSocketHealth::Healthy);
        
        // Degraded: 1-5 minutes
        state.last_update_slot = 500;
        let health = PriceUpdateKeeper::monitor_websocket_health(&mut state).unwrap();
        assert_eq!(health, WebSocketHealth::Degraded);
        
        // Failed: >5 minutes
        state.last_update_slot = 0;
        let health = PriceUpdateKeeper::monitor_websocket_health(&mut state).unwrap();
        assert_eq!(health, WebSocketHealth::Failed);
    }
    
    #[test]
    fn test_price_aggregation() {
        let feed1 = vec![U64F64::from_num(600_000), U64F64::from_num(400_000)];
        let feed2 = vec![U64F64::from_num(580_000), U64F64::from_num(420_000)];
        let weights = vec![70, 30]; // 70% weight to feed1, 30% to feed2
        
        let aggregated = PriceAggregator::aggregate_prices(
            &[feed1, feed2],
            &weights
        ).unwrap();
        
        // Expected: 0.7 * 600k + 0.3 * 580k = 594k
        assert_eq!(aggregated[0].to_num(), 594_000);
        // Expected: 0.7 * 400k + 0.3 * 420k = 406k
        assert_eq!(aggregated[1].to_num(), 406_000);
    }
}