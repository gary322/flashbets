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
    pda::PositionPDA,
    state::accounts::Position,
    math::U64F64,
};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

/// Performance constants for 4k liquidations/sec from Part 7
pub const TARGET_LIQUIDATIONS_PER_SECOND: u32 = 4_000;
pub const LIQUIDATIONS_PER_SLOT: u32 = 1_600; // 4000/sec * 0.4s/slot
pub const PARALLEL_LIQUIDATION_THREADS: usize = 4; // Match shard count
pub const BATCH_SIZE_PER_THREAD: usize = 400; // 1600/4 threads
pub const MAX_QUEUE_SIZE: usize = 10_000;
pub const CU_PER_LIQUIDATION: u64 = 20_000; // From Part 7 spec

/// High-performance liquidation position
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct LiquidationCandidate {
    pub position_id: Pubkey,
    pub user: Pubkey,
    pub market_id: Pubkey,
    pub health_ratio: U64F64,
    pub size: u64,
    pub leverage: u64,
    pub entry_price: u64,
    pub liquidation_price: u64,
    pub priority_score: u64,
    pub added_slot: u64,
}

impl Eq for LiquidationCandidate {}

impl PartialEq for LiquidationCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.position_id == other.position_id
    }
}

impl Ord for LiquidationCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority score = higher priority
        other.priority_score.cmp(&self.priority_score)
            .then_with(|| self.health_ratio.cmp(&other.health_ratio))
    }
}

impl PartialOrd for LiquidationCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Parallel liquidation batch
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct LiquidationBatch {
    pub batch_id: u64,
    pub thread_id: u8,
    pub candidates: Vec<LiquidationCandidate>,
    pub start_slot: u64,
    pub end_slot: u64,
    pub processed_count: u32,
    pub failed_count: u32,
}

/// High-performance liquidation engine state
#[derive(BorshSerialize, BorshDeserialize)]
pub struct LiquidationEngine {
    pub is_active: bool,
    pub total_liquidations_processed: u64,
    pub current_slot: u64,
    pub last_batch_slot: u64,
    pub liquidations_this_slot: u32,
    pub average_processing_time_ms: u64,
    pub queue_size: u32,
    pub thread_states: [ThreadState; PARALLEL_LIQUIDATION_THREADS],
}

impl LiquidationEngine {
    pub const SIZE: usize = 1 + // is_active
        8 + // total_liquidations_processed
        8 + // current_slot
        8 + // last_batch_slot
        4 + // liquidations_this_slot
        8 + // average_processing_time_ms
        4 + // queue_size
        ThreadState::SIZE * PARALLEL_LIQUIDATION_THREADS;
}

/// Thread state for parallel processing
#[derive(BorshSerialize, BorshDeserialize, Clone)]
#[derive(Debug)]
pub struct ThreadState {
    pub thread_id: u8,
    pub is_busy: bool,
    pub current_batch_id: u64,
    pub positions_processed: u32,
    pub positions_failed: u32,
    pub last_update_slot: u64,
}

impl ThreadState {
    pub const SIZE: usize = 1 + 1 + 8 + 4 + 4 + 8;
}

/// Priority queue for liquidations
pub struct LiquidationQueue {
    pub heap: BinaryHeap<LiquidationCandidate>,
    pub position_map: HashMap<Pubkey, LiquidationCandidate>,
    pub max_size: usize,
}

impl LiquidationQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(max_size),
            position_map: HashMap::new(),
            max_size,
        }
    }

    /// Add candidate with priority calculation
    pub fn add_candidate(
        &mut self,
        candidate: LiquidationCandidate,
    ) -> Result<(), ProgramError> {
        if self.heap.len() >= self.max_size {
            // Remove lowest priority if full
            if let Some(lowest) = self.heap.peek() {
                if candidate.priority_score > lowest.priority_score {
                    if let Some(removed) = self.heap.pop() {
                        self.position_map.remove(&removed.position_id);
                    }
                } else {
                    return Err(BettingPlatformError::QueueFull.into());
                }
            }
        }

        self.position_map.insert(candidate.position_id, candidate.clone());
        self.heap.push(candidate);
        Ok(())
    }

    /// Get next batch for processing
    pub fn get_next_batch(&mut self, size: usize) -> Vec<LiquidationCandidate> {
        let mut batch = Vec::with_capacity(size);
        
        for _ in 0..size {
            if let Some(candidate) = self.heap.pop() {
                self.position_map.remove(&candidate.position_id);
                batch.push(candidate);
            } else {
                break;
            }
        }
        
        batch
    }

    /// Remove specific position
    pub fn remove_position(&mut self, position_id: &Pubkey) -> Option<LiquidationCandidate> {
        self.position_map.remove(position_id)
    }
}

/// High-performance liquidation processor
pub struct LiquidationProcessor {
    pub engine: LiquidationEngine,
    pub queue: LiquidationQueue,
}

