//! Glamsterdam Ethereum STF.
//!
//! Glamsterdam adds block-level access lists ([EIP-7928]) and the slot number
//! ([EIP-7843]) to the block. The access list commits to every state access of
//! the block, so the STF is split: [`state_consistency`] witnesses the header,
//! its parent and the access list, and verifies the parent state root against
//! the header's state root through the access list alone; the transactions
//! are then checked against the access list independently.
//!
//! Nothing here depends on the `System` traits: the functions take an oracle
//! and an allocator only, so they run natively and on the proving target
//! without the rest of the bootloader.
//!
//! [EIP-7928]: https://eips.ethereum.org/EIPS/eip-7928
//! [EIP-7843]: https://eips.ethereum.org/EIPS/eip-7843

pub mod block_header;
pub mod oracle_queries;
pub mod state_consistency;

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
    /// An access list value is out of range (a block access index above `u32`)
    AccessListValue,
    /// A code change sets code above the EIP-7954 size limit
    CodeSizeExceeded,
    /// More accounts or slots than the tables can hold
    Capacity(&'static str),
    /// A trie operation failed (a missing or malformed witness node)
    Mpt(&'static str),
    /// A leaf of the accounts trie is not an account encoding
    AccountEncoding,
    /// A leaf of a storage trie is not a slot value encoding
    SlotEncoding,
    /// The state root computed from the access list differs from the header's
    StateRootMismatch,
}
