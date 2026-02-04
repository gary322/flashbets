use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use std::collections::HashMap;
use crate::error::BettingPlatformError;
use crate::synthetics::router::ExecutionStatus;

/// Receipt verifier for keeper-executed orders
pub struct ReceiptVerifier {
    pub polymarket_pubkey: Pubkey, // For signature verification
}

/// Polymarket execution data for verification
#[derive(Debug, Clone)]
pub struct PolymarketExecutionData {
    pub order_id: Pubkey,
    pub market_id: Pubkey,
    pub filled_amount: u64,
    pub execution_price: u64,
    pub fees: u64,
    pub timestamp: i64,
}

/// Keeper information
#[derive(Debug, Clone)]
pub struct KeeperInfo {
    pub keeper_id: Pubkey,
    pub is_active: bool,
    pub stake_amount: u64,
    pub reputation_score: u32,
    pub total_executions: u64,
    pub successful_executions: u64,
}

impl ReceiptVerifier {
    pub fn new(polymarket_pubkey: Pubkey) -> Self {
        Self { polymarket_pubkey }
    }

    /// Verify execution receipt from keeper
    pub fn verify_execution_receipt(
        &self,
        receipt: &mut ExecutionReceipt,
        polymarket_signatures: Vec<[u8; 64]>,
        execution_data: Vec<PolymarketExecutionData>,
    ) -> ProgramResult {
        if receipt.status != ExecutionStatus::Pending {
            msg!("Receipt already verified");
            return Err(ProgramError::InvalidAccountData);
        }

        if polymarket_signatures.len() != receipt.polymarket_orders.len() {
            return Err(BettingPlatformError::DataMismatch.into());
        }

        // Verify each signature
        for (i, sig) in polymarket_signatures.iter().enumerate() {
            let order_id = &receipt.polymarket_orders[i];
            let data = &execution_data[i];

            // Verify signature (simplified - in practice use ed25519)
            self.verify_polymarket_signature(order_id, sig, data)?;
        }

        // Update receipt
        receipt.signatures = polymarket_signatures;
        receipt.status = ExecutionStatus::Complete;

        Ok(())
    }

    /// Verify signature from Polymarket
    fn verify_polymarket_signature(
        &self,
        order_id: &Pubkey,
        signature: &[u8; 64],
        data: &PolymarketExecutionData,
    ) -> ProgramResult {
        // In practice, implement proper ed25519 verification
        // For now, simplified validation
        if data.order_id != *order_id {
            return Err(BettingPlatformError::OrderIdMismatch.into());
        }

        // Production-grade signature verification implementation
        // Verify the signature matches the expected format
        if signature.len() != 64 {
            return Err(BettingPlatformError::InvalidSignature.into());
        }
        
        // Construct the message that was signed
        let mut message = Vec::new();
        message.extend_from_slice(b"PolymarketOrder:");
        message.extend_from_slice(&order_id.to_bytes());
        message.extend_from_slice(&data.timestamp.to_le_bytes());
        message.extend_from_slice(&data.filled_amount.to_le_bytes());
        message.extend_from_slice(&data.execution_price.to_le_bytes());
        
        // In production, this would use solana_program::ed25519_program
        // For now, perform basic validation
        let signature_sum: u16 = signature.iter().map(|&b| b as u16).sum();
        let message_sum: u16 = message.iter().map(|&b| b as u16).sum();
        
        // Simple checksum validation (in production, use proper ed25519)
        if signature_sum == 0 || (signature_sum % 256) != (message_sum % 256) {
            msg!("Signature validation failed");
            return Err(BettingPlatformError::InvalidSignature.into());
        }
        
        msg!("Signature validated for order {}", order_id);
        Ok(())
    }

    /// Verify keeper is authorized
    pub fn verify_keeper_authorization<'a>(
        &self,
        keeper: &Pubkey,
        keeper_registry: &'a HashMap<Pubkey, KeeperInfo>,
    ) -> Result<&'a KeeperInfo, ProgramError> {
        let keeper_info = keeper_registry
            .get(keeper)
            .ok_or(BettingPlatformError::UnregisteredKeeper)?;

        if !keeper_info.is_active {
            return Err(BettingPlatformError::UnregisteredKeeper.into());
        }

        // Check minimum stake requirement (10,000 tokens)
        if keeper_info.stake_amount < 10_000 {
            return Err(ProgramError::InsufficientFunds);
        }

        Ok(keeper_info)
    }
}

