use zk_ee::{system::VERSIONED_HASH_VERSION_KZG, utils::Bytes32};

use crate::bootloader::errors::{InvalidTransaction, TxError};

use super::rlp_encoded::BlobHashesList;

pub fn parse_blobs_list<const MAX_BLOBS_IN_TX: usize>(
    blobs_list: BlobHashesList<'_>,
) -> Result<arrayvec::ArrayVec<Bytes32, MAX_BLOBS_IN_TX>, TxError> {
    let mut result = arrayvec::ArrayVec::<_, MAX_BLOBS_IN_TX>::new();
    if blobs_list.count > MAX_BLOBS_IN_TX {
        return Err(TxError::Validation(InvalidTransaction::BlobListTooLong));
    }

    for blob_hash in blobs_list.iter() {
        let blob_hash = blob_hash?;

        if blob_hash[0] != VERSIONED_HASH_VERSION_KZG {
            return Err(TxError::Validation(
                InvalidTransaction::BlobElementIsNotSupported,
            ));
        }

        // NOTE: we do NOT check that this blob hash is meaningful - we are not worried about block validity
        // from consensus perspective. And KZG blob precompile requires explicit preimage anyway
        let blob_hash = Bytes32::from_array(*blob_hash);
        result.push(blob_hash);
    }

    if result.is_empty() {
        // transactions that allow blobs should have at least one
        return Err(TxError::Validation(InvalidTransaction::EmptyBlobList));
    }

    Ok(result)
}
