//! Chain Event Logging for Audit Trails
//!
//! Implements ChainEvent(step, r_i, eff_lev) logging as specified in Q30

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    msg,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::{
    events::{Event, EventType},
    instruction::ChainStepType,
    math::U64F64,
};

/// Enhanced chain event with step returns and effective leverage
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainEvent {
    /// Chain ID
    pub chain_id: u128,
    /// User public key
    pub user: Pubkey,
    /// Step index in the chain
    pub step: u8,
    /// Step type (Borrow, Liquidate, Stake, etc.)
    pub step_type: ChainStepType,
    /// Return from this step (r_i) in basis points
    pub step_return_bps: i64,
    /// Effective leverage after this step (eff_lev)
    pub effective_leverage: u64,
    /// Base leverage
    pub base_leverage: u64,
    /// Cumulative multiplier (product of all (1 + r_i))
    pub cumulative_multiplier: U64F64,
    /// Amount processed in this step
    pub step_amount: u64,
    /// Current balance after step
    pub current_balance: u64,
    /// Timestamp
    pub timestamp: i64,
    /// Slot
    pub slot: u64,
}

impl Event for ChainEvent {
    fn event_type() -> EventType {
        EventType::ChainStepExecuted
    }
}

/// Chain audit trail to track all steps
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainAuditTrail {
    /// Chain ID
    pub chain_id: u128,
    /// All step events
    pub steps: Vec<ChainStepSummary>,
    /// Final effective leverage
    pub final_effective_leverage: u64,
    /// Total return across all steps
    pub total_return_bps: i64,
    /// Success status
    pub success: bool,
}

/// Summary of a single chain step for audit
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct ChainStepSummary {
    pub step: u8,
    pub step_type: ChainStepType,
    pub r_i: i64, // Return in basis points
    pub eff_lev: u64, // Effective leverage after this step
}

/// Log a chain event with all required data
pub fn log_chain_event(
    chain_id: u128,
    user: &Pubkey,
    step: u8,
    step_type: ChainStepType,
    step_return_bps: i64,
    effective_leverage: u64,
    base_leverage: u64,
    cumulative_multiplier: U64F64,
    step_amount: u64,
    current_balance: u64,
) {
    let clock = Clock::get().unwrap();
    
    let event = ChainEvent {
        chain_id,
        user: *user,
        step,
        step_type: step_type.clone(),
        step_return_bps,
        effective_leverage,
        base_leverage,
        cumulative_multiplier,
        step_amount,
        current_balance,
        timestamp: clock.unix_timestamp,
        slot: clock.slot,
    };
    
    // Log structured data for off-chain indexing
    msg!("CHAIN_EVENT_V2");
    msg!("chain_id: {}", chain_id);
    msg!("step: {}", step);
    msg!("r_i: {}", step_return_bps);
    msg!("eff_lev: {}", effective_leverage);
    msg!("step_type: {:?}", step_type);
    
    // Emit full event
    event.emit();
}

/// Calculate step return (r_i) based on step type and outcome
pub fn calculate_step_return(
    step_type: &ChainStepType,
    initial_amount: u64,
    final_amount: u64,
) -> i64 {
    if initial_amount == 0 {
        return 0;
    }
    
    // Calculate return as (final - initial) / initial * 10000 (basis points)
    let return_ratio = if final_amount >= initial_amount {
        let profit = final_amount - initial_amount;
        (profit as i128 * 10000 / initial_amount as i128) as i64
    } else {
        let loss = initial_amount - final_amount;
        -((loss as i128 * 10000 / initial_amount as i128) as i64)
    };
    
    return_ratio
}

/// Build complete audit trail for a chain
pub fn build_chain_audit_trail(
    chain_id: u128,
    steps: Vec<ChainStepSummary>,
    final_effective_leverage: u64,
    success: bool,
) -> ChainAuditTrail {
    let total_return_bps = steps.iter()
        .map(|s| s.r_i)
        .fold(0i64, |acc, r| {
            // Compound returns: (1 + acc/10000) * (1 + r/10000) - 1
            let factor = ((10000 + acc) as i128 * (10000 + r) as i128) / 10000;
            (factor - 10000) as i64
        });
    
    ChainAuditTrail {
        chain_id,
        steps,
        final_effective_leverage,
        total_return_bps,
        success,
    }
}

/// Store chain audit trail on-chain (compressed)
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ChainAuditAccount {
    /// Discriminator
    pub discriminator: [u8; 8],
    /// Chain ID
    pub chain_id: u128,
    /// User
    pub user: Pubkey,
    /// Creation slot
    pub created_slot: u64,
    /// Number of steps
    pub num_steps: u8,
    /// Step data (compressed)
    pub step_data: Vec<u8>,
    /// Final metrics
    pub final_effective_leverage: u64,
    pub total_return_bps: i64,
    /// IPFS hash for full data
    pub ipfs_hash: Option<[u8; 32]>,
}