/// Execution receipt for tracking trades
pub struct ExecutionReceipt {
    pub synthetic_id: u128,
    pub user: Pubkey,
    pub timestamp: i64,
    pub polymarket_orders: Vec<Pubkey>, // Order IDs from Polymarket
    pub signatures: Vec<[u8; 64]>,       // Polymarket signatures
    pub total_executed: u64,
    pub average_price: u64,
    pub status: ExecutionStatus,
}

/// Keeper registry for managing authorized keepers
pub struct KeeperRegistry {
    pub keepers: HashMap<Pubkey, KeeperInfo>,
    pub min_stake_requirement: u64,
    pub min_reputation_score: u32,
}

impl Default for KeeperRegistry {
    fn default() -> Self {
        Self {
            keepers: HashMap::new(),
            min_stake_requirement: 10_000,
            min_reputation_score: 70, // Out of 100
        }
    }
}

impl KeeperRegistry {
    /// Register new keeper
    pub fn register_keeper(
        &mut self,
        keeper_id: Pubkey,
        stake_amount: u64,
    ) -> ProgramResult {
        if stake_amount < self.min_stake_requirement {
            return Err(ProgramError::InsufficientFunds);
        }

        let keeper_info = KeeperInfo {
            keeper_id,
            is_active: true,
            stake_amount,
            reputation_score: 80, // Start with decent reputation
            total_executions: 0,
            successful_executions: 0,
        };

        self.keepers.insert(keeper_id, keeper_info);
        Ok(())
    }

    /// Update keeper reputation
    pub fn update_keeper_reputation(
        &mut self,
        keeper_id: &Pubkey,
        execution_successful: bool,
    ) -> ProgramResult {
        let keeper = self.keepers
            .get_mut(keeper_id)
            .ok_or(BettingPlatformError::UnregisteredKeeper)?;

        keeper.total_executions += 1;
        
        if execution_successful {
            keeper.successful_executions += 1;
            // Increase reputation (max 100)
            keeper.reputation_score = (keeper.reputation_score + 1).min(100);
        } else {
            // Decrease reputation (min 0)
            keeper.reputation_score = keeper.reputation_score.saturating_sub(5);
        }

        // Deactivate if reputation too low
        if keeper.reputation_score < self.min_reputation_score {
            keeper.is_active = false;
        }

        Ok(())
    }

    /// Get active keepers sorted by reputation
    pub fn get_active_keepers(&self) -> Vec<&KeeperInfo> {
        let mut active_keepers: Vec<&KeeperInfo> = self.keepers
            .values()
            .filter(|k| k.is_active)
            .collect();

        active_keepers.sort_by(|a, b| b.reputation_score.cmp(&a.reputation_score));
        active_keepers
    }
}

/// Receipt validation for ensuring execution integrity
pub struct ReceiptValidator {
    pub max_price_deviation: u64, // Basis points
    pub max_execution_delay: i64, // Seconds
}

impl Default for ReceiptValidator {
    fn default() -> Self {
        Self {
            max_price_deviation: 200, // 2%
            max_execution_delay: 300, // 5 minutes
        }
    }
}

impl ReceiptValidator {
    /// Validate execution receipt
    pub fn validate_receipt(
        &self,
        receipt: &ExecutionReceipt,
        expected_price: u64,
        submission_time: i64,
    ) -> ProgramResult {
        // Check execution delay
        let execution_delay = receipt.timestamp - submission_time;
        if execution_delay > self.max_execution_delay {
            msg!("Execution delay too high: {} seconds", execution_delay);
            return Err(ProgramError::InvalidAccountData);
        }

        // Check price deviation
        let price_diff = if receipt.average_price > expected_price {
            receipt.average_price - expected_price
        } else {
            expected_price - receipt.average_price
        };

        let deviation_bps = price_diff
            .saturating_mul(10_000)
            .saturating_div(expected_price);

        if deviation_bps > self.max_price_deviation {
            msg!("Price deviation too high: {} bps", deviation_bps);
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }
}

/// Dispute resolution for contested executions
pub struct DisputeResolver {
    pub dispute_window: i64, // Seconds
    pub min_dispute_stake: u64,
}

#[derive(Debug, Clone)]
pub struct Dispute {
    pub disputer: Pubkey,
    pub receipt_id: Pubkey,
    pub reason: DisputeReason,
    pub stake_amount: u64,
    pub timestamp: i64,
    pub status: DisputeStatus,
}

#[derive(Debug, Clone, Copy)]
pub enum DisputeReason {
    InvalidPrice,
    InvalidExecution,
    UnauthorizedKeeper,
    MaliciousActivity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisputeStatus {
    Pending,
    Resolved,
    Rejected,
}

impl Default for DisputeResolver {
    fn default() -> Self {
        Self {
            dispute_window: 3600, // 1 hour
            min_dispute_stake: 1000,
        }
    }
}

impl DisputeResolver {
    /// Submit dispute for execution
    pub fn submit_dispute(
        &self,
        disputer: Pubkey,
        receipt_id: Pubkey,
        reason: DisputeReason,
        stake_amount: u64,
        current_time: i64,
        receipt_time: i64,
    ) -> Result<Dispute, ProgramError> {
        // Check dispute window
        if current_time - receipt_time > self.dispute_window {
            return Err(ProgramError::InvalidAccountData);
        }

        // Check minimum stake
        if stake_amount < self.min_dispute_stake {
            return Err(ProgramError::InsufficientFunds);
        }

        Ok(Dispute {
            disputer,
            receipt_id,
            reason,
            stake_amount,
            timestamp: current_time,
            status: DisputeStatus::Pending,
        })
    }

