use crate::abi::Encodable;
use crate::{abi::Encoder, types::ints::U256};

#[test]
fn uint() {
    let mut encoder = Encoder::new(1 << 5);

    let int = U256::from_usize(123);

    let r = int.encode(&mut encoder);

    assert!(r.is_ok());

    println!("{:?}", encoder)
}

#[test]
fn ref_string() {
    let mut encoder = Encoder::new(1 << 10);
    let str = "str";
    let r = str.encode(&mut encoder);

    assert!(r.is_ok());

    assert_eq!(format!("{:?}", encoder),
    "Encoder { buf: [115, 116, 114, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }"
);
    println!("{:?}", encoder)
}
