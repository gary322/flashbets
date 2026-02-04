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
use crate::{
    error::BettingPlatformError,
    events::{emit_event, EventType},
    market_ingestion::INGESTION_INTERVAL_SLOTS,
    synthetics::MarketData,
};
use std::collections::{HashMap, VecDeque};

/// WebSocket subscription constants from Part 7 spec
pub const MAX_CONCURRENT_SUBSCRIPTIONS: usize = 1000;
pub const SUBSCRIPTION_BUFFER_SIZE: usize = 100;
pub const HEARTBEAT_INTERVAL_SLOTS: u64 = 25; // ~10 seconds
pub const RECONNECT_DELAY_SLOTS: u64 = 12; // ~5 seconds
pub const MAX_RECONNECT_ATTEMPTS: u8 = 5;
pub const POLLING_INTERVAL_SLOTS: u64 = 5; // From spec: every 5 slots

/// WebSocket subscription types
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq)]
pub enum SubscriptionType {
    MarketPrices,
    OrderBook,
    Trades,
    Liquidations,
    VerseUpdates,
}

/// WebSocket connection state
#[derive(BorshSerialize, BorshDeserialize, Clone, PartialEq, Debug)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Market update from WebSocket
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MarketUpdate {
    pub market_id: Pubkey,
    pub update_type: UpdateType,
    pub timestamp: i64,
    pub slot: u64,
    pub data: UpdateData,
}

/// Update types
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum UpdateType {
    Price { yes: u64, no: u64 },
    Volume { amount: u64 },
    Trade { size: u64, price: u64, is_buy: bool },
    Liquidation { position_id: Pubkey, size: u64 },
    OrderBook { bids: Vec<(u64, u64)>, asks: Vec<(u64, u64)> },
}

/// Update data wrapper
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum UpdateData {
    Price(PriceUpdate),
    Volume(VolumeUpdate),
    Trade(TradeUpdate),
    Liquidation(LiquidationUpdate),
    OrderBook(OrderBookUpdate),
}

/// Price update
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PriceUpdate {
    pub yes_price: u64,
    pub no_price: u64,
    pub change_24h: i64,
}

/// Volume update
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct VolumeUpdate {
    pub volume_24h: u64,
    pub trades_24h: u32,
}

/// Trade update
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct TradeUpdate {
    pub size: u64,
    pub price: u64,
    pub is_buy: bool,
    pub maker: Option<Pubkey>,
    pub taker: Option<Pubkey>,
}

/// Liquidation update
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LiquidationUpdate {
    pub position_id: Pubkey,
    pub user: Pubkey,
    pub size: u64,
    pub collateral_seized: u64,
}

/// Order book update
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OrderBookUpdate {
    pub bids: Vec<OrderBookEntry>,
    pub asks: Vec<OrderBookEntry>,
    pub spread: u64,
}

/// Order book entry
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct OrderBookEntry {
    pub price: u64,
    pub size: u64,
    pub orders: u32,
}

/// WebSocket subscription
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct Subscription {
    pub id: u64,
    pub subscriber: Pubkey,
    pub subscription_type: SubscriptionType,
    pub markets: Vec<Pubkey>,
    pub created_slot: u64,
    pub last_update_slot: u64,
    pub update_count: u64,
    pub is_active: bool,
}

/// WebSocket manager state
#[derive(BorshSerialize, BorshDeserialize)]
pub struct WebSocketManager {
    pub connection_state: ConnectionState,
    pub subscriptions: HashMap<u64, Subscription>,
    pub update_buffer: VecDeque<MarketUpdate>,
    pub last_heartbeat_slot: u64,
    pub reconnect_attempts: u8,
    pub total_updates_processed: u64,
    pub dropped_updates: u64,
    pub active_connections: u32,
}

impl WebSocketManager {
    pub const SIZE: usize = 1 + // connection_state
        4 + (64 * MAX_CONCURRENT_SUBSCRIPTIONS) + // subscriptions
        4 + (200 * SUBSCRIPTION_BUFFER_SIZE) + // update_buffer
        8 + // last_heartbeat_slot
        1 + // reconnect_attempts
        8 + // total_updates_processed
        8 + // dropped_updates
        4; // active_connections