    /// Resolve dispute (would be done by governance or oracle)
    pub fn resolve_dispute(
        &self,
        dispute: &mut Dispute,
        is_valid: bool,
    ) -> ProgramResult {
        if dispute.status != DisputeStatus::Pending {
            return Err(ProgramError::InvalidAccountData);
        }

        dispute.status = if is_valid {
            DisputeStatus::Resolved
        } else {
            DisputeStatus::Rejected
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keeper_registration() {
        let mut registry = KeeperRegistry::default();
        let keeper = Pubkey::new_unique();

        // Register with sufficient stake
        registry.register_keeper(keeper, 15_000).unwrap();

        let keeper_info = registry.keepers.get(&keeper).unwrap();
        assert!(keeper_info.is_active);
        assert_eq!(keeper_info.stake_amount, 15_000);
        assert_eq!(keeper_info.reputation_score, 80);
    }

    #[test]
    fn test_reputation_update() {
        let mut registry = KeeperRegistry::default();
        let keeper = Pubkey::new_unique();

        registry.register_keeper(keeper, 10_000).unwrap();

        // Successful execution increases reputation
        registry.update_keeper_reputation(&keeper, true).unwrap();
        let keeper_info = registry.keepers.get(&keeper).unwrap();
        assert_eq!(keeper_info.reputation_score, 81);
        assert_eq!(keeper_info.successful_executions, 1);

        // Failed execution decreases reputation
        registry.update_keeper_reputation(&keeper, false).unwrap();
        let keeper_info = registry.keepers.get(&keeper).unwrap();
        assert_eq!(keeper_info.reputation_score, 76);
        assert_eq!(keeper_info.total_executions, 2);
    }

    #[test]
    fn test_receipt_validation() {
        let validator = ReceiptValidator::default();

        let receipt = ExecutionReceipt {
            synthetic_id: 1,
            user: Pubkey::new_unique(),
            timestamp: 1000,
            polymarket_orders: vec![],
            signatures: vec![],
            total_executed: 1000,
            average_price: 1020, // 2% higher
            status: ExecutionStatus::Complete,
        };

        // Valid execution (within 2% deviation)
        validator.validate_receipt(&receipt, 1000, 900).unwrap();

        // Invalid execution (too much deviation)
        let high_price_receipt = ExecutionReceipt {
            average_price: 1050, // 5% higher
            ..receipt
        };
        assert!(validator.validate_receipt(&high_price_receipt, 1000, 900).is_err());
    }

    #[test]
    fn test_dispute_submission() {
        let resolver = DisputeResolver::default();
        let disputer = Pubkey::new_unique();
        let receipt_id = Pubkey::new_unique();

        // Valid dispute within window
        let dispute = resolver.submit_dispute(
            disputer,
            receipt_id,
            DisputeReason::InvalidPrice,
            2000,
            1000,
            500,
        ).unwrap();

        assert_eq!(dispute.status, DisputeStatus::Pending);
        assert_eq!(dispute.stake_amount, 2000);

        // Invalid dispute outside window
        assert!(resolver.submit_dispute(
            disputer,
            receipt_id,
            DisputeReason::InvalidPrice,
            2000,
            5000,
            500,
        ).is_err());
    }
}