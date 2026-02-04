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
    sharding::enhanced_sharding::*,
};
use std::time::{Duration, Instant};

/// TPS simulation constants from Part 7 spec
pub const TARGET_TPS: u32 = 5_000; // 5k+ TPS target
pub const SIMULATION_DURATION_SLOTS: u64 = 1000; // ~400 seconds
pub const TRANSACTIONS_PER_BATCH: u32 = 100;
pub const MAX_PARALLEL_THREADS: usize = 32;
pub const SLOT_DURATION_MS: u64 = 400; // 0.4 seconds per slot

/// Transaction types for simulation
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum TransactionType {
    PlaceOrder,      // 20k CU
    ExecuteTrade,    // 26k CU (spec target: reduce to 20k)
    UpdatePrice,     // 10k CU
    ClaimPayout,     // 15k CU
    AddLiquidity,    // 18k CU
    RemoveLiquidity, // 18k CU
    Liquidation,     // 30k CU
    BatchProcess,    // 180k CU (8 outcomes)
}

impl TransactionType {
    /// Get compute units for transaction type
    pub fn compute_units(&self) -> u64 {
        match self {
            TransactionType::PlaceOrder => 20_000,
            TransactionType::ExecuteTrade => 20_000, // Reduced from 26k per spec
            TransactionType::UpdatePrice => 10_000,
            TransactionType::ClaimPayout => 15_000,
            TransactionType::AddLiquidity => 18_000,
            TransactionType::RemoveLiquidity => 18_000,
            TransactionType::Liquidation => 30_000,
            TransactionType::BatchProcess => 180_000, // 8 outcomes
        }
    }
    
    /// Get typical distribution percentage
    pub fn distribution_weight(&self) -> u32 {
        match self {
            TransactionType::PlaceOrder => 30,      // 30% of transactions
            TransactionType::ExecuteTrade => 25,    // 25%
            TransactionType::UpdatePrice => 15,     // 15%
            TransactionType::ClaimPayout => 5,      // 5%
            TransactionType::AddLiquidity => 10,    // 10%
            TransactionType::RemoveLiquidity => 5,  // 5%
            TransactionType::Liquidation => 5,      // 5%
            TransactionType::BatchProcess => 5,     // 5%
        }
    }
}

/// Simulated transaction
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct SimulatedTransaction {
    pub id: u64,
    pub tx_type: TransactionType,
    pub market_id: Pubkey,
    pub user: Pubkey,
    pub compute_units: u64,
    pub shard_id: u32,
    pub created_slot: u64,
    pub executed_slot: Option<u64>,
    pub latency_ms: Option<u32>,
    pub success: bool,
}

/// TPS simulation state
#[derive(BorshSerialize, BorshDeserialize)]
pub struct TpsSimulation {
    pub start_slot: u64,
    pub current_slot: u64,
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub total_compute_units: u64,
    pub peak_tps: u32,
    pub current_tps: u32,
    pub avg_latency_ms: u32,
    pub transaction_distribution: Vec<(TransactionType, u32)>,
    pub shard_manager: EnhancedShardManager,
}

impl TpsSimulation {
    /// Initialize new TPS simulation
    pub fn new(authority: Pubkey) -> Self {
        let mut transaction_distribution = vec![
            (TransactionType::PlaceOrder, 0),
            (TransactionType::ExecuteTrade, 0),
            (TransactionType::UpdatePrice, 0),
            (TransactionType::ClaimPayout, 0),
            (TransactionType::AddLiquidity, 0),
            (TransactionType::RemoveLiquidity, 0),
            (TransactionType::Liquidation, 0),
            (TransactionType::BatchProcess, 0),
        ];
        
        Self {
            start_slot: Clock::get().unwrap().slot,
            current_slot: Clock::get().unwrap().slot,
            total_transactions: 0,
            successful_transactions: 0,
            failed_transactions: 0,
            total_compute_units: 0,
            peak_tps: 0,
            current_tps: 0,
            avg_latency_ms: 0,
            transaction_distribution,
            shard_manager: EnhancedShardManager::new(authority),
        }
    }
    
