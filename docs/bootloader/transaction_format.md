# Transaction formats

ZKsyncOS transactions have two encoding formats:

1. ABI-encoded: used for L1->L2 transactions, upgrade transactions and 712 transactions. This format is defined in the next section.
2. RLP-encoded: used for L2 transactions. These follow the standard Ethereum RLP encoding for legacy, EIP-2930, EIP-1559 and EIP-7702 transactions.

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

Note that transaction types 0,1,2 and 4 are used for RLP-encoded L2 transactions, representing legacy, EIP-2930, EIP-1559 and EIP-7702 transactions.s

| Value   | Description                                                                                       |
|---------|---------------------------------------------------------------------------------------------------|
| `0x71`  | EIP-712 transaction following the [Era format](https://docs.zksync.io/zksync-protocol/rollup/transaction-lifecycle#eip-712-0x71). |
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