impl ChainAuditAccount {
    pub const DISCRIMINATOR: [u8; 8] = [0x43, 0x68, 0x61, 0x69, 0x6E, 0x41, 0x75, 0x64]; // "ChainAud"
    
    /// Compress step data for on-chain storage
    pub fn compress_steps(steps: &[ChainStepSummary]) -> Vec<u8> {
        let mut compressed = Vec::new();
        
        for step in steps {
            // Pack step data efficiently
            compressed.push(step.step);
            compressed.push(match &step.step_type {
                ChainStepType::Long { .. } => 0,
                ChainStepType::Short { .. } => 1,
                ChainStepType::Lend { .. } => 2,
                ChainStepType::Borrow { .. } => 3,
                ChainStepType::Liquidity { .. } => 4,
                ChainStepType::Stake { .. } => 5,
                ChainStepType::ClosePosition => 6,
                ChainStepType::TakeProfit { .. } => 7,
                ChainStepType::StopLoss { .. } => 8,
            });
            // Store r_i as i16 (divide by 100 to fit)
            let r_i_compressed = (step.r_i / 100).max(i16::MIN as i64).min(i16::MAX as i64) as i16;
            compressed.extend_from_slice(&r_i_compressed.to_le_bytes());
            // Store eff_lev as u16 (good up to 655x)
            let eff_lev_compressed = step.eff_lev.min(u16::MAX as u64) as u16;
            compressed.extend_from_slice(&eff_lev_compressed.to_le_bytes());
        }
        
        compressed
    }
    
    /// Decompress step data
    pub fn decompress_steps(&self) -> Vec<ChainStepSummary> {
        let mut steps = Vec::new();
        let mut offset = 0;
        
        while offset + 6 <= self.step_data.len() {
            let step = self.step_data[offset];
            let step_type_byte = self.step_data[offset + 1];
            let r_i_bytes = &self.step_data[offset + 2..offset + 4];
            let eff_lev_bytes = &self.step_data[offset + 4..offset + 6];
            
            let r_i = i16::from_le_bytes(r_i_bytes.try_into().unwrap()) as i64 * 100;
            let eff_lev = u16::from_le_bytes(eff_lev_bytes.try_into().unwrap()) as u64;
            
            steps.push(ChainStepSummary {
                step,
                step_type: ChainStepType::from_u8(step_type_byte).unwrap_or(ChainStepType::Borrow { amount: 0 }),
                r_i,
                eff_lev,
            });
            
            offset += 6;
        }
        
        steps
    }
}

/// Helper to emit chain completion event with full audit trail
pub fn emit_chain_completion(
    chain_id: u128,
    user: &Pubkey,
    audit_trail: ChainAuditTrail,
) {
    msg!("CHAIN_COMPLETION_AUDIT");
    msg!("chain_id: {}", chain_id);
    msg!("user: {}", user);
    msg!("steps: {}", audit_trail.steps.len());
    msg!("final_eff_lev: {}", audit_trail.final_effective_leverage);
    msg!("total_return_bps: {}", audit_trail.total_return_bps);
    msg!("success: {}", audit_trail.success);
    
    // Log each step for complete audit trail
    for (i, step) in audit_trail.steps.iter().enumerate() {
        msg!("  step[{}]: type={:?}, r_i={}, eff_lev={}", 
            i, step.step_type, step.r_i, step.eff_lev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_step_return_calculation() {
        // 10% profit
        let return_bps = calculate_step_return(&ChainStepType::Borrow { amount: 1000 }, 1000, 1100);
        assert_eq!(return_bps, 1000); // 10% = 1000 bps
        
        // 5% loss
        let return_bps = calculate_step_return(&ChainStepType::Long { outcome: 0, leverage: 1 }, 1000, 950);
        assert_eq!(return_bps, -500); // -5% = -500 bps
    }
    
    #[test]
    fn test_audit_compression() {
        let steps = vec![
            ChainStepSummary {
                step: 0,
                step_type: ChainStepType::Borrow { amount: 1000 },
                r_i: 1500, // 15%
                eff_lev: 150, // 1.5x
            },
            ChainStepSummary {
                step: 1,
                step_type: ChainStepType::Stake { amount: 2000 },
                r_i: 1000, // 10%
                eff_lev: 165, // 1.65x
            },
        ];
        
        let compressed = ChainAuditAccount::compress_steps(&steps);
        assert_eq!(compressed.len(), 12); // 6 bytes per step
        
        // Test decompression
        let audit = ChainAuditAccount {
            discriminator: ChainAuditAccount::DISCRIMINATOR,
            chain_id: 12345,
            user: Pubkey::new_unique(),
            created_slot: 1000,
            num_steps: 2,
            step_data: compressed,
            final_effective_leverage: 165,
            total_return_bps: 2650,
            ipfs_hash: None,
        };
        
        let decompressed = audit.decompress_steps();
        assert_eq!(decompressed.len(), 2);
        assert_eq!(decompressed[0].r_i, 1500);
        assert_eq!(decompressed[1].eff_lev, 165);
    }
}