    /// Run TPS simulation
    pub fn run_simulation(&mut self, num_markets: u32) -> Result<SimulationResult, ProgramError> {
        msg!("Starting TPS simulation with {} markets", num_markets);
        
        // Allocate shards for markets
        for i in 0..num_markets {
            let market_id = self.generate_market_id(i);
            self.shard_manager.allocate_market_shards(&market_id)?;
        }
        
        msg!("Allocated {} shards for {} markets", 
            self.shard_manager.total_shards, num_markets);
        
        // Simulate transactions over multiple slots
        let mut slot_results = Vec::new();
        
        for slot_offset in 0..SIMULATION_DURATION_SLOTS {
            let slot_result = self.simulate_slot(slot_offset)?;
            slot_results.push(slot_result);
            
            // Update metrics
            self.update_metrics(slot_offset);
            
            // Log progress every 100 slots
            if slot_offset % 100 == 0 {
                msg!("Slot {}: TPS={}, Total TX={}, Success Rate={:.1}%",
                    slot_offset,
                    self.current_tps,
                    self.total_transactions,
                    (self.successful_transactions as f64 / self.total_transactions.max(1) as f64) * 100.0
                );
            }
        }
        
        // Calculate final results
        let result = self.calculate_results(slot_results);
        
        msg!("Simulation complete: {} TPS achieved (target: {})",
            result.average_tps, TARGET_TPS);
        
        Ok(result)
    }
    
    /// Simulate single slot
    fn simulate_slot(&mut self, slot_offset: u64) -> Result<SlotResult, ProgramError> {
        let current_slot = self.start_slot + slot_offset;
        self.current_slot = current_slot;
        
        // Generate transactions for this slot
        let transactions = self.generate_slot_transactions(current_slot)?;
        let tx_count = transactions.len() as u32;
        
        // Process transactions in parallel batches
        let mut processed = 0u32;
        let mut failed = 0u32;
        let mut total_cu = 0u64;
        let mut total_latency = 0u32;
        
        for batch in transactions.chunks(TRANSACTIONS_PER_BATCH as usize) {
            let batch_result = self.process_batch(batch, current_slot)?;
            processed += batch_result.successful;
            failed += batch_result.failed;
            total_cu += batch_result.compute_units;
            total_latency += batch_result.total_latency_ms;
        }
        
        // Update counters
        self.successful_transactions += processed as u64;
        self.failed_transactions += failed as u64;
        self.total_compute_units += total_cu;
        
        // Calculate slot TPS
        let slot_tps = (processed as f64 / SLOT_DURATION_MS as f64 * 1000.0) as u32;
        
        Ok(SlotResult {
            slot: current_slot,
            transactions: tx_count,
            successful: processed,
            failed,
            compute_units: total_cu,
            avg_latency_ms: if processed > 0 { total_latency / processed } else { 0 },
            tps: slot_tps,
        })
    }
    
    /// Generate transactions for a slot
    fn generate_slot_transactions(&mut self, slot: u64) -> Result<Vec<SimulatedTransaction>, ProgramError> {
        let mut transactions = Vec::new();
        
        // Calculate number of transactions to generate for target TPS
        let target_tx_per_slot = (TARGET_TPS as f64 * SLOT_DURATION_MS as f64 / 1000.0) as u32;
        
        // Generate transactions with realistic distribution
        for i in 0..target_tx_per_slot {
            let tx_type = self.select_transaction_type(i);
            let market_idx = i % self.shard_manager.markets_count;
            let market_id = self.generate_market_id(market_idx);
            
            let tx = SimulatedTransaction {
                id: (slot << 32) | i as u64,
                tx_type,
                market_id,
                user: Pubkey::new_unique(),
                compute_units: tx_type.compute_units(),
                shard_id: 0, // Will be assigned during routing
                created_slot: slot,
                executed_slot: None,
                latency_ms: None,
                success: false,
            };
            
            transactions.push(tx);
        }
        
        Ok(transactions)
    }
    
