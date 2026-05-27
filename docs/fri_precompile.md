# FRI precompile (Gateway-only)

The FRI precompile lets contracts on a Gateway-mode chain check, from
inside a transaction, whether a specific `statement_versioned_hash`
is in the current tx-scoped verified-statements list. The verification
is part of transaction validation in the server, as well as host and
proving paths within zksync-os. The proof bytes
themselves are supplied off-tx via an oracle sidecar; the
`FriProofTx` transaction type declares the list of statement hashes
the transaction sender claims to have proofs for.

This page documents the design end-to-end. For the field-level
encoding of the tx see
[transaction_format.md](./bootloader/transaction_format.md). For the
precompile hook surface see [system_hooks.md](./system_hooks.md). For
the oracle query see [oracles.md](./system/io/oracles.md).

## Roles, in one sentence

**Tx declares hashes**, **oracle supplies the proof witness**,
**admission and proving verify**, **precompile only checks tx-scoped
statement membership**.

Sequencer admission (server) verifies proofs before accepting the tx. During
bootloader execution, FRI verification runs only in configs where
`Config::VERIFY_FRI_PROOFS = true`; in those configs both native
prover-input generation and RISC-V proving verify the oracle stream.
Proof verification happens in:

1. **Server Admission** (`forward_system::run::validate_fri_statement`) —
   before the tx is admitted. Runs the host airbender verifier on the
   raw sidecar bytes and rejects on any failure.
2. **Prover-input run** (`Config::VERIFY_FRI_PROOFS = true`,
   `BasicBootloaderProvingExecutionConfig`) — the host re-verifies
   while recording the witness stream so the RISC-V replay sees
   identical bytes.
3. **Proving / RISC-V guest** — the airbender unified verifier runs
   on CSR-backed non-determinism and is the final authority.

## Motivation

Today, an L2 settles through the usual `Commit` → `Prove` → `Execute`
flow. The `Prove` call carries a SNARK proof, and the settlement
contract verifies it inside the EVM. That works because SNARK proofs are
small and their verifier mostly reduces to elliptic-curve operations
backed by Ethereum precompiles.

FRI proofs do not fit that model. They are much larger, roughly on the
order of 1 MB, and verification is dominated by many hash-heavy query
checks and Merkle path validations. Putting the full proof in calldata
and verifying it as ordinary EVM code would be too expensive.

FRI verification also has a versioning problem. Unlike the SNARK
verifier, where the verifier code has been relatively stable and mostly
the verification key changes, the FRI proof format and verifier logic may
change across Airbender versions. ZKsync OS therefore needs to own the
verification path so the server, native execution, and RISC-V proving
path use the same verifier version.

The design keeps the EVM-facing contract flow lightweight:

1. The operator submits a `FriProofTx` that lists the
   `statement_versioned_hash` values it wants to expose to contracts.
2. The large proof bytes stay out of transaction calldata and are
   provided through a sidecar oracle.
3. Server admission and proving-time execution verify the proofs using
   the ZKsync OS / Airbender verifier path.
4. During the transaction body, contracts call the Gateway-only FRI
   precompile to check whether a specific statement hash is present in
   the tx-scoped verified-statements list.

## Components

| Component | Crate | Role |
|---|---|---|
| `FriProofTx` (type `0x7C`) | `basic_bootloader` | RLP-encoded tx carrying the `statement_versioned_hashes` list. Signed by an EOA. |
| Validator: `build_verified_fri_statements_list` | `basic_bootloader::bootloader::transaction_flow::zk::fri` | Structural admission — Gateway gate, cap, dedup. Runs in every config. Populates the tx-scoped verified list the precompile reads. |
| Verifier driver: `drive_fri_verification` | same module | Issues one `FRI_PROOF_QUERY_ID` oracle query per hash and dispatches to host/guest verifier. Runs only when `Config::VERIFY_FRI_PROOFS = true`. |
| Host verifier helper: `verify_host_fri_statement` | `basic_bootloader::bootloader::fri_host_verifier` | Wraps the airbender host verifier with a dedicated 128 MiB-stack thread. Used by admission and by the host recording pass. |
| Oracle responder: `FriProofResponder` | `forward_system::run::query_processors::fri_proof` | Sequencer-side. Pulls bincode `UnrolledProgramProof` bytes from a `FriProofSidecarSource`, decodes, flattens via `execution_utils::flatten_proof_into_responses_for_unified_recursion`, packs into the host-u64/guest-u32 oracle response. |
| Admission API: `validate_fri_statement(hash, bytes, artifacts)` | `forward_system::run::fri_admission` | Standalone entry the server calls before admitting `FriProofTx`. Returns `Ok(())` only if the proof verifies *and* its derived hash matches. |
| Precompile hook | `system_hooks::call_hooks::fri_precompile` (address `0x7003`) | Read-only membership check against the tx-scoped verified-hash list. Returns ABI-encoded `bool`. |

## End-to-end flow

```
operator                       sequencer (zksync-os-server)         zksync-os bootloader
--------                       ----------------------------         --------------------
build FriProofTx(hashes)
sign with EOA
                               admission
                                 validate_fri_statement
                                   = decode_and_flatten
                                   + host airbender verifier
                                   + statement-hash equality
                                 reject on any failure
                               admit tx
                                                                    run_block (forward / proving)
                                                                    validator: build_verified_fri_statements_list
                                                                      gateway check, cap, dedup
                                                                      populate tx-scoped list (NO verification)
                                                                    drive_fri_verification    (proving config only)
                                                                      for each hash:
                                                                        oracle query FRI_PROOF_QUERY_ID
                                                                        native: drain + verify (recording pass)
                                                                        riscv: airbender unified verifier
                                                                    EVM frame runs
                                                                    precompile 0x7003 -> tx-scoped list membership
```

