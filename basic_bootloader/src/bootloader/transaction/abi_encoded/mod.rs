//! # ABI Encoded Transaction Format
//!
//! This module contains ZKsync's custom transaction structure definition including
//! validation methods and hash calculation functions.
//!
//! ## Why AbiEncodedTransaction is needed
//!
//! AbiEncodedTransaction exists alongside Ethereum RLP transactions to support ZKsync's
//! unique Layer 2 features that cannot be expressed in standard
//! Ethereum transaction formats. This includes:
//!
//! - **`L1_L2_TX_TYPE` (0x7f)**: Transactions initiated from L1 (deposits, forced transactions)
//! - **`UPGRADE_TX_TYPE` (0x7e)**: System upgrade transactions (protocol changes)

use crate::bootloader::errors::TxError;

use self::u256be_ptr::U256BEPtr;
use core::alloc::Allocator;
use core::ops::Range;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use zk_ee::internal_error;
use zk_ee::system::Resources;
use zk_ee::utils::UsizeAlignedByteBox;

use super::charge_keccak;

#[cfg(test)]
mod tests;
pub mod u256be_ptr;

/// The generic transaction format. The structure fields are slices/references in fact.
///
/// NOTE: this is self-reference, but relatively easy one. Do NOT derive clone one it,
/// as it's unsound
pub struct AbiEncodedTransaction<A: Allocator> {
    underlying_buffer: UsizeAlignedByteBox<A>,
    // fields below are parsed
    /// The type of the transaction.
    pub tx_type: ParsedValue<u8>,
    /// The caller.
    pub from: ParsedValue<B160>,
    /// The callee.
    pub to: ParsedValue<B160>,
    /// The gasLimit to pass with the transaction.
    /// It has the same meaning as Ethereum's gasLimit.
    pub gas_limit: ParsedValue<u64>,
    /// The maximum amount of gas the user is willing to pay for a byte of pubdata.
    #[allow(dead_code)]
    pub gas_per_pubdata_limit: ParsedValue<u32>,
    /// The maximum fee per gas that the user is willing to pay.
    /// It is akin to EIP1559's maxFeePerGas.
    pub max_fee_per_gas: ParsedValue<U256>,
    /// The maximum priority fee per gas that the user is willing to pay.
    /// It is akin to EIP1559's maxPriorityFeePerGas.
    pub max_priority_fee_per_gas: ParsedValue<U256>,
    /// The transaction's paymaster. If there is no paymaster, it is equal to 0.
    pub paymaster: ParsedValue<B160>,
    /// The nonce of the transaction.
    pub nonce: ParsedValue<U256>,
    /// The value to pass with the transaction.
    pub value: ParsedValue<U256>,
    /// In the future, we might want to add some
    /// new fields to the struct. This struct
    /// is to be passed to account and any changes to its structure
    /// would mean a breaking change to these accounts. In order to prevent this,
    /// we should keep some fields as "reserved".
    ///
    /// Now `reserved[0]` is used as a flag to distinguish EIP-155(with chain id) legacy transactions.
    /// `reserved[1]` is used as EVM deployment transaction flag(`to` == null in such case).
    pub reserved: [ParsedValue<U256>; 4],
    /// The transaction's calldata.
    pub data: ParsedValue<()>,
    /// The signature of the transaction.
    pub signature: ParsedValue<()>,
    /// The properly formatted hashes of bytecodes that must be published on L1
    /// with the inclusion of this transaction. Note, that a bytecode has been published
    /// before, the user won't pay fees for its republishing.
    pub factory_deps: ParsedValue<()>,
    /// The input to the paymaster.
    pub paymaster_input: ParsedValue<()>,
    /// Field used for extra functionality.
    /// Currently, it's used for access and authorization lists.
    /// The field is encoded as a list, to be able to extend it in the
    /// future. The field is encoded a the ABI encoding of a bytestring
    /// containing the ABI encoding of the list itself.
    /// Currently the list contains 2 elements:
    /// 1. The access list: encoded as `tuple(address, bytes32[])[]`,
    ///    i.e. a list of (address, keys) pairs.
    /// 2. The authorization list: encoded as
    ///    `tuple(chain_id, address, nonce, y_parity, r, s)[]`.
    pub reserved_dynamic: ParsedValue<()>,
}

