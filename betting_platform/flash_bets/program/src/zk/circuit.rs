use ark_ff::PrimeField;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

/// Flash market outcome circuit for Groth16 proofs.
///
/// Demo-friendly design:
/// - Public inputs bind the proof to `(verse_id, outcome_index, slot)`.
/// - A single checksum witness prevents completely unconstrained circuits while
///   staying cheap to prove/verify in tests.
pub struct FlashOutcomeCircuit<F: PrimeField> {
    pub verse_id: Option<F>,
    pub outcome_index: Option<F>,
    pub slot: Option<F>,
    pub checksum: Option<F>,
}

impl<F: PrimeField> FlashOutcomeCircuit<F> {
    pub fn new(verse_id: u128, outcome_index: u8, slot: u64) -> Self {
        let verse_id_f = F::from(verse_id);
        let outcome_index_f = F::from(outcome_index as u128);
        let slot_f = F::from(slot);
        let checksum = verse_id_f + outcome_index_f + slot_f;

        Self {
            verse_id: Some(verse_id_f),
            outcome_index: Some(outcome_index_f),
            slot: Some(slot_f),
            checksum: Some(checksum),
        }
    }

    pub fn setup_example() -> Self {
        Self::new(1, 1, 1)
    }
}

impl<F: PrimeField> ConstraintSynthesizer<F> for FlashOutcomeCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        use ark_r1cs_std::fields::fp::FpVar;
        use ark_r1cs_std::prelude::*;

        // Public inputs
        let verse_id_var = FpVar::new_input(cs.clone(), || {
            self.verse_id.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let outcome_index_var = FpVar::new_input(cs.clone(), || {
            self.outcome_index.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let slot_var = FpVar::new_input(cs.clone(), || self.slot.ok_or(SynthesisError::AssignmentMissing))?;

        // Witness
        let checksum_var =
            FpVar::new_witness(cs.clone(), || self.checksum.ok_or(SynthesisError::AssignmentMissing))?;

        // checksum == verse_id + outcome_index + slot
        let expected = verse_id_var + outcome_index_var + slot_var;
        checksum_var.enforce_equal(&expected)?;

        Ok(())
    }
}

/// Quantum collapse circuit for Groth16 proofs.
///
/// Public inputs bind the proof to `(position_id, verse_id, leverage, winning_outcome)`.
pub struct QuantumCollapseCircuit<F: PrimeField> {
    pub position_id: Option<F>,
    pub verse_id: Option<F>,
    pub leverage: Option<F>,
    pub winning_outcome: Option<F>,
    pub checksum: Option<F>,
}

impl<F: PrimeField> QuantumCollapseCircuit<F> {
    pub fn new(position_id: u128, verse_id: u128, leverage: u8, winning_outcome: u8) -> Self {
        let position_id_f = F::from(position_id);
        let verse_id_f = F::from(verse_id);
        let leverage_f = F::from(leverage as u128);
        let winning_outcome_f = F::from(winning_outcome as u128);
        let checksum = position_id_f + verse_id_f + leverage_f + winning_outcome_f;

        Self {
            position_id: Some(position_id_f),
            verse_id: Some(verse_id_f),
            leverage: Some(leverage_f),
            winning_outcome: Some(winning_outcome_f),
            checksum: Some(checksum),
        }
    }

    pub fn setup_example() -> Self {
        Self::new(1, 1, 1, 1)
    }
}

impl<F: PrimeField> ConstraintSynthesizer<F> for QuantumCollapseCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        use ark_r1cs_std::fields::fp::FpVar;
        use ark_r1cs_std::prelude::*;

        // Public inputs
        let position_id_var = FpVar::new_input(cs.clone(), || {
            self.position_id.ok_or(SynthesisError::AssignmentMissing)
        })?;
        let verse_id_var =
            FpVar::new_input(cs.clone(), || self.verse_id.ok_or(SynthesisError::AssignmentMissing))?;
        let leverage_var =
            FpVar::new_input(cs.clone(), || self.leverage.ok_or(SynthesisError::AssignmentMissing))?;
        let winning_outcome_var = FpVar::new_input(cs.clone(), || {
            self.winning_outcome.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // Witness
        let checksum_var =
            FpVar::new_witness(cs.clone(), || self.checksum.ok_or(SynthesisError::AssignmentMissing))?;

        // checksum == position_id + verse_id + leverage + winning_outcome
        let expected = position_id_var + verse_id_var + leverage_var + winning_outcome_var;
        checksum_var.enforce_equal(&expected)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::Fr;
    use ark_relations::r1cs::ConstraintSystem;

    #[test]
    fn flash_circuit_is_satisfiable() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = FlashOutcomeCircuit::<Fr>::new(42, 1, 123);
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(cs.is_satisfied().unwrap());
    }

    #[test]
    fn quantum_circuit_is_satisfiable() {
        let cs = ConstraintSystem::<Fr>::new_ref();
        let circuit = QuantumCollapseCircuit::<Fr>::new(7, 42, 10, 1);
        circuit.generate_constraints(cs.clone()).unwrap();
        assert!(cs.is_satisfied().unwrap());
    }
}

