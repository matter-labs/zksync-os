//! The Glamsterdam block header: the Pectra/Osaka header plus
//! `block_access_list_hash` (EIP-7928) and `slot_number` (EIP-7843), and the
//! invariants between a header and its parent.

use super::GlamsterdamError;
use crate::bootloader::block_flow::ethereum::block_header::{
    PectraForkHeader, PectraForkHeaderReflection,
};
use crate::bootloader::block_flow::ethereum::utils::fake_exponential;
use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{Rlp, RlpListDecode};
use ruint::aliases::U256;
use zk_ee::system::GAS_PER_BLOB;
use zk_ee::utils::Bytes32;

/// `keccak256(rlp([]))`: the ommers hash of every post-merge block
pub const EMPTY_OMMERS_HASH: Bytes32 =
    Bytes32::from_hex("1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347");

pub const MIN_BASE_FEE_PER_BLOB_GAS: u64 = 1;
/// EIP-7918: the blob base fee is floored at this many execution gas worth of fee per blob
pub const BLOB_BASE_COST: u64 = 1 << 13;
pub const EIP_1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: u64 = 8;
pub const EIP_1559_ELASTICITY_MULTIPLIER: u64 = 2;
pub const EIP_1559_MIN_GAS_LIMIT: u64 = 5000;

/// A blob count schedule (EIP-7892 blob parameter only forks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobSchedule {
    pub target_blobs_per_block: u64,
    pub max_blobs_per_block: u64,
    pub base_fee_update_fraction: u64,
}

impl BlobSchedule {
    pub const OSAKA: Self = Self {
        target_blobs_per_block: 6,
        max_blobs_per_block: 9,
        base_fee_update_fraction: 5007716,
    };
    pub const BPO1: Self = Self {
        target_blobs_per_block: 10,
        max_blobs_per_block: 15,
        base_fee_update_fraction: 8346193,
    };
    pub const BPO2: Self = Self {
        target_blobs_per_block: 14,
        max_blobs_per_block: 21,
        base_fee_update_fraction: 11684671,
    };

    pub fn target_blob_gas_per_block(&self) -> u64 {
        self.target_blobs_per_block * GAS_PER_BLOB
    }

    pub fn max_blob_gas_per_block(&self) -> u64 {
        self.max_blobs_per_block * GAS_PER_BLOB
    }

    /// EIP-4844 blob base fee for the given excess blob gas
    pub fn blob_base_fee(&self, excess_blob_gas: u64) -> U256 {
        fake_exponential(
            U256::from(MIN_BASE_FEE_PER_BLOB_GAS),
            &U256::from(excess_blob_gas),
            &U256::from(self.base_fee_update_fraction),
        )
    }

    /// The excess blob gas of the child of a block with the given header fields
    /// (EIP-4844 with the EIP-7918 reserve price)
    pub fn next_excess_blob_gas(
        &self,
        parent_excess_blob_gas: u64,
        parent_blob_gas_used: u64,
        parent_base_fee_per_gas: u64,
    ) -> u64 {
        let target = self.target_blob_gas_per_block();
        if parent_excess_blob_gas + parent_blob_gas_used < target {
            return 0;
        }
        let blob_base_fee = self.blob_base_fee(parent_excess_blob_gas);
        if U256::from(BLOB_BASE_COST) * U256::from(parent_base_fee_per_gas)
            > U256::from(GAS_PER_BLOB) * blob_base_fee
        {
            parent_excess_blob_gas
                + parent_blob_gas_used * (self.max_blobs_per_block - self.target_blobs_per_block)
                    / self.max_blobs_per_block
        } else {
            parent_excess_blob_gas + parent_blob_gas_used - target
        }
    }
}

/// Zero-copy view of a Glamsterdam header's RLP
#[derive(Clone, Copy, Debug)]
pub struct GlamsterdamHeaderReflection<'a> {
    pub base: PectraForkHeaderReflection<'a>,
    pub block_access_list_hash: &'a [u8; 32],
    pub slot_number: u64,
}

impl<'a> RlpListDecode<'a> for GlamsterdamHeaderReflection<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let base = PectraForkHeaderReflection::decode_list_body(r)?;
        let block_access_list_hash = r.bytes_exact()?;
        let slot_number = r.u64()?;
        Ok(Self {
            base,
            block_access_list_hash,
            slot_number,
        })
    }
}

/// A parsed Glamsterdam header
#[derive(Clone, Copy, Debug)]
pub struct GlamsterdamHeader {
    pub base: PectraForkHeader,
    pub block_access_list_hash: Bytes32,
    pub slot_number: u64,
}

