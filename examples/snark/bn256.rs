use std::rc::Rc;

use rings_snark::circuit;
use rings_snark::prelude::nova;
use rings_snark::prelude::nova::provider::PallasEngine;
use rings_snark::prelude::nova::provider::VestaEngine;
use rings_snark::prelude::nova::traits::Engine;
use rings_snark::r1cs;
use rings_snark::snark;

#[tokio::main]
async fn main() {
    type E1 = VestaEngine;
    type E2 = PallasEngine;
    type EE1 = nova::provider::ipa_pc::EvaluationEngine<E1>;
    type EE2 = nova::provider::ipa_pc::EvaluationEngine<E2>;
    type S1 = nova::spartan::snark::RelaxedR1CSSNARK<E1, EE1>; // non-preprocessing SNARK
    type S2 = nova::spartan::snark::RelaxedR1CSSNARK<E2, EE2>; // non-preprocessing SNARK
    type F1 = <E1 as Engine>::Scalar;
    type F2 = <E2 as Engine>::Scalar;

    let r1cs = r1cs::load_r1cs::<F1>(
        r1cs::Path::Local("examples/snark/circoms/simple_bn256.r1cs".to_string()),
        r1cs::Format::Bin,
    )
    .await
    .unwrap();
    let witness_calculator = r1cs::load_circom_witness_calculator(r1cs::Path::Local(
        "examples/snark/circoms/simple_bn256_js/simple_bn256.wasm".to_string(),
    ))
    .await
    .unwrap();

    let circuit_generator = circuit::WasmCircuitGenerator::<F1>::new(r1cs, witness_calculator);

    // recursion based circuit example
    //
    // prepare inputs
    let input_0: Vec<(String, Vec<F1>)> =
        vec![("step_in".to_string(), vec![F1::from(4u64), F1::from(2u64)])];

    let recursive_circuits = circuit_generator
        .gen_recursive_circuit(input_0.clone(), 5, true)
        .unwrap();
    assert_eq!(recursive_circuits.len(), 5);
    let pp = snark::SNARK::<E1, E2>::gen_pp::<EE1, EE2, S1, S2>(recursive_circuits[0].clone());
    let pp_ref = Rc::new(pp);

    let mut rec_snark_iter = snark::SNARK::<E1, E2>::new::<EE1, EE2, S1, S2>(
        &Rc::new(recursive_circuits[0].clone()),
        input_0.clone(),
        pp_ref.clone(),
    )
    .unwrap();
    for c in recursive_circuits {
        rec_snark_iter.foldr(pp_ref.clone(), &c).unwrap();
    }
    rec_snark_iter
        .verify(
            pp_ref.clone(),
            5,
            &vec![F1::from(4u64), F1::from(2u64)],
            &vec![F2::from(0)],
        )
        .unwrap();
    println!("success on create recursive snark");
    let (pk, vk) =
        snark::SNARK::<E1, E2>::compress_setup::<EE1, EE2, S1, S2>(pp_ref.clone()).unwrap();
    let pk_ref = Rc::new(pk);

    let compress_snark = rec_snark_iter
        .compress_prove::<EE1, EE2, S1, S2>(pp_ref.clone(), pk_ref)
        .unwrap();
    //    let snark_iter = snark::SNARK::<E1, E2>::new::<EE1, EE2, S1, S2>(iterator_circuits, inputs[0].clone());

    println!("test")
}
