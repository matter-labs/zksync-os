# Data Availability Commitment Schemes

ZKsync OS supports multiple Data Availability (DA) commitment schemes to accommodate different deployment scenarios, from full rollups using Ethereum calldata/blobs to validiums with external DA solutions. This document provides a comprehensive overview of the implemented DA commitment schemes and their technical details.

## Overview

DA commitment schemes determine how pubdata (the data needed to reconstruct chain state) is committed to and made available for verification. The choice of scheme affects:
- **Cost**: Different schemes have varying costs for data publication
- **Security**: The level of data availability guarantees
- **Compatibility**: Integration with different settlement layers and DA solutions

## Pubdata Stream Layout

The pubdata stream produced per block by [`write_pubdata`](../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/mod.rs) is chosen by the pubdata content, and the **exact same bytes are streamed to the DA commitment and to the sequencer/prover** — the reported pubdata is byte-for-byte what the batch commits to. Every layout starts with a shared two-byte header: the encoding version (`3` — versions 1/2 were the pre-split full-pubdata formats without a mode byte) followed by a `PubdataContent` mode byte that selects the payload:

- **Full pubdata (`FullPubdata`) — mode 0**: `[version, mode, block_hash, timestamp, state diffs, logs, message payloads]`. The full pubdata; the payload after the header is unchanged from version 2.
- **Logs-only (`LogsOnly`) — mode 1**: `[version, mode, logs_count, log records]` only. Log records include user-message logs, L1-tx result logs and interop commitment tree (IMT) leaf logs. State diffs and message payloads are neither committed nor part of this stream; the sequencer receives them through the dedicated result-keeper channels (`storage_diffs`, `logs`).

This guarantees that L2 -> L1 logs and IMT leaves are always publicly available (the settlement layer always validates the DA commitment via blobs or calldata), while state diffs and message payloads can be left to the operator for validium-style chains. Pubdata *charging* follows the same split — a logs-only tx pays only for the committed log records (see the Pubdata content section).

## Two orthogonal axes: scheme (mechanism) and content (scope)

DA is configured along two independent axes:

- **`DACommitmentScheme`** — the commitment *mechanism*: how the committed bytes are published/hashed (calldata keccak vs EIP-4844 blobs). Sourced per batch from the oracle and committed in the batch output.
- **`PubdataContent`** — the committed *scope*: `FullPubdata` commits the whole pubdata; `LogsOnly` commits only the mandatory logs prefix. `PubdataContent` is a chain-level rule carried in [`ChainConfig`](../zk_ee/src/system/metadata/chain_config.rs) and thereby committed into the public input via the chain config hash, so the settlement layer can enforce the chain's configured mode. On zksync-os it is read together with the rest of the chain config.

The scheme selects the generator; the mode selects whether the full stream or only the mandatory prefix is fed into it.

## Supported DA Commitment Schemes

ZKsync OS defines the following DA commitment scheme (mechanism) IDs in [`da_commitment_scheme.rs`](../zk_ee/src/common_structs/da_commitment_scheme.rs):

### 1. None (ID: 0)
**Purpose**: Invalid/uninitialized state

**Implementation**: No-op

**Use Case**: Internal system state only

### 2. EmptyNoDA (ID: 1)
**Purpose**: No data availability guarantees (zero commitment)

**Implementation**: [`NopCommitmentGenerator`](../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/mod.rs) — always returns the zero hash.

**Note**: Validium chains that must keep logs/IMT reconstructible do **not** use this; they use a real mechanism (calldata or blobs) with `PubdataContent::LogsOnly`.

### 3. PubdataKeccak256 (ID: 2)
**Purpose**: Custom DA solutions using keccak256

**Status**: Currently not supported

**Use Case**: Third-party DA layers (Celestia, Avail, etc.)

### 4. BlobsAndPubdataKeccak256 (ID: 3)
**Purpose**: Traditional rollup mode using Ethereum calldata

**Implementation**: [`Keccak256CommitmentGenerator`](../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/keccak256_commitment_generator.rs)

