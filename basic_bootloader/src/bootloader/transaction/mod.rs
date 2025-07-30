//!
//! This module contains transaction structure definition including
//! a bunch of methods needed for the validation.
//!

use self::u256be_ptr::U256BEPtr;
use crate::bootloader::rlp;
#[cfg(feature = "pectra")]
use authorization_list::AuthorizationListItem;
use core::ops::Range;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use errors::{BootloaderSubsystemError, InvalidTransaction};
use reserved_dynamic_parser::ReservedDynamicParser;
use ruint::aliases::U256;
use zk_ee::internal_error;
use zk_ee::system::errors::{internal::InternalError, runtime::RuntimeError, system::SystemError};

mod abi_utils;
pub mod access_list_parser;
#[cfg(feature = "pectra")]
pub mod authorization_list;
pub mod reserved_dynamic_parser;
use self::access_list_parser::*;

#[cfg(test)]
mod tests;
pub mod u256be_ptr;

use super::{
    rlp::{apply_list_length_encoding_to_hash, estimate_length_encoding_len, ADDRESS_ENCODING_LEN},
    *,
};

///
/// The generic transaction format. The structure fields are slices/references in fact.
///
pub struct ZkSyncTransaction<'a> {
    underlying_buffer: &'a mut [u8],
    // field below are parsed
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
    pub max_fee_per_gas: ParsedValue<u128>,
    /// The maximum priority fee per gas that the user is willing to pay.
    /// It is akin to EIP1559's maxPriorityFeePerGas.
    pub max_priority_fee_per_gas: ParsedValue<u128>,
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
    pub reserved_dynamic: ReservedDynamicParser,
}

#[allow(dead_code)]
impl<'a> ZkSyncTransaction<'a> {
    /// The type id of legacy transactions.
    pub const LEGACY_TX_TYPE: u8 = 0x0;
    /// The type id of EIP2930 transactions.
    pub const EIP_2930_TX_TYPE: u8 = 0x01;
    /// The type id of EIP1559 transactions.
    pub const EIP_1559_TX_TYPE: u8 = 0x02;
    #[cfg(feature = "pectra")]
    /// The type id of EIP7702 transactions.
    pub const EIP_7702_TX_TYPE: u8 = 0x04;
    /// The type id of EIP712 transactions.
    pub const EIP_712_TX_TYPE: u8 = 0x71;
    /// The type id of protocol upgrade transactions.
    pub const UPGRADE_TX_TYPE: u8 = 0xFE;
    /// The type id of L1 -> L2 transactions.
    pub const L1_L2_TX_TYPE: u8 = 0xFF;

    /// Expected dynamic part(tail) offset in the transaction encoding.
    /// 16 fields, reserved takes 4 words in the static part(head) as static array.
    const DYNAMIC_PART_EXPECTED_OFFSET: usize = 19 * U256::BYTES;
    const ADDRESS_BIT_LENGTH: usize = 160;

    /// Data start position in the transaction encoding,
    /// needed to create a calldata memory region during the execution
    pub const DATA_START: usize = Self::DYNAMIC_PART_EXPECTED_OFFSET + U256::BYTES;

