use std::collections::HashMap;

use crate::{
    storage::kinds::{Mapping, Stored}, system::{self, env::Env}, types::{ints::{U256, U256BE}, Address}
};

struct TestEnv {
    storage: HashMap<U256BE, U256BE>,
}

impl Env for TestEnv {
    fn get_selector(&self) -> u32 {
        todo!()
    }

    fn get_calldata(&self) -> &[u8] {
        todo!()
    }

    fn storage_read_s(&self, ix: &crate::types::ints::U256BE) -> crate::types::ints::U256BE {
        self.storage[&ix].clone()
    }

    fn storage_write_s(&mut self, ix: &crate::types::ints::U256BE, v: &crate::types::ints::U256BE) {
        self.storage.entry(ix.clone()).insert_entry(v.clone());
    }
}

#[ignore = "test has todo"]
#[test]
fn mapping_simple() {
    let env = TestEnv {
        storage: HashMap::new(),
    };
    system::env::set(Box::new(env));

    let mut mapping = Stored::<Mapping<Address, U256>>::new(0, Mapping::new(U256BE::from_usize(0)));

    mapping.write(&Address::from_usize(1), &U256::from_usize(0xabcd));
    let x = mapping.read(&Address::from_usize(1));

    assert_eq!(U256::from_usize(0xabcd), x);
}

#[ignore = "test has todo"]
#[test]
fn mapping_nested() {
    let env = TestEnv {
        storage: HashMap::new(),
    };
    system::env::set(Box::new(env));

    let mapping = 
        Stored::<Mapping<Address, Mapping<Address, U256>>>
        ::new(0, Mapping::new(U256BE::from_usize(0)));

    let mut inner = mapping.get(&Address::from_usize(1));

    inner.write(&Address::from_usize(2), &U256::from_usize(0xabcd));

    let inner = mapping.get(&Address::from_usize(1));
    let x = inner.read(&Address::from_usize(2));

    assert_eq!(U256::from_usize(0xabcd), x);
}