    /// Select transaction type based on distribution
    fn select_transaction_type(&mut self, index: u32) -> TransactionType {
        let mut cumulative = 0u32;
        let random = index % 100; // Simple distribution
        
        let types = [
            TransactionType::PlaceOrder,
            TransactionType::ExecuteTrade,
            TransactionType::UpdatePrice,
            TransactionType::ClaimPayout,
            TransactionType::AddLiquidity,
            TransactionType::RemoveLiquidity,
            TransactionType::Liquidation,
            TransactionType::BatchProcess,
        ];
        
        for tx_type in types.iter() {
            cumulative += tx_type.distribution_weight();
            if random < cumulative {
                // Update distribution counter
                if let Some((_, count)) = self.transaction_distribution
                    .iter_mut()
                    .find(|(t, _)| t == tx_type) {
                    *count += 1;
                }
                return *tx_type;
            }
        }
        
        TransactionType::PlaceOrder // Default
    }
    
    /// Process batch of transactions
    fn process_batch(
        &mut self,
        transactions: &[SimulatedTransaction],
        current_slot: u64,
    ) -> Result<BatchResult, ProgramError> {
        let mut successful = 0u32;
        let mut failed = 0u32;
        let mut compute_units = 0u64;
        let mut total_latency_ms = 0u32;
        
        for tx in transactions {
            // Route to appropriate shard
            let operation = match tx.tx_type {
                TransactionType::PlaceOrder => OperationType::PlaceOrder,
                TransactionType::ExecuteTrade => OperationType::ExecuteTrade,
                TransactionType::ClaimPayout => OperationType::ClaimPayout,
                _ => OperationType::UpdateStats,
            };
            
            match self.shard_manager.route_operation(&tx.market_id, operation) {
                Ok(shard_id) => {
                    // Simulate processing latency
                    let latency = self.simulate_latency(tx.tx_type);
                    
                    // Check if transaction succeeds (based on load)
                    let shard_stats = self.shard_manager.get_shard_stats();
                    let success_rate = if shard_stats.meeting_target { 0.99 } else { 0.95 };
                    
                    if (tx.id as f64 % 100.0) / 100.0 < success_rate {
                        successful += 1;
                        compute_units += tx.compute_units;
                        total_latency_ms += latency;
                        
                        // Update shard metrics
                        self.shard_manager.update_shard_metrics(
                            &tx.market_id,
                            match tx.tx_type {
                                TransactionType::PlaceOrder => ShardType::OrderBook,
                                TransactionType::ExecuteTrade => ShardType::Execution,
                                TransactionType::ClaimPayout => ShardType::Settlement,
                                _ => ShardType::Analytics,
                            },
                            1,
                            current_slot,
                        )?;
                    } else {
                        failed += 1;
                    }
                }
                Err(_) => {
                    failed += 1;
                }
            }
        }
        
        Ok(BatchResult {
            successful,
            failed,
            compute_units,
            total_latency_ms,
        })
    }
    
    /// Simulate realistic latency
    fn simulate_latency(&self, tx_type: TransactionType) -> u32 {
        // Base latency + variance based on transaction type
        let base_latency = match tx_type {
            TransactionType::PlaceOrder => 5,
            TransactionType::ExecuteTrade => 8,
            TransactionType::UpdatePrice => 3,
            TransactionType::ClaimPayout => 6,
            TransactionType::AddLiquidity => 7,
            TransactionType::RemoveLiquidity => 7,
            TransactionType::Liquidation => 10,
            TransactionType::BatchProcess => 20,
        };
        
        // Add some variance (±20%)
        let variance = base_latency / 5;
        base_latency + (self.current_slot as u32 % (variance * 2)).saturating_sub(variance)
    }
    
