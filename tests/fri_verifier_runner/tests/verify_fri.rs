//! Host-side smoke test for the Airbender unified recursion verifier,
//! targeting gzip-compressed `UnrolledProgramProof` blobs produced by
//! `ethereum-prover`.
//!
//! `matter-labs_*.bin` (default via `$FRI_ORACLE_PATH`) is expected to be the
//! raw proof payload submitted to EthProofs: gzip-compressed bytes that
//! decompress into a `bincode`-serialized `UnrolledProgramProof`.
//!
//! The test mirrors `proof_verifier_js/wasm` from `ethereum-prover`:
//! 1. Gzip-decompress the proof bytes.
//! 2. `bincode::serde::decode_from_slice::<UnrolledProgramProof>`.
//! 3. Decode the matching setup/layout artifacts.
//! 4. Flatten `(setup, proof)` into the unified verifier oracle stream.
//! 5. Install the iterator into the thread-local non-determinism source.
//! 6. Call `verify_unrolled_or_unified_circuit_recursion_layer()`.
//!
//! This verifies the proof under its own claimed public input / statement.
//!
//! By default the setup/layout artifacts are loaded from the local clone at
//! `/tmp/ethereum-prover/artifacts/`. Override with:
//! - `FRI_SETUP_PATH`
//! - `FRI_LAYOUT_PATH`

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;

use cs::one_row_compiler::CompiledCircuitArtifact;
use flate2::read::GzDecoder;
use full_statement_verifier::definitions::{
    OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT,
    OP_VERIFY_UNROLLED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT,
};
use prover::common_constants;
use prover::common_constants::TimestampScalar;
use prover::cs::utils::split_timestamp;
use prover::prover_stages::unrolled_prover::UnrolledModeProof;
use prover::prover_stages::Proof;
use serde::{Deserialize, Serialize};
use verifier_common::field::Mersenne31Field;
use verifier_common::proof_flattener;
use verifier_common::prover::definitions::MerkleTreeCap;
use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;

const CAP_SIZE: usize = 64;
const NUM_COSETS: usize = 2;
const FRI_VERIFIER_MODE_MAGIC: u32 = 0x4652_4956;

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
struct CompiledCircuitsSet {
    compiled_circuit_families: BTreeMap<u8, CompiledCircuitArtifact<Mersenne31Field>>,
    compiled_inits_and_teardowns: Option<CompiledCircuitArtifact<Mersenne31Field>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct FinalRegisterValue {
    value: u32,
    last_access_timestamp: TimestampScalar,
}

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
struct UnrolledProgramSetup {
    expected_final_pc: u32,
    binary_hash: [u8; 32],
    circuit_families_setups: BTreeMap<u8, [MerkleTreeCap<CAP_SIZE>; NUM_COSETS]>,
    inits_and_teardowns_setup: [MerkleTreeCap<CAP_SIZE>; NUM_COSETS],
    end_params: [u32; 8],
}

impl UnrolledProgramSetup {
    fn flatten_for_recursion(&self) -> Vec<u32> {
        let mut result = vec![];
        for (_, caps) in &self.circuit_families_setups {
            result.extend_from_slice(MerkleTreeCap::flatten(caps));
        }
        result.extend_from_slice(MerkleTreeCap::flatten(&self.inits_and_teardowns_setup));
        result
    }

