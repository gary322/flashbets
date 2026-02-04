//! Cross-Shard Communication Module
//! 
//! Enables efficient communication between shards for operations that span multiple shards
//! Implements message passing and state synchronization for 5k+ TPS

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::{
    error::BettingPlatformError,
    sharding::enhanced_sharding::{ShardType, ShardAssignment},
};

/// Message types for cross-shard communication
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    /// Order placed on one shard needs execution on another
    OrderRouting,
    /// Trade executed, update analytics shard
    TradeUpdate,
    /// Settlement completed, update all shards
    SettlementNotification,
    /// State sync between shards
    StateSync,
    /// Rebalancing notification
    RebalanceNotification,
    /// Emergency halt across all shards
    EmergencyHalt,
}

/// Cross-shard message structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CrossShardMessage {
    pub message_id: u64,
    pub message_type: MessageType,
    pub source_shard: u32,
    pub target_shard: u32,
    pub market_id: Pubkey,
    pub payload: Vec<u8>,
    pub timestamp: i64,
    pub priority: MessagePriority,
    pub retry_count: u8,
}

impl CrossShardMessage {
    pub const SIZE: usize = 8 + // message_id
        1 + // message_type
        4 + // source_shard
        4 + // target_shard
        32 + // market_id
        4 + 256 + // payload (max 256 bytes)
        8 + // timestamp
        1 + // priority
        1; // retry_count
    
    pub fn new(
        message_type: MessageType,
        source_shard: u32,
        target_shard: u32,
        market_id: Pubkey,
        payload: Vec<u8>,
    ) -> Result<Self, ProgramError> {
        if payload.len() > 256 {
            return Err(BettingPlatformError::TooLargePayload.into());
        }
        
        Ok(Self {
            message_id: Clock::get()?.unix_timestamp as u64,
            message_type,
            source_shard,
            target_shard,
            market_id,
            payload,
            timestamp: Clock::get()?.unix_timestamp,
            priority: MessagePriority::from_message_type(message_type),
            retry_count: 0,
        })
    }
}

/// Message priority for queue management
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Critical = 0,  // Emergency halts, critical updates
    High = 1,      // Trade execution, settlements
    Medium = 2,    // Order routing, updates
    Low = 3,       // Analytics, non-critical sync
}

impl MessagePriority {
    fn from_message_type(msg_type: MessageType) -> Self {
        match msg_type {
            MessageType::EmergencyHalt => MessagePriority::Critical,
            MessageType::SettlementNotification => MessagePriority::High,
            MessageType::TradeUpdate => MessagePriority::High,
            MessageType::OrderRouting => MessagePriority::Medium,
            MessageType::StateSync => MessagePriority::Low,
            MessageType::RebalanceNotification => MessagePriority::Low,
        }
    }
}

/// Message queue for cross-shard communication
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MessageQueue {
    pub shard_id: u32,
    pub messages: Vec<CrossShardMessage>,
    pub processed_count: u64,
    pub failed_count: u64,
    pub last_processed_slot: u64,
}

impl MessageQueue {
    pub const MAX_MESSAGES: usize = 100;
    pub const SIZE: usize = 4 + // shard_id
        4 + (Self::MAX_MESSAGES * CrossShardMessage::SIZE) + // messages
        8 + // processed_count
        8 + // failed_count
        8; // last_processed_slot
    
    pub fn new(shard_id: u32) -> Self {
        Self {
            shard_id,
            messages: Vec::with_capacity(Self::MAX_MESSAGES),
            processed_count: 0,
            failed_count: 0,
            last_processed_slot: 0,
        }
    }
    
    /// Add message to queue with priority ordering
    pub fn enqueue(&mut self, message: CrossShardMessage) -> Result<(), ProgramError> {
        if self.messages.len() >= Self::MAX_MESSAGES {
            // Remove lowest priority message if queue is full
            self.messages.sort_by(|a, b| b.priority.cmp(&a.priority));
            self.messages.pop();
        }
        
        self.messages.push(message);
        self.messages.sort_by(|a, b| a.priority.cmp(&b.priority));
        
        Ok(())
    }
    
