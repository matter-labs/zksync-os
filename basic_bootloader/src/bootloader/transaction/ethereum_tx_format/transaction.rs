use crate::bootloader::errors::TxError;
use crate::bootloader::transaction::charge_keccak;
use crate::bootloader::transaction::ethereum_tx_format::eip_2930_tx::{
    AccessList, AccessListForAddress,
};
use crate::bootloader::transaction::ethereum_tx_format::eip_7702_tx::{
    AuthorizationEntry, AuthorizationList,
};
use core::alloc::Allocator;

use super::*;
use ruint::aliases::{B160, U256};
use zk_ee::system::Resources;
use zk_ee::utils::UsizeAlignedByteBox;

// NOTE: this is self-reference, but relatively easy one. Do NOT derive clone one it,
// as it's unsound
pub struct EthereumTransaction<A: Allocator> {
    buffer: UsizeAlignedByteBox<A>,
    inner: EthereumTxInner<'static>,
    chain_id: u64,
    sig_hash: Bytes32,
    // Lazy field, computed only when calling transaction_hash() for the first
    // time.
    tx_hash: Option<Bytes32>,
    // Note: this field is not the recovered signer, but rather an address
    // passed by oracle. Needs to be checked to be equal to recovered address.
    from: B160,
}

impl<A: Allocator> core::fmt::Debug for EthereumTransaction<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EthereumTransaction")
            .field("buffer", &self.buffer.as_slice())
            .field("inner", &self.inner)
            .field("chain_id", &self.chain_id)
            .field("sig_hash", &self.sig_hash)
            .field("tx_hash", &self.tx_hash)
            .field("from", &self.from)
            .finish()
    }
}

impl<A: Allocator> EthereumTransaction<A> {
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

        let (inner, sig_hash): (EthereumTxInner<'static>, Bytes32) =
            EthereumTxInner::parse_and_compute_signed_hash(
                unsafe { core::mem::transmute::<&[u8], &[u8]>(buffer.as_slice()) },
                expected_chain_id,
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

    pub fn chain_id(&self) -> Option<u64> {
        match &self.inner {
            EthereumTxInner::Legacy(_, _) => None,
            _ => Some(self.chain_id),
        }
    }

    pub fn nonce(&self) -> u64 {
        match &self.inner {
            EthereumTxInner::Legacy(tx, _) | EthereumTxInner::LegacyWithEIP155(tx, _) => tx.nonce,
            EthereumTxInner::EIP2930(tx, _) => tx.nonce,
            EthereumTxInner::EIP1559(tx, _) => tx.nonce,
            EthereumTxInner::EIP7702(tx, _) => tx.nonce,
        }
    }

    pub fn value(&self) -> &U256 {
        match &self.inner {
            EthereumTxInner::Legacy(tx, _) | EthereumTxInner::LegacyWithEIP155(tx, _) => &tx.value,
            EthereumTxInner::EIP2930(tx, _) => &tx.value,
            EthereumTxInner::EIP1559(tx, _) => &tx.value,
            EthereumTxInner::EIP7702(tx, _) => &tx.value,
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
            EthereumTxInner::Legacy(_, _) | EthereumTxInner::LegacyWithEIP155(_, _) => 0,
            EthereumTxInner::EIP2930(_, _) => 1,
            EthereumTxInner::EIP1559(_, _) => 2,
            EthereumTxInner::EIP7702(_, _) => 4,
        }
    }

    pub fn calldata<'a>(&'a self) -> &'a [u8] {
        match &self.inner {
            EthereumTxInner::Legacy(tx, _) | EthereumTxInner::LegacyWithEIP155(tx, _) => tx.data,
            EthereumTxInner::EIP2930(tx, _) => tx.data,
            EthereumTxInner::EIP1559(tx, _) => tx.data,
            EthereumTxInner::EIP7702(tx, _) => tx.data,
        }
    }

