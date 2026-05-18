# Transport Migration: Remaining Work

## 1. Remove UsizeSerializable/UsizeDeserializable trait bounds from the type system

Several core traits still require `UsizeSerializable + UsizeDeserializable` as bounds on their associated types. These bounds must be relaxed to `Serialize + DeserializeOwned` (or removed entirely) before the impl blocks and the `usize_serialization` module can be deleted.

Traits with bounds to update:
- `StorageModel::StorageCommitment` in `storage_models/src/common_structs/traits/storage_model.rs:24`
- `SystemIOTypesConfig` associated types (Address, StorageKey, StorageValue, etc.)
- `StateRootView` bounds in `zk_ee/src/common_structs/proof_data.rs`
- `IOTeardown::IOStateCommitment` bounds

Once the bounds are updated, ~50 `impl UsizeSerializable/UsizeDeserializable` blocks can be deleted from:
- `zk_ee/src/utils/bytes32.rs`
- `zk_ee/src/system/metadata/zk_metadata.rs` (BlockHashes, BlockMetadataFromOracle)
- `zk_ee/src/execution_environment_type.rs`
- `zk_ee/src/common_structs/proof_data.rs`
- `zk_ee/src/storage_types/storage_address.rs`
- `zk_ee/src/storage_types/initial_storage_slot_data.rs`
- `basic_system/src/system_implementation/ethereum_storage_model/caches/account_properties.rs`
- `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs` (~20 impls for FlatStorageLeaf, FlatStorageCommitment, LeafProof, ExistingReadProof, ValueAtIndexProof, and their variants)

## 2. Remove SimpleOracleQuery trait and all impls

`SimpleOracleQuery` defines typed oracle queries with `QUERY_ID`, `Input`, and `Output` associated types. It is still used at ~37 call sites for two purposes:
- `SimpleOracleQuery::QUERY_ID` as a named constant for query IDs
- `SimpleOracleQuery::get(&mut oracle, &input)` as a convenience wrapper around `oracle.query()`

Migration: extract the `QUERY_ID` constants to standalone `const` items (or into the query struct itself), replace `::get()` calls with direct `oracle.query()` calls, then delete the trait and all 8 impls:
- `InitialStorageSlotQuery`, `DisconnectOracleQuery`, `ZKProofDataQuery` in `zk_ee/src/oracle/basic_queries.rs`
- `EthereumAccountPropertiesQuery` in `basic_system/.../account_properties.rs`
- `PreimageLengthQuery` in `basic_system/.../preimage.rs`
- `PreviousIndexQuery`, `ExactIndexQuery`, `ProofForIndexQuery` in `basic_system/.../simple_growable_storage.rs`
- `HistoricalHashQuery` in `basic_bootloader/.../block_hashes_cache.rs`

Also delete `zk_ee/src/oracle/simple_oracle_query.rs` and `zk_ee/src/oracle/basic_queries.rs`.

## 3. Delete usize_serialization module

Once items 1 and 2 are done, delete the entire `zk_ee/src/oracle/usize_serialization/` directory. This removes:
- `UsizeSerializable` and `UsizeDeserializable` trait definitions
- All primitive impls (u8, u32, u64, bool, U256, B160, tuples, arrays)
- `DynUsizeIterator` (unsafe lifetime transmute utility)
- `ExactSizeChain` / `ExactSizeChainN` (iterator composition)

## 4. Consolidate duplicate oracle response types

The callable oracle response types are defined in two places:
- `basic_system/src/oracle_types.rs`
- `basic_bootloader/src/bootloader/oracle_types.rs`

These are identical structs (`DivRemResponse`, `WideDivRemResponse`, `ModexpResponse`, `FieldSqrtResponse`, `FieldInverseResponse`). They should live in `basic_system` only (since `basic_bootloader` depends on `basic_system`, not vice versa). `basic_bootloader` should re-export or import from `basic_system`.

## 5. Fix coinbase_regression test

`tests/instances/unit/src/coinbase_regression.rs:16` has `#[should_panic]` on `test_invalid_coinbase`. This test creates an invalid B160 (all limbs set to `u64::MAX` — 24 bytes of 0xFF instead of 20) and expects a panic during execution.

