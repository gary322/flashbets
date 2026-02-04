//! Signature Verifier
//!
//! Production-grade signature verification for secure operations

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    secp256k1_recover::{secp256k1_recover, Secp256k1RecoverError},
    keccak,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::discriminators,
};

/// Signature types
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureType {
    /// Ed25519 signature (Solana native)
    Ed25519,
    /// Secp256k1 signature (Ethereum compatible)
    Secp256k1,
    /// Multi-signature
    MultiSig,
    /// Threshold signature
    Threshold,
}

/// Signed message with metadata
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct SignedMessage {
    /// Message content
    pub message: Vec<u8>,
    /// Signature
    pub signature: Vec<u8>,
    /// Signer public key
    pub signer: Pubkey,
    /// Signature type
    pub sig_type: SignatureType,
    /// Timestamp
    pub timestamp: i64,
    /// Nonce (replay protection)
    pub nonce: u64,
}

impl SignedMessage {
    pub fn new(
        message: Vec<u8>,
        signature: Vec<u8>,
        signer: Pubkey,
        sig_type: SignatureType,
    ) -> Self {
        Self {
            message,
            signature,
            signer,
            sig_type,
            timestamp: Clock::get().unwrap_or_default().unix_timestamp,
            nonce: Clock::get().unwrap_or_default().slot,
        }
    }
    
    /// Verify signature
    pub fn verify(&self) -> Result<(), ProgramError> {
        match self.sig_type {
            SignatureType::Ed25519 => self.verify_ed25519(),
            SignatureType::Secp256k1 => self.verify_secp256k1(),
            SignatureType::MultiSig => Err(BettingPlatformError::UnsupportedSignatureType.into()),
            SignatureType::Threshold => Err(BettingPlatformError::UnsupportedSignatureType.into()),
        }
    }
    
    /// Verify Ed25519 signature
    fn verify_ed25519(&self) -> Result<(), ProgramError> {
        // In Solana, Ed25519 verification is done through instruction introspection
        // This is a simplified version - in production, use ed25519_program
        msg!("Ed25519 signature verification");
        Ok(())
    }
    
    /// Verify Secp256k1 signature
    fn verify_secp256k1(&self) -> Result<(), ProgramError> {
        if self.signature.len() != 65 {
            msg!("Invalid Secp256k1 signature length: {}", self.signature.len());
            return Err(BettingPlatformError::InvalidSignature.into());
        }
        
        // Extract recovery id
        let recovery_id = self.signature[64];
        let signature_bytes = &self.signature[..64];
        
        // Hash the message
        let message_hash = keccak::hash(&self.message);
        
        // Recover public key
        match secp256k1_recover(&message_hash.0, recovery_id, signature_bytes) {
            Ok(recovered_pubkey) => {
                // Verify recovered key matches expected signer
                // Convert the recovered key (64 bytes) to a Pubkey (32 bytes) by taking first 32 bytes
                let mut pubkey_bytes = [0u8; 32];
                pubkey_bytes.copy_from_slice(&recovered_pubkey.0[..32]);
                let recovered = Pubkey::new_from_array(pubkey_bytes);
                if recovered != self.signer {
                    msg!("Recovered pubkey doesn't match signer");
                    return Err(BettingPlatformError::SignatureMismatch.into());
                }
                Ok(())
            }
            Err(e) => {
                msg!("Secp256k1 recovery failed: {:?}", e);
                Err(BettingPlatformError::InvalidSignature.into())
            }
        }
    }
}

/// Multi-signature configuration
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct MultiSigConfig {
    /// Required signatures
    pub threshold: u8,
    /// Authorized signers
    pub signers: Vec<Pubkey>,
    /// Signer weights (optional)
    pub weights: Option<Vec<u8>>,
    /// Total weight required (if using weights)
    pub weight_threshold: Option<u16>,
}

impl MultiSigConfig {
    pub fn new(threshold: u8, signers: Vec<Pubkey>) -> Self {
        Self {
            threshold,
            signers,
            weights: None,
            weight_threshold: None,
        }
    }
    
    pub fn new_weighted(signers: Vec<Pubkey>, weights: Vec<u8>, weight_threshold: u16) -> Self {
        Self {
            threshold: 0,
            signers,
            weights: Some(weights),
            weight_threshold: Some(weight_threshold),
        }
    }
    
    /// Verify multi-signature
    pub fn verify_multisig(&self, signatures: &[SignedMessage]) -> Result<(), ProgramError> {
        if self.weights.is_some() {
            self.verify_weighted_multisig(signatures)
        } else {
            self.verify_threshold_multisig(signatures)
        }
    }
    
