use std::collections::HashMap;

use syslib::{system::env::Env, types::ints::{U256, U256BE}};


#[derive(Debug)]
struct TestEnv {
    calldata: Vec<u8>,
    storage: HashMap<U256BE, U256BE>,
    selector: u32,
}

impl Env for TestEnv {
    fn get_calldata(&self) -> &[u8] {
        &self.calldata
    }

    fn storage_read_s(&self, ix: &U256BE) -> U256BE {
        println!("read s: {:?}", ix);

        println!("Available keys:");
        for e in self.storage.keys() {
            println!("  {:?}", e);
        }
        self.storage[&ix].clone()
    }

    fn get_selector(&self) -> u32 {
        self.selector
    }

    fn storage_write_s(&mut self, ix: &U256BE, v: &U256BE) {
        self.storage.entry(ix.clone()).and_modify(|e| *e = v.clone()).or_insert(v.clone());
    }
}

#[test]
fn mint_for() {
    let mut storage = HashMap::new();

    storage.insert(
        U256BE::from_hex("0x0000000000000000000000000000000000000000000000000000000000000000"), 
        U256BE::from_hex("0x0000000000000000000000000000000000000000000000000000000000000000"), 
    );

    storage.insert(
        U256BE::from_hex("0x479405233a6ec90a1f9044ac9597ab3dc68c58831067759d089edf557df31b0f"),
        U256BE::from_hex("0x0000000000000000000000000000000000000000000000000000000000000000"), 
    );

    let env = TestEnv {
        calldata: vec_from_u256s(&[
            U256::from_usize(1),
            U256::from_usize(10)
        ]),
        storage,
        selector: 0x6B04F110
    };

    syslib::system::env::set(Box::new(env));

    let r = crate::runtime();

    println!("{:#?}", r.as_ref());

    
}

fn vec_from_u256s(values: &[U256]) -> Vec<u8> {
    let mut vec = Vec::with_capacity(values.len() * 32);

    for value in values {
        vec.extend_from_slice(value.as_bytes());
    }

    vec
}