impl LiquidationProcessor {
    /// Initialize new processor
    pub fn new() -> Self {
        let mut thread_states = vec![];
        for i in 0..PARALLEL_LIQUIDATION_THREADS {
            thread_states.push(ThreadState {
                thread_id: i as u8,
                is_busy: false,
                current_batch_id: 0,
                positions_processed: 0,
                positions_failed: 0,
                last_update_slot: 0,
            });
        }

        Self {
            engine: LiquidationEngine {
                is_active: true,
                total_liquidations_processed: 0,
                current_slot: 0,
                last_batch_slot: 0,
                liquidations_this_slot: 0,
                average_processing_time_ms: 0,
                queue_size: 0,
                thread_states: thread_states.try_into().unwrap(),
            },
            queue: LiquidationQueue::new(MAX_QUEUE_SIZE),
        }
    }

    /// Process liquidations for current slot
    pub fn process_slot(
        &mut self,
        current_slot: u64,
    ) -> Result<ProcessingResult, ProgramError> {
        // Check if new slot
        if current_slot > self.engine.current_slot {
            self.engine.current_slot = current_slot;
            self.engine.liquidations_this_slot = 0;
        }

        // Check if we've hit the limit for this slot
        if self.engine.liquidations_this_slot >= LIQUIDATIONS_PER_SLOT as u32 {
            return Ok(ProcessingResult {
                processed: 0,
                failed: 0,
                remaining_capacity: 0,
            });
        }

        let start_time = Clock::get()?.unix_timestamp as u64;
        let mut total_processed = 0u32;
        let mut total_failed = 0u32;

        // Process in parallel threads
        for thread_id in 0..PARALLEL_LIQUIDATION_THREADS {
            if self.engine.thread_states[thread_id].is_busy {
                continue;
            }

            // Get batch for thread
            let batch = self.queue.get_next_batch(BATCH_SIZE_PER_THREAD);
            if batch.is_empty() {
                continue;
            }

            // Mark thread as busy
            self.engine.thread_states[thread_id].is_busy = true;
            self.engine.thread_states[thread_id].current_batch_id = current_slot * 1000 + thread_id as u64;

            // Process batch (simulated parallel execution)
            let (processed, failed) = self.process_batch(&batch, thread_id)?;
            
            total_processed += processed;
            total_failed += failed;

            // Update thread state
            self.engine.thread_states[thread_id].positions_processed += processed;
            self.engine.thread_states[thread_id].positions_failed += failed;
            self.engine.thread_states[thread_id].last_update_slot = current_slot;
            self.engine.thread_states[thread_id].is_busy = false;
        }

        // Update engine stats
        self.engine.liquidations_this_slot += total_processed;
        self.engine.total_liquidations_processed += total_processed as u64;
        self.engine.queue_size = self.queue.heap.len() as u32;

        // Calculate average processing time
        let elapsed = Clock::get()?.unix_timestamp as u64 - start_time;
        self.engine.average_processing_time_ms = 
            (self.engine.average_processing_time_ms * 9 + elapsed * 1000) / 10;

        let remaining_capacity = LIQUIDATIONS_PER_SLOT
            .saturating_sub(self.engine.liquidations_this_slot);

        msg!(
            "Processed {} liquidations in {}ms ({} failed), {} remaining capacity",
            total_processed,
            elapsed * 1000,
            total_failed,
            remaining_capacity
        );

        Ok(ProcessingResult {
            processed: total_processed,
            failed: total_failed,
            remaining_capacity,
        })
    }

    /// Process a batch of liquidations
    fn process_batch(
        &self,
        batch: &[LiquidationCandidate],
        thread_id: usize,
    ) -> Result<(u32, u32), ProgramError> {
        let mut processed = 0u32;
        let mut failed = 0u32;

        for candidate in batch {
            // Simulate liquidation execution
            match self.execute_liquidation(candidate) {
                Ok(_) => processed += 1,
                Err(e) => {
                    msg!("Liquidation failed for {}: {:?}", candidate.position_id, e);
                    failed += 1;
                }
            }
        }

        msg!(
            "Thread {} processed batch: {} success, {} failed",
            thread_id,
            processed,
            failed
        );

        Ok((processed, failed))
    }

    /// Execute single liquidation
    fn execute_liquidation(
        &self,
        candidate: &LiquidationCandidate,
    ) -> Result<(), ProgramError> {
        // Verify position is still liquidatable
        if candidate.health_ratio > U64F64::from_num(1_000_000) { // 1.0
            return Err(BettingPlatformError::PositionHealthy.into());
        }

        // Simulate liquidation logic
        msg!(
            "Liquidating position {} for user {}, health ratio: {}",
            candidate.position_id,
            candidate.user,
            candidate.health_ratio.to_num()
        );

        // In production, this would:
        // 1. Transfer collateral to liquidator
        // 2. Close position
        // 3. Update user stats
        // 4. Emit events

        Ok(())
    }