    ///
    /// Create structure from slice.
    ///
    /// Validates that all the fields are correctly and tightly packed.
    /// Also validate that all the fields set correctly, in accordance to its type.
    ///
    #[allow(clippy::result_unit_err)]
    pub fn try_from_slice(slice: &'a mut [u8]) -> Result<Self, ()> {
        if slice.len() <= TX_OFFSET {
            return Err(());
        }
        if slice.len() >= u32::MAX as usize {
            // degenerate case
            return Err(());
        }
        let mut parser = Parser::new(&*slice);
        // ignore entry part
        parser.offset = TX_OFFSET;

        let tx_type = parser.parse_u8()?;
        let from = parser.parse_address()?;
        let to = parser.parse_address()?;
        let gas_limit = parser.parse_u64()?;
        let gas_per_pubdata_limit = parser.parse_u32()?;
        let max_fee_per_gas = parser.parse_u128()?;
        let max_priority_fee_per_gas = parser.parse_u128()?;
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
        if data_offset.read() != (parser.offset - TX_OFFSET) as u32 {
            return Err(());
        }
        let data = parser.parse_bytes()?;

        if signature_offset.read() != (parser.offset - TX_OFFSET) as u32 {
            return Err(());
        }
        let signature = parser.parse_bytes()?;

        if factory_deps_offset.read() != (parser.offset - TX_OFFSET) as u32 {
            return Err(());
        }
        let factory_deps = parser.parse_bytes32_vector()?;

        if paymaster_input_offset.read() != (parser.offset - TX_OFFSET) as u32 {
            return Err(());
        }
        let paymaster_input = parser.parse_bytes()?;

        if reserved_dynamic_offset.read() != (parser.offset - TX_OFFSET) as u32 {
            return Err(());
        }

        let reserved_dynamic = ReservedDynamicParser::new(slice, parser.offset)?;
        // "Consume bytes"
        parser.parse_bytes()?;

        if parser.slice().is_empty() == false {
            return Err(());
        }

        let new = Self {
            underlying_buffer: slice,
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
    /// Validate that all the fields set correctly, in accordance to its type
    ///
    #[allow(clippy::result_unit_err)]
    fn validate_structure(&self) -> Result<(), ()> {
        let tx_type = self.tx_type.read();

        match tx_type {
            Self::LEGACY_TX_TYPE
            | Self::EIP_2930_TX_TYPE
            | Self::EIP_1559_TX_TYPE
            | Self::EIP_712_TX_TYPE
            | Self::UPGRADE_TX_TYPE
            | Self::L1_L2_TX_TYPE => {}
            #[cfg(feature = "pectra")]
            Self::EIP_7702_TX_TYPE => {}
            _ => return Err(()),
        }

        match tx_type {
            Self::LEGACY_TX_TYPE | Self::EIP_2930_TX_TYPE => {
                if self.max_fee_per_gas.read() != self.max_priority_fee_per_gas.read() {
                    return Err(());
                }
            }
            _ => {}
        }

        // paymasters can be used only with EIP712 txs
        match tx_type {
            Self::EIP_712_TX_TYPE => {}
            _ => {
                if self.paymaster.read() != B160::ZERO {
                    return Err(());
                }
            }
        }

        // reserved[0] is EIP-155 flag for legacy txs,
        // mint_value for l1 to l2 and upgrade txs,
        // for other types should be zero
        match tx_type {
            Self::LEGACY_TX_TYPE | Self::L1_L2_TX_TYPE | Self::UPGRADE_TX_TYPE => {}
            _ => {
                if !self.reserved[0].read().is_zero() {
                    return Err(());
                }
            }
        }
        // reserved[1] = refund recipient for l1 to l2 and upgrade txs,
        // for Ethereum(legacy, 1559, 2930) types it's a "to == null" flag(deployment tx),
        // for EIP712 txs should be zero
        match tx_type {
            Self::L1_L2_TX_TYPE | Self::UPGRADE_TX_TYPE => {
                // TODO: validate address?
            }
            Self::EIP_712_TX_TYPE => {
                if !self.reserved[1].read().is_zero() {
                    return Err(());
                }
            }
            _ => {
                if !self.reserved[1].read().is_zero() && self.to.read() != B160::ZERO {
                    return Err(());
                }
            }
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
            // TODO: with AA we should allow other signature length for EIP-712 txs
            _ => {
                if self.signature.range.len() != 65 {
                    return Err(());
                }
            }
        }

        // paymasters can be used only with EIP712 txs
        match tx_type {
            Self::EIP_712_TX_TYPE => {}
            _ => {
                if !self.paymaster_input.range.is_empty() {
                    return Err(());
                }
            }
        }

        // factory deps allowed only for eip712, or l1 to l2/upgrade txs
        // we ignore factory deps, as deployments performed via bytecode,
        // but we allowed them for backward compatibility with some Era VM tests
        match tx_type {
            Self::EIP_712_TX_TYPE | Self::L1_L2_TX_TYPE | Self::UPGRADE_TX_TYPE => {}
            _ => {
                if !self.factory_deps.range.is_empty() {
                    return Err(());
                }
            }
        }

        // Access list is empty except for txs that support it
        match tx_type {
            Self::EIP_1559_TX_TYPE | Self::EIP_2930_TX_TYPE => {}
            #[cfg(feature = "pectra")]
            Self::EIP_7702_TX_TYPE => {}
            _ => {
                if !self
                    .reserved_dynamic
                    .access_list_is_empty(&self.underlying_buffer)?
                {
                    return Err(());
                }
            }
        }

        // Authorization list is empty except for txs that support it
        // For EIP 7702, authorization list cannot be empty
        #[cfg(feature = "pectra")]
        match tx_type {
            Self::EIP_7702_TX_TYPE => {
                if self
                    .reserved_dynamic
                    .authorization_list_is_empty(&self.underlying_buffer)?
                {
                    return Err(());
                }
            }
            _ => {
                if !self
                    .reserved_dynamic
                    .authorization_list_is_empty(&self.underlying_buffer)?
                {
                    return Err(());
                }
            }
        }

        // Null destination is not allowed for some transactions
        #[allow(clippy::single_match)]
        match tx_type {
            #[cfg(feature = "pectra")]
            Self::EIP_7702_TX_TYPE => {
                if self.to.read() == B160::ZERO {
                    return Err(());
                }
            }
            _ => {}
        }

        Ok(())
    }

    // To be used only with field belonging to this transaction
    pub fn encoding<T: 'static + Clone + Copy + core::fmt::Debug>(
        &self,
        field: ParsedValue<T>,
    ) -> &[u8] {
        unsafe { self.underlying_buffer.get_unchecked(field.range) }
    }

    ///
    /// Uses the default constant for non EIP-712 transactions.
    ///
    pub fn get_user_gas_per_pubdata_limit(&self) -> U256 {
        if self.is_eip_712() {
            U256::from(self.gas_per_pubdata_limit.read())
        } else {
            crate::bootloader::constants::DEFAULT_GAS_PER_PUBDATA
        }
    }

    pub fn tx_body_length(&self) -> usize {
        self.underlying_buffer.len() - TX_OFFSET
    }

    pub fn underlying_buffer(&mut self) -> &mut [u8] {
        self.underlying_buffer
    }

    pub fn calldata(&self) -> &[u8] {
        unsafe {
            self.underlying_buffer
                .get_unchecked(self.data.range.clone())
        }
    }

    pub fn signature(&self) -> &[u8] {
        unsafe {
            self.underlying_buffer
                .get_unchecked(self.signature.range.clone())
        }
    }

    pub fn paymaster_input(&self) -> &[u8] {
        unsafe {
            self.underlying_buffer
                .get_unchecked(self.paymaster_input.range.clone())
        }
    }

    pub fn pre_tx_buffer(&mut self) -> &mut [u8] {
        unsafe { self.underlying_buffer.get_unchecked_mut(0..TX_OFFSET) }
    }
    ///
    /// Calculate the signed transaction hash.
    /// i.e. the one should be signed for the EOA accounts.
    ///
    pub fn calculate_signed_hash<R: Resources>(
        &self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let tx_type = self.tx_type.read();
        match tx_type {
            Self::LEGACY_TX_TYPE => self.legacy_tx_calculate_hash(chain_id, true, resources),
            Self::EIP_2930_TX_TYPE => self.eip2930_tx_calculate_hash(chain_id, true, resources),
            Self::EIP_1559_TX_TYPE => self.eip1559_tx_calculate_hash(chain_id, true, resources),
            #[cfg(feature = "pectra")]
            Self::EIP_7702_TX_TYPE => self.eip7702_tx_calculate_hash(chain_id, true, resources),
            Self::EIP_712_TX_TYPE => self.eip712_tx_calculate_signed_hash(chain_id, resources),
            _ => Err(
                internal_error!("Invalid type for signed hash, most likely l1 or upgrade").into(),
            ),
        }
    }

    ///
    /// Calculate the transaction hash.
    /// i.e. the transaction hash to be used in the explorer.
    ///
    pub fn calculate_hash<R: Resources>(
        &self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let tx_type = self.tx_type.read();
        match tx_type {
            Self::LEGACY_TX_TYPE => self.legacy_tx_calculate_hash(chain_id, false, resources),
            Self::EIP_2930_TX_TYPE => self.eip2930_tx_calculate_hash(chain_id, false, resources),
            Self::EIP_1559_TX_TYPE => self.eip1559_tx_calculate_hash(chain_id, false, resources),
            #[cfg(feature = "pectra")]
            Self::EIP_7702_TX_TYPE => self.eip7702_tx_calculate_hash(chain_id, false, resources),
            Self::EIP_712_TX_TYPE => self.eip712_tx_calculate_hash(chain_id, resources),
            Self::L1_L2_TX_TYPE => self.l1_tx_calculate_hash(resources),
            Self::UPGRADE_TX_TYPE => self.l1_tx_calculate_hash(resources),
            _ => Err(internal_error!("Type should be validated").into()),
        }
    }

    ///
    /// If signed == `false` calculate tx hash with signature(to be used in the explorer):
    /// Keccak256(RLP(nonce, gasPrice, gasLimit, to, value, data, r, s, v)),
    /// note that`v` is set to 35 + y + 2 * chainId for EIP-155 txs.
    ///
    /// If signed == `true` calculate signed tx hash(the one that should be signed by the sender):
    /// - RLP(nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0) for EIP-155 txs
    /// - RLP(nonce, gasPrice, gasLimit, to, value, data) for pre EIP-155 txs
    ///
    fn legacy_tx_calculate_hash<R: Resources>(
        &self,
        chain_id: u64,
        signed: bool,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let mut total_list_len =
            rlp::estimate_number_encoding_len(self.nonce.encoding(&self.underlying_buffer))
                + rlp::estimate_number_encoding_len(
                    self.max_fee_per_gas.encoding(&self.underlying_buffer),
                )
                + rlp::estimate_number_encoding_len(
                    self.gas_limit.encoding(&self.underlying_buffer),
                );

        // Handle `to` == null, indicates EVM deployment transaction
        if self.reserved[1].read().is_zero() {
            total_list_len += rlp::ADDRESS_ENCODING_LEN;
        } else {
            total_list_len += rlp::estimate_bytes_encoding_len(&[]);
        }

        total_list_len +=
            rlp::estimate_number_encoding_len(self.value.encoding(&self.underlying_buffer))
                + rlp::estimate_bytes_encoding_len(self.data.encoding(&self.underlying_buffer));

        // Encode `chainId` for signed hash according to EIP-155, but only if the `chainId` is specified in the transaction.
        if signed && !self.reserved[0].read().is_zero() {
            total_list_len += rlp::estimate_number_encoding_len(&chain_id.to_be_bytes());
            total_list_len += 2;
        }

        // Add signature if not signed hash
        if !signed {
            // r
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
            );
            // s
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
            );
            // v
            let mut v = self.signature.encoding(&self.underlying_buffer)[64] as u128;
            if !self.reserved[0].read().is_zero() {
                v += 8 + chain_id as u128 * 2;
            }
            total_list_len += rlp::estimate_number_encoding_len(&v.to_be_bytes());
        }