    fn flatten_unified_for_recursion(&self) -> Vec<u32> {
        assert_eq!(self.circuit_families_setups.len(), 1);
        let mut result = vec![];
        for (_, caps) in &self.circuit_families_setups {
            result.extend_from_slice(MerkleTreeCap::flatten(caps));
        }
        result
    }
}

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
struct UnrolledProgramProof {
    final_pc: u32,
    final_timestamp: TimestampScalar,
    circuit_families_proofs: BTreeMap<u8, Vec<UnrolledModeProof>>,
    inits_and_teardowns_proofs: Vec<UnrolledModeProof>,
    delegation_proofs: BTreeMap<u32, Vec<Proof>>,
    register_final_values: [FinalRegisterValue; 32],
    recursion_chain_preimage: Option<[u32; 16]>,
    recursion_chain_hash: Option<[u32; 8]>,
    pow_challenge: u64,
}

impl UnrolledProgramProof {
    fn flatten_into_responses(
        &self,
        allowed_delegation_circuits: &[u32],
        compiled_layouts: &CompiledCircuitsSet,
    ) -> Vec<u32> {
        let mut responses = Vec::with_capacity(32 + 32 * 2);

        for final_values in &self.register_final_values {
            responses.push(final_values.value);
            let (low, high) = split_timestamp(final_values.last_access_timestamp);
            responses.push(low);
            responses.push(high);
        }

        responses.push(self.final_pc);
        let (low, high) = split_timestamp(self.final_timestamp);
        responses.push(low);
        responses.push(high);

        for (family, proofs) in &self.circuit_families_proofs {
            responses.push(proofs.len() as u32);
            for proof in proofs {
                let Some(artifact) = compiled_layouts.compiled_circuit_families.get(family) else {
                    panic!("missing compiled circuit artifact for family {}", family);
                };
                responses.extend(proof_flattener::flatten_full_unrolled_proof(proof, artifact));
            }
        }

        if let Some(compiled_inits_and_teardowns) =
            compiled_layouts.compiled_inits_and_teardowns.as_ref()
        {
            responses.push(self.inits_and_teardowns_proofs.len() as u32);
            for proof in &self.inits_and_teardowns_proofs {
                responses.extend(proof_flattener::flatten_full_unrolled_proof(
                    proof,
                    compiled_inits_and_teardowns,
                ));
            }
        } else {
            responses.push(0);
        }

        for delegation_type in allowed_delegation_circuits {
            if *delegation_type == common_constants::NON_DETERMINISM_CSR {
                continue;
            }
            if let Some(proofs) = self.delegation_proofs.get(delegation_type) {
                responses.push(proofs.len() as u32);
                for proof in proofs {
                    responses.extend(proof_flattener::flatten_full_proof(proof, 0));
                }
            } else {
                responses.push(0);
            }
        }

        responses.push(self.pow_challenge as u32);
        responses.push((self.pow_challenge >> 32) as u32);

        if let Some(preimage) = self.recursion_chain_preimage {
            responses.extend(preimage);
        }

        responses
    }
}

fn flatten_proof_into_responses_for_unified_recursion(
    proof: &UnrolledProgramProof,
    setup: &UnrolledProgramSetup,
    compiled_layouts: &CompiledCircuitsSet,
    input_is_unrolled: bool,
) -> Vec<u32> {
    let mut responses = vec![];
    let op = if input_is_unrolled {
        OP_VERIFY_UNROLLED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT
    } else {
        OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT
    };
    responses.push(op);
    if input_is_unrolled {
        responses.extend(setup.flatten_for_recursion());
    } else {
        responses.extend(setup.flatten_unified_for_recursion());
    }
    responses.extend(proof.flatten_into_responses(
        &[
            common_constants::delegation_types::blake2s_with_control::BLAKE2S_DELEGATION_CSR_REGISTER,
        ],
        compiled_layouts,
    ));
    responses
}

fn resolve_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_MANIFEST_DIR has at least two ancestors")
        .to_path_buf()
}

fn resolve_proof_path() -> PathBuf {
    let repo_root = resolve_repo_root();
    if let Ok(override_path) = std::env::var("FRI_ORACLE_PATH") {
        let p = PathBuf::from(&override_path);
        if p.is_absolute() {
            p
        } else {
            repo_root.join(p)
        }
    } else {
        repo_root.join("matter-labs_cb06e41d-6666-4a7c-be76-ac626fb19313_10591799.bin")
    }
}

fn resolve_artifact_path(env_var: &str, default: &str) -> PathBuf {
    if let Ok(override_path) = std::env::var(env_var) {
        PathBuf::from(override_path)
    } else {
        PathBuf::from(default)
    }
}

fn resolve_riscv_bin_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("FRI_RISCV_BIN_PATH") {
        PathBuf::from(override_path)
    } else {
        resolve_repo_root().join("zksync_os/for_tests.bin")
    }
}

fn maybe_decompress_gzip(bytes: &[u8], what: &str) -> Vec<u8> {
    const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
    if bytes.starts_with(&GZIP_MAGIC) {
        let mut decoded = Vec::new();
        GzDecoder::new(bytes)
            .read_to_end(&mut decoded)
            .unwrap_or_else(|e| panic!("failed to decompress gzip-compressed {what}: {e}"));
        decoded
    } else {
        bytes.to_vec()
    }
}

fn decode_exact<T: serde::de::DeserializeOwned>(bytes: &[u8], what: &str) -> T {
    let (value, bytes_read): (T, usize) =
        bincode::serde::decode_from_slice(bytes, bincode::config::standard())
            .unwrap_or_else(|e| panic!("failed to parse {what}: {e}"));
    assert_eq!(
        bytes_read,
        bytes.len(),
        "failed to parse {what}: trailing {} byte(s) indicate an incompatible format",
        bytes.len() - bytes_read
    );
    value
}

