//! Implementation of Rings Snark
//! ==============
use std::ops::Deref;

use ff::Field;
use serde::Deserialize;
use serde::Serialize;

use crate::circuit::flat_input;
use crate::circuit::Circuit;
use crate::circuit::TyInput;
use crate::error::Result;
use crate::prelude::nova::spartan::snark::RelaxedR1CSSNARK;
use crate::prelude::nova::traits::circuit::TrivialCircuit;
use crate::prelude::nova::traits::evaluation::EvaluationEngineTrait;
use crate::prelude::nova::traits::snark::RelaxedR1CSSNARKTrait;
use crate::prelude::nova::traits::Engine;
use crate::prelude::nova::CompressedSNARK;
use crate::prelude::nova::ProverKey;
use crate::prelude::nova::PublicParams;
use crate::prelude::nova::RecursiveSNARK;
use crate::prelude::nova::VerifierKey;

#[derive(Serialize, Deserialize)]
pub struct SNARK<E1, E2>
where
    E1: Engine<Base = <E2 as Engine>::Scalar>,
    E2: Engine<Base = <E1 as Engine>::Scalar>,
{
    /// recursive snark
    #[serde(flatten)]
    pub snark: RecursiveSNARK<E1, E2, Circuit<<E1 as Engine>::Scalar>, TrivialCircuit<E2::Scalar>>,
}

impl<E1, E2> Deref for SNARK<E1, E2>
where
    E1: Engine<Base = <E2 as Engine>::Scalar>,
    E2: Engine<Base = <E1 as Engine>::Scalar>,
{
    type Target =
        RecursiveSNARK<E1, E2, Circuit<<E1 as Engine>::Scalar>, TrivialCircuit<E2::Scalar>>;
    fn deref(&self) -> &Self::Target {
        &self.snark
    }
}

impl<E1, E2> SNARK<E1, E2>
where
    E1: Engine<Base = <E2 as Engine>::Scalar>,
    E2: Engine<Base = <E1 as Engine>::Scalar>,
{
    /// Create public params with circom, and public input
    pub fn new<EE1, EE2, S1, S2>(
        circom: Circuit<E1::Scalar>,
        public_inputs: TyInput<E1::Scalar>,
    ) -> Result<(
        Self,
        PublicParams<E1, E2, Circuit<<E1 as Engine>::Scalar>, TrivialCircuit<E2::Scalar>>,
    )>
    where
        EE1: EvaluationEngineTrait<E1>,
        EE2: EvaluationEngineTrait<E2>,
        S1: RelaxedR1CSSNARKTrait<E1>,
        S2: RelaxedR1CSSNARKTrait<E2>,
    {
        // flat public input here
        let public_inputs = flat_input::<E1::Scalar>(public_inputs);
        let circuit_primary = circom.clone();
        let circuit_secondary = TrivialCircuit::<E2::Scalar>::default();
        // Create pp here
        let pp = PublicParams::setup(
            &circuit_primary,
            &circuit_secondary,
            &*S1::ck_floor(),
            &*S2::ck_floor(),
        );
        // default input for secondary on initialize round is [0]
        let secondary_inputs = [<<E2 as Engine>::Scalar as Field>::ZERO];
        let snark = RecursiveSNARK::new(
            &pp,
            &circuit_primary,
            &circuit_secondary,
            &public_inputs,
            &secondary_inputs,
        )?;

        Ok((Self { snark }, pp))
    }

    /// Verify the correctness of the `RecursiveSNARK`
    pub fn verify(
        &self,
        pp: &PublicParams<E1, E2, Circuit<<E1 as Engine>::Scalar>, TrivialCircuit<E2::Scalar>>,
        num_steps: usize,
        z0_primary: &[E1::Scalar],
        z0_secondary: &[E2::Scalar],
    ) -> Result<(Vec<E1::Scalar>, Vec<E2::Scalar>)> {
        Ok(self.snark.verify(pp, num_steps, z0_primary, z0_secondary)?)
    }

    /// Gen compress snark, return (pk, vk)
    pub fn compress<EE1, EE2>(
        pp: &PublicParams<E1, E2, Circuit<<E1 as Engine>::Scalar>, TrivialCircuit<E2::Scalar>>,
    ) -> Result<(
        ProverKey<
            E1,
            E2,
            Circuit<<E1 as Engine>::Scalar>,
            TrivialCircuit<<E2 as Engine>::Scalar>,
            RelaxedR1CSSNARK<E1, EE1>,
            RelaxedR1CSSNARK<E2, EE2>,
        >,
        VerifierKey<
            E1,
            E2,
            Circuit<<E1 as Engine>::Scalar>,
            TrivialCircuit<<E2 as Engine>::Scalar>,
            RelaxedR1CSSNARK<E1, EE1>,
            RelaxedR1CSSNARK<E2, EE2>,
        >,
    )> where
        EE1: EvaluationEngineTrait<E1>,
        EE2: EvaluationEngineTrait<E2>,
    {
        Ok(CompressedSNARK::setup(&pp)?)
    }
}