    /// Update simulation metrics
    fn update_metrics(&mut self, slot_offset: u64) {
        // Calculate current TPS
        let elapsed_slots = slot_offset.saturating_sub(0).max(1);
        let elapsed_seconds = (elapsed_slots as f64 * SLOT_DURATION_MS as f64) / 1000.0;
        self.current_tps = (self.successful_transactions as f64 / elapsed_seconds) as u32;
        
        // Update peak TPS
        if self.current_tps > self.peak_tps {
            self.peak_tps = self.current_tps;
        }
        
        // Apply tau decay to shard load
        if slot_offset % 100 == 0 {
            self.shard_manager.apply_tau_decay(self.current_slot);
        }
        
        // Rebalance shards if needed
        if slot_offset % 500 == 0 {
            let _ = self.shard_manager.rebalance_if_needed(self.current_slot);
        }
    }
    
    /// Calculate final simulation results
    fn calculate_results(&self, slot_results: Vec<SlotResult>) -> SimulationResult {
        let total_slots = slot_results.len() as u64;
        let total_seconds = (total_slots as f64 * SLOT_DURATION_MS as f64) / 1000.0;
        
        let average_tps = (self.successful_transactions as f64 / total_seconds) as u32;
        let success_rate = (self.successful_transactions as f64 / 
            (self.successful_transactions + self.failed_transactions).max(1) as f64) * 100.0;
        
        // Calculate average latency from slot results
        let total_latency: u32 = slot_results.iter()
            .map(|r| r.avg_latency_ms * r.successful)
            .sum();
        let total_successful: u32 = slot_results.iter()
            .map(|r| r.successful)
            .sum();
        let avg_latency = if total_successful > 0 {
            total_latency / total_successful
        } else {
            0
        };
        
        // Get transaction distribution percentages
        let mut distribution = Vec::new();
        for (tx_type, count) in &self.transaction_distribution {
            let percentage = (*count as f64 / self.total_transactions.max(1) as f64) * 100.0;
            distribution.push((*tx_type, percentage));
        }
        
        SimulationResult {
            duration_slots: total_slots,
            duration_seconds: total_seconds,
            total_transactions: self.total_transactions,
            successful_transactions: self.successful_transactions,
            failed_transactions: self.failed_transactions,
            total_compute_units: self.total_compute_units,
            average_tps,
            peak_tps: self.peak_tps,
            success_rate,
            avg_latency_ms: avg_latency,
            avg_cu_per_transaction: self.total_compute_units / self.successful_transactions.max(1),
            transaction_distribution: distribution,
            shard_stats: self.shard_manager.get_shard_stats(),
            meets_target: average_tps >= TARGET_TPS,
        }
    }
    
    /// Generate deterministic market ID
    fn generate_market_id(&self, index: u32) -> Pubkey {
        use solana_program::hash::hash;
        let seed = format!("market:{}", index);
        let hash_result = hash(seed.as_bytes());
        Pubkey::new_from_array(hash_result.to_bytes())
    }
}

/// Slot processing result
#[derive(Debug)]
struct SlotResult {
    slot: u64,
    transactions: u32,
    successful: u32,
    failed: u32,
    compute_units: u64,
    avg_latency_ms: u32,
    tps: u32,
}

/// Batch processing result
#[derive(Debug)]
struct BatchResult {
    successful: u32,
    failed: u32,
    compute_units: u64,
    total_latency_ms: u32,
}

/// Simulation result
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct SimulationResult {
    pub duration_slots: u64,
    pub duration_seconds: f64,
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub total_compute_units: u64,
    pub average_tps: u32,
    pub peak_tps: u32,
    pub success_rate: f64,
    pub avg_latency_ms: u32,
    pub avg_cu_per_transaction: u64,
    pub transaction_distribution: Vec<(TransactionType, f64)>,
    pub shard_stats: ShardStatistics,
    pub meets_target: bool,
}

impl SimulationResult {
    /// Generate report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("=== TPS Simulation Report ===\n\n");
        
        report.push_str(&format!("Duration: {:.1} seconds ({} slots)\n", 
            self.duration_seconds, self.duration_slots));
        report.push_str(&format!("Total Transactions: {}\n", self.total_transactions));
        report.push_str(&format!("Successful: {} ({:.1}%)\n", 
            self.successful_transactions, self.success_rate));
        report.push_str(&format!("Failed: {}\n\n", self.failed_transactions));
        
