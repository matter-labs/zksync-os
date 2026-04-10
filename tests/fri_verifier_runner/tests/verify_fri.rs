//! Host-side smoke test for the airbender recursion-layer FRI verifier,
//! targeting proofs produced by zksync-os-server at proving version V6
//! (airbender v0.5.2).
//!
//! `fri_3291.bin` (default) or `$FRI_ORACLE_PATH` is the raw bytes of
//! `RealFriProof::proof()` as returned by the server's
//! `/prover-jobs/v1/peek/{from}/{to}` endpoint (base64-decoded) — i.e. a
//! `bincode`-serialized `execution_utils::ProgramProof`, encoded with
//! `bincode::config::standard()`.
//!
//! The test mirrors `fri_proof_verifier::verify_fri_proof` in
//! zksync-os-server (node/bin/src/prover_api/fri_proof_verifier.rs):
//!
//!   1. `bincode::serde::decode_from_slice::<ProgramProof>` the file bytes.
//!   2. Split into `(ProofMetadata, ProofList)` via
//!      `ProgramProof::to_metadata_and_proof_list`.
//!   3. Flatten to a `Vec<u32>` oracle stream via
//!      `execution_utils::generate_oracle_data_from_metadata_and_proof_list`.
//!   4. Install the iterator into the thread-local non-determinism source.
//!   5. Call `full_statement_verifier::verify_recursion_layer()`.
//!   6. Assert the entire oracle was consumed.
//!
//! A structurally valid proof returns `[u32; 16]` without panicking. An
//! invalid proof panics out of one of the internal `assert!`s (or
//! `verify_recursion_layer` itself asserts something later).
//!
//! Note: this test does NOT cross-check the final registers against the
//! batch's public input hash (which is what the server's `verify_fri_proof`
//! does on top). The server's check requires the previous state commitment,
//! stored batch info, and VK hash — none of which are in the proof bytes
//! themselves. This test verifies only the proof's internal consistency:
//! the FRI argument, transcript, grand product, etc. That's enough to catch
//! a corrupted or malformed proof.
//!
//! Run with:
//!   cd tests/fri_verifier_runner && cargo test --release -- --nocapture

use std::path::{Path, PathBuf};

use execution_utils::{
    generate_oracle_data_from_metadata_and_proof_list, ProgramProof,
};

/// Resolves the proof file path.
///
/// Priority:
///   1. `FRI_ORACLE_PATH` env var (absolute or relative to the repo root).
///   2. `<repo-root>/fri_3291.bin`.
///
/// The repo root is computed from `CARGO_MANIFEST_DIR`, which points at
/// `tests/fri_verifier_runner` — two levels up is the zksync-os root.
fn resolve_proof_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_MANIFEST_DIR has at least two ancestors")
        .to_path_buf();

    if let Ok(override_path) = std::env::var("FRI_ORACLE_PATH") {
        let p = PathBuf::from(&override_path);
        if p.is_absolute() {
            p
        } else {
            repo_root.join(p)
        }
    } else {
        repo_root.join("fri_3291.bin")
    }
}

#[test]
fn verify_fri_proof() {
    let proof_path = resolve_proof_path();
    println!("Loading proof from: {}", proof_path.display());

    let proof_bytes = std::fs::read(&proof_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", proof_path.display()));
    println!("Proof file size: {} bytes", proof_bytes.len());

    // Step 1: bincode-decode the proof. Matches the server's decoding in
    // `FriJobManager::verify_proof` (fri_job_manager.rs:246-252):
    //     bincode::serde::decode_from_slice(proof_bytes, bincode::config::standard())
    let (program_proof, consumed): (ProgramProof, usize) =
        bincode::serde::decode_from_slice(&proof_bytes, bincode::config::standard())
            .expect("bincode::serde::decode_from_slice ProgramProof");
    println!(
        "Decoded ProgramProof (consumed {consumed}/{} bytes)",
        proof_bytes.len()
    );
    println!(
        "  base_layer_proofs: {}",
        program_proof.base_layer_proofs.len()
    );
    println!(
        "  delegation_proofs (by type): {:?}",
        program_proof
            .delegation_proofs
            .iter()
            .map(|(k, v)| (*k, v.len()))
            .collect::<Vec<_>>()
    );
    println!("  end_params: {:?}", program_proof.end_params);
    println!(
        "  recursion_chain_hash: {:?}",
        program_proof.recursion_chain_hash
    );

    // Step 2: split into metadata + proof list.
    let (metadata, proof_list) = ProgramProof::to_metadata_and_proof_list(program_proof);
    println!(
        "ProofMetadata: basic={}, reduced={}, reduced_log_23={}, deprecated_final={}",
        metadata.basic_proof_count,
        metadata.reduced_proof_count,
        metadata.reduced_log_23_proof_count,
        metadata.deprecated_final_proof_count,
    );

    // Step 3: flatten to a `Vec<u32>` oracle stream. Same helper the server
    // uses via `extract_final_register_values`.
    let oracle_data = generate_oracle_data_from_metadata_and_proof_list(&metadata, &proof_list);
    println!("Oracle data: {} u32 words", oracle_data.len());

    // Step 4: install into the thread-local non-determinism source.
    full_statement_verifier::verifier_common::prover::nd_source_std::set_iterator(
        oracle_data.into_iter(),
    );

    // Step 5: run the recursion-layer verifier. Matches the comment in
    // `extract_final_register_values`:
    //     "Assume that program proof has only recursion proofs."
    //     assert!(metadata.reduced_proof_count > 0);
    assert!(
        metadata.reduced_proof_count > 0,
        "expected a recursion-layer proof (reduced_proof_count > 0), got {}",
        metadata.reduced_proof_count
    );
    let final_register_values: [u32; 16] = full_statement_verifier::verify_recursion_layer();
    println!("final_register_values: {final_register_values:#010x?}");

    // Step 6: assert the oracle was fully consumed. A leftover word means
    // either the proof had more data than the verifier read, or the
    // flattener produced more than required — either way, a mismatch.
    assert!(
        full_statement_verifier::verifier_common::prover::nd_source_std::try_read_word().is_none(),
        "oracle stream was not fully consumed"
    );

    println!("Proof verified successfully (no panic, oracle fully consumed).");
}