        let encoding_length = rlp::estimate_length_encoding_len(total_list_len) + total_list_len;
        charge_keccak(encoding_length, resources)?;

        let mut hasher = Keccak256::new();
        rlp::apply_list_length_encoding_to_hash(total_list_len, &mut hasher);
        rlp::apply_number_encoding_to_hash(
            self.nonce.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.max_fee_per_gas.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.gas_limit.encoding(&self.underlying_buffer),
            &mut hasher,
        );

        if self.reserved[1].read().is_zero() {
            rlp::apply_bytes_encoding_to_hash(
                &self.to.encoding(&self.underlying_buffer)[12..],
                &mut hasher,
            );
        } else {
            rlp::apply_bytes_encoding_to_hash(&[], &mut hasher);
        }

        rlp::apply_number_encoding_to_hash(
            self.value.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_bytes_encoding_to_hash(self.data.encoding(&self.underlying_buffer), &mut hasher);

        if signed && !self.reserved[0].read().is_zero() {
            rlp::apply_number_encoding_to_hash(&chain_id.to_be_bytes(), &mut hasher);
            rlp::apply_number_encoding_to_hash(&[], &mut hasher);
            rlp::apply_number_encoding_to_hash(&[], &mut hasher);
        }

        // Add signature if not signed hash
        if !signed {
            // r
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
                &mut hasher,
            );
            // s
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
                &mut hasher,
            );
            // v
            let mut v = self.signature.encoding(&self.underlying_buffer)[64] as u128;
            if !self.reserved[0].read().is_zero() {
                v += 8 + chain_id as u128 * 2;
            }
            rlp::apply_number_encoding_to_hash(&v.to_be_bytes(), &mut hasher);
        };
        Ok(hasher.finalize())
    }

    ///
    /// Estimates the length of the payload of the access list encoding
    ///
    fn estimate_access_list_raw_length(&self) -> Result<usize, ()> {
        let iter = self
            .reserved_dynamic
            .access_list_iter(&self.underlying_buffer)?;
        let mut sum = 0;
        for res in iter {
            let (_, keys) = res?;
            let (item_length, _, _) = estimate_access_list_item_length(keys.count);
            sum += item_length
        }
        Ok(sum)
    }

    ///
    /// Applies hash of the access list
    ///
    fn apply_access_list_encoding_to_hash(
        &self,
        total_access_list_length: usize,
        hasher: &mut Keccak256,
    ) -> Result<(), ()> {
        let iter = self
            .reserved_dynamic
            .access_list_iter(&self.underlying_buffer)?;
        // Length of access list
        apply_list_length_encoding_to_hash(total_access_list_length, hasher);
        for res in iter {
            let (address, keys) = res?;
            let (_, item_raw_length, keys_raw_length) =
                estimate_access_list_item_length(keys.count);
            // Length of [address, [keys]]
            apply_list_length_encoding_to_hash(item_raw_length, hasher);
            // Address
            rlp::apply_bytes_encoding_to_hash(&address.to_be_bytes::<{ B160::BYTES }>(), hasher);
            // Length of [keys]
            apply_list_length_encoding_to_hash(keys_raw_length, hasher);
            // Keys
            for key in keys {
                let key = key?;
                rlp::apply_bytes_encoding_to_hash(key.as_u8_ref(), hasher);
            }
        }
        Ok(())
    }

    ///
    /// Estimates the length of the payload of the access list encoding
    ///
    #[cfg(feature = "pectra")]
    fn estimate_authorization_list_raw_length(&self) -> Result<usize, ()> {
        let iter = self
            .reserved_dynamic
            .authorization_list_iter(&self.underlying_buffer)?;
        let mut sum = 0;
        for res in iter {
            let item = res?;
            let item_payload_length = estimate_authorization_list_item_length(&item);
            let item_length =
                rlp::estimate_length_encoding_len(item_payload_length) + item_payload_length;
            sum += item_length
        }
        Ok(sum)
    }

    ///
    /// Applies hash of the access list
    ///
    #[cfg(feature = "pectra")]
    fn apply_authorization_list_encoding_to_hash(
        &self,
        total_authorization_list_length: usize,
        hasher: &mut Keccak256,
    ) -> Result<(), ()> {
        let iter = self
            .reserved_dynamic
            .authorization_list_iter(&self.underlying_buffer)?;
        // Length of authorization list
        apply_list_length_encoding_to_hash(total_authorization_list_length, hasher);
        for res in iter {
            let item = res?;
            let item_raw_length = estimate_authorization_list_item_length(&item);
            apply_list_length_encoding_to_hash(item_raw_length, hasher);
            let AuthorizationListItem {
                chain_id,
                address,
                nonce,
                y_parity,
                r,
                s,
            } = item;
            rlp::apply_number_encoding_to_hash(&chain_id.to_be_bytes::<32>(), hasher);
            rlp::apply_bytes_encoding_to_hash(&address.to_be_bytes::<{ B160::BYTES }>(), hasher);
            rlp::apply_number_encoding_to_hash(&nonce.to_be_bytes(), hasher);
            rlp::apply_number_encoding_to_hash(&y_parity.to_be_bytes(), hasher);
            rlp::apply_number_encoding_to_hash(&r.to_be_bytes::<32>(), hasher);
            rlp::apply_number_encoding_to_hash(&s.to_be_bytes::<32>(), hasher);
        }
        Ok(())
    }

    ///
    /// Parse and validate access list, while warming up accounts and
    /// storage slots.
    ///
    pub fn parse_and_warm_up_access_list<S: EthereumLikeTypes>(
        &self,
        system: &mut System<S>,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>
    where
        S::IO: IOSubsystemExt,
    {
        let iter = self
            .reserved_dynamic
            .access_list_iter(&self.underlying_buffer)
            .map_err(|()| InvalidTransaction::InvalidStructure)?;
        for res in iter {
            let (address, keys) = res.map_err(|()| InvalidTransaction::InvalidStructure)?;
            system
                .io
                .touch_account(ExecutionEnvironmentType::NoEE, resources, &address, true)?;
            for key in keys {
                let key = key.map_err(|()| InvalidTransaction::InvalidStructure)?;
                system.io.storage_touch(
                    ExecutionEnvironmentType::NoEE,
                    resources,
                    &address,
                    &key,
                    true,
                )?;
            }
        }
        Ok(())
    }

    ///
    /// If signed == `false` calculate tx hash with signature(to be used in the explorer):
    /// Keccak256(0x01 || RLP(chain_id, nonce, gas_price, gas_limit, destination, amount, data, access_list, r, s, v))
    ///
    /// If signed == `true` calculate signed tx hash(the one that should be signed by the sender):
    /// Keccak256(0x01 || RLP(chain_id, nonce, gas_price, gas_limit, destination, amount, data, access_list))
    ///
    pub fn eip2930_tx_calculate_hash<R: Resources>(
        &self,
        chain_id: u64,
        signed: bool,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let mut total_list_len = rlp::estimate_number_encoding_len(&chain_id.to_be_bytes())
            + rlp::estimate_number_encoding_len(self.nonce.encoding(&self.underlying_buffer))
            + rlp::estimate_number_encoding_len(
                self.max_fee_per_gas.encoding(&self.underlying_buffer),
            )
            + rlp::estimate_number_encoding_len(self.gas_limit.encoding(&self.underlying_buffer));

        // Handle `to` == null, indicates EVM deployment transaction
        if self.reserved[1].read().is_zero() {
            total_list_len += rlp::ADDRESS_ENCODING_LEN;
        } else {
            total_list_len += rlp::estimate_bytes_encoding_len(&[]);
        }

        let access_list_raw_length = self
            .estimate_access_list_raw_length()
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        total_list_len +=
            rlp::estimate_number_encoding_len(self.value.encoding(&self.underlying_buffer))
                + rlp::estimate_bytes_encoding_len(self.data.encoding(&self.underlying_buffer))
                + rlp::estimate_length_encoding_len(access_list_raw_length)
                + access_list_raw_length;

        // Add signature if not signed hash
        if !signed {
            // r
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
            );
            // s
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
            );
            // v
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[64..65],
            );
        }

        let encoding_length = rlp::estimate_length_encoding_len(total_list_len) + total_list_len;
        charge_keccak(encoding_length, resources)?;

        let mut hasher = Keccak256::new();
        hasher.update([0x01]);
        rlp::apply_list_length_encoding_to_hash(total_list_len, &mut hasher);
        rlp::apply_number_encoding_to_hash(&chain_id.to_be_bytes(), &mut hasher);
        rlp::apply_number_encoding_to_hash(
            self.nonce.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.max_fee_per_gas.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.gas_limit.encoding(&self.underlying_buffer),
            &mut hasher,
        );

        if self.reserved[1].read().is_zero() {
            rlp::apply_bytes_encoding_to_hash(
                &self.to.encoding(&self.underlying_buffer)[12..],
                &mut hasher,
            );
        } else {
            rlp::apply_bytes_encoding_to_hash(&[], &mut hasher);
        }

        rlp::apply_number_encoding_to_hash(
            self.value.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_bytes_encoding_to_hash(self.data.encoding(&self.underlying_buffer), &mut hasher);
        self.apply_access_list_encoding_to_hash(access_list_raw_length, &mut hasher)
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        // Add signature if not signed hash
        if !signed {
            // r
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
                &mut hasher,
            );
            // s
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
                &mut hasher,
            );
            // v
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[64..65],
                &mut hasher,
            );
        }

        Ok(hasher.finalize())
    }

    ///
    /// If signed == `false` calculate tx hash with signature(to be used in the explorer):
    /// Keccak256(0x02 || RLP(chain_id, nonce, max_priority_fee_per_gas, max_fee_per_gas, gas_limit, destination, amount, data, access_list, r, s, v))
    ///
    /// If signed == `true` calculate signed tx hash(the one that should be signed by the sender):
    /// Keccak256(0x02 || RLP(chain_id, nonce, max_priority_fee_per_gas, max_fee_per_gas, gas_limit, destination, amount, data, access_list))
    ///
    pub fn eip1559_tx_calculate_hash<R: Resources>(
        &self,
        chain_id: u64,
        signed: bool,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let mut total_list_len = rlp::estimate_number_encoding_len(&chain_id.to_be_bytes())
            + rlp::estimate_number_encoding_len(self.nonce.encoding(&self.underlying_buffer))
            + rlp::estimate_number_encoding_len(
                self.max_priority_fee_per_gas
                    .encoding(&self.underlying_buffer),
            )
            + rlp::estimate_number_encoding_len(
                self.max_fee_per_gas.encoding(&self.underlying_buffer),
            )
            + rlp::estimate_number_encoding_len(self.gas_limit.encoding(&self.underlying_buffer));

        // Handle `to` == null, indicates EVM deployment transaction
        if self.reserved[1].read().is_zero() {
            total_list_len += rlp::ADDRESS_ENCODING_LEN;
        } else {
            total_list_len += rlp::estimate_bytes_encoding_len(&[]);
        }

        let access_list_raw_length = self
            .estimate_access_list_raw_length()
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        total_list_len +=
            rlp::estimate_number_encoding_len(self.value.encoding(&self.underlying_buffer))
                + rlp::estimate_bytes_encoding_len(self.data.encoding(&self.underlying_buffer))
                + rlp::estimate_length_encoding_len(access_list_raw_length)
                + access_list_raw_length;

        // Add signature if not signed hash
        if !signed {
            // r
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
            );
            // s
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
            );
            // v
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[64..65],
            );
        }

        let encoding_length = rlp::estimate_length_encoding_len(total_list_len) + total_list_len;
        charge_keccak(encoding_length, resources)?;

        let mut hasher = Keccak256::new();
        hasher.update([0x02]);
        rlp::apply_list_length_encoding_to_hash(total_list_len, &mut hasher);
        rlp::apply_number_encoding_to_hash(&chain_id.to_be_bytes(), &mut hasher);
        rlp::apply_number_encoding_to_hash(
            self.nonce.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.max_priority_fee_per_gas
                .encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.max_fee_per_gas.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.gas_limit.encoding(&self.underlying_buffer),
            &mut hasher,
        );

        if self.reserved[1].read().is_zero() {
            rlp::apply_bytes_encoding_to_hash(
                &self.to.encoding(&self.underlying_buffer)[12..],
                &mut hasher,
            );
        } else {
            rlp::apply_bytes_encoding_to_hash(&[], &mut hasher);
        }

        rlp::apply_number_encoding_to_hash(
            self.value.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_bytes_encoding_to_hash(self.data.encoding(&self.underlying_buffer), &mut hasher);
        self.apply_access_list_encoding_to_hash(access_list_raw_length, &mut hasher)
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;
        // Add signature if not signed hash
        if !signed {
            // r
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
                &mut hasher,
            );
            // s
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
                &mut hasher,
            );
            // v
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[64..65],
                &mut hasher,
            );
        }

        Ok(hasher.finalize())
    }

    ///
    /// If signed == `false` calculate tx hash with signature(to be used in the explorer):
    /// Keccak256(0x04 || RLP(chain_id, nonce, max_priority_fee_per_gas, max_fee_per_gas, gas_limit, destination, amount, data, access_list, authorization_list, r, s, v))
    ///
    /// If signed == `true` calculate signed tx hash(the one that should be signed by the sender):
    /// Keccak256(0x04 || RLP(chain_id, nonce, max_priority_fee_per_gas, max_fee_per_gas, gas_limit, destination, amount, data, access_list, access_list))
    ///
    #[cfg(feature = "pectra")]
    pub fn eip7702_tx_calculate_hash<R: Resources>(
        &self,
        chain_id: u64,
        signed: bool,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let mut total_list_len = rlp::estimate_number_encoding_len(&chain_id.to_be_bytes())
            + rlp::estimate_number_encoding_len(self.nonce.encoding(&self.underlying_buffer))
            + rlp::estimate_number_encoding_len(
                self.max_priority_fee_per_gas
                    .encoding(&self.underlying_buffer),
            )
            + rlp::estimate_number_encoding_len(
                self.max_fee_per_gas.encoding(&self.underlying_buffer),
            )
            + rlp::estimate_number_encoding_len(self.gas_limit.encoding(&self.underlying_buffer));

        total_list_len += rlp::ADDRESS_ENCODING_LEN;

        let access_list_raw_length = self
            .estimate_access_list_raw_length()
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        let authorization_list_raw_length = self
            .estimate_authorization_list_raw_length()
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        total_list_len +=
            rlp::estimate_number_encoding_len(self.value.encoding(&self.underlying_buffer))
                + rlp::estimate_bytes_encoding_len(self.data.encoding(&self.underlying_buffer))
                + rlp::estimate_length_encoding_len(access_list_raw_length)
                + access_list_raw_length
                + rlp::estimate_length_encoding_len(authorization_list_raw_length)
                + authorization_list_raw_length;

        // Add signature if not signed hash
        if !signed {
            // r
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
            );
            // s
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
            );
            // v
            total_list_len += rlp::estimate_number_encoding_len(
                &self.signature.encoding(&self.underlying_buffer)[64..65],
            );
        }

        let encoding_length = rlp::estimate_length_encoding_len(total_list_len) + total_list_len;
        charge_keccak(encoding_length, resources)?;

        let mut hasher = Keccak256::new();
        hasher.update([0x04]);
        rlp::apply_list_length_encoding_to_hash(total_list_len, &mut hasher);
        rlp::apply_number_encoding_to_hash(&chain_id.to_be_bytes(), &mut hasher);
        rlp::apply_number_encoding_to_hash(
            self.nonce.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.max_priority_fee_per_gas
                .encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.max_fee_per_gas.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_number_encoding_to_hash(
            self.gas_limit.encoding(&self.underlying_buffer),
            &mut hasher,
        );

        if self.reserved[1].read().is_zero() {
            rlp::apply_bytes_encoding_to_hash(
                &self.to.encoding(&self.underlying_buffer)[12..],
                &mut hasher,
            );
        } else {
            rlp::apply_bytes_encoding_to_hash(&[], &mut hasher);
        }

        rlp::apply_number_encoding_to_hash(
            self.value.encoding(&self.underlying_buffer),
            &mut hasher,
        );
        rlp::apply_bytes_encoding_to_hash(self.data.encoding(&self.underlying_buffer), &mut hasher);
        self.apply_access_list_encoding_to_hash(access_list_raw_length, &mut hasher)
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;
        self.apply_authorization_list_encoding_to_hash(authorization_list_raw_length, &mut hasher)
            .map_err(|()| TxError::Validation(InvalidTransaction::InvalidEncoding))?;
        // Add signature if not signed hash
        if !signed {
            // r
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[0..32],
                &mut hasher,
            );
            // s
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[32..64],
                &mut hasher,
            );
            // v
            rlp::apply_number_encoding_to_hash(
                &self.signature.encoding(&self.underlying_buffer)[64..65],
                &mut hasher,
            );
        }
        Ok(hasher.finalize())
    }

    // Keccak256 of:
    // EIP712Domain(string name,string version,uint256 chainId)
    // = c2f8787176b8ac6bf7215b4adcc1e069bf4ab82d9ab1df05a57a91d425935b6e
    const DOMAIN_TYPE_HASH: [u8; 32] = [
        0xc2, 0xf8, 0x78, 0x71, 0x76, 0xb8, 0xac, 0x6b, 0xf7, 0x21, 0x5b, 0x4a, 0xdc, 0xc1, 0xe0,
        0x69, 0xbf, 0x4a, 0xb8, 0x2d, 0x9a, 0xb1, 0xdf, 0x05, 0xa5, 0x7a, 0x91, 0xd4, 0x25, 0x93,
        0x5b, 0x6e,
    ];

    // Keccak256 of:
    // zkSync
    // = 19b453ce45aaaaf3a300f5a9ec95869b4f28ab10430b572ee218c3a6a5e07d6f
    const DOMAIN_NAME_HASH: [u8; 32] = [
        0x19, 0xb4, 0x53, 0xce, 0x45, 0xaa, 0xaa, 0xf3, 0xa3, 0x00, 0xf5, 0xa9, 0xec, 0x95, 0x86,
        0x9b, 0x4f, 0x28, 0xab, 0x10, 0x43, 0x0b, 0x57, 0x2e, 0xe2, 0x18, 0xc3, 0xa6, 0xa5, 0xe0,
        0x7d, 0x6f,
    ];
    // Keccak256 of:
    // 2
    // = ad7c5bef027816a800da1736444fb58a807ef4c9603b7848673f7e3a68eb14a5
    const DOMAIN_VERSION_HASH: [u8; 32] = [
        0xad, 0x7c, 0x5b, 0xef, 0x02, 0x78, 0x16, 0xa8, 0x00, 0xda, 0x17, 0x36, 0x44, 0x4f, 0xb5,
        0x8a, 0x80, 0x7e, 0xf4, 0xc9, 0x60, 0x3b, 0x78, 0x48, 0x67, 0x3f, 0x7e, 0x3a, 0x68, 0xeb,
        0x14, 0xa5,
    ];

    fn domain_hash_struct<R: Resources>(
        chain_id: u64,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let len = Self::DOMAIN_TYPE_HASH.len()
            + Self::DOMAIN_NAME_HASH.len()
            + Self::DOMAIN_VERSION_HASH.len()
            + U256::BYTES;
        charge_keccak(len, resources)?;

        let mut hasher = Keccak256::new();
        hasher.update(Self::DOMAIN_TYPE_HASH);
        hasher.update(Self::DOMAIN_NAME_HASH);
        hasher.update(Self::DOMAIN_VERSION_HASH);
        hasher.update(U256::from(chain_id).to_be_bytes::<32>());
        Ok(*hasher.finalize().split_first_chunk::<32>().unwrap().0)
    }

    // Keccak256 of:
    // Transaction(uint256 txType,uint256 from,uint256 to,uint256 gasLimit,uint256 gasPerPubdataByteLimit,uint256 maxFeePerGas,uint256 maxPriorityFeePerGas,uint256 paymaster,uint256 nonce,uint256 value,bytes data,bytes32[] factoryDeps,bytes paymasterInput)
    // = 848e1bfa1ac4e3576b728bda6721b215c70a7799a5b4866282a71bab954baac8
    const TYPE_HASH: [u8; 32] = [
        0x84, 0x8e, 0x1b, 0xfa, 0x1a, 0xc4, 0xe3, 0x57, 0x6b, 0x72, 0x8b, 0xda, 0x67, 0x21, 0xb2,
        0x15, 0xc7, 0x0a, 0x77, 0x99, 0xa5, 0xb4, 0x86, 0x62, 0x82, 0xa7, 0x1b, 0xab, 0x95, 0x4b,
        0xaa, 0xc8,
    ];

    fn hash_struct<R: Resources>(&self, resources: &mut R) -> Result<[u8; 32], TxError> {
        let len = U256::BYTES * 14;
        charge_keccak(len, resources)?;

        let mut hasher = Keccak256::new();
        hasher.update(Self::TYPE_HASH);
        hasher.update(self.tx_type.encoding(&self.underlying_buffer));
        hasher.update(self.from.encoding(&self.underlying_buffer));
        hasher.update(self.to.encoding(&self.underlying_buffer));
        hasher.update(self.gas_limit.encoding(&self.underlying_buffer));
        hasher.update(self.gas_per_pubdata_limit.encoding(&self.underlying_buffer));
        hasher.update(self.max_fee_per_gas.encoding(&self.underlying_buffer));
        hasher.update(
            self.max_priority_fee_per_gas
                .encoding(&self.underlying_buffer),
        );
        hasher.update(self.paymaster.encoding(&self.underlying_buffer));
        hasher.update(self.nonce.encoding(&self.underlying_buffer));
        hasher.update(self.value.encoding(&self.underlying_buffer));

        charge_keccak(self.data.range.len(), resources)?;
        let data_hash =
            <Keccak256 as MiniDigest>::digest(self.data.encoding(&self.underlying_buffer));
        hasher.update(&data_hash);

        charge_keccak(self.factory_deps.range.len(), resources)?;
        let factory_deps_hash =
            <Keccak256 as MiniDigest>::digest(self.factory_deps.encoding(&self.underlying_buffer));
        hasher.update(&factory_deps_hash);

        charge_keccak(self.paymaster_input.range.len(), resources)?;
        let paymaster_input_hash = <Keccak256 as MiniDigest>::digest(
            self.paymaster_input.encoding(&self.underlying_buffer),
        );
        hasher.update(&paymaster_input_hash);

        Ok(hasher.finalize())
    }

    ///
    /// Calculate signed tx hash(the one that should be signed by the sender):
    /// Keccak256(0x19 0x01 ‖ domainSeparator ‖ hashStruct(tx))
    ///
    fn eip712_tx_calculate_signed_hash<R: Resources>(
        &self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let domain_separator = Self::domain_hash_struct(chain_id, resources)?;
        let hs = self.hash_struct(resources)?;
        charge_keccak(2 + 2 * U256::BYTES, resources)?;
        let mut hasher = Keccak256::new();
        hasher.update([0x19, 0x01]);
        hasher.update(domain_separator);
        hasher.update(hs);

        Ok(hasher.finalize())
    }

    ///
    /// Calculate tx hash with signature(to be used in the explorer):
    /// Keccak256(signed_hash || Keccak256(signature))
    ///
    fn eip712_tx_calculate_hash<R: Resources>(
        &self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<[u8; 32], TxError> {
        let signed_hash = self.eip712_tx_calculate_signed_hash(chain_id, resources)?;
        charge_keccak(U256::BYTES * 2 + self.signature.range.len(), resources)?;
        let signature_hash =
            <Keccak256 as MiniDigest>::digest(self.signature.encoding(&self.underlying_buffer));

        let mut hasher = Keccak256::new();
        hasher.update(signed_hash);
        hasher.update(signature_hash);

        Ok(hasher.finalize())
    }

    ///
    /// Calculate l1 tx hash:
    /// Keccak256(abi.encode(transaction))
    ///
    fn l1_tx_calculate_hash<R: Resources>(&self, resources: &mut R) -> Result<[u8; 32], TxError> {
        charge_keccak(32 + self.underlying_buffer[TX_OFFSET..].len(), resources)?;
        let mut hasher = Keccak256::new();
        // Note, that the correct ABI encoding of the Transaction structure starts with 0x20
        hasher.update(&U256::from(0x20).to_be_bytes::<32>());
        hasher.update(&self.underlying_buffer[TX_OFFSET..]);
        Ok(hasher.finalize())
    }

    /// Checks if the transaction is of type EIP-712
    pub fn is_eip_712(&self) -> bool {
        self.tx_type.read() == Self::EIP_712_TX_TYPE
    }

    /// Returns the balance required to process the transaction.
    pub fn required_balance(&self) -> Result<U256, InternalError> {
        if self.is_eip_712() && self.paymaster.read() != B160::ZERO {
            Ok(self.value.read())
        } else {
            let fee_amount = self
                .max_fee_per_gas
                .read()
                .checked_mul(self.gas_limit.read() as u128)
                .ok_or(internal_error!("mfpg*gl"))?;
            self.value
                .read()
                .checked_add(U256::from(fee_amount))
                .ok_or(internal_error!("fa+v"))
        }
    }

    ///
    /// Parse length of authorization list.
    ///
    #[cfg(feature = "pectra")]
    pub fn parse_authorization_list_length(&self) -> Result<u64, TxError> {
        let iter = self
            .reserved_dynamic
            .authorization_list_iter(&self.underlying_buffer)
            .map_err(|()| InvalidTransaction::InvalidStructure)?;
        Ok(iter.count as u64)
    }

    ///
    /// Parse and validate authorization list, while applying delegations.
    ///
    #[cfg(feature = "pectra")]
    pub fn parse_authorization_list_and_apply_delegations<S: EthereumLikeTypes>(
        &self,
        system: &mut System<S>,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>
    where
        S::IO: IOSubsystemExt,
    {
        let iter = self
            .reserved_dynamic
            .authorization_list_iter(&self.underlying_buffer)
            .map_err(|()| InvalidTransaction::InvalidStructure)?;
        for res in iter {
            let authorization = res.map_err(|()| InvalidTransaction::InvalidStructure)?;
            let success = authorization.validate_and_apply_delegation(system, resources)?;
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Delegation success: {success}\n"));

            if !success {}
        }
        Ok(())
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

    fn encoding<'a>(&self, source: &'a [u8]) -> &'a [u8] {
        unsafe { source.get_unchecked(self.range.clone()) }
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

    fn parse_u128(&mut self) -> Result<ParsedValue<u128>, ()> {
        let (v, _) = U256BEPtr::try_from_slice(self.slice())?;
        let v = v.validate_u128()?;
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

fn charge_keccak<R: Resources>(len: usize, resources: &mut R) -> Result<(), TxError> {
    let native_cost = basic_system::system_functions::keccak256::keccak256_native_cost::<R>(len);
    resources
        .charge(&R::from_native(native_cost))
        .map_err(|e| match e {
            SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                internal_error!("Charging for keccak is not supposed to consume ergs").into()
            }
            SystemError::LeafDefect(e) => BootloaderSubsystemError::LeafDefect(e),
            SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc)) => {
                BootloaderSubsystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc))
            }
        })
        .map_err(TxError::oon_as_validation)
}

