use alloy::consensus::{TxEip1559, TxEip2930, TxEip7702, TxEnvelope, TxLegacy};
use alloy::eips::eip2930::{AccessList as AlloyAccessList, AccessListItem as AlloyALItem};
use alloy::eips::eip7702::{Authorization as AlloyAuthorization, SignedAuthorization};
use alloy::primitives::*;
use serde::{Deserialize, Deserializer};
use zksync_os_rig::utils::encode_envelope_2718;
use zksync_os_rig::{zksync_os_api::helpers, zksync_os_interface::traits::EncodedTx};

use crate::{
    test::test_structure::transaction_section::TransactionSection, vm::zk_ee::ZKsyncOSEVMContext,
};

#[derive(Debug, Clone, Copy)]
pub struct FieldTo(pub Option<Address>);

impl<'de> Deserialize<'de> for FieldTo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = FieldTo;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("An empty string or correct address")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let res = if value.is_empty() {
                    None
                } else {
                    Some(value.parse::<Address>().unwrap())
                };

                Ok(FieldTo(res))
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<U256>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationListItem {
    pub chain_id: web3::types::U256,
    pub address: Address,
    pub nonce: web3::types::U256,
    pub v: Option<web3::types::U256>,
    pub r: web3::types::U256,
    pub s: web3::types::U256,
    pub signer: Option<Address>,
    pub y_parity: web3::types::U256,
}

#[derive(Debug, Clone)]
pub struct TxCommon {
    pub data: Bytes,
    pub gas_limit: U256,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub nonce: U256,
    pub to: FieldTo,
    pub sender: Option<Address>,
    pub value: U256,
    pub access_list: Option<Vec<AccessListItem>>,
    pub authorization_list: Option<Vec<AuthorizationListItem>>,
}

#[derive(Debug, Clone)]
pub struct TransactionReq {
    pub common: TxCommon,
    pub secret_key: B256,
}

#[derive(Debug, Clone)]
pub struct SignedTransaction {
    pub common: TxCommon,
    pub ty: u8,
    pub v: u8,
    pub r: U256,
    pub s: U256,
}

#[derive(Debug, Clone)]
pub enum Transaction {
    Request(TransactionReq),
    Signed(SignedTransaction),
}

impl Transaction {
    pub fn common(&self) -> &TxCommon {
        match self {
            Self::Request(r) => &r.common,
            Self::Signed(r) => &r.common,
        }
    }
}

pub fn transaction_from_tx_section(
    tx: &TransactionSection,
    value: U256,
    data: &Bytes,
    gas_limit: U256,
    access_list: Option<Vec<AccessListItem>>,
) -> Transaction {
    let common = TxCommon {
        data: data.clone(),
        gas_limit,
        gas_price: tx.gas_price,
        nonce: tx.nonce,
        to: tx.to,
        sender: tx.sender,
        value,
        max_fee_per_gas: tx.max_fee_per_gas,
        max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
        access_list,
        authorization_list: tx.authorization_list.clone(),
    };
    match tx.secret_key {
        Some(sk) => Transaction::Request(TransactionReq {
            common,
            secret_key: sk,
        }),
        None => Transaction::Signed(SignedTransaction {
            common,
            ty: tx
                .ty
                .expect("Signed txs should have type field")
                .try_into()
                .expect("tx type overflow"),
            v: tx
                .v
                .expect("Signed txs should have signature fields")
                .try_into()
                .expect("tx v overflow"),
            r: tx.r.expect("Signed txs should have signature fields"),
            s: tx.s.expect("Signed txs should have signature fields"),
        }),
    }
}

