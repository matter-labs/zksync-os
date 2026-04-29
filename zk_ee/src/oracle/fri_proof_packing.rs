//! On-the-wire packing format for `FRI_PROOF_QUERY_ID` responses.
//!
//! This module is the single source of truth for how the FRI proof
//! oracle response is laid out. Four sites participate in this format
//! and must agree bit-for-bit; keeping the spec here makes the contract
//! discoverable and the two `Vec`-based sites share code.
//!
//! # Format
//!
//! A successful FRI oracle response is a sequence of `usize` words:
//!
//! ```text
//!   [oracle_stream_len, packed_0, packed_1, ..., packed_{ceil(N/2)-1}]
//! ```
//!
//! where
//!
//! - `oracle_stream_len = N` is the total number of `u32` verifier
//!   words the Airbender unified verifier expects to read.
//! - Each `packed_i` carries **two** verifier words — the low `u32` in
//!   the lower 32 bits, the high `u32` in the upper 32 bits:
//!
//!   ```text
//!   packed_i = low_i | (high_i << 32)
//!   ```
//!
//! - When `N` is odd, the last `packed_i` carries only the low half
//!   (one valid verifier word); its high half is an unused zero
//!   padding. The verifier never reads that padding u32.
//!
//! An empty response (zero words total, i.e. `Vec::new()`) signals
//! "sidecar has no entry for this statement hash" and is distinct from
//! a response of `[0]` (which would mean "known statement, zero
//! verifier words" — not a valid FRI proof but structurally allowed by
//! the framing).
//!
//! # Why this packing
//!
//! The proving-mode CSR bridge transports each host `usize` as two
//! 32-bit reads (low half then high half). By packing two verifier
//! words into each payload `usize` on the host side, the airbender
//! unified verifier on RISC-V sees the low/high halves as
//! *consecutive* verifier `u32` words without any extra repacking.
//! That coupling is the whole reason we don't just emit one verifier
//! word per `usize`.
//!
//! # Participating sites
//!
//! - **Producer** (host): the `FriProofResponder` in `forward_system`
//!   produces the response via [`pack_fri_oracle_response`].
//! - **Consumer** (host): the bootloader in
//!   `basic_bootloader::bootloader::transaction_flow::zk::fri`
//!   unpacks the payload via [`unpack_fri_oracle_payload`].
//! - **Recorder** (host): `ReadWitnessSource` in `oracle_provider`
//!   records each transported word pair for later replay. It does not
//!   use these helpers because it operates on a streaming iterator
//!   with `.inspect()` side effects rather than a `Vec` transform; it
//!   must match the format by hand.
//! - **CSR bridge** (guest): `CsrBasedIOOracleIterator` in
//!   `proof_running_system` validates the framing via the same
//!   invariants but consumes CSR reads word-by-word in `no_std` and
//!   cannot use `alloc`. It must also match the format by hand.
//!
//! Any change to the format requires updating all four sites and the
//! round-trip test in this module.

// The packer/unpacker operate on `usize`-sized containers that carry
// two `u32` verifier words each (low | high << 32). That packing is
// only meaningful when `usize` is at least 64 bits wide — every
// participating site (the forward-mode responder, the host-side
// bootloader consumer, and the forward-mode recorder) runs on host,
// where `usize == u64`. The RISC-V guest (`target_arch = "riscv32"`,
// `usize == u32`) doesn't call these helpers at all: it consumes
// payload words directly via the CSR bridge as raw `u32` halves, not
// as packed `usize`s. So we scope the helpers and their tests to
// host builds; compiling them on the guest target would produce
// `usize` shift overflows that are dead code anyway.
#[cfg(not(target_arch = "riscv32"))]
mod host_impl {
    extern crate alloc;

    use alloc::vec::Vec;

    /// Pack an FRI verifier word stream into the oracle response shape.
    ///
    /// Returns a `Vec<usize>` in the format:
    /// `[oracle_stream_len, word_0 | (word_1 << 32), ...]`.
    ///
    /// Roundtrips with [`unpack_fri_oracle_payload`]: feeding the payload
    /// portion of this output (everything after the length prefix) into
    /// the unpacker with `verifier_word_count = stream.len()` yields the
    /// original `stream`.
    pub fn pack_fri_oracle_response(stream: &[u32]) -> Vec<usize> {
        let mut response = Vec::with_capacity(1 + stream.len().div_ceil(2));
        response.push(stream.len());
        for pair in stream.chunks(2) {
            let low = pair[0] as usize;
            let high = pair.get(1).copied().unwrap_or(0) as usize;
            response.push(low | (high << 32));
        }
        response
    }

