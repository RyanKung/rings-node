//! Implementation of Circuit
//! ==========================
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use bellpepper_core::num::AllocatedNum;
use bellpepper_core::ConstraintSystem;
use bellpepper_core::LinearCombination;
use bellpepper_core::SynthesisError;
use circom_scotia::r1cs::R1CS;
use circom_scotia::witness::WitnessCalculator;
use ff::PrimeField;
use nova_snark::traits::circuit::StepCircuit;

use crate::error::Result;
use crate::r1cs::TyWitness;

/// Input of witness
pub type TyInput<F> = Vec<(String, Vec<F>)>;

/// Flat a witness input to values
pub fn flat_input<F: PrimeField>(input: TyInput<F>) -> Vec<F> {
    input.into_iter().flat_map(|(_, v)| v).collect()
}

/// Calculate length of input
pub fn input_len<F: PrimeField>(input: &TyInput<F>) -> usize {
    input.iter().flat_map(|(_, v)| v).collect::<Vec<&F>>().len()
}

/// Circuit
#[derive(Clone, Debug)]
pub struct Circuit<F: PrimeField> {
    r1cs: Arc<R1CS<F>>,
    witness: TyWitness<F>,
}

impl<F: PrimeField> AsRef<Circuit<F>> for &Circuit<F> {
    fn as_ref(&self) -> &Circuit<F> {
        self
    }
}

/// Wasm based circuit generator
pub struct WasmCircuitGenerator<F: PrimeField> {
    r1cs: Arc<R1CS<F>>,
    calculator: Rc<RefCell<WitnessCalculator>>,
}

impl<F: PrimeField> WasmCircuitGenerator<F> {
    /// Crate new instance
    pub fn new(r1cs: R1CS<F>, calculator: WitnessCalculator) -> Self {
        Self {
            r1cs: Arc::new(r1cs),
            calculator: Rc::new(RefCell::new(calculator)),
        }
    }

    /// Generate iterator circuit list
    /// Which iterate inputs and generate circuit
    pub fn gen_circuit(&self, input: TyInput<F>, sanity_check: bool) -> Result<Circuit<F>>
    where F: PrimeField {
        let mut calc = self.calculator.borrow_mut();
        let witness: TyWitness<F> = calc.calculate_witness::<F>(input.clone(), sanity_check)?;
        let circom = Circuit::<F> {
            r1cs: self.r1cs.clone(),
            witness,
        };
        Ok(circom)
    }

    /// Generate recursive circuit list
    /// Which use $output_{i-1}$ as $input_i$
    pub fn gen_recursive_circuit(
        &self,
        public_input: TyInput<F>,
        private_inputs: Vec<TyInput<F>>,
        times: usize,
        sanity_check: bool,
    ) -> Result<Vec<Circuit<F>>>
    where
        F: PrimeField,
    {
        fn reshape<F: PrimeField>(
            input: &[(String, Vec<F>)],
            output: &[F],
        ) -> Vec<(String, Vec<F>)> {
            let mut ret = vec![];
            let mut iter = output.iter();

            for (val, vec) in input.iter() {
                let size = vec.len();
                let mut new_vec: Vec<F> = Vec::with_capacity(size);
                for _ in 0..size {
                    if let Some(item) = iter.next() {
                        new_vec.push(*item);
                    } else {
                        panic!(
                            "Failed on reshape output {:?} as input format {:?}",
                            output, input
                        )
                    }
                }
                ret.push((val.clone(), new_vec));
            }
            ret
        }

        let mut ret = vec![];
        let mut calc = self.calculator.borrow_mut();
        let mut latest_output: Vec<(String, Vec<F>)> = vec![];
        let input_len = input_len(&public_input);

        for i in 0..times {
            let witness: TyWitness<F> = if latest_output.is_empty() {
                let mut input = public_input.clone();
                if let Some(p) = private_inputs.get(i) {
                    input.extend(p.clone());
                }
                calc.calculate_witness::<F>(input, sanity_check)?
            } else {
                let mut input = latest_output.clone();
                if let Some(p) = private_inputs.get(i) {
                    input.extend(p.clone());
                }
                calc.calculate_witness::<F>(input, sanity_check)?
            };
            let circom = Circuit::<F> {
                r1cs: self.r1cs.clone(),
                witness: witness.clone(),
            };
            log::trace!("witness: {:?}, r1cs: {:?}", witness, self.r1cs);
            latest_output = reshape(&public_input, &circom.get_public_outputs(input_len));
            ret.push(circom);
        }
        Ok(ret)
    }
}

impl<F: PrimeField> Circuit<F> {
    /// Create a new instance
    pub fn new(r1cs: Arc<R1CS<F>>, witness: TyWitness<F>) -> Self {
        Self { r1cs, witness }
    }

    /// get public outputs from witness
    pub fn get_public_outputs(&self, input_size: usize) -> Vec<F> {
        // witness: <1> <Outputs> <Inputs> <Auxs>
        // NOTE: assumes exactly half of the (public inputs + outputs) are outputs
        let output_count = self.r1cs.num_inputs - input_size - 1;
        self.witness[1..output_count + 1].to_vec()
    }
}

/// Implement StepCircuit for our Circuit
/// Reference work: Nota-Scotia :: CircomCircuit
/// `<https://github.com/nalinbhardwaj/Nova-Scotia/blob/main/src/circom/circuit.rs>`
/// NOTE: assumes exactly half of the (public inputs + outputs) are outputs
impl<F: PrimeField> StepCircuit<F> for Circuit<F> {
    fn arity(&self) -> usize {
        (self.r1cs.num_inputs - 1) / 2
    }

    /// Simple synthesize
    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> core::result::Result<Vec<AllocatedNum<F>>, SynthesisError> {
        let mut vars: Vec<AllocatedNum<F>> = vec![];
        let mut z_out: Vec<AllocatedNum<F>> = vec![];
        let pub_output_count = (self.r1cs.num_inputs - 1) / 2;

        for i in 1..self.r1cs.num_inputs {
            // Public inputs do not exist, so we alloc, and later enforce equality from z values
            let f: F = self.witness[i];
            let v = AllocatedNum::alloc(cs.namespace(|| format!("public_{}", i)), || Ok(f))?;

            vars.push(v.clone());
            if i <= pub_output_count {
                // public output
                z_out.push(v);
            }
        }
        for i in 0..self.r1cs.num_aux {
            // Private witness trace
            let f: F = self.witness[i + self.r1cs.num_inputs];
            let v = AllocatedNum::alloc(cs.namespace(|| format!("aux_{}", i)), || Ok(f))?;
            vars.push(v);
        }

        let make_lc = |lc_data: Vec<(usize, F)>| {
            let res = lc_data.iter().fold(
                LinearCombination::<F>::zero(),
                |lc: LinearCombination<F>, (index, coeff)| {
                    lc + if *index > 0_usize {
                        (*coeff, vars[*index - 1].get_variable())
                    } else {
                        (*coeff, CS::one())
                    }
                },
            );
            res
        };
        for (i, constraint) in self.r1cs.constraints.iter().enumerate() {
            cs.enforce(
                || format!("constraint {}", i),
                |_| make_lc(constraint.0.clone()),
                |_| make_lc(constraint.1.clone()),
                |_| make_lc(constraint.2.clone()),
            );
        }

        for i in (pub_output_count + 1)..self.r1cs.num_inputs {
            cs.enforce(
                || format!("pub input enforce {}", i),
                |lc| lc + z[i - 1 - pub_output_count].get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + vars[i - 1].get_variable(),
            );
        }

        Ok(z_out)
    }
}
