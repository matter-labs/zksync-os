use syslib::abi::{Encodable, Encoder};

#[ignore = "u32 not yet implemented"]
#[test]
fn u32() {
    let mut encoder = Encoder::new(1 << 5); // Arbitrary enough.

    let int = 123u32;

    let r = int.encode(&mut encoder);

    assert!(r.is_ok());

    println!("{:?}", encoder)
}

#[test]
fn intx() {
    // let mut encoder = Encoder::new(1 << 5); // Arbitrary enough.
    //
    // let int = U256::from_usize(123);
    //
    // let r = int.encode(&mut encoder);
    //
    // assert!(r.is_ok());
    //
    // println!("{:?}", encoder)
}
