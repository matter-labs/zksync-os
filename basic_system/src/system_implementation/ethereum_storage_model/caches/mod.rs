// we want to cache:
// - address -> account data
// - address + slot -> value
// - preimages (here we only need bytecode)

// Our strategy is:
// - if account is accessed we query an oracle to get account state and we blindly believe it
// - storage of per-address data is - map of maps
// - we commit/persist only when execution is done

pub mod account_cache;
pub mod account_properties;
pub mod full_storage_cache;
pub mod preimage;

use zk_ee::utils::Bytes32;

pub const EMPTY_STRING_KECCAK_HASH: Bytes32 =
    Bytes32::from_hex("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
