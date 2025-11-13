# Data Availability Commitment Schemes

ZKsync OS supports multiple Data Availability (DA) commitment schemes to accommodate different deployment scenarios, from full rollups using Ethereum calldata/blobs to validiums with external DA solutions. This document provides a comprehensive overview of the implemented DA commitment schemes and their technical details.

## Overview

DA commitment schemes determine how pubdata (the data needed to reconstruct chain state) is committed to and made available for verification. The choice of scheme affects:
- **Cost**: Different schemes have varying costs for data publication
- **Security**: The level of data availability guarantees
- **Compatibility**: Integration with different settlement layers and DA solutions

## Supported DA Commitment Schemes

ZKsync OS implements five distinct DA commitment schemes, defined in [`da_commitment_scheme.rs`](../zk_ee/src/common_structs/da_commitment_scheme.rs):

### 1. None (ID: 0)
**Purpose**: Invalid/uninitialized state

**Implementation**: No-op

**Use Case**: Internal system state only

### 2. EmptyNoDA (ID: 1)
**Purpose**: Validium mode - no data availability guarantees

**Implementation**: [`NopCommitmentGenerator`](../basic_system/src/system_implementation/system/da_commitment_generator/mod.rs#L32)

**Commitment**: Always returns zero hash (`0x000...000`)

**Use Case**:
- Validiums where DA is handled off-chain
- Private chains where data availability is not required
- Reduced fees due to lower pubdata costs

### 3. PubdataKeccak256 (ID: 2)
**Purpose**: Custom DA solutions using keccak256

**Status**: Currently not supported

**Use Case**: Third-party DA layers (Celestia, Avail, etc.)

### 4. BlobsAndPubdataKeccak256 (ID: 3)
**Purpose**: Traditional rollup mode using Ethereum calldata

**Implementation**: [`Keccak256CommitmentGenerator`](../basic_system/src/system_implementation/system/da_commitment_generator/keccak256_commitment_generator.rs)

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

**Implementation**: [`BlobCommitmentGenerator`](../basic_system/src/system_implementation/system/da_commitment_generator/blob_commitment_generator/mod.rs)

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
- **Generators**: `basic_system/src/system_implementation/system/da_commitment_generator/`
- **Blob Implementation**: `basic_system/src/system_implementation/system/da_commitment_generator/blob_commitment_generator/`
- **Testing**: `tests/instances/unit/src/kzg_blobs.rs`