#[allow(dead_code)]
impl<A: Allocator> AbiEncodedTransaction<A> {
    /// The type id of protocol upgrade transactions.
    pub const UPGRADE_TX_TYPE: u8 = 0x7e;
    /// The type id of L1 -> L2 transactions.
    pub const L1_L2_TX_TYPE: u8 = 0x7f;

    /// Expected dynamic part(tail) offset in the transaction encoding.
    /// 16 fields, reserved takes 4 words in the static part(head) as static array.
    const DYNAMIC_PART_EXPECTED_OFFSET: usize = 19 * U256::BYTES;
    const ADDRESS_BIT_LENGTH: usize = 160;

    /// Data start position in the transaction encoding,
    /// needed to create a calldata memory region during the execution
    pub const DATA_START: usize = Self::DYNAMIC_PART_EXPECTED_OFFSET + U256::BYTES;

    ///
    /// Create structure from buffer.
    ///
    /// Validates that all the fields are correctly and tightly packed.
    /// Also validate that all the fields set correctly, in accordance with its type.
    ///
    #[allow(clippy::result_unit_err)]
    pub fn try_from_buffer(buffer: UsizeAlignedByteBox<A>) -> Result<Self, ()> {
        // We are free to move this structure as UsizeAlignedByteBox has a box inside and guarantees stable
        // address of the slice that we will use to parse a transaction, so we will not make a long code with
        // partial init and drop guards, but instead will parse via 'static transmute
        let mut parser: Parser<'static> =
            Parser::new(unsafe { core::mem::transmute::<&[u8], &[u8]>(buffer.as_slice()) });

        let tx_type = parser.parse_u8()?;
        let from = parser.parse_address()?;
        let to = parser.parse_address()?;
        let gas_limit = parser.parse_u64()?;
        let gas_per_pubdata_limit = parser.parse_u32()?;
        let max_fee_per_gas = parser.parse_u256()?;
        let max_priority_fee_per_gas = parser.parse_u256()?;
        let paymaster = parser.parse_address()?;
        let nonce = parser.parse_u256()?;
        let value = parser.parse_u256()?;

        let reserved_0 = parser.parse_u256()?;
        let reserved_1 = parser.parse_u256()?;
        let reserved_2 = parser.parse_u256()?;
        let reserved_3 = parser.parse_u256()?;

        let data_offset = parser.parse_u32()?;
        let signature_offset = parser.parse_u32()?;
        let factory_deps_offset = parser.parse_u32()?;
        let paymaster_input_offset = parser.parse_u32()?;
        let reserved_dynamic_offset = parser.parse_u32()?;

        // Validate dynamic part
        let expected_offset = Self::DYNAMIC_PART_EXPECTED_OFFSET as u32;

        if data_offset.read() != expected_offset {
            return Err(());
        }
        if data_offset.read() != parser.offset as u32 {
            return Err(());
        }
        let data = parser.parse_bytes()?;

        if signature_offset.read() != parser.offset as u32 {
            return Err(());
        }
        let signature = parser.parse_bytes()?;

        if factory_deps_offset.read() != parser.offset as u32 {
            return Err(());
        }
        let factory_deps = parser.parse_bytes32_vector()?;

        if paymaster_input_offset.read() != parser.offset as u32 {
            return Err(());
        }
        let paymaster_input = parser.parse_bytes()?;

        if reserved_dynamic_offset.read() != parser.offset as u32 {
            return Err(());
        }

        // "Consume bytes"
        let reserved_dynamic = parser.parse_bytes()?;

        if parser.slice().is_empty() == false {
            return Err(());
        }

        let new = Self {
            underlying_buffer: buffer,
            tx_type,
            from,
            to,
            gas_limit,
            gas_per_pubdata_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            paymaster,
            nonce,
            value,
            reserved: [reserved_0, reserved_1, reserved_2, reserved_3],
            data,
            signature,
            factory_deps,
            paymaster_input,
            reserved_dynamic,
        };

        new.validate_structure()?;

        Ok(new)
    }

