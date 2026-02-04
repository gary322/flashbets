use solana_program::{
    program_error::ProgramError,
    msg,
};
use super::groth16_verifier::Groth16Verifier;

/// Verify outcome proof for flash markets
pub fn verify_outcome_proof(
    proof_bytes: &[u8],
    verse_id: u128,
    outcome_index: u8,
    slot: u64,
) -> Result<bool, ProgramError> {
    let proof_valid = Groth16Verifier::verify_flash_outcome_proof(proof_bytes, verse_id, outcome_index, slot)?;

    if proof_valid {
        msg!("Flash Groth16 proof verified for verse {} outcome {}", verse_id, outcome_index);
    } else {
        msg!("Flash Groth16 proof verification failed");
    }

    Ok(proof_valid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::prover::FlashProver;
    
    #[test]
    fn test_outcome_proof_verification() {
        let prover = FlashProver::new();
        let verse_id = 12345u128;
        let outcome_index = 1u8;
        let slot = 1000u64;

        let proof = prover
            .prove_outcome(verse_id, outcome_index, slot)
            .expect("proof");

        let result = verify_outcome_proof(&proof, verse_id, outcome_index, slot).unwrap();
        assert!(result);
    }
}