    /// Initialize new WebSocket manager
    pub fn new() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            subscriptions: HashMap::new(),
            update_buffer: VecDeque::with_capacity(SUBSCRIPTION_BUFFER_SIZE),
            last_heartbeat_slot: 0,
            reconnect_attempts: 0,
            total_updates_processed: 0,
            dropped_updates: 0,
            active_connections: 0,
        }
    }

    /// Add subscription
    pub fn add_subscription(
        &mut self,
        subscriber: Pubkey,
        subscription_type: SubscriptionType,
        markets: Vec<Pubkey>,
        current_slot: u64,
    ) -> Result<u64, ProgramError> {
        if self.subscriptions.len() >= MAX_CONCURRENT_SUBSCRIPTIONS {
            return Err(BettingPlatformError::TooManySubscriptions.into());
        }

        let id = self.generate_subscription_id(current_slot);
        let markets_len = markets.len();
        
        let subscription = Subscription {
            id,
            subscriber,
            subscription_type,
            markets,
            created_slot: current_slot,
            last_update_slot: current_slot,
            update_count: 0,
            is_active: true,
        };

        self.subscriptions.insert(id, subscription);
        
        msg!(
            "Added subscription {} for {} markets",
            id,
            markets_len
        );

        Ok(id)
    }

    /// Remove subscription
    pub fn remove_subscription(&mut self, id: u64) -> Result<(), ProgramError> {
        self.subscriptions.remove(&id)
            .ok_or(BettingPlatformError::SubscriptionNotFound)?;
        
        msg!("Removed subscription {}", id);
        Ok(())
    }

    /// Process incoming update
    pub fn process_update(
        &mut self,
        update: MarketUpdate,
    ) -> Result<(), ProgramError> {
        // Add to buffer
        if self.update_buffer.len() >= SUBSCRIPTION_BUFFER_SIZE {
            // Remove oldest update
            self.update_buffer.pop_front();
            self.dropped_updates += 1;
        }

        self.update_buffer.push_back(update.clone());
        self.total_updates_processed += 1;

        // Notify relevant subscribers
        let mut notified_count = 0;
        for subscription in self.subscriptions.values_mut() {
            if subscription.is_active && subscription.markets.contains(&update.market_id) {
                subscription.last_update_slot = update.slot;
                subscription.update_count += 1;
                notified_count += 1;
            }
        }

        msg!(
            "Processed update for market {}, notified {} subscribers",
            update.market_id,
            notified_count
        );

        Ok(())
    }

    /// Handle connection heartbeat
    pub fn heartbeat(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        // Check if heartbeat is due
        if current_slot.saturating_sub(self.last_heartbeat_slot) < HEARTBEAT_INTERVAL_SLOTS {
            return Ok(());
        }

        self.last_heartbeat_slot = current_slot;

        match self.connection_state {
            ConnectionState::Connected => {
                msg!("WebSocket heartbeat OK at slot {}", current_slot);
            }
            ConnectionState::Disconnected | ConnectionState::Failed => {
                self.attempt_reconnect(current_slot)?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Attempt to reconnect
    fn attempt_reconnect(&mut self, current_slot: u64) -> Result<(), ProgramError> {
        if self.reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
            self.connection_state = ConnectionState::Failed;
            return Err(BettingPlatformError::WebSocketConnectionFailed.into());
        }

        self.connection_state = ConnectionState::Reconnecting;
        self.reconnect_attempts += 1;

        msg!(
            "Attempting WebSocket reconnect (attempt {}/{})",
            self.reconnect_attempts,
            MAX_RECONNECT_ATTEMPTS
        );

        // In production, this would trigger actual reconnection logic
        // For now, simulate successful reconnection
        self.connection_state = ConnectionState::Connected;
        self.reconnect_attempts = 0;
        self.active_connections = 1;

        Ok(())
    }

    /// Get subscription statistics
    pub fn get_stats(&self) -> SubscriptionStats {
        let active_subscriptions = self.subscriptions.values()
            .filter(|s| s.is_active)
            .count();

        let total_markets = self.subscriptions.values()
            .flat_map(|s| &s.markets)
            .collect::<std::collections::HashSet<_>>()
            .len();

        SubscriptionStats {
            active_subscriptions: active_subscriptions as u32,
            total_markets_monitored: total_markets as u32,
            updates_processed: self.total_updates_processed,
            updates_dropped: self.dropped_updates,
            buffer_utilization: (self.update_buffer.len() as f64 / SUBSCRIPTION_BUFFER_SIZE as f64) * 100.0,
            connection_state: self.connection_state.clone(),
        }
    }

    /// Drain update buffer for processing
    pub fn drain_updates(&mut self, max_count: usize) -> Vec<MarketUpdate> {
        let mut updates = Vec::new();
        
        for _ in 0..max_count.min(self.update_buffer.len()) {
            if let Some(update) = self.update_buffer.pop_front() {
                updates.push(update);
            }
        }

        updates
    }

    /// Generate unique subscription ID
    fn generate_subscription_id(&self, current_slot: u64) -> u64 {
        // Simple ID generation using slot and count
        (current_slot << 32) | (self.subscriptions.len() as u64)
    }

    /// Clean up inactive subscriptions
    pub fn cleanup_inactive(
        &mut self,
        current_slot: u64,
        inactive_threshold_slots: u64,
    ) -> u32 {
        let mut removed = 0;
        
        self.subscriptions.retain(|_, subscription| {
            let is_inactive = current_slot.saturating_sub(subscription.last_update_slot) 
                > inactive_threshold_slots;
            
            if is_inactive {
                removed += 1;
                false
            } else {
                true
            }
        });

        if removed > 0 {
            msg!("Cleaned up {} inactive subscriptions", removed);
        }

        removed
    }
}

/// Subscription statistics
#[derive(Debug)]
pub struct SubscriptionStats {
    pub active_subscriptions: u32,
    pub total_markets_monitored: u32,
    pub updates_processed: u64,
    pub updates_dropped: u64,
    pub buffer_utilization: f64,
    pub connection_state: ConnectionState,
}

/// WebSocket polling mode (fallback when real-time not available)
pub struct PollingMode {
    pub last_poll_slot: u64,
    pub markets_to_poll: Vec<Pubkey>,
    pub poll_interval_slots: u64,
}

impl PollingMode {
    pub fn new() -> Self {
        Self {
            last_poll_slot: 0,
            markets_to_poll: Vec::new(),
            poll_interval_slots: POLLING_INTERVAL_SLOTS, // 5 slots from spec
        }
    }

    /// Check if polling is due
    pub fn should_poll(&self, current_slot: u64) -> bool {
        current_slot.saturating_sub(self.last_poll_slot) >= self.poll_interval_slots
    }

    /// Update poll timestamp
    pub fn mark_polled(&mut self, current_slot: u64) {
        self.last_poll_slot = current_slot;
    }

    /// Add markets for polling
    pub fn add_markets(&mut self, markets: &[Pubkey]) {
        for market in markets {
            if !self.markets_to_poll.contains(market) {
                self.markets_to_poll.push(*market);
            }
        }
    }
}

/// Initialize WebSocket manager
pub fn initialize_websocket_manager(
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing WebSocket manager");
    msg!("Max concurrent subscriptions: {}", MAX_CONCURRENT_SUBSCRIPTIONS);
    msg!("Polling interval: {} slots", POLLING_INTERVAL_SLOTS);
    msg!("Heartbeat interval: {} slots", HEARTBEAT_INTERVAL_SLOTS);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_management() {
        let mut manager = WebSocketManager::new();
        let subscriber = Pubkey::new_unique();
        let markets = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        
        // Add subscription
        let id = manager.add_subscription(
            subscriber,
            SubscriptionType::MarketPrices,
            markets.clone(),
            100,
        ).unwrap();
        
        assert_eq!(manager.subscriptions.len(), 1);
        assert!(manager.subscriptions.contains_key(&id));
        
        // Remove subscription
        manager.remove_subscription(id).unwrap();
        assert_eq!(manager.subscriptions.len(), 0);
    }

    #[test]
    fn test_update_processing() {
        let mut manager = WebSocketManager::new();
        let market_id = Pubkey::new_unique();
        
        // Add subscription
        let subscriber = Pubkey::new_unique();
        manager.add_subscription(
            subscriber,
            SubscriptionType::MarketPrices,
            vec![market_id],
            100,
        ).unwrap();
        
        // Process update
        let update = MarketUpdate {
            market_id,
            update_type: UpdateType::Price { yes: 6000, no: 4000 },
            timestamp: 0,
            slot: 101,
            data: UpdateData::Price(PriceUpdate {
                yes_price: 6000,
                no_price: 4000,
                change_24h: 100,
            }),
        };
        
        manager.process_update(update).unwrap();
        
        assert_eq!(manager.total_updates_processed, 1);
        assert_eq!(manager.update_buffer.len(), 1);
    }

    #[test]
    fn test_polling_mode() {
        let mut polling = PollingMode::new();
        
        // Should poll initially
        assert!(polling.should_poll(10));
        
        // Mark as polled
        polling.mark_polled(10);
        
        // Should not poll immediately
        assert!(!polling.should_poll(11));
        
        // Should poll after interval
        assert!(polling.should_poll(10 + POLLING_INTERVAL_SLOTS));
    }
}