// Encode and potentially sign the transaction
pub fn encode_transaction(
    transaction: &Transaction,
    system_context: &ZKsyncOSEVMContext,
) -> EncodedTx {
    match transaction {
        Transaction::Request(tx) => {
            #[allow(deprecated)]
            use alloy::primitives::Signature;
            let access_list = transaction.common().access_list.clone().map(|v| {
                alloy::eips::eip2930::AccessList(
                    v.into_iter()
                        .map(
                            |AccessListItem {
                                 address,
                                 storage_keys,
                             }| {
                                let storage_keys = storage_keys
                                    .into_iter()
                                    .map(|k| {
                                        let buffer: [u8; 32] = k.to_be_bytes();
                                        alloy::primitives::FixedBytes::from_slice(&buffer)
                                    })
                                    .collect();
                                alloy::eips::eip2930::AccessListItem {
                                    address: alloy::primitives::Address::from_slice(
                                        address.as_ref(),
                                    ),
                                    storage_keys,
                                }
                            },
                        )
                        .collect(),
                )
            });
            let authorization_list = transaction.common().authorization_list.clone().map(|v| {
                v.into_iter()
                    .map(
                        |AuthorizationListItem {
                             nonce,
                             chain_id,
                             address,
                             v: _,
                             r,
                             s,
                             signer: _,
                             y_parity,
                         }| {
                            let mut r_buf = [0u8; 32];
                            r.to_big_endian(&mut r_buf);
                            let mut s_buf = [0u8; 32];
                            s.to_big_endian(&mut s_buf);
                            let y_parity = !y_parity.is_zero();

                            #[allow(deprecated)]
                            let signature = Signature::from_scalars_and_parity(
                                alloy::primitives::FixedBytes::from_slice(&r_buf),
                                alloy::primitives::FixedBytes::from_slice(&s_buf),
                                y_parity,
                            );
                            alloy::eips::eip7702::Authorization {
                                chain_id: chain_id.into(),
                                nonce: nonce.as_u64(),
                                address: alloy::primitives::Address::from_slice(address.as_ref()),
                            }
                            .into_signed(signature)
                        },
                    )
                    .collect()
            });
            let request = alloy::rpc::types::TransactionRequest {
                chain_id: Some(system_context.chain_id),
                nonce: Some(tx.common.nonce.try_into().expect("Nonce overflow")),
                max_fee_per_gas: tx
                    .common
                    .max_fee_per_gas
                    .map(|v| v.try_into().expect("Max fee per gas overflow")),
                max_priority_fee_per_gas: tx
                    .common
                    .max_priority_fee_per_gas
                    .map(|v| v.try_into().expect("Max priority fee per gas overflow")),
                gas_price: tx
                    .common
                    .gas_price
                    .map(|v| v.try_into().expect("gas price overflow")),
                gas: Some(tx.common.gas_limit.try_into().expect("gas limit overflow")),
                to: Some(
                    tx.common
                        .to
                        .0
                        .map_or(alloy::primitives::TxKind::Create, |addr| {
                            alloy::primitives::TxKind::Call(alloy::primitives::Address::from_slice(
                                addr.as_ref(),
                            ))
                        }),
                ),
                value: Some(tx.common.value),
                input: tx.common.data.clone().into(),
                access_list,
                authorization_list,
                ..Default::default()
            };

            let wallet = zksync_os_rig::alloy::signers::local::PrivateKeySigner::from_slice(
                tx.secret_key.as_slice(),
            )
            .unwrap();
            helpers::sign_and_encode_transaction_request(request, &wallet)
        }
        Transaction::Signed(tx) => {
            let env = to_alloy_envelope(tx, system_context.chain_id);
            let bytes = encode_envelope_2718(&env);
            let from = tx.common.sender.expect("Tx must have sender");
            EncodedTx::Rlp(bytes, from)
        }
    }
}

