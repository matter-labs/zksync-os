# Transaction formats

ZKsyncOS transactions have two encoding formats:

1. ABI-encoded: used for L1->L2 transactions and upgrade transactions. This format is defined in the next section.
2. RLP-encoded: used for L2 transactions. These follow the standard Ethereum RLP encoding for legacy, EIP-2930, EIP-1559 and EIP-7702 transactions. EIP-4844 blob transactions are implemented but not enabled in production. In addition, we include a custom "service transaction", used for system work. These service transactions aren't signed and have whitelist of allowed destinations. The encoding for these is `0x7D || rlp([destination, data, salt])`. Gateway-mode chains also accept the `0x7C` "FRI proof transaction"; see the section below.

## ABI-encoded ZKsync-specific transactions

| Field                     | Type         | Description                                                                                                                                                                                                                     |
|---------------------------|--------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `tx_type`                 | `u8`         | Type of the transaction. See the table below for supported values.                                                                                                                                                              |
| `from`                    | `B160`       | Caller.                                                                                                                                                                                                                         |
| `to`                      | `B160`       | Callee.                                                                                                                                                                                                                         |
| `gas_limit`               | `u64`        | Same meaning as Ethereum's `gasLimit`.                                                                                                                                                                                          |
| `gas_per_pubdata_limit`   | `u32`        | Maximum gas the user is willing to pay for a byte of [pubdata](https://docs.zksync.io/zksync-protocol/contracts/handling-pubdata).                                                                                               |
| `max_fee_per_gas`         | `u128`       | Maximum fee per gas the user is willing to pay. Akin to EIP-1559's `maxFeePerGas`.                                                                                                                                               |
| `max_priority_fee_per_gas`| `u128`       | Maximum priority fee per gas the user is willing to pay. Akin to EIP-1559's `maxPriorityFeePerGas`.                                                                                                                             |
| `paymaster`               | `B160`       | Transaction's paymaster. Legacy field, unused currently.                                                                                                                                                                             |
| `nonce`                   | `U256`       | Nonce of the transaction.                                                                                                                                                                                                       |
| `value`                   | `U256`       | Value to pass with the transaction.                                                                                                                                                                                             |
| `reserved`                | `[U256; 4]`  | Extra data for future use. See the table below for details on reserved fields.                                                                                                                                                   |
| `data`                    | `bytes`      | The calldata.                                                                                                                                                                                                                   |
| `signature`               | `bytes`      | Signature of the transaction.                                                                                                                                                                                                   |
| `factory_deps`            | `bytes`      | Only for EraVM. Properly formatted hashes of bytecodes to be published on L1 with this transaction. Previously published bytecodes won't incur additional fees.                                                                  |
| `paymaster_input`         | `bytes`      | Input for the paymaster. Legacy field, unused currently.                                                                                                                                                                                                        |
| `reserved_dynamic`        | `bytes`      | Field used for extra functionality.                                                  |

### Transaction Types

Note that transaction types 0,1,2 and 4 are used for RLP-encoded L2 transactions, representing legacy, EIP-2930, EIP-1559 and EIP-7702 transactions.

| Value   | Description                                                                                       |
|---------|---------------------------------------------------------------------------------------------------|
| `0x7C`  | FRI proof transaction (Gateway-only).                                                            |
| `0x7D`  | Service transaction.
| `0x7E`  | Upgrade transaction.                                                                             |
| `0x7F`  | L1 -> L2 transaction.                                                                            |

### Reserved Fields

| Index   | L2 Transactions Description                                                                 | L1 Transactions Description                                                                 |
|---------|---------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `0`     | Distinguishes EIP-155 (chain id) legacy transactions.                                       | Holds the total deposit.                                                                    |
| `1`     | EVM deployment transaction flag.                                                            | Refund recipient.                                                                           |
| `2`     | Reserved for future use.                                                                    | Reserved for future use.                                                                    |
| `3`     | Reserved for future use.                                                                    | Reserved for future use.                                                                    |

These transactions are encoded using the tightly packed ABI encoding for this list of fields. All numeric types are encoded as big-endian `U256`. Encoding and hashing of transactions is implemented in this [module](../../basic_bootloader/src/bootloader/transaction/abi_encoded/mod.rs).

## FRI proof transaction (`0x7C`, Gateway-only)

The `FriProofTx` carries a signed list of `statement_versioned_hash`
entries, while the corresponiding FRI proofs are provided in the sidecar.
The tx then runs as a normal EVM transaction whose contract code can
query the [FRI precompile](../system_hooks.md#fri-precompile-gateway-only)
to check whether it is in tx-scoped verified list. 
Actual proof verification happens initially in the server, so the assumption
is that only transactions with valid proofs make it to the sequencer.

### Field semantics

| Field | Notes |
|---|---|
| Up to `access_list` | Identical layout and semantics to EIP-1559 (`chain_id`, `nonce`, `max_priority_fee_per_gas`, `max_fee_per_gas`, `gas_limit`, `to`, `value`, `input`, `access_list`). |
| `to` | **Exactly 20 bytes.** A `FriProofTx` cannot deploy; a missing or wrong-length `to` is a validation failure. |
| `statement_versioned_hashes` | New field. RLP list whose entries are each **exactly 32 bytes** (`[u8; 32]`). Wrong-length entries fail validation. List length is capped at `MAX_FRI_STATEMENTS_PER_TX = 8`. |
| Signature | `signature_y_parity, r, s` cover the entire payload including the hash list — the EOA signature binds the claim. |

### Validation

- Reject with `FriProofTxNotSupported` if the block is not in Gateway
  mode (`metadata.is_gateway() == false`).
- Reject with `TooManyFriStatements` if the list exceeds the cap.
- After signature recovery, the list is deduplicated for verifier work
  and storage; the user still pays gas / native for the **submitted**
  count (including duplicates).

### Encoding

The transaction is RLP-encoded with tx-type byte `0x7C`:

```
0x7C || rlp([
  chain_id, nonce, max_priority_fee_per_gas, max_fee_per_gas, gas_limit,
  to, value, input, access_list,
  statement_versioned_hashes,                // list of [u8; 32]
  signature_y_parity, signature_r, signature_s
])
```

See [the FRI precompile design](../fri_precompile.md) for the end-to-end
flow including gas/resource accounting.
