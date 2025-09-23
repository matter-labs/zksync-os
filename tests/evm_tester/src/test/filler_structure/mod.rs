use alloy::primitives::*;
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Clone, Hash, PartialEq)]
pub enum U256Parsed {
    Value(U256),
    Any,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseU256Error(String);

impl U256Parsed {
    pub fn from_generic_deserialized_value(
        value: GenericSerializedSimpleValue,
    ) -> Result<Self, ParseU256Error> {
        Self::from_str(&value.as_string())
    }

    pub fn as_value(&self) -> Option<U256> {
        match self {
            U256Parsed::Value(u256) => Some(*u256),
            U256Parsed::Any => None,
        }
    }
}

impl FromStr for U256Parsed {
    type Err = ParseU256Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = &value.replace("_", "");
        if value.to_uppercase() == "ANY" {
            return Ok(U256Parsed::Any);
        }

        if let Some(value) = value.strip_prefix("0x") {
            if let Ok(value) = U256::from_str_radix(value, 16) {
                Ok(U256Parsed::Value(value))
            } else {
                Err(ParseU256Error(format!(
                    "Failed to parse {} as radix-16",
                    value
                )))
            }
        } else {
            let res_10 = U256::from_str_radix(value, 10);
            if res_10.is_ok() {
                Ok(U256Parsed::Value(res_10.unwrap()))
            } else {
                let res_16 = U256::from_str_radix(value, 16);
                if res_16.is_ok() {
                    Ok(U256Parsed::Value(res_16.unwrap()))
                } else {
                    Err(ParseU256Error(format!("Invalid input: {}", value)))
                }
            }
        }
    }
}

impl Eq for U256Parsed {}

impl<'de> Deserialize<'de> for U256Parsed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U256ParsedVisitor;

        impl<'de> serde::de::Visitor<'de> for U256ParsedVisitor {
            type Value = U256Parsed;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("u256 value")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(U256Parsed::from_str(&value.to_string()).unwrap())
            }

            fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(U256Parsed::from_str(&value.to_string()).unwrap())
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(U256Parsed::from_str(&value).unwrap())
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(U256Parsed::from_str(value).unwrap())
            }
        }

        let res = deserializer.deserialize_any(U256ParsedVisitor);

        if res.is_err() {
            println!("Invalid parsing Uint256: {:?}", res);
        }

        res
    }
}

#[derive(Debug, Clone)]
pub struct AccountCode(pub Bytes);

impl<'de> Deserialize<'de> for AccountCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = AccountCode;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("A smart contract bytecode")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                let res = if value.is_empty() {
                    Bytes::default()
                } else {
                    let stripped = value.strip_prefix("0x").unwrap_or(value);

                    Bytes::from(hex::decode(stripped).unwrap())
                };

                Ok(AccountCode(res))
            }
        }
        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AccountFillerStruct {
    pub balance: Option<U256Parsed>,
    pub code: Option<AccountCode>,
    pub nonce: Option<U256Parsed>,
    pub storage: Option<HashMap<GenericSerializedSimpleValue, GenericSerializedSimpleValue>>,
}

impl AccountFillerStruct {
    pub fn get_storage_value(
        storage: &HashMap<U256Parsed, U256Parsed>,
        key: &U256Parsed,
    ) -> Option<U256Parsed> {
        storage.get(key).cloned()
    }

    pub fn parse_storage(
        map: &HashMap<GenericSerializedSimpleValue, GenericSerializedSimpleValue>,
    ) -> HashMap<U256Parsed, U256Parsed> {
        let mut storage = HashMap::new();

        for (key, value) in map {
            if key.is_string() && key.as_string().starts_with("//") {
                continue;
            }

            let key_v = U256Parsed::from_generic_deserialized_value(key.clone()).unwrap();

            let val_v = U256Parsed::from_generic_deserialized_value(value.clone()).unwrap();

            storage.insert(key_v, val_v);
        }

        storage
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Labels {
    Single(LabelValue),
    Multiple(Vec<LabelValue>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LabelValue {
    String(String),
    Number(isize),
}

impl LabelValue {
    pub fn as_isize(&self) -> isize {
        match self {
            LabelValue::String(str) => panic!("Invalid label: {str}"),
            LabelValue::Number(val) => *val,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExpectedIndexesStructure {
    pub data: Labels,
    pub value: Option<Labels>,
    pub gas: Option<Labels>,
}

#[derive(Debug, Deserialize, Clone, Eq, PartialEq, Hash)]
#[serde(untagged)]
pub enum AddressMaybe {
    Val(Address),
    Comment(String),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum AccountFillerStructMaybe {
    Val(AccountFillerStruct),
    Comment(String),
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExpectStructure {
    pub indexes: Option<ExpectedIndexesStructure>,
    pub result: HashMap<AddressMaybe, AccountFillerStructMaybe>,
    pub expect_exception:
        Option<HashMap<GenericSerializedSimpleValue, GenericSerializedSimpleValue>>,
}

impl ExpectStructure {
    pub fn get_expected_result(
        map: &HashMap<AddressMaybe, AccountFillerStructMaybe>,
    ) -> HashMap<Address, AccountFillerStruct> {
        let mut storage = HashMap::new();

        for (key, value) in map {
            if let AddressMaybe::Val(addr) = key {
                match value {
                    AccountFillerStructMaybe::Val(account_struct) => {
                        let account_struct = account_struct.clone();

                        storage.insert(*addr, account_struct);
                    }
                    AccountFillerStructMaybe::Comment(comment) => {
                        panic!("Unexpected value instead of account struct: {comment}");
                    }
                };
            } else {
                println!("Incorrect key: {:?}", key);
            }
        }

        storage
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum GenericSerializedSimpleValue {
    U64(u64),
    String(String),
}

impl<'de> Deserialize<'de> for GenericSerializedSimpleValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U256ParsedVisitor;

        impl<'de> serde::de::Visitor<'de> for U256ParsedVisitor {
            type Value = GenericSerializedSimpleValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("u256 value")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(GenericSerializedSimpleValue::U64(value))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GenericSerializedSimpleValue::String(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GenericSerializedSimpleValue::String(value.to_string()))
            }
        }

        let res = deserializer.deserialize_any(U256ParsedVisitor);

        if res.is_err() {
            println!("Failed parsing: {res:?}");
        }

        res
    }
}

impl GenericSerializedSimpleValue {
    pub fn is_string(&self) -> bool {
        if let GenericSerializedSimpleValue::String(_) = self {
            true
        } else {
            false
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            GenericSerializedSimpleValue::String(str) => str.clone(),
            GenericSerializedSimpleValue::U64(val) => val.to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct FillerStructure {
    pub expect: Vec<ExpectStructure>,
}