    /// Get next message to process
    pub fn dequeue(&mut self) -> Option<CrossShardMessage> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.remove(0))
        }
    }
    
    /// Get messages for specific target shard
    pub fn get_messages_for_shard(&self, target_shard: u32) -> Vec<&CrossShardMessage> {
        self.messages.iter()
            .filter(|msg| msg.target_shard == target_shard)
            .collect()
    }
}

/// Cross-shard communication coordinator
pub struct CrossShardCoordinator;

impl CrossShardCoordinator {
    /// Send message between shards
    pub fn send_message(
        source_queue: &mut MessageQueue,
        message: CrossShardMessage,
    ) -> ProgramResult {
        msg!("Sending cross-shard message: {:?} from shard {} to shard {}", 
            message.message_type, message.source_shard, message.target_shard);
        
        source_queue.enqueue(message)?;
        
        Ok(())
    }
    
    /// Process messages for a shard
    pub fn process_messages(
        queue: &mut MessageQueue,
        process_fn: impl Fn(CrossShardMessage) -> Result<(), ProgramError>,
    ) -> ProgramResult {
        let current_slot = Clock::get()?.slot;
        let mut processed = 0;
        let mut failed = 0;
        
        // Process up to 10 messages per call to avoid CU limits
        for _ in 0..10 {
            if let Some(mut message) = queue.dequeue() {
                match process_fn(message.clone()) {
                    Ok(()) => {
                        processed += 1;
                        queue.processed_count += 1;
                    }
                    Err(e) => {
                        msg!("Failed to process message: {:?}", e);
                        failed += 1;
                        queue.failed_count += 1;
                        
                        // Retry critical messages
                        if message.priority == MessagePriority::Critical && message.retry_count < 3 {
                            message.retry_count += 1;
                            queue.enqueue(message)?;
                        }
                    }
                }
            } else {
                break;
            }
        }
        
        queue.last_processed_slot = current_slot;
        
        msg!("Processed {} messages, {} failed for shard {}", 
            processed, failed, queue.shard_id);
        
        Ok(())
    }
    
    /// Broadcast message to all shards
    pub fn broadcast_message(
        all_queues: &mut [MessageQueue],
        message_type: MessageType,
        source_shard: u32,
        market_id: Pubkey,
        payload: Vec<u8>,
    ) -> ProgramResult {
        msg!("Broadcasting {:?} from shard {} to all shards", message_type, source_shard);
        
        for (i, queue) in all_queues.iter_mut().enumerate() {
            if queue.shard_id != source_shard {
                let message = CrossShardMessage::new(
                    message_type,
                    source_shard,
                    queue.shard_id,
                    market_id,
                    payload.clone(),
                )?;
                
                queue.enqueue(message)?;
            }
        }
        
        Ok(())
    }
    
    /// Synchronize state between shards
    pub fn sync_state(
        source_shard: u32,
        target_shard: u32,
        market_id: Pubkey,
        state_data: Vec<u8>,
        queue: &mut MessageQueue,
    ) -> ProgramResult {
        let message = CrossShardMessage::new(
            MessageType::StateSync,
            source_shard,
            target_shard,
            market_id,
            state_data,
        )?;
        
        queue.enqueue(message)?;
        
        msg!("State sync initiated from shard {} to shard {}", source_shard, target_shard);
        
        Ok(())
    }
    
    /// Handle emergency halt across all shards
    pub fn emergency_halt_all_shards(
        all_queues: &mut [MessageQueue],
        source_shard: u32,
        reason: &str,
    ) -> ProgramResult {
        let payload = reason.as_bytes().to_vec();
        
        Self::broadcast_message(
            all_queues,
            MessageType::EmergencyHalt,
            source_shard,
            Pubkey::default(), // Global halt, no specific market
            payload,
        )?;
        
        msg!("Emergency halt broadcast from shard {}: {}", source_shard, reason);
        
        Ok(())
    }
}