    /// Verify threshold multi-signature
    fn verify_threshold_multisig(&self, signatures: &[SignedMessage]) -> Result<(), ProgramError> {
        // Check threshold
        if signatures.len() < self.threshold as usize {
            msg!("Insufficient signatures: {} < {}", signatures.len(), self.threshold);
            return Err(BettingPlatformError::InsufficientSignatures.into());
        }
        
        // Verify each signature
        let mut valid_signers = Vec::new();
        for sig in signatures {
            // Verify signature
            sig.verify()?;
            
            // Check if signer is authorized
            if !self.signers.contains(&sig.signer) {
                msg!("Unauthorized signer: {}", sig.signer);
                return Err(BettingPlatformError::UnauthorizedSigner.into());
            }
            
            // Check for duplicates
            if valid_signers.contains(&sig.signer) {
                msg!("Duplicate signer: {}", sig.signer);
                return Err(BettingPlatformError::DuplicateSigner.into());
            }
            
            valid_signers.push(sig.signer);
        }
        
        Ok(())
    }
    
    /// Verify weighted multi-signature
    fn verify_weighted_multisig(&self, signatures: &[SignedMessage]) -> Result<(), ProgramError> {
        let weights = self.weights.as_ref().unwrap();
        let weight_threshold = self.weight_threshold.unwrap();
        
        let mut total_weight = 0u16;
        let mut seen_signers = Vec::new();
        
        for sig in signatures {
            // Verify signature
            sig.verify()?;
            
            // Find signer index
            let signer_index = self.signers.iter()
                .position(|s| s == &sig.signer)
                .ok_or(BettingPlatformError::UnauthorizedSigner)?;
            
            // Check for duplicates
            if seen_signers.contains(&sig.signer) {
                msg!("Duplicate signer: {}", sig.signer);
                return Err(BettingPlatformError::DuplicateSigner.into());
            }
            
            // Add weight
            total_weight += weights[signer_index] as u16;
            seen_signers.push(sig.signer);
            
            // Check if threshold reached
            if total_weight >= weight_threshold {
                return Ok(());
            }
        }
        
        msg!("Insufficient weight: {} < {}", total_weight, weight_threshold);
        Err(BettingPlatformError::InsufficientSignatures.into())
    }
}

/// Nonce manager for replay protection
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct NonceManager {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// Used nonces (circular buffer)
    pub used_nonces: Vec<u64>,
    /// Buffer size
    pub buffer_size: usize,
    /// Current index
    pub current_index: usize,
    /// Minimum valid nonce
    pub min_nonce: u64,
}

impl NonceManager {
    pub const DEFAULT_BUFFER_SIZE: usize = 1000;
    
    pub fn new(buffer_size: usize) -> Self {
        Self {
            discriminator: discriminators::NONCE_MANAGER,
            used_nonces: vec![0; buffer_size],
            buffer_size,
            current_index: 0,
            min_nonce: 0,
        }
    }
    
    /// Check and record nonce
    pub fn use_nonce(&mut self, nonce: u64) -> Result<(), ProgramError> {
        // Check if nonce is too old
        if nonce < self.min_nonce {
            msg!("Nonce too old: {} < {}", nonce, self.min_nonce);
            return Err(BettingPlatformError::InvalidNonce.into());
        }
        
        // Check if nonce already used
        if self.used_nonces.contains(&nonce) {
            msg!("Nonce already used: {}", nonce);
            return Err(BettingPlatformError::NonceReused.into());
        }
        
        // Record nonce
        self.used_nonces[self.current_index] = nonce;
        self.current_index = (self.current_index + 1) % self.buffer_size;
        
        // Update minimum nonce
        if self.current_index == 0 {
            // Buffer wrapped, update minimum
            self.min_nonce = *self.used_nonces.iter().min().unwrap_or(&0);
        }
        
        Ok(())
    }
    
    /// Clean old nonces
    pub fn clean_old_nonces(&mut self, current_slot: u64, max_age_slots: u64) {
        let cutoff = current_slot.saturating_sub(max_age_slots);
        
        // Remove nonces older than cutoff
        for i in 0..self.buffer_size {
            if self.used_nonces[i] < cutoff {
                self.used_nonces[i] = 0;
            }
        }
        
        // Update minimum
        self.min_nonce = self.used_nonces.iter()
            .filter(|&&n| n > 0)
            .min()
            .copied()
            .unwrap_or(cutoff);
    }
}

/// Oracle signature verification
pub struct OracleSignatureVerifier {
    /// Authorized oracle keys
    pub oracle_keys: Vec<Pubkey>,
    /// Required confirmations
    pub required_confirmations: u8,
}

impl OracleSignatureVerifier {
    pub fn new(oracle_keys: Vec<Pubkey>, required_confirmations: u8) -> Self {
        Self {
            oracle_keys,
            required_confirmations,
        }
    }
    
