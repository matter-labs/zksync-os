use zk_ee::internal_error;
use zk_ee::oracle::query_ids::{GENERIC_SUBSPACE_MASK, PREIMAGE_SUBSPACE_MASK};
use zk_ee::oracle::usize_serialization::UsizeDeserializable;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::Bytes32;

pub const ETHEREUM_QUERIES_SUBSPACE_MASK: u32 = 0x00_00_e0_00;

pub const ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 1;
pub const ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 2;

pub const ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 3;
pub const ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 4;

pub const ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 5;
pub const ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 6;

pub const ETHEREUM_BLOB_POINT_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 0x01_00;

pub(crate) fn fill_blob_point_from_oracle<const N: usize, const TO_FILL: usize>(
    x: &mut crypto::BigInt<N>,
    y: &mut crypto::BigInt<N>,
    versioned_hash: &Bytes32,
    oracle: &mut impl IOOracle,
) -> Result<(), InternalError> {
    assert!(TO_FILL <= N);

    let mut dst_it = oracle.raw_query(ETHEREUM_BLOB_POINT_QUERY_ID, versioned_hash)?;
    if dst_it.len() < TO_FILL * <u64 as UsizeDeserializable>::USIZE_LEN {
        return Err(internal_error!("not enough input data"));
    }

    for dst in x.0[..TO_FILL].iter_mut() {
        *dst = UsizeDeserializable::from_iter(&mut dst_it)?;
    }
    for dst in y.0[..TO_FILL].iter_mut() {
        *dst = UsizeDeserializable::from_iter(&mut dst_it)?;
    }

    Ok(())
}