impl GlamsterdamHeader {
    /// Parses the RLP encoding of a header, which must be consumed exactly
    pub fn parse(encoding: &[u8]) -> Result<Self, InvalidTransaction> {
        let reflection = GlamsterdamHeaderReflection::decode_list_full(encoding)?;
        Ok(Self {
            base: PectraForkHeader::from_relection(reflection.base),
            block_access_list_hash: Bytes32::from_array(*reflection.block_access_list_hash),
            slot_number: reflection.slot_number,
        })
    }

    /// Checks the invariants of a header with respect to its parent: numbering,
    /// timestamps and slots, the post-merge constants, the EIP-1559 gas limit
    /// and base fee, and the EIP-4844 excess blob gas under `schedule`.
    /// `parent_hash` is the hash of the parent's encoding.
    pub fn check_against_parent(
        &self,
        parent: &GlamsterdamHeader,
        parent_hash: &Bytes32,
        schedule: &BlobSchedule,
    ) -> Result<(), GlamsterdamError> {
        use GlamsterdamError::ParentConsistency;
        let header = &self.base;
        let parent_header = &parent.base;

        if &header.parent_hash != parent_hash {
            return Err(GlamsterdamError::ParentHashMismatch);
        }
        if parent_header.number.checked_add(1) != Some(header.number) {
            return Err(GlamsterdamError::ParentNumberMismatch);
        }
        if header.timestamp <= parent_header.timestamp {
            return Err(ParentConsistency("timestamp does not increase"));
        }
        if self.slot_number <= parent.slot_number {
            return Err(ParentConsistency("slot number does not increase"));
        }

        // post-merge constants
        if !header.difficulty.is_zero() {
            return Err(ParentConsistency("difficulty is not zero"));
        }
        if header.nonce != [0u8; 8] {
            return Err(ParentConsistency("nonce is not zero"));
        }
        if header.ommers_hash != EMPTY_OMMERS_HASH {
            return Err(ParentConsistency("ommers hash is not of the empty list"));
        }

        // EIP-1559: gas limit within 1/1024 of the parent's, base fee from the parent's usage
        {
            let parent_gas_limit = parent_header.gas_limit;
            let slack = parent_gas_limit / 1024;
            if header.gas_limit >= parent_gas_limit + slack {
                return Err(ParentConsistency("gas limit increased too much"));
            }
            if header.gas_limit + slack <= parent_gas_limit {
                return Err(ParentConsistency("gas limit decreased too much"));
            }
            if header.gas_limit < EIP_1559_MIN_GAS_LIMIT {
                return Err(ParentConsistency("gas limit below the minimum"));
            }
            let parent_gas_target = parent_gas_limit / EIP_1559_ELASTICITY_MULTIPLIER;
            let parent_base_fee = parent_header.base_fee_per_gas;
            let parent_gas_used = parent_header.gas_used;
            if parent_gas_target == 0 {
                return Err(ParentConsistency("parent gas target is zero"));
            }
            let expected_base_fee = if parent_gas_used == parent_gas_target {
                U256::from(parent_base_fee)
            } else if parent_gas_used > parent_gas_target {
                let gas_used_delta = parent_gas_used - parent_gas_target;
                let delta = (U256::from(parent_base_fee) * U256::from(gas_used_delta)
                    / U256::from(parent_gas_target)
                    / U256::from(EIP_1559_BASE_FEE_MAX_CHANGE_DENOMINATOR))
                .max(U256::from(1u64));
                U256::from(parent_base_fee) + delta
            } else {
                let gas_used_delta = parent_gas_target - parent_gas_used;
                let delta = U256::from(parent_base_fee) * U256::from(gas_used_delta)
                    / U256::from(parent_gas_target)
                    / U256::from(EIP_1559_BASE_FEE_MAX_CHANGE_DENOMINATOR);
                U256::from(parent_base_fee) - delta
            };
            if U256::from(header.base_fee_per_gas) != expected_base_fee {
                return Err(ParentConsistency("base fee per gas"));
            }
        }

        // EIP-4844 / EIP-7918 / EIP-7892
        {
            let expected = schedule.next_excess_blob_gas(
                parent_header.excess_blob_gas,
                parent_header.blob_gas_used,
                parent_header.base_fee_per_gas,
            );
            if header.excess_blob_gas != expected {
                return Err(ParentConsistency("excess blob gas"));
            }
            if header.blob_gas_used > schedule.max_blob_gas_per_block()
                || !header.blob_gas_used.is_multiple_of(GAS_PER_BLOB)
            {
                return Err(ParentConsistency("blob gas used"));
            }
        }

        Ok(())
    }
}
