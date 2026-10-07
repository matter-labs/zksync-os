//! The linkage between the programs of a Glamsterdam block: the pre-execution
//! check and the transaction ranges each publish the hash of one
//! [`ProgramOutput`], a plain dense structure, as their public output. The
//! fields that identify the block and the initial value tables must agree
//! across the programs; `bal_mask` commits to the accounts and slots of the
//! block access list a program accessed (all clear for the pre-execution
//! check, which executes nothing), and `num_unique_bal_records` is the length
//! of that mask. An aggregation program ([`LinkageAggregation`]) takes the
//! outputs' preimages and the masks' preimages, checks them against the public
//! outputs, requires the shared fields to agree and the union of the masks to
//! be full: every record of the access list is accessed somewhere in the block.

use super::index_mask::IndexMask;
use super::state_consistency::StateConsistencyOutput;
use super::tx_range::block_flow::GlamsterdamRangeOutput;
use super::GlamsterdamError;
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// What every program of a block publishes, hashed
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgramOutput {
    /// The header hash
    pub block_hash: Bytes32,
    /// keccak of the pre-block account leaf table (`InitialValues::accounts`)
    pub initial_accounts: Bytes32,
    /// keccak of the pre-block slot leaf table (`InitialValues::slots`)
    pub initial_slots: Bytes32,
    /// keccak of the fresh account mask (`InitialValues::fresh_accounts`)
    pub fresh_accounts_mask: Bytes32,
    /// keccak of the mask of the access list records the program accessed
    /// (the accounts by address index, then the slots by position), all clear
    /// for the pre-execution check
    pub bal_mask: Bytes32,
    /// The number of records of the access list (accounts and slots): the
    /// length of `bal_mask`'s preimage
    pub num_unique_bal_records: u32,
}

impl ProgramOutput {
    /// The dense encoding: the fields in order, the count little-endian
    pub const ENCODED_LEN: usize = 5 * 32 + 4;

    pub fn encode(&self) -> [u8; Self::ENCODED_LEN] {
        let mut bytes = [0u8; Self::ENCODED_LEN];
        for (chunk, field) in bytes.as_chunks_mut::<32>().0.iter_mut().zip([
            &self.block_hash,
            &self.initial_accounts,
            &self.initial_slots,
            &self.fresh_accounts_mask,
            &self.bal_mask,
        ]) {
            *chunk = field.as_u8_array();
        }
        bytes[5 * 32..].copy_from_slice(&self.num_unique_bal_records.to_le_bytes());
        bytes
    }

    /// keccak of the encoding: the public output of the program
    pub fn hash(&self) -> Bytes32 {
        keccak(&self.encode())
    }

    /// The hash as the words a program returns
    pub fn hash_words(&self) -> [u32; 8] {
        let mut words = [0u32; 8];
        for (word, chunk) in words
            .iter_mut()
            .zip(self.hash().as_u8_array_ref().as_chunks::<4>().0)
        {
            *word = u32::from_le_bytes(*chunk);
        }
        words
    }

    /// The output of the pre-execution check: the tables it produced and a
    /// clear mask
    pub fn of_pre_execution<A: Allocator>(
        state: &StateConsistencyOutput<A>,
        allocator: A,
    ) -> Result<Self> {
        let [initial_accounts, fresh_accounts_mask, initial_slots] = state.initial_values_hashes();
        let num_records = state.counts.accounts + state.counts.slots;
        let clear = IndexMask::zeroed(num_records, allocator)?;
        Ok(Self {
            block_hash: state.header_hash,
            initial_accounts,
            initial_slots,
            fresh_accounts_mask,
            bal_mask: mask_hash(&clear),
            num_unique_bal_records: clear.len(),
        })
    }

    /// The output of a transaction range: the tables it witnessed and the
    /// records it accessed
    pub fn of_range<A: Allocator>(output: &GlamsterdamRangeOutput<A>) -> Self {
        let [initial_accounts, fresh_accounts_mask, initial_slots] = output.initial_values.hashes;
        Self {
            block_hash: output.header_hash,
            initial_accounts,
            initial_slots,
            fresh_accounts_mask,
            bal_mask: mask_hash(&output.accessed),
            num_unique_bal_records: output.accessed.len(),
        }
    }

    /// Whether `other` describes the same block and tables
    fn same_block(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
            && self.initial_accounts == other.initial_accounts
            && self.initial_slots == other.initial_slots
            && self.fresh_accounts_mask == other.fresh_accounts_mask
            && self.num_unique_bal_records == other.num_unique_bal_records
    }
}

/// keccak of a mask's bytes
pub fn mask_hash<A: Allocator>(mask: &IndexMask<A>) -> Bytes32 {
    keccak(mask.bytes())
}

