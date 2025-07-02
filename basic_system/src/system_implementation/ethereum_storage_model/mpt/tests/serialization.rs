use crypto::MiniDigest;
use num_bigint::BigUint;
use num_traits::Num;

use crate::system_implementation::ethereum_storage_model::mpt::path_char_to_digit;

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct TestJsonResponse {
    pub(crate) result: AccountProof,
}

// #[derive(Clone, Debug, serde::Deserialize)]
// pub(crate) struct AccountProofResult {
//     #[serde(rename = "accountProof")]
//     pub(crate) account_proof: AccountProof,
// }

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct AccountProof {
    #[serde(rename = "accountProof")]
    #[serde(deserialize_with = "vec_hex_strings_to_vecs")]
    pub(crate) account_proof: Vec<Vec<u8>>,
    #[serde(deserialize_with = "hex_string_to_vec")]
    pub(crate) address: Vec<u8>,
    #[serde(deserialize_with = "hex_string_to_biguint")]
    pub(crate) balance: BigUint,
    #[serde(rename = "codeHash")]
    #[serde(deserialize_with = "hex_string_to_vec")]
    pub(crate) code_hash: Vec<u8>,
    #[serde(deserialize_with = "hex_string_to_biguint")]
    pub(crate) nonce: BigUint,
    #[serde(rename = "storageProof")]
    pub(crate) storage_proof: Vec<StorageProof>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StorageKeyLike {
    pub(crate) key: Vec<u8>,
    pub(crate) trie_pos: Vec<u8>,
    pub(crate) trie_pos_digits_string: Vec<u8>,
    pub(crate) trie_pos_digits: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub(crate) struct StorageProof {
    #[serde(deserialize_with = "hex_string_to_storage_key")]
    pub(crate) key: StorageKeyLike,
    #[serde(deserialize_with = "vec_hex_strings_to_vecs")]
    pub(crate) proof: Vec<Vec<u8>>,
    #[serde(deserialize_with = "hex_string_to_biguint")]
    pub(crate) value: BigUint,
}

fn hex_string_to_vec<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let buf = String::deserialize(deserializer)?;

    let input = if buf.starts_with("0x") {
        &buf[2..]
    } else {
        &buf
    };
    let data = if input.len() % 2 == 1 {
        let s = format!("0{}", input);
        hex::decode(&s).unwrap()
    } else {
        hex::decode(input).unwrap()
    };

    Ok(data)
}

fn hex_string_to_storage_key<'de, D>(deserializer: D) -> Result<StorageKeyLike, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let buf = String::deserialize(deserializer)?;

    let input = if buf.starts_with("0x") {
        &buf[2..]
    } else {
        &buf
    };
    assert_eq!(input.len(), 64);
    let trie_pos = crypto::sha3::Keccak256::digest(&hex::decode(input).unwrap()).to_vec();
    // format as string and get bytes
    let trie_pos_digits_string = hex::encode(&trie_pos).as_bytes().to_vec();
    let trie_pos_digits = trie_pos_digits_string
        .iter()
        .map(|el| path_char_to_digit(*el))
        .collect();

    Ok(StorageKeyLike {
        key: hex::decode(input).unwrap(),
        trie_pos,
        trie_pos_digits_string,
        trie_pos_digits,
    })
}

fn hex_string_to_biguint<'de, D>(deserializer: D) -> Result<BigUint, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let buf = String::deserialize(deserializer)?;

    let input = if buf.starts_with("0x") {
        &buf[2..]
    } else {
        &buf
    };
    let value = BigUint::from_str_radix(input, 16).unwrap();

    Ok(value)
}

fn storage_slot_index_hex_string_to_vec<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let buf = String::deserialize(deserializer)?;

    let input = if buf.starts_with("0x") {
        &buf[2..]
    } else {
        &buf
    };
    let data = hex::decode(input).unwrap();

    Ok(data)
}

fn vec_hex_strings_to_vecs<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let input = Vec::<String>::deserialize(deserializer)?;
    let result = input
        .into_iter()
        .map(|buf| {
            let input = if buf.starts_with("0x") {
                &buf[2..]
            } else {
                &buf
            };

            hex::decode(input).unwrap()
        })
        .collect();

    Ok(result)
}