    /// Add position to liquidation queue
    pub fn add_to_queue(
        &mut self,
        position: &Position,
        mark_price: u64,
    ) -> Result<(), ProgramError> {
        // Calculate health ratio
        let health_ratio = calculate_health_ratio(position, mark_price)?;
        
        // Only add if unhealthy
        if health_ratio >= U64F64::from_num(1_000_000) { // 1.0
            return Ok(());
        }

        // Calculate priority score
        let priority_score = calculate_priority_score(
            health_ratio,
            position.size,
            position.leverage,
        );

        let candidate = LiquidationCandidate {
            position_id: Pubkey::new_from_array(position.position_id),
            user: position.user,
            market_id: Pubkey::default(), // Position doesn't have market_id field
            health_ratio,
            size: position.size,
            leverage: position.leverage,
            entry_price: position.entry_price,
            liquidation_price: position.liquidation_price,
            priority_score,
            added_slot: Clock::get()?.slot,
        };

        self.queue.add_candidate(candidate)?;
        self.engine.queue_size = self.queue.heap.len() as u32;

        Ok(())
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> PerformanceStats {
        let mut total_thread_processed = 0u32;
        let mut total_thread_failed = 0u32;

        for thread in &self.engine.thread_states {
            total_thread_processed += thread.positions_processed;
            total_thread_failed += thread.positions_failed;
        }

        PerformanceStats {
            total_liquidations: self.engine.total_liquidations_processed,
            current_queue_size: self.engine.queue_size,
            avg_processing_time_ms: self.engine.average_processing_time_ms,
            liquidations_per_second: if self.engine.average_processing_time_ms > 0 {
                (self.engine.liquidations_this_slot as u64 * 1000) 
                    / self.engine.average_processing_time_ms
            } else {
                0
            },
            thread_utilization: self.calculate_thread_utilization(),
            success_rate: if total_thread_processed + total_thread_failed > 0 {
                (total_thread_processed as f64 * 100.0) 
                    / (total_thread_processed + total_thread_failed) as f64
            } else {
                100.0
            },
        }
    }

    /// Calculate thread utilization percentage
    fn calculate_thread_utilization(&self) -> f64 {
        let busy_threads = self.engine.thread_states.iter()
            .filter(|t| t.is_busy)
            .count();
        
        (busy_threads as f64 / PARALLEL_LIQUIDATION_THREADS as f64) * 100.0
    }
}

/// Calculate health ratio for position
fn calculate_health_ratio(
    position: &Position,
    mark_price: u64,
) -> Result<U64F64, ProgramError> {
    let mark_price_fp = U64F64::from_num(mark_price);
    let entry_price_fp = U64F64::from_num(position.entry_price);
    let leverage_fp = U64F64::from_num(position.leverage);
    
    // Calculate P&L
    let price_change = if position.is_long {
        mark_price_fp.checked_sub(entry_price_fp)?
    } else {
        entry_price_fp.checked_sub(mark_price_fp)?
    };
    
    let pnl_ratio = price_change.checked_div(entry_price_fp)?;
    let leveraged_pnl = pnl_ratio.checked_mul(leverage_fp)?;
    
    // Health = 1 + leveraged_pnl
    let health = U64F64::from_num(1_000_000).checked_add(leveraged_pnl)?;
    
    Ok(health)
}

/// Calculate priority score for liquidation
fn calculate_priority_score(
    health_ratio: U64F64,
    size: u64,
    leverage: u64,
) -> u64 {
    // Lower health = higher priority
    let health_score = U64F64::from_num(1_000_000)
        .checked_sub(health_ratio)
        .unwrap_or(U64F64::from_num(0))
        .to_num();
    
    // Larger positions = higher priority
    let size_score = size / 1_000_000; // Normalize
    
    // Higher leverage = higher priority
    let leverage_score = leverage * 100;
    
    // Combined score
    health_score.saturating_add(size_score).saturating_add(leverage_score)
}

/// Processing result
#[derive(Debug)]
pub struct ProcessingResult {
    pub processed: u32,
    pub failed: u32,
    pub remaining_capacity: u32,
}

/// Performance statistics
#[derive(Debug)]
pub struct PerformanceStats {
    pub total_liquidations: u64,
    pub current_queue_size: u32,
    pub avg_processing_time_ms: u64,
    pub liquidations_per_second: u64,
    pub thread_utilization: f64,
    pub success_rate: f64,
}

/// Initialize high-performance liquidation engine
pub fn initialize_liquidation_engine(
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Initializing high-performance liquidation engine");
    msg!("Target: {} liquidations/second", TARGET_LIQUIDATIONS_PER_SECOND);
    msg!("Parallel threads: {}", PARALLEL_LIQUIDATION_THREADS);
    msg!("Batch size per thread: {}", BATCH_SIZE_PER_THREAD);
    
    Ok(())
}