    ///
    /// Validate that all the fields set correctly, in accordance with its type
    ///
    #[allow(clippy::result_unit_err)]
    fn validate_structure(&self) -> Result<(), ()> {
        let tx_type = self.tx_type.read();

        match tx_type {
            Self::UPGRADE_TX_TYPE | Self::L1_L2_TX_TYPE => {}
            _ => return Err(()),
        }

        // gas_per_pubdata_limit should be zero for non L1 transactions
        match tx_type {
            Self::UPGRADE_TX_TYPE | Self::L1_L2_TX_TYPE => {}
            _ => {
                if self.gas_per_pubdata_limit.read() != 0 {
                    return Err(());
                }
            }
        }

        // paymasters are not supported
        if self.paymaster.read() != B160::ZERO {
            return Err(());
        }

        // reserved[0] is EIP-155 flag for legacy txs,
        // mint_value for l1 to l2 and upgrade txs,
        // for other types should be zero
        match tx_type {
            Self::L1_L2_TX_TYPE | Self::UPGRADE_TX_TYPE => {}
            _ => {
                if !self.reserved[0].read().is_zero() {
                    return Err(());
                }
            }
        }
        // reserved[1] = refund recipient for l1 to l2 and upgrade txs
        match tx_type {
            Self::L1_L2_TX_TYPE | Self::UPGRADE_TX_TYPE => {
                // TODO: validate address?
            }
            _ => unreachable!(),
        }

        // reserved[2] and reserved[3] fields currently not used
        if !self.reserved[2].read().is_zero() || !self.reserved[3].read().is_zero() {
            return Err(());
        }

        match tx_type {
            Self::L1_L2_TX_TYPE | Self::UPGRADE_TX_TYPE => {
                if !self.signature.range.is_empty() {
                    return Err(());
                }
            }
            _ => {
                if self.signature.range.len() != 65 {
                    return Err(());
                }
            }
        }

        // paymasters are not supported
        if !self.paymaster_input.range.is_empty() {
            return Err(());
        }

        // Reserved dynamic is not supported
        if !self.reserved_dynamic.range.is_empty() {
            return Err(());
        }

        Ok(())
    }

    // To be used only with field belonging to this transaction
    pub fn encoding<T: 'static + Clone + Copy + core::fmt::Debug>(
        &self,
        field: ParsedValue<T>,
    ) -> &[u8] {
        unsafe { self.underlying_buffer.as_slice().get_unchecked(field.range) }
    }

    pub fn calldata(&self) -> &[u8] {
        unsafe {
            self.underlying_buffer
                .as_slice()
                .get_unchecked(self.data.range.clone())
        }
    }

    pub fn sig_parity_r_s<'a>(&'a self) -> (bool, &'a [u8], &'a [u8]) {
        let signature = unsafe {
            self.underlying_buffer
                .as_slice()
                .get_unchecked(self.signature.range.clone())
        };
        let r = &signature[..32];
        let s = &signature[32..64];
        let v = &signature[64];
        // Pre checked, but just in case
        assert!(*v == 27 || *v == 28);
        let parity = v - 27 == 1;
        (parity, r, s)
    }

    ///
    /// Calculate the transaction hash.
    /// i.e. the transaction hash to be used in the explorer.
    ///
    pub fn calculate_hash<R: Resources>(&self, resources: &mut R) -> Result<[u8; 32], TxError> {
        let tx_type = self.tx_type.read();
        match tx_type {
            Self::L1_L2_TX_TYPE => self.l1_tx_calculate_hash(resources),
            Self::UPGRADE_TX_TYPE => self.l1_tx_calculate_hash(resources),
            _ => Err(internal_error!("Type should be validated").into()),
        }
    }

    ///
    /// Calculate l1 tx hash:
    /// Keccak256(abi.encode(transaction))
    ///
    fn l1_tx_calculate_hash<R: Resources>(&self, resources: &mut R) -> Result<[u8; 32], TxError> {
        charge_keccak(32 + self.underlying_buffer.len(), resources)?;
        let mut hasher = Keccak256::new();
        // Note, that the correct ABI encoding of the Transaction structure starts with 0x20
        hasher.update(&U256::from(0x20).to_be_bytes::<32>());
        hasher.update(&self.underlying_buffer.as_slice());
        Ok(hasher.finalize())
    }

    /// Returns the balance required to process the transaction.
    /// If the calculation overflows, returns `None`.
    pub fn required_balance(&self) -> Option<U256> {
        let fee_amount = self
            .max_fee_per_gas
            .read()
            .checked_mul(U256::from(self.gas_limit.read()))?;
        self.value.read().checked_add(U256::from(fee_amount))
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.underlying_buffer.len()
    }
}

