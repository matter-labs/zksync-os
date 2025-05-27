use crate::bootloader::rlp;
use arrayvec::ArrayVec;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use zk_ee::utils::Bytes32;

// Keccak256(RLP([])) = 0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347
pub const EMPTY_OMMER_ROOT_HASH: [u8; 32] = [
    0x1d, 0xcc, 0x4d, 0xe8, 0xde, 0xc7, 0x5d, 0x7a, 0xab, 0x85, 0xb5, 0x67, 0xb6, 0xcc, 0xd4, 0x1a,
    0xd3, 0x12, 0x45, 0x1b, 0x94, 0x8a, 0x74, 0x13, 0xf0, 0xa1, 0x42, 0xfd, 0x40, 0xd4, 0x93, 0x47,
];

// based on https://github.com/alloy-rs/alloy/blob/main/crates/consensus/src/block/header.rs#L23
/// Ethereum Block header
/// This header doesn’t include:
/// - BlobGasUsed, ExcessBlobGas, TargetBlobsPerBlock (EIP-4844 and EIP-7742)
/// - WithdrawalsHash ( EIP-4895 )
/// - ParentBeaconRoot ( EIP-4788 )
/// - RequestsHash( EIP-7685 )
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockHeader {
    /// The Keccak 256-bit hash of the parent
    /// block’s header, in its entirety; formally Hp.
    pub parent_hash: Bytes32,
    /// The Keccak 256-bit hash of the ommers list portion of this block; formally Ho.
    pub ommers_hash: Bytes32,
    /// The 160-bit address to which all fees collected from the successful mining of this block
    /// be transferred; formally Hc.
    pub beneficiary: B160,
    /// The Keccak 256-bit hash of the root node of the state trie, after all transactions are
    /// executed and finalisations applied; formally Hr.
    pub state_root: Bytes32,
    /// The Keccak 256-bit hash of the root node of the trie structure populated with each
    /// transaction in the transactions list portion of the block; formally Ht.
    pub transactions_root: Bytes32,
    /// The Keccak 256-bit hash of the root node of the trie structure populated with the receipts
    /// of each transaction in the transactions list portion of the block; formally He.
    pub receipts_root: Bytes32,
    /// The Bloom filter composed from indexable information (logger address and log topics)
    /// contained in each log entry from the receipt of each transaction in the transactions list;
    /// formally Hb.
    pub logs_bloom: [u8; 256],
    /// A scalar value corresponding to the difficulty level of this block. This can be calculated
    /// from the previous block’s difficulty level and the timestamp; formally Hd.
    pub difficulty: U256,
    /// A scalar value equal to the number of ancestor blocks. The genesis block has a number of
    /// zero; formally Hi.
    pub number: u64,
    /// A scalar value equal to the current limit of gas expenditure per block; formally Hl.
    pub gas_limit: u64,
    /// A scalar value equal to the total gas used in transactions in this block; formally Hg.
    pub gas_used: u64,
    /// A scalar value equal to the reasonable output of Unix’s time() at this block’s inception;
    /// formally Hs.
    pub timestamp: u64,
    /// An arbitrary byte array containing data relevant to this block. This must be 32 bytes or
    /// fewer; formally Hx.
    pub extra_data: ArrayVec<u8, 32>,
    /// A 256-bit hash which, combined with the
    /// nonce, proves that a sufficient amount of computation has been carried out on this block;
    /// formally Hm.
    pub mix_hash: Bytes32,
    /// A 64-bit value which, combined with the mixhash, proves that a sufficient amount of
    /// computation has been carried out on this block; formally Hn.
    pub nonce: [u8; 8],
    /// A scalar representing EIP1559 base fee which can move up or down each block according
    /// to a formula which is a function of gas used in parent block and gas target
    /// (block gas limit divided by elasticity multiplier) of parent block.
    /// The algorithm results in the base fee per gas increasing when blocks are
    /// above the gas target, and decreasing when blocks are below the gas target. The base fee per
    /// gas is burned.
    pub base_fee_per_gas: u64,
}

