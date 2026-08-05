use crate::bootloader::errors::TxError;
use crate::bootloader::transaction::charge_keccak;
use crate::bootloader::transaction::rlp_encoded::transaction_types::eip_2930_tx::{
    AccessList, AccessListForAddress,
};
use crate::bootloader::transaction::rlp_encoded::transaction_types::eip_7702_tx::{
    AuthorizationEntry, AuthorizationList,
};
use core::alloc::Allocator;

use super::*;
use ruint::aliases::{B160, U256};
use transaction_types::eip_4844_tx::BlobHashesList;
use zk_ee::system::MAX_TX_GAS_LIMIT;
use zk_ee::system::{Resources, GAS_PER_BLOB};
use zk_ee::utils::UsizeAlignedByteBox;

// NOTE: this is self-reference, but relatively easy one. Do NOT derive clone one it,
// as it's unsound
pub struct RlpEncodedTransaction<A: Allocator> {
    buffer: UsizeAlignedByteBox<A>,
    inner: RlpEncodedTxInner<'static>,
    chain_id: u64,
    sig_hash: Bytes32,
    // Lazy field, computed only when calling transaction_hash() for the first
    // time.
    tx_hash: Option<Bytes32>,
    // Note: this field is not the recovered signer, but rather an address
    // passed by oracle. Needs to be checked to be equal to recovered address.
    from: B160,
}

impl<A: Allocator> core::fmt::Debug for RlpEncodedTransaction<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RlpEncodedTransaction")
            .field("buffer", &self.buffer.as_slice())
            .field("inner", &self.inner)
            .field("chain_id", &self.chain_id)
            .field("sig_hash", &self.sig_hash)
            .field("tx_hash", &self.tx_hash)
            .field("from", &self.from)
            .finish()
    }
}

impl<A: Allocator> RlpEncodedTransaction<A> {
    pub fn tx_encoding(&self) -> &[u8] {
        self.buffer.as_slice()
    }

