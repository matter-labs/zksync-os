//! The EIP-7685 requests of a block as the ranges establish them. The
//! requests hash of the header is `sha256(h_0 || h_1 || ...)` over the
//! non-empty request types, `h_t = sha256(t || data_t)`. Of the request
//! types, only the deposit requests (type 0, EIP-6110) come out of the logs:
//! the deposit events of the whole block's transactions, in order. The others
//! (the withdrawal and consolidation requests of EIP-7002 and EIP-7251, the
//! builder deposit and exit requests of EIP-8282, types 1 to 4) are dequeued
//! from the storage of their system contracts by the system calls of the
//! post-transaction phase, so the range that runs it has them in full.
//!
//! A range sees the deposit events of its own transactions only, so the
//! hashing of `0x00 || deposit requests` is carried across the ranges as a
//! sha256 state ([`Sha256Accumulator`], the hasher's own words and buffer as
//! bytes): a range witnesses the state after the transactions before it
//! (unverified, as it witnesses its start gas; the first range starts from
//! the state after the type byte), absorbs its own deposit requests after its
//! transactions ran, and publishes both states. The aggregation requires the
//! states to chain: each range starts where the previous one ended. The range
//! that runs the post-transaction phase finishes the hashing, adds the hashes
//! of the other types, and compares the result with the header's requests
//! hash itself.

use super::super::GlamsterdamError;
use super::oracle_queries::GLAMSTERDAM_DEPOSIT_REQUESTS_HASHER_QUERY_ID;
use crate::bootloader::block_flow::ethereum::eip_6110_deposit_events_parser::{
    eip6110_deposit_requests, DepositRequestsSink, DEPOSIT_REQUEST_EIP_7685_TYPE,
    DEPOSIT_REQUEST_LEN,
};
use crate::bootloader::supported_ees::EthereumLikeSystem;
use crypto::sha256::Sha256;
// the array type of sha2 0.10's compression function
#[allow(deprecated)]
use sha2::digest::generic_array::{typenum::U64, GenericArray};
use zk_ee::internal_error;
use zk_ee::oracle::memory_io::{ContinuousDeserializable, ContinuousSerializable, OracleQuery};
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::{IOSubsystemExt, IOTeardown, System};
use zk_ee::utils::Bytes32;

/// The request types of the post-transaction phase, in order: withdrawal
/// (EIP-7002), consolidation (EIP-7251), builder deposit and builder exit
/// (EIP-8282)
pub const SYSTEM_REQUEST_TYPES: usize = 4;

/// A sha256 computation in progress, as plain bytes: the eight state words,
/// the bytes of the block in progress, their number, and the number of
/// blocks compressed so far. The programs of a block pass it to each other
/// through their public outputs (`linkage::ProgramOutput`) and compare it
/// byte for byte, so it is `#[repr(C)]` and dense (asserted below), its
/// bytes being its encoding, and canonical: the buffer is zero past the
/// bytes in progress.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sha256Accumulator {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: u32,
    blocks: u32,
}

/// The length of the accumulator, which is its own encoding
pub const SHA256_ACCUMULATOR_LEN: usize = 8 * 4 + 64 + 4 + 4;

const _: () = {
    assert!(core::mem::size_of::<Sha256Accumulator>() == SHA256_ACCUMULATOR_LEN);
    assert!(core::mem::align_of::<Sha256Accumulator>() == 4);
    assert!(core::mem::offset_of!(Sha256Accumulator, state) == 0);
    assert!(core::mem::offset_of!(Sha256Accumulator, buffer) == 32);
    assert!(core::mem::offset_of!(Sha256Accumulator, buffered) == 96);
    assert!(core::mem::offset_of!(Sha256Accumulator, blocks) == 100);
    // the words' bytes are their little-endian encoding on every target
    assert!(cfg!(target_endian = "little"));
};

