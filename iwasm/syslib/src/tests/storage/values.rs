use std::collections::HashMap;

use crate::{storage::kinds::{Stored, Value}, system::{self, env::Env}, types::ints::{U256, U256BE}};

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

#[test]
fn value_get_set() {
    let env = TestEnv {
        storage: HashMap::new(),
    };
    system::env::set(Box::new(env));

    let mut storage = Stored::<Value<U256>>::new(0, Value::<U256>::default());

    storage.write(&U256::from_usize(123));
    let val = storage.read();

    assert_eq!(U256::from_usize(123), val);
}