#[derive(Clone, Debug)]
pub struct ParsedValue<T: 'static + Clone + Copy + core::fmt::Debug> {
    pub value: T,
    pub range: Range<usize>,
}

impl<T: 'static + Clone + Copy + core::fmt::Debug> ParsedValue<T> {
    pub fn read(&self) -> T {
        self.value
    }

    pub fn read_ref(&self) -> &T {
        &self.value
    }
}

struct Parser<'a> {
    slice: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(slice: &'a [u8]) -> Self {
        Self { slice, offset: 0 }
    }

    fn slice(&self) -> &[u8] {
        &self.slice[self.offset..]
    }

    fn parse_u8(&mut self) -> Result<ParsedValue<u8>, ()> {
        let (v, _) = U256BEPtr::try_from_slice(self.slice())?;
        let v = v.validate_u8()?;
        let value = ParsedValue {
            value: v,
            range: self.offset..self.offset + 32,
        };
        self.offset += 32;

        Ok(value)
    }

    fn parse_u32(&mut self) -> Result<ParsedValue<u32>, ()> {
        let (v, _) = U256BEPtr::try_from_slice(self.slice())?;
        let v = v.validate_u32()?;
        let value = ParsedValue {
            value: v,
            range: self.offset..self.offset + 32,
        };
        self.offset += 32;

        Ok(value)
    }

    fn parse_u64(&mut self) -> Result<ParsedValue<u64>, ()> {
        let (v, _) = U256BEPtr::try_from_slice(self.slice())?;
        let v = v.validate_u64()?;
        let value = ParsedValue {
            value: v,
            range: self.offset..self.offset + 32,
        };
        self.offset += 32;

        Ok(value)
    }

    fn parse_address(&mut self) -> Result<ParsedValue<B160>, ()> {
        let (v, _) = U256BEPtr::try_from_slice(self.slice())?;
        let v = v.validate_address()?;
        let value = ParsedValue {
            value: v,
            range: self.offset..self.offset + 32,
        };
        self.offset += 32;

        Ok(value)
    }

    fn parse_u256(&mut self) -> Result<ParsedValue<U256>, ()> {
        let (v, _) = U256BEPtr::try_from_slice(self.slice())?;
        let v = v.read();
        let value = ParsedValue {
            value: v,
            range: self.offset..self.offset + 32,
        };
        self.offset += 32;

        Ok(value)
    }

    // we are only interested in range
    fn parse_bytes(&mut self) -> Result<ParsedValue<()>, ()> {
        let length = self.parse_u32()?;

        let length_words = length.read().div_ceil(U256::BYTES as u32);
        let padded_len = length_words.checked_mul(U256::BYTES as u32).ok_or(())?;

        if (self.slice().len() as u32) < padded_len {
            return Err(());
        }

        let start = self.offset;
        let end = self.offset.checked_add(padded_len as usize).ok_or(())?;

        // check that it's padded with zeroes
        if length.read() % (U256::BYTES as u32) != 0 {
            let zero_bytes = (U256::BYTES as u32) - (length.read() % (U256::BYTES as u32));
            #[allow(clippy::needless_range_loop)]
            for i in padded_len - zero_bytes..padded_len {
                if self.slice()[i as usize] != 0 {
                    return Err(());
                }
            }
        }

        self.offset = end;

        let value = ParsedValue {
            value: (),
            range: start..(start + length.value as usize),
        };

        Ok(value)
    }

    // we are only interested in range
    fn parse_bytes32_vector(&mut self) -> Result<ParsedValue<()>, ()> {
        let num_elements = self.parse_u32()?;
        let slice_len = num_elements
            .read()
            .checked_mul(U256::BYTES as u32)
            .ok_or(())?;

        if (self.slice().len() as u32) < slice_len {
            return Err(());
        }

        let start = self.offset;
        let end = self.offset.checked_add(slice_len as usize).ok_or(())?;

        self.offset = end;

        let value = ParsedValue {
            value: (),
            range: start..end,
        };

        Ok(value)
    }
}
