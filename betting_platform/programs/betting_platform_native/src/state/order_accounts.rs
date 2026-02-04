//! Advanced order account structures
//!
//! Account types for iceberg, TWAP, and dark pool orders

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    pubkey::Pubkey,
    program_error::ProgramError,
};

use crate::account_validation::DISCRIMINATOR_SIZE;
use crate::instruction::{OrderSide, TimeInForce};

/// Discriminators for order account types
pub mod discriminators {
    pub const ICEBERG_ORDER: [u8; 8] = [234, 167, 89, 45, 201, 78, 156, 23];
    pub const TWAP_ORDER: [u8; 8] = [45, 201, 156, 78, 89, 23, 234, 167];
    pub const DARK_POOL: [u8; 8] = [156, 78, 23, 234, 167, 45, 89, 201];
    pub const DARK_ORDER: [u8; 8] = [89, 234, 201, 45, 156, 167, 78, 23];
    pub const STOP_ORDER: [u8; 8] = [201, 156, 89, 234, 78, 45, 23, 167];
}

/// Iceberg order (shows only partial size)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct IcebergOrder {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Order ID
    pub order_id: u64,
    
    /// User placing the order
    pub user: Pubkey,
    
    /// Market ID
    pub market_id: u128,
    
    /// Outcome to trade
    pub outcome: u8,
    
    /// Visible size shown to market
    pub visible_size: u64,
    
    /// Total size (hidden)
    pub total_size: u64,
    
    /// Executed size so far
    pub executed_size: u64,
    
    /// Order side
    pub side: OrderSide,
    
    /// Limit price (0 for market order)
    pub limit_price: u64,
    
    /// Order status
    pub status: OrderStatus,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last execution timestamp
    pub last_execution: Option<i64>,
    
    /// Average execution price
    pub avg_execution_price: u64,
    
    /// Number of refills
    pub refill_count: u32,
}

impl IcebergOrder {
    pub fn new(
        order_id: u64,
        user: Pubkey,
        market_id: u128,
        outcome: u8,
        visible_size: u64,
        total_size: u64,
        side: OrderSide,
        limit_price: u64,
        created_at: i64,
    ) -> Result<Self, ProgramError> {
        if visible_size == 0 || visible_size > total_size {
            return Err(ProgramError::InvalidArgument);
        }
        
        Ok(Self {
            discriminator: discriminators::ICEBERG_ORDER,
            order_id,
            user,
            market_id,
            outcome,
            visible_size,
            total_size,
            executed_size: 0,
            side,
            limit_price,
            status: OrderStatus::Active,
            created_at,
            last_execution: None,
            avg_execution_price: 0,
            refill_count: 0,
        })
    }
    
    pub fn execute_fill(&mut self, fill_size: u64, price: u64, timestamp: i64) -> Result<(), ProgramError> {
        if fill_size > self.get_current_visible_size() {
            return Err(ProgramError::InvalidArgument);
        }
        
        // Update average price
        let new_total_value = (self.avg_execution_price * self.executed_size) + (price * fill_size);
        self.executed_size += fill_size;
        self.avg_execution_price = new_total_value / self.executed_size;
        
        self.last_execution = Some(timestamp);
        
        // Check if order is complete
        if self.executed_size >= self.total_size {
            self.status = OrderStatus::Filled;
        } else if self.executed_size % self.visible_size == 0 {
            // Refill visible portion
            self.refill_count += 1;
        }
        
        Ok(())
    }
    
