use ark_bls12_381::{Bls12_381, Fr};
use ark_groth16::Groth16;
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use std::time::Instant;

use super::circuit::{FlashOutcomeCircuit, QuantumCollapseCircuit};
use super::groth16_verifier::{flash_proving_key, quantum_proving_key};

/// Flash proof generator for demo/testing.
///
/// Note: In a production deployment, proof generation would be off-chain and
/// keys would be managed securely. For the demo scope, we generate deterministic
/// proofs using cached proving keys.
pub struct FlashProver;

impl FlashProver {
    pub fn new() -> Self {
        Self
    }

    /// Generate a Groth16 proof for resolving a flash market outcome.
    pub fn prove_outcome(
        &self,
        verse_id: u128,
        outcome_index: u8,
        slot: u64,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let start = Instant::now();

        let circuit = FlashOutcomeCircuit::<Fr>::new(verse_id, outcome_index, slot);
        let pk = flash_proving_key();

        // Deterministic RNG keeps tests stable and avoids needing OS randomness.
        let mut rng = StdRng::seed_from_u64(slot ^ (verse_id as u64));
        let proof = Groth16::<Bls12_381>::prove(pk, circuit, &mut rng)?;

        let mut bytes = Vec::new();
        proof.serialize_compressed(&mut bytes)?;

        // Keep prior UX expectation that flash proving completes quickly.
        if start.elapsed().as_secs() > 10 {
            return Err("Proof generation exceeded flash time budget".into());
        }

        Ok(bytes)
    }

    /// Generate a Groth16 proof for collapsing a quantum flash position.
    pub fn prove_quantum_collapse(
        &self,
        position_id: u128,
        verse_id: u128,
        leverage: u8,
        winning_outcome: u8,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let start = Instant::now();

        let circuit =
            QuantumCollapseCircuit::<Fr>::new(position_id, verse_id, leverage, winning_outcome);
        let pk = quantum_proving_key();

        let mut rng = StdRng::seed_from_u64((position_id as u64) ^ (verse_id as u64));
        let proof = Groth16::<Bls12_381>::prove(pk, circuit, &mut rng)?;

        let mut bytes = Vec::new();
        proof.serialize_compressed(&mut bytes)?;

        if start.elapsed().as_secs() > 10 {
            return Err("Quantum proof generation exceeded time budget".into());
        }

        Ok(bytes)
    }

    /// Verify proof is correctly formatted (canonical Groth16 proof bytes).
    pub fn verify_proof_format(&self, proof: &[u8]) -> bool {
        use ark_groth16::Proof;
        use ark_serialize::CanonicalDeserialize;

        let mut reader = &proof[..];
        Proof::<Bls12_381>::deserialize_compressed(&mut reader).is_ok()
    }
}

impl Default for FlashProver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::groth16_verifier::Groth16Verifier;

    #[test]
    fn test_outcome_proof_generation_and_verification() {
        let prover = FlashProver::new();
        let verse_id = 12345u128;
        let outcome_index = 1u8;
        let slot = 123u64;

        let proof = prover.prove_outcome(verse_id, outcome_index, slot).unwrap();
        assert!(prover.verify_proof_format(&proof));

        let ok =
            Groth16Verifier::verify_flash_outcome_proof(&proof, verse_id, outcome_index, slot).unwrap();
        assert!(ok);
    }

    #[test]
    fn test_quantum_collapse_proof_generation_and_verification() {
        let prover = FlashProver::new();
        let position_id = 54321u128;
        let verse_id = 12345u128;
        let leverage = 50u8;
        let winning_outcome = 1u8;

        let proof = prover
            .prove_quantum_collapse(position_id, verse_id, leverage, winning_outcome)
            .unwrap();
        assert!(prover.verify_proof_format(&proof));

        let ok = Groth16Verifier::verify_quantum_collapse_proof(
            &proof,
            position_id,
            verse_id,
            leverage,
            winning_outcome,
        )
        .unwrap();
        assert!(ok);
    }
}

