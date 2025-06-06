// Based on https://github.com/RustCrypto/sponges/blob/13e501fa4e9b7d23c550a53f75e27ec291afc32a/keccak/src/unroll.rs implementation

/// unroll5
#[cfg(not(feature = "no_unroll"))]
#[macro_export]
macro_rules! unroll5 {
    ($var:ident, $body:block) => {
        { const $var: usize = 0; $body; }
        { const $var: usize = 1; $body; }
        { const $var: usize = 2; $body; }
        { const $var: usize = 3; $body; }
        { const $var: usize = 4; $body; }
    };
}

/// unroll5
#[cfg(feature = "no_unroll")]
#[macro_export]
macro_rules! unroll5 {
    ($var:ident, $body:block) => {
        for $var in 0..5 $body
    }
}

/// unroll24
#[cfg(not(feature = "no_unroll"))]
#[macro_export]
macro_rules! unroll24 {
    ($var: ident, $body: block) => {
        { const $var: usize = 0; $body; }
        { const $var: usize = 1; $body; }
        { const $var: usize = 2; $body; }
        { const $var: usize = 3; $body; }
        { const $var: usize = 4; $body; }
        { const $var: usize = 5; $body; }
        { const $var: usize = 6; $body; }
        { const $var: usize = 7; $body; }
        { const $var: usize = 8; $body; }
        { const $var: usize = 9; $body; }
        { const $var: usize = 10; $body; }
        { const $var: usize = 11; $body; }
        { const $var: usize = 12; $body; }
        { const $var: usize = 13; $body; }
        { const $var: usize = 14; $body; }
        { const $var: usize = 15; $body; }
        { const $var: usize = 16; $body; }
        { const $var: usize = 17; $body; }
        { const $var: usize = 18; $body; }
        { const $var: usize = 19; $body; }
        { const $var: usize = 20; $body; }
        { const $var: usize = 21; $body; }
        { const $var: usize = 22; $body; }
        { const $var: usize = 23; $body; }
    };
}

/// unroll24
#[cfg(feature = "no_unroll")]
#[macro_export]
macro_rules! unroll24 {
    ($var:ident, $body:block) => {
        for $var in 0..24 $body
    }
}