    pub fn access_list<'a>(&'a self) -> Option<AccessList<'a>> {
        match &self.inner {
            EthereumTxInner::Legacy(_, _) | EthereumTxInner::LegacyWithEIP155(_, _) => None,
            EthereumTxInner::EIP2930(tx, _) => Some(tx.access_list),
            EthereumTxInner::EIP1559(tx, _) => Some(tx.access_list),
            EthereumTxInner::EIP7702(tx, _) => Some(tx.access_list),
        }
    }

    pub fn access_list_iter<'a>(
        &'a self,
    ) -> Option<impl Iterator<Item = AccessListForAddress<'a>> + Clone> {
        match &self.inner {
            EthereumTxInner::Legacy(_, _) | EthereumTxInner::LegacyWithEIP155(_, _) => None,
            EthereumTxInner::EIP2930(tx, _) => Some(tx.access_list.iter()),
            EthereumTxInner::EIP1559(tx, _) => Some(tx.access_list.iter()),
            EthereumTxInner::EIP7702(tx, _) => Some(tx.access_list.iter()),
        }
    }

    pub fn authorization_list<'a>(&'a self) -> Option<AuthorizationList<'a>> {
        match &self.inner {
            EthereumTxInner::EIP7702(tx, _) => Some(tx.authorization_list),
            _ => None,
        }
    }

    pub fn authorization_list_iter<'a>(
        &'a self,
    ) -> Option<impl Iterator<Item = AuthorizationEntry<'a>> + Clone> {
        match &self.inner {
            EthereumTxInner::EIP7702(tx, _) => Some(tx.authorization_list.iter()),
            _ => None,
        }
    }

    pub fn from(&self) -> &B160 {
        &self.from
    }

    pub fn sig_parity_r_s<'a>(&'a self) -> (bool, &'a [u8], &'a [u8]) {
        match &self.inner {
            EthereumTxInner::Legacy(_, sig) => {
                ((sig.v - 27) == 1, sig.r, sig.s) // prechecked
            }
            EthereumTxInner::LegacyWithEIP155(_, sig) => {
                let chain_id = self.chain_id;
                let parity = sig.v - 35 - (chain_id * 2); // no underflows
                (parity == 1, sig.r, sig.s)
            }
            EthereumTxInner::EIP2930(_, sig) => (sig.y_parity, sig.r, sig.s),
            EthereumTxInner::EIP1559(_, sig) => (sig.y_parity, sig.r, sig.s),
            EthereumTxInner::EIP7702(_, sig) => (sig.y_parity, sig.r, sig.s),
        }
    }

    pub fn required_balance(&self) -> Option<U256> {
        let fee_amount = self
            .max_fee_per_gas()
            .checked_mul(U256::from(self.gas_limit()))?;
        self.value().checked_add(U256::from(fee_amount))
    }

    pub fn gas_limit(&self) -> u64 {
        match &self.inner {
            EthereumTxInner::Legacy(tx, _) | EthereumTxInner::LegacyWithEIP155(tx, _) => {
                tx.gas_limit
            }
            EthereumTxInner::EIP2930(tx, _) => tx.gas_limit,
            EthereumTxInner::EIP1559(tx, _) => tx.gas_limit,
            EthereumTxInner::EIP7702(tx, _) => tx.gas_limit,
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
            EthereumTxInner::Legacy(tx, _) | EthereumTxInner::LegacyWithEIP155(tx, _) => {
                map_fn(tx.to)
            }
            EthereumTxInner::EIP2930(tx, _) => map_fn(tx.to),
            EthereumTxInner::EIP1559(tx, _) => map_fn(tx.to),
            EthereumTxInner::EIP7702(tx, _) => Some(B160::from_be_bytes(*tx.to)),
        }
    }

    pub fn max_fee_per_gas(&self) -> &U256 {
        match &self.inner {
            EthereumTxInner::Legacy(tx, _) | EthereumTxInner::LegacyWithEIP155(tx, _) => {
                &tx.gas_price
            }
            EthereumTxInner::EIP2930(tx, _) => &tx.gas_price,
            EthereumTxInner::EIP1559(tx, _) => &tx.max_fee_per_gas,
            EthereumTxInner::EIP7702(tx, _) => &tx.max_fee_per_gas,
        }
    }

    pub fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        match &self.inner {
            EthereumTxInner::Legacy(_, _) | EthereumTxInner::LegacyWithEIP155(_, _) => None,
            EthereumTxInner::EIP2930(_, _) => None,
            EthereumTxInner::EIP1559(tx, _) => Some(&tx.max_priority_fee_per_gas),
            EthereumTxInner::EIP7702(tx, _) => Some(&tx.max_priority_fee_per_gas),
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}