/// The sha256 initial state (FIPS 180-4)
const SHA256_INITIAL_STATE: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

impl Sha256Accumulator {
    /// All zero: not a computation; the marker of a program that carries none
    pub const ZERO: Self = Self {
        state: [0; 8],
        buffer: [0; 64],
        buffered: 0,
        blocks: 0,
    };

    /// The empty computation
    pub fn new() -> Self {
        Self {
            state: SHA256_INITIAL_STATE,
            buffer: [0; 64],
            buffered: 0,
            blocks: 0,
        }
    }

    /// Where the deposit request hashing of every block starts: the type
    /// byte absorbed, no request yet
    pub fn deposit_requests_head() -> Self {
        let mut head = Self::new();
        head.absorb(&[DEPOSIT_REQUEST_EIP_7685_TYPE]);
        head
    }

    /// The number of bytes absorbed
    pub fn len(&self) -> u64 {
        self.blocks as u64 * 64 + self.buffered as u64
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[allow(deprecated)]
    fn compress(&mut self, block: &[u8; 64]) {
        // SAFETY: `GenericArray<u8, U64>` and `[u8; 64]` have the same layout
        // (what `compress256` itself relies on)
        let block: &GenericArray<u8, U64> =
            unsafe { &*(block as *const [u8; 64] as *const GenericArray<u8, U64>) };
        sha2::compress256(&mut self.state, core::slice::from_ref(block));
        self.blocks += 1;
    }

    /// Absorbs the bytes
    pub fn absorb(&mut self, mut bytes: &[u8]) {
        let buffered = self.buffered as usize;
        if buffered > 0 {
            let take = bytes.len().min(64 - buffered);
            self.buffer[buffered..buffered + take].copy_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if buffered + take < 64 {
                self.buffered = (buffered + take) as u32;
                return;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buffer = [0; 64];
            self.buffered = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<64>();
        for block in blocks {
            self.compress(block);
        }
        self.buffer[..rest.len()].copy_from_slice(rest);
        self.buffered = rest.len() as u32;
    }

    /// The digest of everything absorbed
    pub fn finalize(mut self) -> Bytes32 {
        let bit_len = self.len() * 8;
        let buffered = self.buffered as usize;
        let mut block = [0u8; 64];
        block[..buffered].copy_from_slice(&self.buffer[..buffered]);
        block[buffered] = 0x80;
        if buffered + 1 > 56 {
            self.compress(&block);
            block = [0u8; 64];
        }
        block[56..].copy_from_slice(&bit_len.to_be_bytes());
        self.compress(&block);
        let mut digest = [0u8; 32];
        for (chunk, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *chunk = word.to_be_bytes();
        }
        Bytes32::from_array(digest)
    }

    /// The bytes: the encoding, without a copy
    pub fn as_bytes(&self) -> &[u8; SHA256_ACCUMULATOR_LEN] {
        // SAFETY: `repr(C)` without padding (asserted above), so every byte
        // is initialized, and the reference keeps it alive and unaliased
        unsafe { &*(self as *const Self as *const [u8; SHA256_ACCUMULATOR_LEN]) }
    }
}

impl Default for Sha256Accumulator {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: `repr(C)` of `u32`s and `u8`s, no padding
unsafe impl ContinuousSerializable for Sha256Accumulator {}

// SAFETY: any bytes form a value; the only invariant is checked
unsafe impl ContinuousDeserializable for Sha256Accumulator {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller
        let this = unsafe { &mut *this };
        if this.buffered >= 64 {
            return Err(internal_error!("a sha256 state with a full buffer"));
        }
        if this.buffer[this.buffered as usize..]
            .iter()
            .any(|b| *b != 0)
        {
            return Err(internal_error!("a sha256 state that is not canonical"));
        }
        Ok(this)
    }
}

impl DepositRequestsSink for Sha256Accumulator {
    fn write(&mut self, bytes: &[u8]) {
        self.absorb(bytes);
    }
}

pub struct DepositRequestsHasherQuery;

impl OracleQuery for DepositRequestsHasherQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_DEPOSIT_REQUESTS_HASHER_QUERY_ID;
    type Input = ();
    type Output = Sha256Accumulator;
}

/// Witnesses the deposit request hashing state after the transactions before
/// the range, for a range that does not start at the first transaction
pub fn witness_deposit_requests_hasher<O: IOOracle>(
    oracle: &mut O,
) -> Result<Sha256Accumulator, GlamsterdamError> {
    DepositRequestsHasherQuery::get(oracle, ()).map_err(|_| GlamsterdamError::Oracle)
}

/// The hashes `sha256(type || data)` of the non-empty system request types,
/// in type order, with the types
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemRequests {
    count: u8,
    types: [u8; SYSTEM_REQUEST_TYPES],
    hashes: [Bytes32; SYSTEM_REQUEST_TYPES],
}

impl SystemRequests {
    /// Adds the hash of the next non-empty type, which must come after the
    /// previous one; at most one per type
    pub fn push(&mut self, request_type: u8, hash: Bytes32) -> Result<(), GlamsterdamError> {
        let index = self.count as usize;
        if index >= SYSTEM_REQUEST_TYPES || (index > 0 && self.types[index - 1] >= request_type) {
            return Err(GlamsterdamError::Aggregation(
                "system request types out of order",
            ));
        }
        self.types[index] = request_type;
        self.hashes[index] = hash;
        self.count += 1;
        Ok(())
    }

    pub fn hashes(&self) -> &[Bytes32] {
        &self.hashes[..self.count as usize]
    }

    /// The non-empty request types, ascending
    pub fn types(&self) -> &[u8] {
        &self.types[..self.count as usize]
    }
}

/// What a range establishes about the requests of the block
pub struct RangeRequests {
    /// The deposit request hashing state the range started from: the head
    /// (`Sha256Accumulator::deposit_requests_head`) for a range starting at
    /// the first transaction, as witnessed otherwise
    pub start: Sha256Accumulator,
    /// The state after the deposit requests of the range's transactions
    pub end: Sha256Accumulator,
    /// For the range that ran the post-transaction phase: the system requests
    /// it hashed into the requests hash (which it checked against the header)
    pub system: Option<SystemRequests>,
}

impl RangeRequests {
    /// Absorbs the deposit requests of the events of the range, in order,
    /// from `start`
    pub fn collect<S: EthereumLikeSystem>(
        system: &System<S>,
        start: Sha256Accumulator,
        system_requests: Option<SystemRequests>,
    ) -> Result<Self, GlamsterdamError>
    where
        S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
    {
        let mut end = start;
        eip6110_deposit_requests(system, &mut end, false)
            .map_err(|()| GlamsterdamError::DepositEvent)?;
        Ok(Self {
            start,
            end,
            system: system_requests,
        })
    }

    /// The number of deposit requests of the range's transactions
    pub fn deposit_requests(&self) -> u64 {
        (self.end.len() - self.start.len()) / DEPOSIT_REQUEST_LEN as u64
    }

    /// The EIP-7685 requests hash of the block, for the range that ran the
    /// post-transaction phase: the deposit requests of the whole block as
    /// hashed across the ranges, then the system requests
    pub fn block_requests_hash(&self) -> Option<Bytes32> {
        self.system
            .as_ref()
            .map(|system| requests_hash(self.end, system))
    }
}

/// The EIP-7685 requests hash of a block from the state of the deposit
/// request hashing after the whole block (`0x00 || requests` absorbed; only
/// the type byte when there is none, in which case the type is skipped) and
/// the system request hashes
pub fn requests_hash(deposits: Sha256Accumulator, system: &SystemRequests) -> Bytes32 {
    use crypto::sha256::Digest as _;
    let mut hasher = <Sha256 as crypto::sha256::Digest>::new();
    if deposits.len() > 1 {
        hasher.update(deposits.finalize().as_u8_array_ref());
    }
    for hash in system.hashes() {
        hasher.update(hash.as_u8_array_ref());
    }
    Bytes32::from_array(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::sha256::Digest as _;

    fn digest(bytes: &[u8]) -> Bytes32 {
        Bytes32::from_array(Sha256::digest(bytes).into())
    }

    #[test]
    fn accumulator_matches_sha256_at_every_split() {
        let data: Vec<u8> = (0..700u32).map(|i| (i * 7 + 3) as u8).collect();
        for len in [
            0usize, 1, 55, 56, 57, 63, 64, 65, 119, 120, 127, 128, 192, 385, 577, 700,
        ] {
            let whole = {
                let mut acc = Sha256Accumulator::new();
                acc.absorb(&data[..len]);
                assert_eq!(acc.len(), len as u64);
                acc
            };
            assert_eq!(whole.finalize(), digest(&data[..len]), "length {len}");
            for split in [0usize, 1, 31, 32, 63, 64, 65, 100] {
                if split > len {
                    continue;
                }
                let mut acc = Sha256Accumulator::new();
                acc.absorb(&data[..split]);
                // through the bytes, as the programs pass it
                let mut copied = Sha256Accumulator::ZERO;
                // SAFETY: the bytes of an accumulator form an accumulator
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        acc.as_bytes().as_ptr(),
                        &mut copied as *mut Sha256Accumulator as *mut u8,
                        SHA256_ACCUMULATOR_LEN,
                    );
                }
                assert_eq!(copied, acc);
                copied.absorb(&data[split..len]);
                assert_eq!(copied, whole, "length {len} split at {split}");
            }
        }
        assert_eq!(
            Sha256Accumulator::ZERO.as_bytes(),
            &[0u8; SHA256_ACCUMULATOR_LEN]
        );
        assert_ne!(Sha256Accumulator::new(), Sha256Accumulator::ZERO);
        assert_eq!(Sha256Accumulator::deposit_requests_head().len(), 1);
    }

    #[test]
    fn requests_hash_follows_eip_7685() {
        let mut system = SystemRequests::default();
        system.push(1, Bytes32::from_array([1; 32])).unwrap();
        let deposits = [7u8; 192];
        // the deposits across two ranges
        let mut acc = Sha256Accumulator::deposit_requests_head();
        acc.absorb(&deposits[..96]);
        let mut second = acc;
        second.absorb(&deposits[96..]);
        let expected = {
            let mut h = Sha256::new();
            h.update(digest(&[&[0u8][..], &deposits[..]].concat()).as_u8_array_ref());
            h.update([1u8; 32]);
            Bytes32::from_array(h.finalize().into())
        };
        assert_eq!(requests_hash(second, &system), expected);
        // no deposits: the type is skipped
        let mut h = Sha256::new();
        h.update([1u8; 32]);
        assert_eq!(
            requests_hash(Sha256Accumulator::deposit_requests_head(), &system),
            Bytes32::from_array(h.finalize().into())
        );
        // no requests at all
        assert_eq!(
            requests_hash(
                Sha256Accumulator::deposit_requests_head(),
                &SystemRequests::default()
            ),
            digest(&[])
        );
        let mut full = SystemRequests::default();
        for request_type in 1..=SYSTEM_REQUEST_TYPES as u8 {
            full.push(request_type, Bytes32::ZERO).unwrap();
        }
        assert_eq!(full.types(), &[1, 2, 3, 4]);
        assert!(full.push(5, Bytes32::ZERO).is_err());
        let mut unordered = SystemRequests::default();
        unordered.push(2, Bytes32::ZERO).unwrap();
        assert!(unordered.push(2, Bytes32::ZERO).is_err());
        assert!(unordered.push(1, Bytes32::ZERO).is_err());
    }
}
