use core::mem::MaybeUninit;

use syslib::{abi::{Encodable, Encoder}, types::ints::{U256, U256BE}};

#[no_mangle]
#[inline(never)]
pub fn msg_from(encoder: &mut Encoder) {
    let _ = syslib::system::msg::sender().encode(encoder);
}

#[no_mangle]
#[inline(never)]
pub fn hash_keccak256(encoder: &mut Encoder) {
    let mut cd = [MaybeUninit::uninit(); 32];
    syslib::system::calldata::read_into(&mut cd);
    let cd: [u8; 32] = unsafe { core::mem::transmute(cd) };

    syslib::system::slice::hash_keccak256(&cd).encode(encoder);
}