The panic came from the old `UsizeDeserializable` impl for B160 which validated the value. With serde serialization, B160 round-trips without validation and the test no longer panics.

Options: update the test to assert the new graceful behavior, add explicit validation elsewhere, or remove the test if the invariant is no longer enforced at this layer.

## 6. Remove RISC-V callable oracle variants

`callable_oracles/src/arithmetic/mod.rs` has both `ArithmeticQuery` (uses `RamPeek` to read guest memory) and `NativeArithmeticQuery` (reads host process memory via raw pointers). Same for `FieldOpsQuery`/`NativeFieldOpsQuery` and `BlobCommitmentAndProofQuery`/`NativeBlobCommitmentAndProofQuery`.

Since all callable oracle responses are now pre-computed in the witness, the RISC-V variants are only needed if someone runs the transpiler with on-the-fly oracle processing (via `run_with_nd_source`). Currently `tests/rig/src/chain.rs` still registers the RISC-V variants for some code paths.

Once the rig is confirmed to only use native variants, the RISC-V variants and their `RamPeek`-based memory reading helpers (`callable_oracles/src/utils/evaluate.rs`) can be deleted.

## 7. Verify airbender-host default-features setting

The workspace dependency for `airbender-host` was changed to `default-features = false` to avoid requiring CUDA (the `gpu-prover` default feature pulls in `era_cudart_sys`). Verify this doesn't break CI where CUDA is available and where the gpu-prover feature may be needed by other crates.

## 8. Fix MULH opcode in guest binary

The `binary_checker` test reports unsupported MULH (signed multiply-high) opcodes in the
singleblock-batch binary. The binary checker config
(`FullIsaMachineWithDelegationNoExceptionHandlingNoSignedMulDiv`) matches the actual prover
config (`IMStandardIsaConfigWithUnsignedMulDiv`) — both have `SUPPORT_SIGNED_MUL: false`,
and the prover uses `mul_div_unsigned_circuit_setup`. MULH should NOT be in the binary.
Verified: draft-0.4.0 binary has zero MULH instructions.

Root cause found: `serde_core::de::WithDecimalPoint` at `serde_core/src/de/mod.rs:2388`.

Serde's `Unexpected` enum has a `Float(f64)` variant. Its `Display` impl formats this
variant using `WithDecimalPoint(f64)` which calls `Display for f64` from `core::fmt::float`.
The float formatting code in `core::num::flt2dec::strategy::grisu` contains MULH.

This is pulled in because any serde `Deserialize` impl can call
`serde::de::Error::invalid_type(Unexpected::..., &expected)`, which monomorphizes the
entire `Unexpected::Display` impl including the `Float` variant's f64 formatting — even
though no code ever constructs `Unexpected::Float`.

Note: airbender-platform examples don't have this because they're small enough that LTO
eliminates the dead float code. The zksync-os binary is large enough that LTO's dead code
elimination doesn't reach it.

Simplest fix: override `invalid_type` and `invalid_value` on bincode's `DecodeError`
to avoid formatting `Unexpected` via `Display`. The default impls call
`format_args!("invalid type: {}, expected {}", unexp, exp)` which monomorphizes
`Unexpected::Display` including the float variant. Overriding to use a simpler message
(e.g., just `Error::custom("invalid type")`) avoids the float monomorphization.

This can be done by:
1. Patching bincode's `impl serde::de::Error for DecodeError` to add:
   ```rust
   fn invalid_type(_: Unexpected, _: &dyn Expected) -> Self {
       Self::OtherString("invalid type".into())
   }
   fn invalid_value(_: Unexpected, _: &dyn Expected) -> Self {
       Self::OtherString("invalid value".into())
   }
   ```
2. OR: wrapping the bincode deserializer in airbender-codec with a custom error type
3. OR: patching serde to not format floats on cfg(target_arch = "riscv32")

Note: `e2e_prove` CI job passes despite the MULH — the prover may handle it at runtime
even though the circuit spec doesn't include it. But the binary checker correctly
identifies this as a spec violation.