    pub fn parse_from_buffer(
        buffer: UsizeAlignedByteBox<A>,
        expected_chain_id: u64,
        from: B160,
    ) -> Result<Self, TxError> {
        // ideally we want partial initialization to be available here, but let's do without. Note that
        // we are free to move this structure as UsizeAlignedByteBox has a box inside and guarantees stable
        // address of the slice that we will use to parse a transaction, so we will not make a long code with
        // partial init and drop guards, but instead will parse via 'static transmute

        let (inner, sig_hash): (RlpEncodedTxInner<'static>, Bytes32) =
            RlpEncodedTxInner::parse_and_compute_signed_hash(
                unsafe { core::mem::transmute::<&[u8], &[u8]>(buffer.as_slice()) },
                expected_chain_id,
                &from,
            )?;

        Ok(Self {
            buffer,
            inner,
            chain_id: expected_chain_id,
            sig_hash,
            tx_hash: None,
            from,
        })
    }

    pub fn is_service(&self) -> bool {
        matches!(&self.inner, RlpEncodedTxInner::Service(_))
    }

    pub fn chain_id(&self) -> Option<u64> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) => None,
            _ => Some(self.chain_id),
        }
    }

    pub fn nonce(&self) -> Option<u64> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(tx, _) | RlpEncodedTxInner::LegacyWithEIP155(tx, _) => {
                Some(tx.nonce)
            }
            RlpEncodedTxInner::EIP2930(tx, _) => Some(tx.nonce),
            RlpEncodedTxInner::EIP1559(tx, _) => Some(tx.nonce),
            RlpEncodedTxInner::EIP4844(tx, _) => Some(tx.nonce),
            RlpEncodedTxInner::EIP7702(tx, _) => Some(tx.nonce),
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    pub fn value(&self) -> &U256 {
        match &self.inner {
            RlpEncodedTxInner::Legacy(tx, _) | RlpEncodedTxInner::LegacyWithEIP155(tx, _) => {
                &tx.value
            }
            RlpEncodedTxInner::EIP2930(tx, _) => &tx.value,
            RlpEncodedTxInner::EIP1559(tx, _) => &tx.value,
            RlpEncodedTxInner::EIP4844(tx, _) => &tx.value,
            RlpEncodedTxInner::EIP7702(tx, _) => &tx.value,
            RlpEncodedTxInner::Service(_) => &U256::ZERO,
        }
    }

    pub fn hash_for_signature_verification(&self) -> &Bytes32 {
        &self.sig_hash
    }

    pub fn transaction_hash<R: Resources>(
        &mut self,
        resources: &mut R,
    ) -> Result<Bytes32, TxError> {
        if self.tx_hash.is_none() {
            charge_keccak(self.buffer.len(), resources)?;
            let mut hasher = crypto::sha3::Keccak256::new();
            hasher.update(self.buffer.as_slice());
            let tx_hash = Bytes32::from_array(hasher.finalize());
            self.tx_hash = Some(tx_hash);
        }

        // Safe to unwrap now
        Ok(self.tx_hash.unwrap())
    }

    pub fn tx_type(&self) -> u8 {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) | RlpEncodedTxInner::LegacyWithEIP155(_, _) => {
                LegacyTXInner::TX_TYPE
            }
            RlpEncodedTxInner::EIP2930(_, _) => EIP2930Tx::TX_TYPE,
            RlpEncodedTxInner::EIP1559(_, _) => EIP1559Tx::TX_TYPE,
            RlpEncodedTxInner::EIP4844(_, _) => EIP4844Tx::TX_TYPE,
            RlpEncodedTxInner::EIP7702(_, _) => EIP7702Tx::TX_TYPE,
            RlpEncodedTxInner::Service(_) => ServiceTx::TX_TYPE,
        }
    }

    pub fn calldata<'a>(&'a self) -> &'a [u8] {
        match &self.inner {
            RlpEncodedTxInner::Legacy(tx, _) | RlpEncodedTxInner::LegacyWithEIP155(tx, _) => {
                tx.data
            }
            RlpEncodedTxInner::EIP2930(tx, _) => tx.data,
            RlpEncodedTxInner::EIP1559(tx, _) => tx.data,
            RlpEncodedTxInner::EIP4844(tx, _) => tx.data,
            RlpEncodedTxInner::EIP7702(tx, _) => tx.data,
            RlpEncodedTxInner::Service(tx) => tx.data,
        }
    }

    pub fn access_list<'a>(&'a self) -> Option<AccessList<'a>> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) | RlpEncodedTxInner::LegacyWithEIP155(_, _) => None,
            RlpEncodedTxInner::EIP2930(tx, _) => Some(tx.access_list),
            RlpEncodedTxInner::EIP1559(tx, _) => Some(tx.access_list),
            RlpEncodedTxInner::EIP4844(tx, _) => Some(tx.access_list),
            RlpEncodedTxInner::EIP7702(tx, _) => Some(tx.access_list),
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    pub fn access_list_iter<'a>(
        &'a self,
    ) -> Option<impl Iterator<Item = AccessListForAddress<'a>> + Clone> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) | RlpEncodedTxInner::LegacyWithEIP155(_, _) => None,
            RlpEncodedTxInner::EIP2930(tx, _) => Some(tx.access_list.iter()),
            RlpEncodedTxInner::EIP1559(tx, _) => Some(tx.access_list.iter()),
            RlpEncodedTxInner::EIP4844(tx, _) => Some(tx.access_list.iter()),
            RlpEncodedTxInner::EIP7702(tx, _) => Some(tx.access_list.iter()),
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    pub fn authorization_list<'a>(&'a self) -> Option<AuthorizationList<'a>> {
        match &self.inner {
            RlpEncodedTxInner::EIP7702(tx, _) => Some(tx.authorization_list),
            _ => None,
        }
    }

    pub fn authorization_list_iter<'a>(
        &'a self,
    ) -> Option<impl Iterator<Item = AuthorizationEntry<'a>> + Clone> {
        match &self.inner {
            RlpEncodedTxInner::EIP7702(tx, _) => Some(tx.authorization_list.iter()),
            _ => None,
        }
    }

    pub fn from(&self) -> &B160 {
        &self.from
    }

    pub fn sig_parity_r_s<'a>(&'a self) -> Option<(bool, &'a [u8], &'a [u8])> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, sig) => {
                let parity = sig.v - U256::from(27) == U256::ONE;
                Some((parity, sig.r, sig.s)) // prechecked
            }
            RlpEncodedTxInner::LegacyWithEIP155(_, sig) => {
                let chain_id = self.chain_id;
                let parity = sig.v - U256::from(35) - (U256::from(chain_id) * U256::from(2)); // no underflows
                Some((parity == 1, sig.r, sig.s))
            }
            RlpEncodedTxInner::EIP2930(_, sig) => Some((sig.y_parity, sig.r, sig.s)),
            RlpEncodedTxInner::EIP1559(_, sig) => Some((sig.y_parity, sig.r, sig.s)),
            RlpEncodedTxInner::EIP4844(_, sig) => Some((sig.y_parity, sig.r, sig.s)),
            RlpEncodedTxInner::EIP7702(_, sig) => Some((sig.y_parity, sig.r, sig.s)),
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    pub fn required_balance(&self) -> Option<U256> {
        match &self.inner {
            RlpEncodedTxInner::EIP4844(tx, _) => {
                let gas_fee = self
                    .max_fee_per_gas()
                    .checked_mul(U256::from(self.gas_limit()))?;
                let blob_gas = GAS_PER_BLOB.checked_mul(tx.blob_versioned_hashes.count as u64)?;
                let blob_fee = tx.max_fee_per_blob_gas.checked_mul(U256::from(blob_gas))?;
                self.value().checked_add(blob_fee)?.checked_add(gas_fee)
            }
            _ => {
                let fee_amount = self
                    .max_fee_per_gas()
                    .checked_mul(U256::from(self.gas_limit()))?;
                self.value().checked_add(U256::from(fee_amount))
            }
        }
    }

    pub fn gas_limit(&self) -> u64 {
        match &self.inner {
            RlpEncodedTxInner::Legacy(tx, _) | RlpEncodedTxInner::LegacyWithEIP155(tx, _) => {
                tx.gas_limit
            }
            RlpEncodedTxInner::EIP2930(tx, _) => tx.gas_limit,
            RlpEncodedTxInner::EIP1559(tx, _) => tx.gas_limit,
            RlpEncodedTxInner::EIP4844(tx, _) => tx.gas_limit,
            RlpEncodedTxInner::EIP7702(tx, _) => tx.gas_limit,
            RlpEncodedTxInner::Service(_) => MAX_TX_GAS_LIMIT,
        }
    }

    pub fn destination(&self) -> Option<B160> {
        let map_fn = |src: &[u8]| {
            if src.is_empty() {
                None
            } else {
                B160::try_from_be_slice(src)
            }
        };
        match &self.inner {
            RlpEncodedTxInner::Legacy(tx, _) | RlpEncodedTxInner::LegacyWithEIP155(tx, _) => {
                map_fn(tx.to)
            }
            RlpEncodedTxInner::EIP2930(tx, _) => map_fn(tx.to),
            RlpEncodedTxInner::EIP1559(tx, _) => map_fn(tx.to),
            RlpEncodedTxInner::EIP4844(tx, _) => Some(B160::from_be_bytes(*tx.to)),
            RlpEncodedTxInner::EIP7702(tx, _) => Some(B160::from_be_bytes(*tx.to)),
            RlpEncodedTxInner::Service(tx) => Some(B160::from_be_bytes(*tx.to)),
        }
    }

    pub fn max_fee_per_gas(&self) -> &U256 {
        match &self.inner {
            RlpEncodedTxInner::Legacy(tx, _) | RlpEncodedTxInner::LegacyWithEIP155(tx, _) => {
                &tx.gas_price
            }
            RlpEncodedTxInner::EIP2930(tx, _) => &tx.gas_price,
            RlpEncodedTxInner::EIP1559(tx, _) => &tx.max_fee_per_gas,
            RlpEncodedTxInner::EIP4844(tx, _) => &tx.max_fee_per_gas,
            RlpEncodedTxInner::EIP7702(tx, _) => &tx.max_fee_per_gas,
            RlpEncodedTxInner::Service(_) => &U256::ZERO,
        }
    }

    pub fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) | RlpEncodedTxInner::LegacyWithEIP155(_, _) => None,
            RlpEncodedTxInner::EIP2930(_, _) => None,
            RlpEncodedTxInner::EIP1559(tx, _) => Some(&tx.max_priority_fee_per_gas),
            RlpEncodedTxInner::EIP4844(tx, _) => Some(&tx.max_priority_fee_per_gas),
            RlpEncodedTxInner::EIP7702(tx, _) => Some(&tx.max_priority_fee_per_gas),
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    pub fn max_fee_per_blob_gas(&self) -> Option<&U256> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) | RlpEncodedTxInner::LegacyWithEIP155(_, _) => None,
            RlpEncodedTxInner::EIP2930(_, _) => None,
            RlpEncodedTxInner::EIP1559(_, _) => None,
            RlpEncodedTxInner::EIP4844(tx, _) => Some(&tx.max_fee_per_blob_gas),
            RlpEncodedTxInner::EIP7702(_, _) => None,
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    pub fn blobs_list<'a>(&'a self) -> Option<BlobHashesList<'a>> {
        match &self.inner {
            RlpEncodedTxInner::Legacy(_, _) | RlpEncodedTxInner::LegacyWithEIP155(_, _) => None,
            RlpEncodedTxInner::EIP2930(_, _) => None,
            RlpEncodedTxInner::EIP1559(_, _) => None,
            RlpEncodedTxInner::EIP4844(tx, _) => Some(tx.blob_versioned_hashes),
            RlpEncodedTxInner::EIP7702(_, _) => None,
            RlpEncodedTxInner::Service(_) => None,
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}