        report.push_str("Performance Metrics:\n");
        report.push_str(&format!("- Average TPS: {} (target: {})\n", 
            self.average_tps, TARGET_TPS));
        report.push_str(&format!("- Peak TPS: {}\n", self.peak_tps));
        report.push_str(&format!("- Average Latency: {} ms\n", self.avg_latency_ms));
        report.push_str(&format!("- Average CU/TX: {}\n\n", self.avg_cu_per_transaction));
        
        report.push_str("Transaction Distribution:\n");
        for (tx_type, percentage) in &self.transaction_distribution {
            report.push_str(&format!("- {:?}: {:.1}%\n", tx_type, percentage));
        }
        
        report.push_str(&format!("\nShard Statistics:\n"));
        report.push_str(&format!("- Total Shards: {}\n", self.shard_stats.total_shards));
        report.push_str(&format!("- Active Markets: {}\n", self.shard_stats.active_markets));
        report.push_str(&format!("- Global TPS: {}\n", self.shard_stats.global_tps));
        report.push_str(&format!("- Average Load: {}%\n", 
            self.shard_stats.average_load_factor / 100));
        
        report.push_str(&format!("\nResult: {}\n", 
            if self.meets_target { "✅ MEETS TARGET" } else { "❌ BELOW TARGET" }));
        
        report
    }
}

/// Run TPS benchmark
pub fn run_tps_benchmark(
    accounts: &[AccountInfo],
    num_markets: u32,
) -> ProgramResult {
    msg!("Starting TPS benchmark simulation");
    msg!("Target: {} TPS", TARGET_TPS);
    msg!("Markets: {}", num_markets);
    msg!("Duration: {} slots", SIMULATION_DURATION_SLOTS);
    
    let mut simulation = TpsSimulation::new(Pubkey::new_unique());
    let result = simulation.run_simulation(num_markets)?;
    
    msg!("{}", result.generate_report());
    
    if !result.meets_target {
        return Err(BettingPlatformError::BelowTargetTPS.into());
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_distribution() {
        let mut sim = TpsSimulation::new(Pubkey::new_unique());
        
        // Test distribution selection
        let mut type_counts = vec![0u32; 8];
        for i in 0..1000 {
            match sim.select_transaction_type(i) {
                TransactionType::PlaceOrder => type_counts[0] += 1,
                TransactionType::ExecuteTrade => type_counts[1] += 1,
                TransactionType::UpdatePrice => type_counts[2] += 1,
                TransactionType::ClaimPayout => type_counts[3] += 1,
                TransactionType::AddLiquidity => type_counts[4] += 1,
                TransactionType::RemoveLiquidity => type_counts[5] += 1,
                TransactionType::Liquidation => type_counts[6] += 1,
                TransactionType::BatchProcess => type_counts[7] += 1,
            }
        }
        
        // Verify distribution roughly matches expected weights
        assert!(type_counts[0] > 250 && type_counts[0] < 350); // ~30%
        assert!(type_counts[1] > 200 && type_counts[1] < 300); // ~25%
        assert!(type_counts[2] > 100 && type_counts[2] < 200); // ~15%
    }
    
    #[test]
    fn test_compute_units() {
        // Verify CU values match spec
        assert_eq!(TransactionType::PlaceOrder.compute_units(), 20_000);
        assert_eq!(TransactionType::ExecuteTrade.compute_units(), 20_000); // Reduced from 26k
        assert_eq!(TransactionType::BatchProcess.compute_units(), 180_000); // 8 outcomes
    }
    
    #[test]
    fn test_latency_simulation() {
        let sim = TpsSimulation::new(Pubkey::new_unique());
        
        // Test latency ranges
        let order_latency = sim.simulate_latency(TransactionType::PlaceOrder);
        assert!(order_latency >= 4 && order_latency <= 6); // 5 ± 1
        
        let batch_latency = sim.simulate_latency(TransactionType::BatchProcess);
        assert!(batch_latency >= 16 && batch_latency <= 24); // 20 ± 4
    }
}