## Resource accounting

The cost split: **gas** is the user-facing meter that bounds EVM and
transaction admission; **native** captures the computational cost the
proving system actually pays. Both are charged per **submitted**
statement count (duplicates are paid for; only verifier work and the
stored list are deduped).

| Quantity | Constant | Value | Multiplier |
|---|---|---|---|
| Intrinsic gas per statement | `FRI_PROOF_TX_INTRINSIC_GAS` | `100_000` | × submitted count |
| Intrinsic native per statement | `FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF` | `17_000_000` | × submitted count |

Both are added to the tx's intrinsic charges in
`calculate_tx_intrinsic_gas` / `calculate_l2_tx_intrinsic_computational_native_resources`
before execution begins; the user must have budget for both.

## Key constants

| Constant | Value | Where |
|---|---|---|
| `FRI_PROOF_TX_TYPE` | `0x7c` | `fri_proof_tx.rs` |
| `FRI_PRECOMPILE_ADDRESS` | `0x0000000000000000000000000000000000007003` | `system_hooks/addresses_constants.rs` |
| `MAX_FRI_STATEMENTS_PER_TX` | `8` | `zk_ee::system::constants` |
| `FRI_STATEMENT_HASH_VERSION` | `1` (first byte of hash) | `basic_bootloader::bootloader::constants` |
| `FRI_PROOF_TX_INTRINSIC_GAS` | `100_000` (per submitted statement) | same |
| `FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF` | `17_000_000` (per submitted statement) | same |
| `FRI_PROOF_QUERY_ID` | `0x40020001` | `zk_ee::oracle::query_ids` |

## Statement hash format

The airbender unified verifier returns `[u32; 16]` — its 16 final
register values. Versioning is applied **only** when we derive
`statement_versioned_hash`:

```
hash = keccak256(verifier_output_words_le)
hash[0] = FRI_STATEMENT_HASH_VERSION   // overwrite first byte
```

The first-byte-version layout is what the signed `FriProofTx` commits
to. The precompile checks 32-byte membership against the tx-scoped statement list.

## Configs

`BasicBootloaderExecutionConfig::VERIFY_FRI_PROOFS` gates whether
`drive_fri_verification` runs. Set to:

- `true` in `BasicBootloaderProvingExecutionConfig` — the bootloader
  verifies FRI proofs. In the native prover-input generation path, it
  drains and reconstructs the oracle response so `ReadWitnessSource`
  records it for RISC-V replay, then runs the native verifier on the
  same recovered word stream. In the RISC-V path, the verifier reads
  the stream through CSR-backed non-determinism.
- `false` in `BasicBootloaderForwardSimulationConfig`,
  `BasicBootloaderForwardETHLikeConfig`,
  `BasicBootloaderCallSimulationConfig` — these configs rely on the
  prior admission check and skip bootloader-level FRI verification.

## Security boundaries

- Oracle responses are **untrusted**. The host responder may return
  garbage; the verifier rejects malformed input by panic. The host
  helper catches panics via `std::thread::Builder::join` and surfaces
  them as `FriHostVerifyError::VerifierRejected`.
- The verifier is pinned to the **unified recursion op type**
  (`OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT`). The
  upstream umbrella entry would also accept unrolled proofs; we read
  the op word ourselves and reject anything else. Both native and
  RISC-V paths apply this pin.
- `nd_source_std` is thread-local; concurrent host verifications use
  independent threads.
- `MAX_FRI_STATEMENTS_PER_TX = 8` bounds verifier work per tx.
- Per-statement gas and native are intrinsic-charged before execution.

## Server side (zksync-os-server)

The server-facing admission API is
`forward_system::run::validate_fri_statement`.

Current state:

- `RunBlockForward` stores `fri_verifier_artifacts:
  Option<Arc<FriVerifierArtifacts>>`. The server constructs these
  artifacts once at startup and `RunBlockForward` passes cheap `Arc`
  clones into each `run_block` call.
- The `RunBlock` interface receives the FRI proof sidecar separately
  per block/transaction source; `forward_system` uses it to answer
  `FRI_PROOF_QUERY_ID` oracle queries.
- Admission calls `validate_fri_statement(hash, proof_bytes,
  &artifacts)` per claimed statement before accepting the tx. Failure
  means the tx is rejected.
- `FriVerifierArtifacts` is still part of the public API and carries
  Airbender setup + compiled layouts. A follow-up may embed these from
  `execution_utils::verifier_binaries::RECURSION_UNIFIED_*` so the
  server-facing API can become `validate_fri_statement(hash, bytes)`.

## References

- Source: `basic_bootloader/src/bootloader/transaction_flow/zk/fri.rs`,
  `basic_bootloader/src/bootloader/fri_verifier.rs`,
  `basic_bootloader/src/bootloader/fri_host_verifier.rs`,
  `forward_system/src/run/fri_admission.rs`,
  `forward_system/src/run/query_processors/fri_proof.rs`,
  `system_hooks/src/call_hooks/fri_precompile.rs`.
- Tests: `tests/instances/system_hooks/src/lib.rs` modules
  `fri_precompile`, `fri_precompile_e2e`, `fri_admission_api`.
