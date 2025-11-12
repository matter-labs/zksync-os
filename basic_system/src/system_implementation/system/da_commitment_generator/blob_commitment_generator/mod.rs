use crate::system_functions::point_evaluation::{parse_g1_compressed, versioned_hash_for_kzg};
use crate::system_implementation::system::da_commitment_generator::DACommitmentGenerator;
use crate::system_implementation::system::da_commitment_generator::blob_commitment_generator::commitment_and_proof_advice::{BlobCommitmentAndProofAdvisor, OracleBasedBlobCommitmentAndProofAdvisor};
use arrayvec::ArrayVec;
use crypto::ark_ff::Field;
use crypto::ark_ff::One;
use crypto::ark_ff::PrimeField;
use crypto::ark_ff::Zero;
use crypto::sha3::Keccak256;
use crypto::{parse_u256_be, MiniDigest};
use zk_ee::oracle::IOOracle;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

pub mod brp_roots_of_unity;
pub mod commitment_and_proof_advice;
pub mod polynomial_evaluation;

///
/// Number of bytes we encode in one blob element
///
pub const BLOB_CHUNK_SIZE: usize = 31;

///
/// Number of field elements per EIP-4844 blob
///
pub const ELEMENTS_PER_4844_BLOB: usize = 4096;

///
/// Number of bytes we can encode in one blob
///
pub const ENCODABLE_BYTES_PER_BLOB: usize = BLOB_CHUNK_SIZE * ELEMENTS_PER_4844_BLOB;

///
/// Maximal number of blobs that we support with blobs DA mode.
///
pub const MAX_NUMBER_OF_BLOBS: usize = 9;

pub const BUFFER_CAPACITY: usize = ENCODABLE_BYTES_PER_BLOB * MAX_NUMBER_OF_BLOBS;

///
/// Blobs DA commitment generator.
///
/// It encodes pubdata into the blobs using alloy default(`SimpleCoder`) encoding.
/// The first element in the first blob used to encode length as `[0, len BE, 23 zeroes] BE`.
/// Then blobs are filled with elements created from data chunked by 31 byte as `[0, chunk(31 byte)] BE`.
///
pub struct BlobCommitmentGenerator {
    buffer: ArrayVec<u8, BUFFER_CAPACITY>,
    versioned_hashes_hasher: Keccak256,
}

impl BlobCommitmentGenerator {
    pub fn new() -> Self {
        let mut buffer = ArrayVec::new();
        // we allocate 31 byte to encode length as a separate field element for convenience
        buffer.extend([0u8; 31]);
        Self {
            buffer,
            versioned_hashes_hasher: Keccak256::new(),
        }
    }
}

impl WriteBytes for BlobCommitmentGenerator {
    fn write(&mut self, buf: &[u8]) {
        // overflow shouldn't be reachable, operator validates pubdata limit during forward run
        self.buffer.try_extend_from_slice(buf).unwrap()
    }
}

impl<O: IOOracle> DACommitmentGenerator<O> for BlobCommitmentGenerator {
    fn finalize(&mut self, oracle: &mut O) -> Bytes32 {
        // len should be [0, len be, 23 zeroes] BE
        let length = self.buffer.len() - 31;
        self.buffer[0..8].copy_from_slice(&(length as u64).to_be_bytes());
        let mut advisor = OracleBasedBlobCommitmentAndProofAdvisor { oracle };
        for chunk in self.buffer.chunks(ENCODABLE_BYTES_PER_BLOB) {
            cycle_marker::wrap!("blob_versioned_hash", {
                self.versioned_hashes_hasher
                    .update(&blob_versioned_hash_with_advisor(chunk, &mut advisor));
            });
        }
        self.versioned_hashes_hasher.finalize_reset().into()
    }
}

///
/// Returns blob versioned hash.
///
/// Please note, that `data` is not the blob itself, but data we encode into the blob.
/// For encoding, we chunk `data` by 31 bytes and interpret each chunk as BE blob element.
///
pub fn blob_versioned_hash_with_advisor(
    data: &[u8],
    advisor: &mut impl BlobCommitmentAndProofAdvisor,
) -> [u8; 32] {
    debug_assert!(data.len() <= ENCODABLE_BYTES_PER_BLOB);

    // We get commitment and proof from an external source (advisor)
    // Correctness is checked below
    let commitment_and_proof = advisor.get_blob_commitment_and_proof_advice(data);

    let commitment = parse_g1_compressed(&commitment_and_proof.commitment)
        .expect("Invalid blob commitment point");
    let proof = parse_g1_compressed(&commitment_and_proof.proof).expect("Invalid blob proof point");
    let versioned_hash = versioned_hash_for_kzg(commitment_and_proof.commitment.as_slice());
    let evaluation_point = calculate_evaluation_point(data, &versioned_hash);
    let opening_value = polynomial_evaluation::evaluate_blob_polynomial(data, &evaluation_point);

    assert!(
        crypto::bls12_381::verify_kzg_proof(
            commitment,
            proof,
            evaluation_point.into_bigint(),
            opening_value.into_bigint(),
        ),
        "Failed to verify blob proof"
    );

    versioned_hash
}

fn calculate_evaluation_point(data: &[u8], versioned_hash: &[u8]) -> crypto::bls12_381::Fr {
    let mut hasher = crypto::blake2s::Blake2s256::new();
    hasher.update(versioned_hash);
    hasher.update(data);
    let hash = hasher.finalize();
    // truncate hash to 128 bits
    // NOTE: it is safe to draw a random scalar at max 128 bits because of the schwartz zippel lemma
    crypto::bls12_381::Fr::from_bigint(parse_u256_be(hash.rsplit_array_ref::<16>().1)).unwrap()
}