    pub fn get_current_visible_size(&self) -> u64 {
        let remaining = self.total_size - self.executed_size;
        remaining.min(self.visible_size)
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::ICEBERG_ORDER {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.visible_size == 0 || self.visible_size > self.total_size {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.executed_size > self.total_size {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// TWAP (Time-Weighted Average Price) order
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TWAPOrder {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Order ID
    pub order_id: u64,
    
    /// User placing the order
    pub user: Pubkey,
    
    /// Market ID
    pub market_id: u128,
    
    /// Outcome to trade
    pub outcome: u8,
    
    /// Total size to execute
    pub total_size: u64,
    
    /// Size per interval
    pub interval_size: u64,
    
    /// Duration in slots
    pub duration: u64,
    
    /// Number of intervals
    pub intervals: u8,
    
    /// Executed intervals
    pub executed_intervals: u8,
    
    /// Order side
    pub side: OrderSide,
    
    /// Status
    pub status: OrderStatus,
    
    /// Start time
    pub start_time: i64,
    
    /// Next execution slot
    pub next_execution_slot: u64,
    
    /// Total executed size
    pub executed_size: u64,
    
    /// Average execution price
    pub avg_execution_price: u64,
    
    /// Price limit (optional)
    pub price_limit: Option<u64>,
}

impl TWAPOrder {
    pub fn new(
        order_id: u64,
        user: Pubkey,
        market_id: u128,
        outcome: u8,
        total_size: u64,
        duration: u64,
        intervals: u8,
        side: OrderSide,
        start_time: i64,
        start_slot: u64,
        price_limit: Option<u64>,
    ) -> Result<Self, ProgramError> {
        if intervals == 0 || duration == 0 {
            return Err(ProgramError::InvalidArgument);
        }
        
        let interval_size = total_size / intervals as u64;
        if interval_size == 0 {
            return Err(ProgramError::InvalidArgument);
        }
        
        let slot_interval = duration / intervals as u64;
        
        Ok(Self {
            discriminator: discriminators::TWAP_ORDER,
            order_id,
            user,
            market_id,
            outcome,
            total_size,
            interval_size,
            duration,
            intervals,
            executed_intervals: 0,
            side,
            status: OrderStatus::Active,
            start_time,
            next_execution_slot: start_slot,
            executed_size: 0,
            avg_execution_price: 0,
            price_limit,
        })
    }
    
    pub fn execute_interval(&mut self, price: u64, current_slot: u64) -> Result<u64, ProgramError> {
        if self.status != OrderStatus::Active {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if current_slot < self.next_execution_slot {
            return Err(ProgramError::InvalidArgument);
        }
        
        // Check price limit if set
        if let Some(limit) = self.price_limit {
            match self.side {
                OrderSide::Buy if price > limit => return Err(ProgramError::Custom(1)),
                OrderSide::Sell if price < limit => return Err(ProgramError::Custom(2)),
                _ => {}
            }
        }
        
        // Calculate execution size for this interval
        let remaining = self.total_size - self.executed_size;
        let exec_size = remaining.min(self.interval_size);
        
        // Update average price
        let new_total_value = (self.avg_execution_price * self.executed_size) + (price * exec_size);
        self.executed_size += exec_size;
        self.avg_execution_price = new_total_value / self.executed_size;
        
        self.executed_intervals += 1;
        
        // Calculate next execution slot
        let slot_interval = self.duration / self.intervals as u64;
        self.next_execution_slot = current_slot + slot_interval;
        
        // Check if complete
        if self.executed_intervals >= self.intervals || self.executed_size >= self.total_size {
            self.status = OrderStatus::Filled;
        }
        
        Ok(exec_size)
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::TWAP_ORDER {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.intervals == 0 || self.duration == 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.executed_intervals > self.intervals {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Dark pool configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct DarkPool {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Market ID
    pub market_id: u128,
    
    /// Minimum order size
    pub minimum_size: u64,
    
    /// Price improvement requirement (basis points)
    pub price_improvement_bps: u16,
    
    /// Total volume traded
    pub total_volume: u64,
    
    /// Number of trades
    pub trade_count: u64,
    
    /// Average trade size
    pub avg_trade_size: u64,
    
    /// Status
    pub status: PoolStatus,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Last match timestamp
    pub last_match: Option<i64>,
}

impl DarkPool {
    pub fn new(
        market_id: u128,
        minimum_size: u64,
        price_improvement_bps: u16,
        created_at: i64,
    ) -> Self {
        Self {
            discriminator: discriminators::DARK_POOL,
            market_id,
            minimum_size,
            price_improvement_bps,
            total_volume: 0,
            trade_count: 0,
            avg_trade_size: 0,
            status: PoolStatus::Active,
            created_at,
            last_match: None,
        }
    }
    
    pub fn record_trade(&mut self, size: u64, timestamp: i64) {
        self.total_volume += size;
        self.trade_count += 1;
        self.avg_trade_size = self.total_volume / self.trade_count;
        self.last_match = Some(timestamp);
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::DARK_POOL {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.minimum_size == 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Dark pool order
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct DarkOrder {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Order ID
    pub order_id: u64,
    
    /// User
    pub user: Pubkey,
    
    /// Market ID
    pub market_id: u128,
    
    /// Side
    pub side: OrderSide,
    
    /// Outcome
    pub outcome: u8,
    
    /// Size
    pub size: u64,
    
    /// Minimum acceptable price (for buys)
    pub min_price: Option<u64>,
    
    /// Maximum acceptable price (for sells)
    pub max_price: Option<u64>,
    
    /// Time in force
    pub time_in_force: TimeInForce,
    
    /// Status
    pub status: OrderStatus,
    
    /// Creation timestamp
    pub created_at: i64,
    
    /// Expiry timestamp (for time-limited orders)
    pub expires_at: Option<i64>,
    
    /// Execution price if matched
    pub execution_price: Option<u64>,
    
    /// Counter-party if matched
    pub counter_party: Option<Pubkey>,
}

impl DarkOrder {
    pub fn new(
        order_id: u64,
        user: Pubkey,
        market_id: u128,
        side: OrderSide,
        outcome: u8,
        size: u64,
        min_price: Option<u64>,
        max_price: Option<u64>,
        time_in_force: TimeInForce,
        created_at: i64,
    ) -> Self {
        let expires_at = match time_in_force {
            TimeInForce::Session => Some(created_at + 86400), // 24 hours
            _ => None,
        };
        
        Self {
            discriminator: discriminators::DARK_ORDER,
            order_id,
            user,
            market_id,
            side,
            outcome,
            size,
            min_price,
            max_price,
            time_in_force,
            status: OrderStatus::Active,
            created_at,
            expires_at,
            execution_price: None,
            counter_party: None,
        }
    }
    
    pub fn can_match(&self, other: &DarkOrder, mid_price: u64) -> bool {
        // Check basic compatibility
        if self.market_id != other.market_id ||
           self.outcome != other.outcome ||
           self.side == other.side ||
           self.status != OrderStatus::Active ||
           other.status != OrderStatus::Active {
            return false;
        }
        
        // Check price constraints
        match (self.side, other.side) {
            (OrderSide::Buy, OrderSide::Sell) => {
                let buy_ok = self.min_price.map_or(true, |min| mid_price >= min);
                let sell_ok = other.max_price.map_or(true, |max| mid_price <= max);
                buy_ok && sell_ok
            }
            (OrderSide::Sell, OrderSide::Buy) => {
                let sell_ok = self.max_price.map_or(true, |max| mid_price <= max);
                let buy_ok = other.min_price.map_or(true, |min| mid_price >= min);
                sell_ok && buy_ok
            }
            _ => false,
        }
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::DARK_ORDER {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.size == 0 {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Stop order
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct StopOrder {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    
    /// Order ID
    pub order_id: [u8; 32],
    
    /// Market ID
    pub market_id: [u8; 32],
    
    /// User
    pub user: Pubkey,
    
    /// Order type
    pub order_type: StopOrderType,
    
    /// Side
    pub side: OrderSide,
    
    /// Size
    pub size: u64,
    
    /// Trigger price
    pub trigger_price: u64,
    
    /// Is active
    pub is_active: bool,
    
    /// Prepaid keeper bounty
    pub prepaid_bounty: u64,
    
    /// Position entry price (for trailing stops)
    pub position_entry_price: u64,
    
    /// Trailing distance
    pub trailing_distance: u64,
    
    /// Trailing price (current)
    pub trailing_price: u64,
}

impl StopOrder {
    pub fn execute(&mut self, _current_price: u64) -> Result<ExecutionResult, ProgramError> {
        self.is_active = false;
        Ok(ExecutionResult {
            executed_value: self.size,
        })
    }
    
    pub fn calculate_priority(&self) -> u64 {
        // Higher bounty = higher priority
        self.prepaid_bounty
    }
    
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::STOP_ORDER {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Stop order type
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum StopOrderType {
    StopLoss,
    TakeProfit,
    TrailingStop,
}

/// Order status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum OrderStatus {
    Active,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

/// Pool status
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum PoolStatus {
    Active,
    Paused,
    Closed,
}

/// Execution result
pub struct ExecutionResult {
    pub executed_value: u64,
}
pub const STOP_LOSS_SEED: &[u8] = b"stop_loss";