fn keccak(bytes: &[u8]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// The aggregation program over the linkage: given the public outputs of the
/// pre-execution check and of the ranges (as proven), their preimages and the
/// masks' preimages, checks that the programs describe one block and that
/// together they accessed every record of the access list
pub struct LinkageAggregation<A: Allocator> {
    pre_execution: ProgramOutput,
    union: IndexMask<A>,
}

impl<A: Allocator> LinkageAggregation<A> {
    /// Starts from the pre-execution check: `public_output` is its proven
    /// output, `pre_execution` the preimage, whose mask must be clear
    pub fn new(
        public_output: Bytes32,
        pre_execution: &ProgramOutput,
        allocator: A,
    ) -> Result<Self> {
        let fail = GlamsterdamError::Aggregation;
        if pre_execution.hash() != public_output {
            return Err(fail("the pre-execution output is not the proven one"));
        }
        let union = IndexMask::zeroed(pre_execution.num_unique_bal_records as usize, allocator)?;
        if mask_hash(&union) != pre_execution.bal_mask {
            return Err(fail("the pre-execution check must publish a clear mask"));
        }
        Ok(Self {
            pre_execution: *pre_execution,
            union,
        })
    }

    /// Adds a range: `public_output` is its proven output, `range` the
    /// preimage and `bal_mask` the preimage of its mask
    pub fn add(
        &mut self,
        public_output: Bytes32,
        range: &ProgramOutput,
        bal_mask: &IndexMask<A>,
    ) -> Result<()> {
        let fail = GlamsterdamError::Aggregation;
        if range.hash() != public_output {
            return Err(fail("the range output is not the proven one"));
        }
        if !self.pre_execution.same_block(range) {
            return Err(fail("the range describes another block or other tables"));
        }
        if mask_hash(bal_mask) != range.bal_mask {
            return Err(fail("the mask is not the range's"));
        }
        self.union.or_assign(bal_mask)
    }

    /// Every record of the access list was accessed by some range
    pub fn finish(self) -> Result<()> {
        if !self.union.is_full() {
            return Err(GlamsterdamError::Aggregation(
                "a record of the access list is never accessed",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;

    fn output(num: u32, mask: &IndexMask<Global>) -> ProgramOutput {
        ProgramOutput {
            block_hash: Bytes32::from_array([1; 32]),
            initial_accounts: Bytes32::from_array([2; 32]),
            initial_slots: Bytes32::from_array([3; 32]),
            fresh_accounts_mask: Bytes32::from_array([4; 32]),
            bal_mask: mask_hash(mask),
            num_unique_bal_records: num,
        }
    }

    #[test]
    fn encoding_is_dense_and_the_hash_covers_it() {
        let clear = IndexMask::zeroed(40, Global).unwrap();
        let out = output(40, &clear);
        let encoded = out.encode();
        assert_eq!(encoded.len(), 164);
        assert_eq!(&encoded[..32], &[1; 32]);
        assert_eq!(&encoded[160..], &40u32.to_le_bytes());
        let mut other = out;
        other.num_unique_bal_records = 41;
        assert_ne!(out.hash(), other.hash());
        let words = out.hash_words();
        assert_eq!(words[0].to_le_bytes(), out.hash().as_u8_array()[..4]);
    }

    #[test]
    fn aggregation_requires_full_union() {
        let clear = IndexMask::zeroed(40, Global).unwrap();
        let pre = output(40, &clear);
        let first = IndexMask::from_flags((0..40).map(|i| i < 25), Global).unwrap();
        let second = IndexMask::from_flags((0..40).map(|i| i >= 20), Global).unwrap();
        let first_out = output(40, &first);
        let second_out = output(40, &second);

        let mut aggregation = LinkageAggregation::new(pre.hash(), &pre, Global).unwrap();
        aggregation
            .add(first_out.hash(), &first_out, &first)
            .unwrap();
        aggregation
            .add(second_out.hash(), &second_out, &second)
            .unwrap();
        aggregation.finish().unwrap();

        let mut partial = LinkageAggregation::new(pre.hash(), &pre, Global).unwrap();
        partial.add(first_out.hash(), &first_out, &first).unwrap();
        assert!(partial.finish().is_err());

        // a wrong public output, a mask that is not the range's, another block, a dirty pre-execution mask
        let mut wrong = LinkageAggregation::new(pre.hash(), &pre, Global).unwrap();
        assert!(wrong.add(second_out.hash(), &first_out, &first).is_err());
        assert!(wrong.add(first_out.hash(), &first_out, &second).is_err());
        let mut foreign = first_out;
        foreign.block_hash = Bytes32::ZERO;
        assert!(wrong.add(foreign.hash(), &foreign, &first).is_err());
        assert!(LinkageAggregation::new(pre.hash(), &first_out, Global).is_err());
        assert!(LinkageAggregation::new(first_out.hash(), &pre, Global).is_err());
    }
}