    /// Unpack the payload portion of an FRI oracle response into the
    /// verifier word stream the Airbender unified verifier expects.
    ///
    /// `packed` must be the sequence of `packed_i` words that follow the
    /// length prefix; the caller is responsible for having already
    /// consumed the prefix and passing its value as `verifier_word_count`.
    ///
    /// Returns `None` if the payload length doesn't match
    /// `ceil(verifier_word_count / 2)`, which indicates a malformed
    /// response (structural mismatch the caller should treat as a
    /// validation failure).
    pub fn unpack_fri_oracle_payload<I>(packed: I, verifier_word_count: usize) -> Option<Vec<u32>>
    where
        I: ExactSizeIterator<Item = usize>,
    {
        if packed.len() != verifier_word_count.div_ceil(2) {
            return None;
        }

        let mut stream = Vec::with_capacity(verifier_word_count);
        for packed_words in packed {
            stream.push(packed_words as u32);
            if stream.len() < verifier_word_count {
                stream.push((packed_words >> 32) as u32);
            }
        }
        Some(stream)
    }
}

#[cfg(not(target_arch = "riscv32"))]
pub use host_impl::{pack_fri_oracle_response, unpack_fri_oracle_payload};

#[cfg(all(test, not(target_arch = "riscv32")))]
mod tests {
    use super::*;

    /// Round-trip: pack → strip length prefix → unpack → original stream.
    /// Exercised across all parities and the empty-stream edge case.
    #[test]
    fn pack_unpack_roundtrip_covers_parities_and_empty() {
        for n in [0usize, 1, 2, 3, 4, 5, 7, 8, 15, 16, 17] {
            let stream: Vec<u32> = (0..n as u32).map(|i| 0xdead0000 | i).collect();
            let packed = pack_fri_oracle_response(&stream);

            // Length prefix is always word 0.
            assert_eq!(packed[0], n, "length prefix for N={}", n);

            // Payload length is ceil(N/2).
            assert_eq!(
                packed.len() - 1,
                n.div_ceil(2),
                "payload length for N={}",
                n
            );

            let payload_iter = packed.iter().copied().skip(1);
            // We need ExactSizeIterator; use Vec::into_iter to get one.
            let payload: Vec<usize> = payload_iter.collect();
            let unpacked =
                unpack_fri_oracle_payload(payload.into_iter(), n).expect("unpack must succeed");
            assert_eq!(unpacked, stream, "round-trip mismatch for N={}", n);
        }
    }

    /// Odd-count packing zero-pads the high half of the last packed
    /// word. That zero is not recorded as a verifier word (unpack
    /// stops after `verifier_word_count` reads) so the padding never
    /// reaches the verifier as an extra word.
    #[test]
    fn odd_stream_last_packed_word_has_zero_high_half() {
        let stream: [u32; 3] = [0x1111_1111, 0x2222_2222, 0x3333_3333];
        let packed = pack_fri_oracle_response(&stream);

        assert_eq!(packed[0], 3); // length prefix
        assert_eq!(packed[1], 0x1111_1111 | (0x2222_2222usize << 32));
        // High half of the last packed word is padding zero.
        assert_eq!(packed[2], 0x3333_3333);
        assert_eq!((packed[2] >> 32) as u32, 0);

        let payload = packed[1..].to_vec();
        let unpacked = unpack_fri_oracle_payload(payload.into_iter(), 3).unwrap();
        assert_eq!(unpacked.as_slice(), stream.as_slice());
    }

    /// A payload length that doesn't match `ceil(N/2)` must be
    /// rejected. This is how the host consumer detects a malformed
    /// response without parsing it.
    #[test]
    fn unpack_rejects_length_mismatch() {
        // Claimed verifier_word_count = 4, so payload must be 2.
        let too_short: Vec<usize> = vec![0xaa];
        assert!(unpack_fri_oracle_payload(too_short.into_iter(), 4).is_none());

        let too_long: Vec<usize> = vec![0xaa, 0xbb, 0xcc];
        assert!(unpack_fri_oracle_payload(too_long.into_iter(), 4).is_none());
    }

    /// Empty-stream edge case: zero verifier words means zero payload
    /// words, and the response is just `[0]` (length prefix only).
    /// Distinct from an empty `Vec` which signals "no entry".
    #[test]
    fn empty_stream_produces_length_prefix_only() {
        let packed = pack_fri_oracle_response(&[]);
        assert_eq!(packed, vec![0]);

        let unpacked = unpack_fri_oracle_payload(Vec::<usize>::new().into_iter(), 0).unwrap();
        assert!(unpacked.is_empty());
    }
}
