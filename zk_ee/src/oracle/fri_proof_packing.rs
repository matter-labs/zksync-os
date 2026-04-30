//! On-the-wire packing format for `FRI_PROOF_QUERY_ID` responses.
//!
//! This module is the single source of truth for how the host-side FRI
//! proof oracle response is laid out.
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
//! - **Consumer**: the FRI verification path in
//!   `basic_bootloader::bootloader::transaction_flow::zk::fri` drains
//!   the count prefix, lets the verifier consume the payload, and
//!   validates the trailing odd-word padding.
//!
//! The recorder (`oracle_provider`) and proving-mode CSR bridge
//! (`proof_running_system`) do not call these helpers, but must still
//! preserve this layout exactly.

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
}

#[cfg(not(target_arch = "riscv32"))]
pub use host_impl::pack_fri_oracle_response;

#[cfg(all(test, not(target_arch = "riscv32")))]
mod tests {
    use super::*;

    /// Odd-count packing zero-pads the high half of the last packed
    /// word.
    #[test]
    fn odd_stream_last_packed_word_has_zero_high_half() {
        let stream: [u32; 3] = [0x1111_1111, 0x2222_2222, 0x3333_3333];
        let packed = pack_fri_oracle_response(&stream);

        assert_eq!(packed[0], 3); // length prefix
        assert_eq!(packed[1], 0x1111_1111 | (0x2222_2222usize << 32));
        // High half of the last packed word is padding zero.
        assert_eq!(packed[2], 0x3333_3333);
        assert_eq!((packed[2] >> 32) as u32, 0);
    }

    /// Empty-stream edge case: zero verifier words means zero payload
    /// words, and the response is just `[0]` (length prefix only).
    #[test]
    fn empty_stream_produces_length_prefix_only() {
        let packed = pack_fri_oracle_response(&[]);
        assert_eq!(packed, vec![0]);
    }
}
