//! Glamsterdam Ethereum STF.
//!
//! Glamsterdam adds block-level access lists ([EIP-7928]) and the slot number
//! ([EIP-7843]) to the block. The access list commits to every state access of
//! the block, so the STF is split: [`state_consistency`] witnesses the header,
//! its parent and the access list, and verifies the parent state root against
//! the header's state root through the access list alone; the transactions
//! are then checked against the access list independently.
//!
//! Nothing in [`state_consistency`] depends on the `System` traits: its
//! functions take an oracle and an allocator only, so they run natively and
//! on the proving target without the rest of the bootloader. The transaction
//! ranges ([`tx_range`]) run the transactions and the system calls of the
//! block, including the EIP-8282 builder request contracts
//! ([`eip_8282_builder_requests`]).
//!
//! [EIP-7928]: https://eips.ethereum.org/EIPS/eip-7928
//! [EIP-7843]: https://eips.ethereum.org/EIPS/eip-7843

pub mod block_aggregation;
pub mod block_header;
pub mod eip_8282_builder_requests;
pub mod index_mask;
pub mod linkage;
pub mod oracle_queries;
pub mod state_consistency;
pub mod tx_range;

/// The accounts the phases around the transactions can touch besides what the
/// transactions pay for: the EIP-4788 and EIP-2935 contracts, the EIP-7002 and
/// EIP-7251 contracts, the two EIP-8282 predeploys, the coinbase and up to 16
/// withdrawal recipients, rounded up
pub const SYSTEM_PHASE_ACCOUNTS: usize = 32;
/// The slots those phases can touch: the ring buffer slots of EIP-4788 (2) and
/// EIP-2935 (1), the queue heads, tails, counts and excesses of the four request
/// contracts and their dequeued records (16 x 3 for EIP-7002, 2 x 4 for EIP-7251,
/// 64 x 6 and 16 x 3 for EIP-8282), rounded up
pub const SYSTEM_PHASE_SLOTS: usize = 1024;

/// The most distinct accounts a block of `gas_limit` gas can touch, so the most
/// accounts a valid block access list carries: the cheapest access of a new
/// account is a cold `BALANCE` (or `EXTCODESIZE`, ...) of a pushed address,
/// `COLD_ACCOUNT_ACCESS + VERYLOW` = 3003 gas under EIP-8038, so about 66,600
/// accounts at 200M gas, plus [`SYSTEM_PHASE_ACCOUNTS`]. The execution gas of a
/// block is its whole gas limit at most (EIP-7778: the state gas is counted
/// alongside, not on top).
pub const fn max_accounts_in_block(gas_limit: u64) -> usize {
    let cheapest_access = (evm_interpreter::v3::gas::COLD_ACCOUNT_ACCESS
        + evm_interpreter::gas_constants::VERYLOW) as u64;
    (gas_limit / cheapest_access) as usize + SYSTEM_PHASE_ACCOUNTS
}

/// The most distinct storage slots a block of `gas_limit` gas can touch, so the
/// most slots a valid block access list carries: the cheapest access of a new
/// slot is a cold `SLOAD` of a pushed key, `COLD_STORAGE_ACCESS + VERYLOW` =
/// 2103 gas under EIP-8038, so about 95,100 slots at 200M gas, plus
/// [`SYSTEM_PHASE_SLOTS`]
pub const fn max_slots_in_block(gas_limit: u64) -> usize {
    let cheapest_access = (evm_interpreter::v3::gas::COLD_STORAGE_ACCESS
        + evm_interpreter::gas_constants::VERYLOW) as u64;
    (gas_limit / cheapest_access) as usize + SYSTEM_PHASE_SLOTS
}

/// EIP-7954: the largest code an account can hold, in bytes (was EIP-170's 24 KiB)
pub const EIP_7954_MAX_CODE_SIZE: usize = 0x10000;
/// EIP-7954: the largest initcode a creation can run, in bytes (was EIP-3860's 48 KiB)
pub const EIP_7954_MAX_INITCODE_SIZE: usize = 0x20000;

/// Why the Glamsterdam checks rejected a block (or could not run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlamsterdamError {
    /// An oracle query failed
    Oracle,
    /// The target header does not decode as a Glamsterdam header
    HeaderEncoding,
    /// The parent header does not decode as a Glamsterdam header
    ParentHeaderEncoding,
    /// The parent header does not hash to the target's parent hash
    ParentHashMismatch,
    /// The parent header's number is not the target's number minus one
    ParentNumberMismatch,
    /// The block gas limit is above what the system can account for
    GasLimitTooHigh,
    /// A header invariant with respect to the parent does not hold
    ParentConsistency(&'static str),
    /// The access list does not hash to the header's `block_access_list_hash`
    AccessListHash,
    /// The access list RLP is malformed
    AccessListEncoding,
    /// The access list violates an EIP-7928 ordering or uniqueness rule
    AccessListOrdering(&'static str),
    /// An access list value is out of range (a block access index above `u32`,
    /// or beyond the block's post-transaction phase)
    AccessListValue,
    /// A code change sets code above the EIP-7954 size limit
    CodeSizeExceeded,
    /// More accounts or slots than the tables can hold
    Capacity(&'static str),
    /// A trie operation failed (a missing or malformed witness node)
    Mpt(&'static str),
    /// Seeking a slot of the account at this address index failed (a missing or
    /// malformed witness node of its storage trie)
    MptSlotSeek(u32),
    /// A leaf of the accounts trie is not an account encoding
    AccountEncoding,
    /// A leaf of a storage trie is not a slot value encoding
    SlotEncoding,
    /// The state root computed from the access list differs from the header's
    StateRootMismatch,
    /// A lookup in the transactions or receipts trie hit malformed or missing data
    IndexTrie(&'static str),
    /// A receipt does not decode
    ReceiptEncoding,
    /// A deposit event of the deposit contract is malformed
    DepositEvent,
    /// The witnessed transaction range is outside the block, or empty inside it
    TransactionRange,
    /// A transactions or receipts trie holds an item past the transaction count,
    /// or an item that is not at an item index
    IndexTrieNotDense,
    /// A witnessed initial value table is malformed
    InitialValuesEncoding,
    /// The outputs of the ranges of a block do not aggregate to the block
    Aggregation(&'static str),
    /// The block hashes a range read do not follow from the header chain
    History(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_bounds_follow_the_gas_limit() {
        // a 200M gas block: ~66.6k accounts and ~95.1k slots
        assert_eq!(
            max_accounts_in_block(200_000_000),
            66_600 + SYSTEM_PHASE_ACCOUNTS
        );
        assert_eq!(max_slots_in_block(200_000_000), 95_102 + SYSTEM_PHASE_SLOTS);
        // the Sepolia limit of the fixtures, and a 60M mainnet-style one
        assert_eq!(
            max_accounts_in_block(199_413_688),
            66_404 + SYSTEM_PHASE_ACCOUNTS
        );
        assert_eq!(max_slots_in_block(60_000_000), 28_530 + SYSTEM_PHASE_SLOTS);
        assert_eq!(max_accounts_in_block(0), SYSTEM_PHASE_ACCOUNTS);
    }
}
