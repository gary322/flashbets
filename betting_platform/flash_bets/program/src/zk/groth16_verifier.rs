use ark_bls12_381::{Bls12_381, Fr};
use ark_groth16::{Groth16, Proof, ProvingKey, VerifyingKey};
use ark_serialize::CanonicalDeserialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use solana_program::program_error::ProgramError;
use std::sync::OnceLock;

use super::circuit::{FlashOutcomeCircuit, QuantumCollapseCircuit};

type FlashKeypair = (ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>);
type QuantumKeypair = (ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>);

static FLASH_KEYS: OnceLock<Result<FlashKeypair, ProgramError>> = OnceLock::new();
static QUANTUM_KEYS: OnceLock<Result<QuantumKeypair, ProgramError>> = OnceLock::new();

fn flash_keys() -> Result<&'static FlashKeypair, ProgramError> {
    FLASH_KEYS
        .get_or_init(|| {
        let mut rng = StdRng::seed_from_u64(42);
        Groth16::<Bls12_381>::circuit_specific_setup(
            FlashOutcomeCircuit::<Fr>::setup_example(),
            &mut rng,
        )
        .map_err(|_| ProgramError::InvalidInstructionData)
        })
        .as_ref()
}

fn quantum_keys() -> Result<&'static QuantumKeypair, ProgramError> {
    QUANTUM_KEYS
        .get_or_init(|| {
        let mut rng = StdRng::seed_from_u64(43);
        Groth16::<Bls12_381>::circuit_specific_setup(
            QuantumCollapseCircuit::<Fr>::setup_example(),
            &mut rng,
        )
        .map_err(|_| ProgramError::InvalidInstructionData)
        })
        .as_ref()
}

fn deserialize_proof(proof_bytes: &[u8]) -> Result<Proof<Bls12_381>, ProgramError> {
    let mut reader = &proof_bytes[..];
    Proof::<Bls12_381>::deserialize_compressed(&mut reader)
        .map_err(|_| ProgramError::InvalidInstructionData)
}

pub(super) fn flash_proving_key() -> &'static ProvingKey<Bls12_381> {
    // Used by demo/testing codepaths only; runtime verification uses `flash_keys()`.
    &flash_keys().expect("flash keys").0
}

pub(super) fn quantum_proving_key() -> &'static ProvingKey<Bls12_381> {
    // Used by demo/testing codepaths only; runtime verification uses `quantum_keys()`.
    &quantum_keys().expect("quantum keys").0
}

/// Groth16 verifier for flash betting proofs (demo scope).
pub struct Groth16Verifier;

impl Groth16Verifier {
    pub fn verify_flash_outcome_proof(
        proof_bytes: &[u8],
        verse_id: u128,
        outcome_index: u8,
        slot: u64,
    ) -> Result<bool, ProgramError> {
        let proof = deserialize_proof(proof_bytes)?;
        let vk = &flash_keys()?.1;

        let public_inputs = vec![
            Fr::from(verse_id),
            Fr::from(outcome_index as u128),
            Fr::from(slot),
        ];

        Groth16::<Bls12_381>::verify(vk, &public_inputs, &proof)
            .map_err(|_| ProgramError::InvalidInstructionData)
    }

    pub fn verify_quantum_collapse_proof(
        proof_bytes: &[u8],
        position_id: u128,
        verse_id: u128,
        leverage: u8,
        winning_outcome: u8,
    ) -> Result<bool, ProgramError> {
        let proof = deserialize_proof(proof_bytes)?;
        let vk = &quantum_keys()?.1;

        let public_inputs = vec![
            Fr::from(position_id),
            Fr::from(verse_id),
            Fr::from(leverage as u128),
            Fr::from(winning_outcome as u128),
        ];

        Groth16::<Bls12_381>::verify(vk, &public_inputs, &proof)
            .map_err(|_| ProgramError::InvalidInstructionData)
    }
}