pub fn to_alloy_envelope(stx: &SignedTransaction, chain_id: u64) -> TxEnvelope {
    let nonce = stx.common.nonce.try_into().unwrap();
    let gas_limit = stx.common.gas_limit.try_into().unwrap();
    let value = stx.common.value;
    let input = Bytes::copy_from_slice(stx.common.data.as_ref());
    let to_kind = to_kind(&stx.common.to);

    let r = FixedBytes::from(stx.r);
    let s = FixedBytes::from(stx.s);

    match stx.ty {
        0 => {
            let (chain_id_opt, parity) = match stx.v {
                27 | 28 => (None, stx.v == 28),
                v if v >= 35 => {
                    let cid = ((v as u64) - 35) / 2;
                    let p = ((v as u64 - 35) % 2) == 1;
                    (Some(cid), p)
                }
                _ => panic!("Invalid value for v in legacy"),
            };
            let gas_price = stx.common.gas_price.unwrap_or_default().try_into().unwrap();
            let tx = TxLegacy {
                chain_id: chain_id_opt,
                nonce,
                gas_price,
                gas_limit,
                to: to_kind,
                value,
                input,
            };
            let sig = Signature::from_scalars_and_parity(r, s, parity);
            let signed = alloy::consensus::Signed::new_unhashed(tx, sig);
            TxEnvelope::from(signed)
        }
        1 => {
            let gas_price = stx.common.gas_price.unwrap_or_default().try_into().unwrap();
            let access_list = to_access_list(&stx.common.access_list);
            let tx = TxEip2930 {
                chain_id,
                nonce,
                gas_price,
                gas_limit,
                to: to_kind,
                value,
                access_list,
                input,
            };
            let parity = stx.v & 1 == 1;
            let sig = Signature::from_scalars_and_parity(r, s, parity);
            let signed = alloy::consensus::Signed::new_unhashed(tx, sig);
            TxEnvelope::from(signed)
        }
        2 => {
            let max_fee = stx.common.max_fee_per_gas.unwrap().try_into().unwrap();
            let max_prio = stx
                .common
                .max_priority_fee_per_gas
                .unwrap()
                .try_into()
                .unwrap();
            let access_list = to_access_list(&stx.common.access_list);
            let tx = TxEip1559 {
                chain_id,
                nonce,
                max_fee_per_gas: max_fee,
                max_priority_fee_per_gas: max_prio,
                gas_limit,
                to: to_kind,
                value,
                access_list,
                input,
            };
            let parity = stx.v & 1 == 1;
            let sig = Signature::from_scalars_and_parity(r, s, parity);
            let signed = alloy::consensus::Signed::new_unhashed(tx, sig);
            TxEnvelope::from(signed)
        }
        4 => {
            let access_list = to_access_list(&stx.common.access_list);
            let auth_list = to_auth_list(&stx.common.authorization_list);
            let to_addr = match &stx.common.to.0 {
                None => panic!("7702 requires destination"),
                Some(a) => *a,
            };
            let tx = TxEip7702 {
                chain_id,
                nonce,
                max_priority_fee_per_gas: stx
                    .common
                    .max_priority_fee_per_gas
                    .unwrap()
                    .try_into()
                    .unwrap(),
                max_fee_per_gas: stx.common.max_fee_per_gas.unwrap().try_into().unwrap(),
                gas_limit,
                to: to_addr,
                value,
                input,
                access_list,
                authorization_list: auth_list,
            };
            let parity = stx.v & 1 == 1;
            let sig = Signature::from_scalars_and_parity(r, s, parity);
            let signed = alloy::consensus::Signed::new_unhashed(tx, sig);
            TxEnvelope::from(signed)
        }
        _ => panic!("Unsupported tx type"),
    }
}

fn to_kind(to: &FieldTo) -> alloy::primitives::TxKind {
    match to.0 {
        None => alloy::primitives::TxKind::Create,
        Some(addr) => alloy::primitives::TxKind::Call((*addr).into()),
    }
}

fn to_access_list(src: &Option<Vec<AccessListItem>>) -> AlloyAccessList {
    let items = src
        .as_ref()
        .map(|v| {
            v.iter()
                .map(|it| AlloyALItem {
                    address: it.address,
                    storage_keys: it
                        .storage_keys
                        .clone()
                        .into_iter()
                        .map(FixedBytes::from)
                        .collect(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    AlloyAccessList(items)
}

fn to_auth_list(src: &Option<Vec<AuthorizationListItem>>) -> Vec<SignedAuthorization> {
    let mut out = Vec::new();
    if let Some(list) = src {
        for a in list {
            let auth = AlloyAuthorization {
                chain_id: w3_u256_to_alloy_u256(&a.chain_id),
                address: a.address,
                nonce: a.nonce.as_u64(),
            };
            let y = (a.y_parity.as_u64() & 1) == 1;
            let sig = Signature::from_scalars_and_parity(
                FixedBytes::from(w3_u256_to_alloy_u256(&a.r)),
                FixedBytes::from(w3_u256_to_alloy_u256(&a.s)),
                y,
            );
            out.push(auth.into_signed(sig));
        }
    }
    out
}

fn w3_u256_to_alloy_u256(x: &web3::types::U256) -> alloy::primitives::U256 {
    let mut buf = [0u8; 32];
    x.to_big_endian(&mut buf);
    alloy::primitives::U256::from_be_bytes(buf)
}