**Commitment Calculation**:
```
da_commitment = keccak256(
    state_diffs_hash,     // 32 bytes (zero-filled for now)
    pubdata_keccak,       // 32 bytes (keccak256 of full pubdata)
    blob_count,           // 1 byte (always 1 for calldata mode)
    blob_hash            // 32 bytes (zero-filled, ignored on settlement layer)
)
```

**Use Case**:
- Traditional Ethereum rollups using calldata for DA
- Compatible with existing rollup infrastructure
- Provides full data availability guarantees through Ethereum

**Technical Details**:
- Maintains backward compatibility with existing rollup validators
- Uses a "fake" blob structure to maintain consistency with blob-based schemes
- State diffs hash is zero-filled as legacy compatibility requirement

### 5. BlobsZKsyncOS (ID: 4)
**Purpose**: EIP-4844 blob-based DA with optimal cost efficiency

**Implementation**: [`BlobCommitmentGenerator`](../basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/blob_commitment_generator/mod.rs)

**Key Parameters**:
- **Blob chunk size**: 31 bytes per field element
- **Elements per blob**: 4,096 field elements (EIP-4844 standard)
- **Encodable bytes per blob**: 126,976 bytes (31 × 4,096)
- **Maximum blobs supported**: 9 blobs
- **Total capacity**: 1,142,784 bytes across all blobs

**Blob Encoding Process**:

1. **Length Encoding**: First 31 bytes encode data length as `[0, length_be_8_bytes, 23_zeros]`
2. **Data Chunking**: Remaining data chunked into 31-byte segments
3. **Field Element Creation**: Each chunk becomes `[0, chunk_31_bytes]` in big-endian
4. **Blob Filling**: Field elements fill blobs sequentially

**Commitment Calculation**:
```
For each blob:
  1. Generate KZG commitment and proof
  2. Calculate versioned_hash = keccak256(0x01 || kzg_commitment)
  3. Verify KZG proof using polynomial evaluation

final_commitment = keccak256(all_versioned_hashes)
```

**Polynomial Evaluation**:
- Uses BLS12-381 curve arithmetic
- Evaluation point derived from `blake2s(versioned_hash || blob_data)` (truncated to 128 bits)
- Supports polynomial interpolation over the blob data

**Use Case**:
- EIP-4844 enabled Ethereum rollups
- Cost-optimized for large amounts of pubdata
- Up to ~90% cost reduction compared to calldata

## Pubdata content (scope)

Validium is expressed by pairing any real mechanism above (calldata `BlobsAndPubdataKeccak256` or blobs `BlobsZKsyncOS`) with `PubdataContent::LogsOnly` in the chain config. In `LogsOnly` mode only the mandatory logs prefix is fed into the chosen generator (so a calldata validium commits `keccak256(structured(prefix))` and a blob validium commits the blob versioned hashes over the prefix); in `FullPubdata` mode the full pubdata stream is committed. The generator (commitment shape) is unchanged by the mode — only the committed byte range differs. `PubdataContent` mirrors era-contracts' `pubdataContent` field carried in diamond storage and hashed into the batch public input via the chain config.

## DA Commitment Generation Process

The DA commitment generation follows a consistent pattern across all schemes:

### 1. Initialization
```rust
let generator = da_commitment_generator_from_scheme(scheme, allocator)?;
```

### 2. Data Accumulation
```rust
impl WriteBytes for Generator {
    fn write(&mut self, buf: &[u8]) {
        // Accumulate pubdata chunks
    }
}
```

### 3. Finalization
```rust
impl DACommitmentGenerator for Generator {
    fn finalize(&mut self, oracle: &mut Oracle) -> Bytes32 {
        // Generate final commitment
    }
}
```

## Related Documentation
- [L1 Integration](./l1_integration.md) - Overall settlement layer integration
- [System Hooks](./system_hooks.md) - L1 messaging and pubdata generation
- [Transaction Processing](./bootloader/transaction_processing.md) - Pubdata considerations in transaction validation

## Implementation Files
- **Core Types**: `zk_ee/src/common_structs/da_commitment_scheme.rs`
- **Generators**: `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/`
- **Blob Implementation**: `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/blob_commitment_generator/`
- **Testing**: `tests/instances/unit/src/kzg_blobs.rs`