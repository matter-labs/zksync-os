use alloy::primitives::*;
use itertools::Itertools;
use serde::{Deserialize, Deserializer};
use zksync_os_rig::zksync_os_api::helpers;

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
) -> Vec<u8> {
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
            let tx_type = tx.ty;
            let from = tx.common.sender.expect("Signed tx must have send").0 .0;
            let to = tx.common.to.0.map(|a| a.0 .0);
            let gas_limit = tx.common.gas_limit.try_into().expect("gas limit overflow");
            let is_eip155 = tx_type == 0 && tx.v >= 35;
            let (max_fee_per_gas, max_priority_fee_per_gas) = if tx_type == 2 || tx_type == 3 {
                (
                    tx.common.max_fee_per_gas.unwrap(),
                    tx.common.max_priority_fee_per_gas,
                )
            } else {
                (tx.common.gas_price.unwrap(), tx.common.gas_price)
            };
            let nonce = tx.common.nonce.try_into().expect("nonce overflow");
            let value = tx.common.value.to_be_bytes();
            let data = tx.common.data.0.to_vec();
            let mut signature: Vec<u8> = vec![0; 65];
            signature[..32].copy_from_slice(&tx.r.to_be_bytes::<32>());
            signature[32..64].copy_from_slice(&tx.s.to_be_bytes::<32>());
            let parity = match tx.v {
                27 | 28 => tx.v,
                n if n >= 35 => ((n - 35) % 2) + 27,
                n => (n & 1) + 27,
            };
            signature[64] = parity;

            let access_list: Option<Vec<([u8; 20], Vec<[u8; 32]>)>> = tx
                .common
                .access_list
                .clone()
                .map(|access_list: Vec<AccessListItem>| {
                    access_list
                        .into_iter()
                        .map(|item| {
                            let address = item.address.into_array();
                            let keys: Vec<[u8; 32]> = item
                                .storage_keys
                                .into_iter()
                                .map(|k| k.to_be_bytes())
                                .collect();
                            (address, keys)
                        })
                        .collect()
                });

            let authorization_list: Vec<(U256, [u8; 20], u64, u8, U256, U256)> = tx
                .common
                .authorization_list
                .clone()
                .map(|authorization_list| {
                    authorization_list
                        .iter()
                        .map(|authorization| {
                            let y_parity = authorization.y_parity;
                            let r = authorization.r;
                            let s = authorization.s;
                            let mut r_buf = [0u8; 32];
                            r.to_big_endian(&mut r_buf);
                            let mut s_buf = [0u8; 32];
                            s.to_big_endian(&mut s_buf);
                            let mut chain_id_buf = [0u8; 32];
                            authorization.chain_id.to_big_endian(&mut chain_id_buf);
                            (
                                ruint::aliases::U256::from_be_bytes(chain_id_buf),
                                authorization.address.into_array(),
                                authorization.nonce.as_u64(),
                                y_parity.as_u32() as u8,
                                ruint::aliases::U256::from_be_bytes(r_buf),
                                ruint::aliases::U256::from_be_bytes(s_buf),
                            )
                        })
                        .collect_vec()
                })
                .unwrap_or_default();

            let reserved_dynamic = access_list.map(|access_list| {
                zksync_os_rig::utils::encode_reserved_dynamic(access_list, authorization_list)
            });
            zksync_os_rig::zksync_os_api::helpers::encode_tx(
                tx_type,
                from,
                to,
                gas_limit,
                None,
                max_fee_per_gas
                    .try_into()
                    .expect("max_fee_per_gas overflow"),
                max_priority_fee_per_gas
                    .map(|v| v.try_into().expect("max_priority_fee_per_gas overflow")),
                None,
                nonce,
                value,
                data,
                signature,
                None,
                reserved_dynamic,
                is_eip155,
            )
        }
    }
}