/// Message handler trait for processing cross-shard messages
pub trait MessageHandler {
    fn handle_order_routing(&mut self, message: CrossShardMessage) -> ProgramResult;
    fn handle_trade_update(&mut self, message: CrossShardMessage) -> ProgramResult;
    fn handle_settlement_notification(&mut self, message: CrossShardMessage) -> ProgramResult;
    fn handle_state_sync(&mut self, message: CrossShardMessage) -> ProgramResult;
    fn handle_rebalance_notification(&mut self, message: CrossShardMessage) -> ProgramResult;
    fn handle_emergency_halt(&mut self, message: CrossShardMessage) -> ProgramResult;
}

/// Default message handler implementation
pub struct DefaultMessageHandler;

impl MessageHandler for DefaultMessageHandler {
    fn handle_order_routing(&mut self, message: CrossShardMessage) -> ProgramResult {
        msg!("Handling order routing: {:?}", message.message_id);
        // Route order to execution shard
        Ok(())
    }
    
    fn handle_trade_update(&mut self, message: CrossShardMessage) -> ProgramResult {
        msg!("Handling trade update: {:?}", message.message_id);
        // Update analytics and state
        Ok(())
    }
    
    fn handle_settlement_notification(&mut self, message: CrossShardMessage) -> ProgramResult {
        msg!("Handling settlement notification: {:?}", message.message_id);
        // Update all relevant shards
        Ok(())
    }
    
    fn handle_state_sync(&mut self, message: CrossShardMessage) -> ProgramResult {
        msg!("Handling state sync: {:?}", message.message_id);
        // Sync state between shards
        Ok(())
    }
    
    fn handle_rebalance_notification(&mut self, message: CrossShardMessage) -> ProgramResult {
        msg!("Handling rebalance notification: {:?}", message.message_id);
        // Update shard assignments
        Ok(())
    }
    
    fn handle_emergency_halt(&mut self, message: CrossShardMessage) -> ProgramResult {
        msg!("EMERGENCY HALT: {:?}", String::from_utf8_lossy(&message.payload));
        // Halt all operations
        Err(BettingPlatformError::EmergencyHaltTriggered.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_queue() {
        let mut queue = MessageQueue::new(1);
        
        // Test enqueue
        let message = CrossShardMessage::new(
            MessageType::OrderRouting,
            1,
            2,
            Pubkey::new_unique(),
            vec![1, 2, 3],
        ).unwrap();
        
        queue.enqueue(message.clone()).unwrap();
        assert_eq!(queue.messages.len(), 1);
        
        // Test dequeue
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.message_id, message.message_id);
        assert_eq!(queue.messages.len(), 0);
    }
    
    #[test]
    fn test_priority_ordering() {
        let mut queue = MessageQueue::new(1);
        
        // Add messages with different priorities
        let low_priority = CrossShardMessage::new(
            MessageType::StateSync,
            1,
            2,
            Pubkey::new_unique(),
            vec![],
        ).unwrap();
        
        let high_priority = CrossShardMessage::new(
            MessageType::SettlementNotification,
            1,
            2,
            Pubkey::new_unique(),
            vec![],
        ).unwrap();
        
        let critical = CrossShardMessage::new(
            MessageType::EmergencyHalt,
            1,
            2,
            Pubkey::new_unique(),
            vec![],
        ).unwrap();
        
        queue.enqueue(low_priority).unwrap();
        queue.enqueue(high_priority).unwrap();
        queue.enqueue(critical).unwrap();
        
        // Critical should be first
        let first = queue.dequeue().unwrap();
        assert_eq!(first.priority, MessagePriority::Critical);
    }
}