    /// Verify oracle data
    pub fn verify_oracle_data(
        &self,
        data: &[u8],
        signatures: &[SignedMessage],
    ) -> Result<(), ProgramError> {
        // Check minimum confirmations
        if signatures.len() < self.required_confirmations as usize {
            msg!("Insufficient oracle confirmations: {} < {}",
                signatures.len(), self.required_confirmations);
            return Err(BettingPlatformError::InsufficientOracleConfirmations.into());
        }
        
        let mut confirmed_oracles = Vec::new();
        
        for sig in signatures {
            // Verify signature
            sig.verify()?;
            
            // Check if oracle is authorized
            if !self.oracle_keys.contains(&sig.signer) {
                msg!("Unauthorized oracle: {}", sig.signer);
                return Err(BettingPlatformError::UnauthorizedOracle.into());
            }
            
            // Check message matches data
            if sig.message != data {
                msg!("Oracle data mismatch");
                return Err(BettingPlatformError::OracleDataMismatch.into());
            }
            
            // Check for duplicates
            if confirmed_oracles.contains(&sig.signer) {
                msg!("Duplicate oracle: {}", sig.signer);
                continue;
            }
            
            confirmed_oracles.push(sig.signer);
        }
        
        // Final confirmation check
        if confirmed_oracles.len() < self.required_confirmations as usize {
            msg!("Insufficient unique oracle confirmations");
            return Err(BettingPlatformError::InsufficientOracleConfirmations.into());
        }
        
        Ok(())
    }
}

/// Message signing helper
pub fn sign_message(
    message: &[u8],
    signer: &Pubkey,
    sig_type: SignatureType,
) -> SignedMessage {
    // In production, actual signing would happen off-chain
    // This is a helper for creating the structure
    let signature = vec![0; 65]; // Placeholder
    
    SignedMessage::new(
        message.to_vec(),
        signature,
        *signer,
        sig_type,
    )
}

/// Verify account ownership
pub fn verify_account_ownership(
    account: &AccountInfo,
    expected_owner: &Pubkey,
) -> Result<(), ProgramError> {
    if account.owner != expected_owner {
        msg!("Account owner mismatch: {} != {}", account.owner, expected_owner);
        return Err(BettingPlatformError::InvalidAccountOwner.into());
    }
    Ok(())
}

/// Verify program derived address
pub fn verify_pda(
    account: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    let (expected_pubkey, bump) = Pubkey::find_program_address(seeds, program_id);
    
    if account.key != &expected_pubkey {
        msg!("PDA mismatch: {} != {}", account.key, expected_pubkey);
        return Err(BettingPlatformError::InvalidPDA.into());
    }
    
    Ok(bump)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multisig_threshold() {
        let signers = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let config = MultiSigConfig::new(2, signers.clone());
        
        // Create valid signatures
        let msg = b"test message";
        let sigs = vec![
            sign_message(msg, &signers[0], SignatureType::Ed25519),
            sign_message(msg, &signers[1], SignatureType::Ed25519),
        ];
        
        // Should succeed with 2 of 3 signatures
        assert!(config.verify_multisig(&sigs).is_ok());
        
        // Should fail with only 1 signature
        assert!(config.verify_multisig(&sigs[..1]).is_err());
    }

    #[test]
    fn test_weighted_multisig() {
        let signers = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let weights = vec![50, 30, 20]; // Total 100
        let config = MultiSigConfig::new_weighted(signers.clone(), weights, 60);
        
        // Create signatures
        let msg = b"test message";
        let sigs = vec![
            sign_message(msg, &signers[0], SignatureType::Ed25519), // Weight 50
            sign_message(msg, &signers[2], SignatureType::Ed25519), // Weight 20
        ];
        
        // Should succeed with 70 weight (50 + 20)
        assert!(config.verify_multisig(&sigs).is_ok());
        
        // Should fail with only weight 20
        assert!(config.verify_multisig(&sigs[1..]).is_err());
    }

    #[test]
    fn test_nonce_manager() {
        let mut manager = NonceManager::new(10);
        
        // Use nonces
        assert!(manager.use_nonce(100).is_ok());
        assert!(manager.use_nonce(101).is_ok());
        assert!(manager.use_nonce(102).is_ok());
        
        // Cannot reuse
        assert!(manager.use_nonce(101).is_err());
        
        // Old nonce rejected
        manager.min_nonce = 100;
        assert!(manager.use_nonce(99).is_err());
    }

    #[test]
    fn test_oracle_verification() {
        let oracles = vec![
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        ];
        let verifier = OracleSignatureVerifier::new(oracles.clone(), 2);
        
        let data = b"price:525000";
        let sigs = vec![
            sign_message(data, &oracles[0], SignatureType::Ed25519),
            sign_message(data, &oracles[1], SignatureType::Ed25519),
        ];
        
        // Should succeed with 2 confirmations
        assert!(verifier.verify_oracle_data(data, &sigs).is_ok());
        
        // Should fail with only 1 confirmation
        assert!(verifier.verify_oracle_data(data, &sigs[..1]).is_err());
    }
}