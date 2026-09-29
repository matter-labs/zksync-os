pub(super) fn valid_input() -> Vec<u8> {
    hex::decode("38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02").unwrap()
}

pub(super) fn infinity_input() -> Vec<u8> {
    hex::decode("6b8d2c81b11b2d699528dde488dbdf2f94293d0d33c32e347f255fa4a6c1f0a9000000000000000000000000000000000000000000000000000000000000001b79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817986b8d2c81b11b2d699528dde488dbdf2f94293d0d33c32e347f255fa4a6c1f0a9").unwrap()
}

pub(super) fn no_sqrt_input() -> Vec<u8> {
    let mut input = valid_input();
    input[64..96].fill(0);
    input[95] = 5;
    input
}

pub(super) fn vectors() -> Vec<(&'static str, Vec<u8>)> {
    let valid = valid_input();
    let mut high_s = valid.clone();
    high_s[96..128].fill(0xff);
    let mut r_at_n = valid.clone();
    r_at_n[64..96].copy_from_slice(
        &hex::decode("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141").unwrap(),
    );
    let mut bad_v = valid.clone();
    bad_v[63] = 29;
    vec![
        ("valid", valid.clone()),
        ("short padded", valid[..126].to_vec()),
        ("long truncated", [valid.clone(), vec![0xff]].concat()),
        ("high s", high_s),
        ("r at n", r_at_n),
        ("bad v", bad_v),
        ("invalid x", no_sqrt_input()),
        ("infinity", infinity_input()),
    ]
}