impl BlockHeader {
    ///
    /// Create ZKsync OS block header.
    /// We are using Ethereum like block format, but set some fields differently.
    ///
    pub fn new(
        parent_hash: Bytes32,
        beneficiary: B160,
        transactions_rolling_hash: Bytes32,
        number: u64,
        gas_limit: u64,
        gas_used: u64,
        timestamp: u64,
        mix_hash: Bytes32,
        base_fee_per_gas: u64,
    ) -> Self {
        Self {
            parent_hash,
            // omners list is empty after EIP-3675
            ommers_hash: Bytes32::from(EMPTY_OMMER_ROOT_HASH),
            beneficiary,
            // for now state root is zero
            state_root: Bytes32::ZERO,
            // for now we'll use rolling hash as txs commitment
            transactions_root: transactions_rolling_hash,
            // for now receipts root is zero
            receipts_root: Bytes32::ZERO,
            // for now logs bloom is zero
            logs_bloom: [0; 256],
            // difficulty is set to zero after EIP-3675
            difficulty: U256::ZERO,
            number,
            gas_limit,
            gas_used,
            timestamp,
            // for now extra data is empty
            extra_data: ArrayVec::new(),
            mix_hash,
            // nonce is set to zero after EIP-3675
            nonce: [0u8; 8],
            // currently operator can set any base_fee_per_gas, in practice it's usually constant
            base_fee_per_gas,
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut total_list_len = 0;
        total_list_len += rlp::estimate_bytes_encoding_len(self.parent_hash.as_u8_ref());
        total_list_len += rlp::estimate_bytes_encoding_len(self.ommers_hash.as_u8_ref());
        // beneficiary
        total_list_len += rlp::ADDRESS_ENCODING_LEN;
        total_list_len += rlp::estimate_bytes_encoding_len(self.state_root.as_u8_ref());
        total_list_len += rlp::estimate_bytes_encoding_len(self.transactions_root.as_u8_ref());
        total_list_len += rlp::estimate_bytes_encoding_len(self.receipts_root.as_u8_ref());
        total_list_len += rlp::estimate_bytes_encoding_len(&self.logs_bloom);
        total_list_len += rlp::estimate_number_encoding_len(&self.difficulty.to_be_bytes::<32>());
        total_list_len += rlp::estimate_number_encoding_len(&self.number.to_be_bytes());
        total_list_len += rlp::estimate_number_encoding_len(&self.gas_limit.to_be_bytes());
        total_list_len += rlp::estimate_number_encoding_len(&self.gas_used.to_be_bytes());
        total_list_len += rlp::estimate_number_encoding_len(&self.timestamp.to_be_bytes());
        total_list_len += rlp::estimate_bytes_encoding_len(self.extra_data.as_slice());
        total_list_len += rlp::estimate_bytes_encoding_len(self.mix_hash.as_u8_ref());
        total_list_len += rlp::estimate_bytes_encoding_len(&self.nonce);
        total_list_len += rlp::estimate_number_encoding_len(&self.base_fee_per_gas.to_be_bytes());

        let mut hasher = Keccak256::new();
        rlp::apply_list_length_encoding_to_hash(total_list_len, &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.parent_hash.as_u8_ref(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.ommers_hash.as_u8_ref(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(&self.beneficiary.to_be_bytes::<20>(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.state_root.as_u8_ref(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.transactions_root.as_u8_ref(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.receipts_root.as_u8_ref(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(&self.logs_bloom, &mut hasher);
        rlp::apply_number_encoding_to_hash(&self.difficulty.to_be_bytes::<32>(), &mut hasher);
        rlp::apply_number_encoding_to_hash(&self.number.to_be_bytes(), &mut hasher);
        rlp::apply_number_encoding_to_hash(&self.gas_limit.to_be_bytes(), &mut hasher);
        rlp::apply_number_encoding_to_hash(&self.gas_used.to_be_bytes(), &mut hasher);
        rlp::apply_number_encoding_to_hash(&self.timestamp.to_be_bytes(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.extra_data.as_slice(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(self.mix_hash.as_u8_ref(), &mut hasher);
        rlp::apply_bytes_encoding_to_hash(&self.nonce, &mut hasher);
        rlp::apply_number_encoding_to_hash(&self.base_fee_per_gas.to_be_bytes(), &mut hasher);

        hasher.finalize()
    }
}
