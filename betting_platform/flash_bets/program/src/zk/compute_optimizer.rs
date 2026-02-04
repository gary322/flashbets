use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    msg,
    program_error::ProgramError,
};
use crate::errors::FlashError;

/// Flash compute optimization for sub-10 second verification
pub struct FlashCompute;

impl FlashCompute {
    /// Base compute units for proof verification
    pub const BASE_UNITS: u32 = 200_000;
    
    /// Units per proof element
    pub const UNITS_PER_ELEMENT: u32 = 1_500;
    
    /// Maximum compute budget for flash verification
    pub const MAX_COMPUTE_UNITS: u32 = 400_000;
    
    /// Optimize compute allocation for flash proof size
    pub fn optimize_for_proof_size(proof_size: usize) -> u32 {
        let base = Self::BASE_UNITS;
        let variable = (proof_size / 32) as u32 * Self::UNITS_PER_ELEMENT;
        (base + variable).min(Self::MAX_COMPUTE_UNITS)
    }
    
    /// Calculate verification time estimate (microseconds)
    pub fn estimate_verification_time(proof_size: usize) -> u64 {
        // Base: 50ms for setup
        let base_time = 50_000u64;
        
        // Variable: 200μs per 32 bytes of proof
        let variable_time = (proof_size / 32) as u64 * 200;
        
        base_time + variable_time
    }
    
    /// Check if proof can be verified within flash constraints
    pub fn can_verify_in_flash_time(proof_size: usize, time_left: u64) -> bool {
        let verification_time = Self::estimate_verification_time(proof_size);
        let time_left_micros = time_left * 1_000_000; // Convert seconds to microseconds
        
        // Require verification to complete in < 50% of remaining time
        verification_time < time_left_micros / 2
    }
}

/// Request additional compute units for large proofs
pub fn request_compute_units(
    proof_size: usize,
    _accounts: &[AccountInfo],
) -> ProgramResult {
    let required_units = FlashCompute::optimize_for_proof_size(proof_size);
    
    if required_units > FlashCompute::BASE_UNITS {
        msg!("Requesting {} compute units for proof verification", required_units);
        
        // In a real implementation, would use ComputeBudgetProgram
        // For now, just validate we have enough
        if required_units > FlashCompute::MAX_COMPUTE_UNITS {
            return Err(FlashError::InsufficientLiquidity.into()); // Reuse existing error
        }
    }
    
    Ok(())
}

/// Validate proof size constraints for flash markets
pub fn validate_flash_proof_constraints(
    proof: &[u8],
    time_left: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let proof_size = proof.len();
    
    // Minimum proof size (simplified Groth16)
    if proof_size < 96 {
        msg!("Proof too small: {} bytes", proof_size);
        return Err(FlashError::InvalidProof.into());
    }
    
    // Maximum proof size for flash verification
    if proof_size > 2048 {
        msg!("Proof too large for flash verification: {} bytes", proof_size);
        return Err(FlashError::InvalidProof.into());
    }
    
    // Check alignment (proofs should be 32-byte aligned)
    if proof_size % 32 != 0 {
        msg!("Proof not aligned to 32 bytes");
        return Err(FlashError::InvalidProof.into());
    }
    
    // Check if verification can complete in time
    if !FlashCompute::can_verify_in_flash_time(proof_size, time_left) {
        msg!("Insufficient time for proof verification");
        return Err(FlashError::MarketExpired.into());
    }
    
    // Request additional compute if needed
    request_compute_units(proof_size, accounts)?;
    
    Ok(())
}

/// Optimized verification for micro-tau flash markets
pub fn verify_flash_optimized(
    proof: &[u8],
    public_inputs: &[u8],
    verification_key: &[u8],
) -> Result<bool, ProgramError> {
    // Simplified verification for flash markets
    // In production, this would use actual ZK verification libraries
    
    if proof.len() < 96 || public_inputs.is_empty() || verification_key.len() < 32 {
        return Ok(false);
    }
    
    // Basic consistency checks
    let proof_hash = {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(proof);
        hasher.update(public_inputs);
        hasher.update(verification_key);
        hasher.finalize()
    };
    
    // For flash markets, use simplified verification
    // Check if first 32 bytes of proof match expected pattern
    let expected = proof_hash[0..32].to_vec();
    let actual = proof[0..32].to_vec();
    
    Ok(expected == actual)
}

/// Cache for verifying keys to reduce loading costs
pub struct VKCache {
    cached_vk: Option<Vec<u8>>,
    cache_slot: u64,
    ttl_slots: u64,
}

impl VKCache {
    pub fn new(ttl_slots: u64) -> Self {
        Self {
            cached_vk: None,
            cache_slot: 0,
            ttl_slots,
        }
    }
    
    /// Get cached VK if still valid
    pub fn get(&self, current_slot: u64) -> Option<&Vec<u8>> {
        if current_slot - self.cache_slot <= self.ttl_slots {
            self.cached_vk.as_ref()
        } else {
            None
        }
    }
    
    /// Update cache
    pub fn set(&mut self, vk: Vec<u8>, current_slot: u64) {
        self.cached_vk = Some(vk);
        self.cache_slot = current_slot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_compute_optimization() {
        let small_proof = FlashCompute::optimize_for_proof_size(96);
        assert_eq!(small_proof, FlashCompute::BASE_UNITS + 3 * FlashCompute::UNITS_PER_ELEMENT);
        
        let large_proof = FlashCompute::optimize_for_proof_size(1024);
        assert!(large_proof <= FlashCompute::MAX_COMPUTE_UNITS);
    }
    
    #[test]
    fn test_verification_time_estimate() {
        let time_96_bytes = FlashCompute::estimate_verification_time(96);
        assert_eq!(time_96_bytes, 50_000 + 3 * 200); // Base + 3 elements
        
        let time_1024_bytes = FlashCompute::estimate_verification_time(1024);
        assert_eq!(time_1024_bytes, 50_000 + 32 * 200); // Base + 32 elements
    }
    
    #[test]
    fn test_flash_time_constraint() {
        // 96-byte proof should verify in 1 minute
        assert!(FlashCompute::can_verify_in_flash_time(96, 60));
        
        // Large proof might not verify in 5 seconds
        assert!(!FlashCompute::can_verify_in_flash_time(2048, 5));
    }
}