#[test]
fn verify_fri_proof() {
    let proof_path = resolve_proof_path();
    println!("Loading proof from: {}", proof_path.display());

    let proof_bytes = std::fs::read(&proof_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", proof_path.display()));
    println!("Proof file size: {} bytes", proof_bytes.len());
    let proof_bytes = maybe_decompress_gzip(&proof_bytes, "proof");
    println!("Decoded proof payload size: {} bytes", proof_bytes.len());

    let proof: UnrolledProgramProof = decode_exact(&proof_bytes, "UnrolledProgramProof");
    println!(
        "Decoded UnrolledProgramProof: circuit families={}, delegation types={}, recursion_chain_hash={:?}",
        proof.circuit_families_proofs.len(),
        proof.delegation_proofs.len(),
        proof.recursion_chain_hash
    );

    let setup_path = resolve_artifact_path(
        "FRI_SETUP_PATH",
        "/tmp/ethereum-prover/artifacts/recursion_unified_setup.bin",
    );
    let layout_path = resolve_artifact_path(
        "FRI_LAYOUT_PATH",
        "/tmp/ethereum-prover/artifacts/recursion_unified_layouts.bin",
    );
    println!("Loading setup from: {}", setup_path.display());
    println!("Loading layout from: {}", layout_path.display());

    let setup_bytes = std::fs::read(&setup_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", setup_path.display()));
    let layout_bytes = std::fs::read(&layout_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", layout_path.display()));
    let setup: UnrolledProgramSetup = decode_exact(&setup_bytes, "UnrolledProgramSetup");
    let layout: CompiledCircuitsSet = decode_exact(&layout_bytes, "CompiledCircuitsSet");

    let responses =
        flatten_proof_into_responses_for_unified_recursion(&proof, &setup, &layout, false);
    println!("Oracle data: {} u32 words", responses.len());

    let final_register_values: [u32; 16] = thread::Builder::new()
        .name("verifier thread".to_string())
        .stack_size(1 << 27)
        .spawn(move || {
            prover::nd_source_std::set_iterator(responses.into_iter());
            let result =
                full_statement_verifier::unified_circuit_statement::verify_unrolled_or_unified_circuit_recursion_layer();
            assert!(
                prover::nd_source_std::try_read_word().is_none(),
                "oracle stream was not fully consumed"
            );
            result
        })
        .expect("must spawn verifier thread")
        .join()
        .expect("verifier thread panicked");

    println!("final_register_values: {final_register_values:#010x?}");
    println!(
        "Claimed public-input hash words from verifier output: {:#010x?}",
        &final_register_values[..8]
    );
    println!(
        "Recursion chain hash words from verifier output: {:#010x?}",
        &final_register_values[8..]
    );
    println!("Proof verified successfully under its claimed public input.");
}

#[test]
fn measure_fri_verifier_cycles_in_riscv() {
    let proof_path = resolve_proof_path();
    let proof_bytes = std::fs::read(&proof_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", proof_path.display()));
    let proof_bytes = maybe_decompress_gzip(&proof_bytes, "proof");
    let proof: UnrolledProgramProof = decode_exact(&proof_bytes, "UnrolledProgramProof");

    let setup_path = resolve_artifact_path(
        "FRI_SETUP_PATH",
        "/tmp/ethereum-prover/artifacts/recursion_unified_setup.bin",
    );
    let layout_path = resolve_artifact_path(
        "FRI_LAYOUT_PATH",
        "/tmp/ethereum-prover/artifacts/recursion_unified_layouts.bin",
    );
    let setup_bytes = std::fs::read(&setup_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", setup_path.display()));
    let layout_bytes = std::fs::read(&layout_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", layout_path.display()));
    let setup: UnrolledProgramSetup = decode_exact(&setup_bytes, "UnrolledProgramSetup");
    let layout: CompiledCircuitsSet = decode_exact(&layout_bytes, "CompiledCircuitsSet");

    let responses =
        flatten_proof_into_responses_for_unified_recursion(&proof, &setup, &layout, false);

    let mut non_determinism_source = QuasiUARTSource::default();
    non_determinism_source.oracle.push_back(FRI_VERIFIER_MODE_MAGIC);
    for word in responses {
        non_determinism_source.oracle.push_back(word);
    }

    let bin_path = resolve_riscv_bin_path();
    println!("Running RISC-V verifier bench binary: {}", bin_path.display());
    let (public_output, effective_cycles) = zksync_os_runner::run_and_get_effective_cycles(
        bin_path,
        200_000_000,
        non_determinism_source,
    );

    println!("RISC-V public output: {public_output:#010x?}");
    println!("RISC-V effective cycles: {:?}", effective_cycles);
    assert!(
        effective_cycles.is_some(),
        "effective cycles were not reported; build the binary with cycle markers enabled"
    );
}