/// Returns (full_item_length, item_raw_length, keys_raw_length)
fn estimate_access_list_item_length(nb_keys: usize) -> (usize, usize, usize) {
    // 32 bytes for key + 1 byte for tag and length.
    let single_key_length = 33;
    let keys_raw_length = single_key_length * nb_keys;
    let keys_length = estimate_length_encoding_len(keys_raw_length) + keys_raw_length;
    let address_length = ADDRESS_ENCODING_LEN;
    let item_raw_length = keys_length + address_length;
    (
        estimate_length_encoding_len(item_raw_length) + item_raw_length,
        item_raw_length,
        keys_raw_length,
    )
}

#[cfg(feature = "pectra")]
fn estimate_authorization_list_item_length(item: &AuthorizationListItem) -> usize {
    let AuthorizationListItem {
        chain_id,
        address: _,
        nonce,
        y_parity,
        r,
        s,
    } = item;
    rlp::estimate_number_encoding_len(&chain_id.to_be_bytes::<32>())
        + ADDRESS_ENCODING_LEN
        + rlp::estimate_number_encoding_len(&nonce.to_be_bytes())
        + rlp::estimate_number_encoding_len(&y_parity.to_be_bytes())
        + rlp::estimate_number_encoding_len(&r.to_be_bytes::<32>())
        + rlp::estimate_number_encoding_len(&s.to_be_bytes::<32>())
}

///
/// Parse a delegation of the format: 0xef0100 || address
///
pub fn parse_delegation(delegation: &[u8]) -> Result<B160, InternalError> {
    if delegation.len() != EIP7702_DELEGATION_MARKER.len() + B160::BYTES {
        return Err(internal_error!("7702 delegation of incorrect length"));
    }
    if delegation[0..3] != EIP7702_DELEGATION_MARKER {
        return Err(internal_error!("7702 delegation has invalid prefix"));
    }
    let Some(address) = B160::try_from_be_slice(&delegation[3..]) else {
        return Err(internal_error!("7702 delegation has invalid address"));
    };
    Ok(address)
}
