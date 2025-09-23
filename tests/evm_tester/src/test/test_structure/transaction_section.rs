use crate::test::case::transaction::{AccessListItem, AuthorizationListItem, FieldTo};
use alloy::primitives::*;
use serde::Deserialize;
use serde::Deserializer;

fn vec_from_one_or_many<'de, D, T>(de: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }

    // Also accept null / missing → empty vec
    let v = Option::<OneOrMany<T>>::deserialize(de)?;
    Ok(match v {
        None => Vec::new(),
        Some(OneOrMany::One(x)) => vec![x],
        Some(OneOrMany::Many(xs)) => xs,
    })
}

fn opt_vec_from_one_or_many<'de, D, T>(de: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }

    let v = Option::<OneOrMany<T>>::deserialize(de)?;
    Ok(match v {
        None => None,
        Some(OneOrMany::One(x)) => Some(vec![x]),
        Some(OneOrMany::Many(xs)) => Some(xs),
    })
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSection {
    #[serde(deserialize_with = "vec_from_one_or_many")]
    pub data: Vec<Bytes>,
    #[serde(deserialize_with = "vec_from_one_or_many")]
    pub gas_limit: Vec<U256>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub nonce: U256,
    pub secret_key: Option<B256>,
    pub to: FieldTo,
    pub sender: Option<Address>,
    #[serde(deserialize_with = "vec_from_one_or_many")]
    pub value: Vec<U256>,
    #[serde(
        default,
        alias = "accessList",
        deserialize_with = "opt_vec_from_one_or_many"
    )]
    pub access_lists: Option<Vec<Option<Vec<AccessListItem>>>>,
    pub authorization_list: Option<Vec<AuthorizationListItem>>,
    #[serde(rename = "type")]
    pub ty: Option<U256>,
    pub v: Option<U256>,
    pub r: Option<U256>,
    pub s: Option<U256>,
}
