//!
//! This module contains utils for pubdata compression that can be reused by different systems/storage models.
//!
use crate::system::IOResultKeeper;
use crate::types_config::SystemIOTypesConfig;
use crate::utils::write_bytes::WriteBytes;
use crate::utils::*;
use ruint::aliases::U256;

///
/// value diff "Era VM" compression, can be used for contracts storage values and account data fields(nonce and balance).
/// Works for 32 bytes values, numbers encoded/decoded as BE.
///
/// There are 4 compression types:
/// - `Nothing`, final 32 byte value.
/// - `Add`, value increased by specified 0-31 byte value.
/// - `Subtract`, value decreased by specified 0-31 byte value.
/// - `Transform`, final 0-31 byte value, leading zeroes removed.
///
#[derive(PartialEq, Eq)]
pub enum ValueDiffCompressionStrategy {
    Nothing,
    Add,
    Sub,
    Transform,
}

impl ValueDiffCompressionStrategy {
    fn compression_length(&self, initial_value: U256, final_value: U256) -> Option<u8> {
        match self {
            Self::Nothing => Some(33), //full value + metadata byte
            Self::Add => {
                let (result, of) = final_value.overflowing_sub(initial_value);
                let length = result.bit_len().div_ceil(8) as u8;
                if of || length == 32 {
                    None
                } else {
                    Some(length + 1)
                }
            }
            Self::Sub => {
                let (result, of) = initial_value.overflowing_sub(final_value);
                let length = result.bit_len().div_ceil(8) as u8;
                if of || length == 32 {
                    None
                } else {
                    Some(length + 1)
                }
            }
            Self::Transform => {
                let length = final_value.bit_len().div_ceil(8) as u8;
                if length == 32 {
                    None
                } else {
                    Some(length + 1)
                }
            }
        }
    }

    fn compress<IOTypes: SystemIOTypesConfig, T: WriteBytes + ?Sized>(
        &self,
        initial_value: U256,
        final_value: U256,
        dst: &mut T,
        result_keeper: &mut impl IOResultKeeper<IOTypes>,
    ) -> Result<(), ()> {
        match self {
            Self::Nothing => {
                let metadata_byte = 0u8;
                dst.write(&[metadata_byte]);
                dst.write(&final_value.to_be_bytes::<32>());
                result_keeper.pubdata(&[metadata_byte]);
                result_keeper.pubdata(&final_value.to_be_bytes::<32>());

                Ok(())
            }
            Self::Add => {
                let (result, of) = final_value.overflowing_sub(initial_value);
                let length = result.bit_len().div_ceil(8) as u8;

                if of || length == 32 {
                    Err(())
                } else {
                    let metadata_byte = (length << 3) | 1;
                    dst.write(&[metadata_byte]);
                    dst.write(&result.to_be_bytes::<32>()[32usize - length as usize..]);
                    result_keeper.pubdata(&[metadata_byte]);
                    result_keeper.pubdata(&result.to_be_bytes::<32>()[32usize - length as usize..]);

                    Ok(())
                }
            }
            Self::Sub => {
                let (result, of) = initial_value.overflowing_sub(final_value);
                let length = result.bit_len().div_ceil(8) as u8;

                if of || length == 32 {
                    Err(())
                } else {
                    let metadata_byte = (length << 3) | 2;
                    dst.write(&[metadata_byte]);
                    dst.write(&result.to_be_bytes::<32>()[32usize - length as usize..]);
                    result_keeper.pubdata(&[metadata_byte]);
                    result_keeper.pubdata(&result.to_be_bytes::<32>()[32usize - length as usize..]);

                    Ok(())
                }
            }
            Self::Transform => {
                let length = final_value.bit_len().div_ceil(8) as u8;
                if length == 32 {
                    Err(())
                } else {
                    let metadata_byte = (length << 3) | 3;
                    dst.write(&[metadata_byte]);
                    dst.write(&final_value.to_be_bytes::<32>()[32usize - length as usize..]);
                    result_keeper.pubdata(&[metadata_byte]);
                    result_keeper
                        .pubdata(&final_value.to_be_bytes::<32>()[32usize - length as usize..]);

                    Ok(())
                }
            }
        }
    }

    pub fn optimal_compression_length_u256(initial_value: U256, final_value: U256) -> u8 {
        Self::optimal_compression_length_u256_optional(initial_value, final_value, false)
    }

    pub fn optimal_compression_length_u256_optional(
        initial_value: U256,
        final_value: U256,
        no_compression: bool,
    ) -> u8 {
        // worst case "Nothing" strategy, always possible to encode
        let mut optimal = Self::Nothing
            .compression_length(initial_value, final_value)
            .unwrap();

        // so we don't check nothing here
        if !no_compression {
            for strategy in [Self::Add, Self::Sub, Self::Transform].iter() {
                if let Some(length) = strategy.compression_length(initial_value, final_value) {
                    optimal = core::cmp::min(optimal, length);
                }
            }
        }

        optimal
    }

    pub fn optimal_compression_length(initial_value: &Bytes32, final_value: &Bytes32) -> u8 {
        let initial_value = initial_value.into_u256_be();
        let final_value = final_value.into_u256_be();
        Self::optimal_compression_length_u256(initial_value, final_value)
    }

    pub fn optimal_compression_u256<IOTypes: SystemIOTypesConfig, T: WriteBytes + ?Sized>(
        initial_value: U256,
        final_value: U256,
        dst: &mut T,
        result_keeper: &mut impl IOResultKeeper<IOTypes>,
    ) {
        // Compute each candidate's `(payload, payload_bytes)` once. The
        // previous shape ran `compression_length` for every strategy *then*
        // recomputed the same arithmetic inside `compress`, doubling the
        // U256 subtract + `bit_len` work on the chosen strategy and burning
        // a redundant `to_be_bytes::<32>()` on top.
        //
        // Tag values match `compress`'s metadata-byte low nibble:
        //   0 = Nothing, 1 = Add, 2 = Sub, 3 = Transform.
        // "Nothing" (33 bytes total: metadata + 32-byte value) is always
        // applicable and is the initial best.
        let mut best_tag = 0u8;
        let mut best_total = 33u8;
        let mut best_payload = final_value;
        let mut best_payload_bytes = 32u8;

        // Add: final - initial (mod 2^256). Applicable iff no overflow and
        // the result fits in <32 bytes (otherwise Nothing is at least as
        // good).
        let (add_diff, add_of) = final_value.overflowing_sub(initial_value);
        if !add_of {
            let bytes = add_diff.bit_len().div_ceil(8) as u8;
            if bytes < 32 && bytes + 1 < best_total {
                best_tag = 1;
                best_total = bytes + 1;
                best_payload = add_diff;
                best_payload_bytes = bytes;
            }
        }

        // Sub: initial - final.
        let (sub_diff, sub_of) = initial_value.overflowing_sub(final_value);
        if !sub_of {
            let bytes = sub_diff.bit_len().div_ceil(8) as u8;
            if bytes < 32 && bytes + 1 < best_total {
                best_tag = 2;
                best_total = bytes + 1;
                best_payload = sub_diff;
                best_payload_bytes = bytes;
            }
        }

        // Transform: emit `final_value` truncated to its high non-zero
        // bytes (BE). Applicable iff `final_value` itself is <32 bytes.
        {
            let bytes = final_value.bit_len().div_ceil(8) as u8;
            if bytes < 32 && bytes + 1 < best_total {
                best_tag = 3;
                best_total = bytes + 1;
                best_payload = final_value;
                best_payload_bytes = bytes;
            }
        }

        // Pack metadata + payload into a single 33-byte stack buffer and
        // emit one slice each to `dst` and `result_keeper`. Previously
        // every value diff did 4 write calls (metadata, payload, ×2 sinks)
        // — for streaming sinks downstream that's a lot of per-call
        // overhead.
        let mut buf = [0u8; 33];
        let total = best_total as usize;
        if best_tag == 0 {
            // Nothing: 0x00 metadata + full 32-byte BE value.
            // SAFETY of slice math: `best_total == 33` here.
            buf[1..33].copy_from_slice(&final_value.to_be_bytes::<32>());
        } else {
            buf[0] = (best_payload_bytes << 3) | best_tag;
            let payload_bytes = best_payload.to_be_bytes::<32>();
            buf[1..total].copy_from_slice(&payload_bytes[32 - best_payload_bytes as usize..]);
        }
        dst.write(&buf[..total]);
        result_keeper.pubdata(&buf[..total]);
    }

    pub fn optimal_compression<IOTypes: SystemIOTypesConfig, T: WriteBytes + ?Sized>(
        initial_value: &Bytes32,
        final_value: &Bytes32,
        dst: &mut T,
        result_keeper: &mut impl IOResultKeeper<IOTypes>,
    ) {
        let initial_value = initial_value.into_u256_be();
        let final_value = final_value.into_u256_be();
        Self::optimal_compression_u256(initial_value, final_value, dst, result_keeper);
    }
}

#[cfg(test)]
mod tests {
    use super::ValueDiffCompressionStrategy;
    use crate::system::IOResultKeeper;
    use crate::types_config::EthereumIOTypesConfig;
    use crate::utils::*;
    use crypto::MiniDigest;

    struct TestResultKeeper {
        pub pubdata: Vec<u8>,
    }

    impl IOResultKeeper<EthereumIOTypesConfig> for TestResultKeeper {
        fn pubdata<'a>(&mut self, value: &'a [u8]) {
            self.pubdata.extend_from_slice(value)
        }
    }

    #[test]
    fn basic_compression_test() {
        let initial = Bytes32::from_array([
            0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ]);
        let r#final = Bytes32::from_array([
            0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 3,
        ]);

        let optimal_length =
            ValueDiffCompressionStrategy::optimal_compression_length(&initial, &r#final);

        let mut nop_hasher = NopHasher::new();
        let mut result_keeper = TestResultKeeper { pubdata: vec![] };

        ValueDiffCompressionStrategy::optimal_compression(
            &initial,
            &r#final,
            &mut nop_hasher,
            &mut result_keeper,
        );
        let compression = result_keeper.pubdata;

        assert_eq!(optimal_length as usize, compression.len());
        // "Addition" strategy is optimal in this case
        assert_eq!(compression.len(), 2);
        println!("{:?}", compression);
        assert_eq!(compression[0], 0b00001001);
        assert_eq!(compression[1], 3);
    }

    fn run(initial: [u8; 32], r#final: [u8; 32]) -> Vec<u8> {
        let initial = Bytes32::from_array(initial);
        let r#final = Bytes32::from_array(r#final);
        let mut nop_hasher = NopHasher::new();
        let mut rk = TestResultKeeper { pubdata: vec![] };
        ValueDiffCompressionStrategy::optimal_compression(
            &initial,
            &r#final,
            &mut nop_hasher,
            &mut rk,
        );
        let len = ValueDiffCompressionStrategy::optimal_compression_length(&initial, &r#final);
        // Reported length must match the emitted bytes — exposing this skew
        // would corrupt pubdata accounting.
        assert_eq!(rk.pubdata.len(), len as usize, "length-vs-emit mismatch");
        rk.pubdata
    }

    fn be32(low: u128) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[16..].copy_from_slice(&low.to_be_bytes());
        out
    }

    #[test]
    fn add_strategy_picked_when_diff_small() {
        // initial = 1, final = 257. Add diff = 256 (2 bytes), Sub diff = ~U256::MAX (32 bytes),
        // Transform = 2 bytes (final fits in 2 bytes). Tie between Add and Transform
        // on length=2, both encode as 3 bytes total (metadata + 2 payload). Selection
        // depends on first-applicable-with-strictly-less-length; Add is tested first
        // so it should win.
        let out = run(be32(1), be32(257));
        // metadata byte: low nibble = 1 (Add), high 5 bits = length (2)
        assert_eq!(out[0], (2u8 << 3) | 1, "metadata = (len<<3)|tag");
        assert_eq!(&out[1..], &[1u8, 0]); // 256 = 0x0100 in BE, top 2 bytes
    }

    #[test]
    fn sub_strategy_picked_when_decreasing() {
        // initial = 1000, final = 998. Add diff overflows (final < initial); Sub diff = 2;
        // Transform encodes final (998 = 0x03E6 = 2 bytes).
        // Sub wins because Sub.length=1 (payload 2) < Transform.length=2.
        let out = run(be32(1000), be32(998));
        assert_eq!(out[0], (1u8 << 3) | 2, "Sub with 1-byte payload");
        assert_eq!(&out[1..], &[2u8]);
    }

    #[test]
    fn transform_strategy_picked_when_only_one_applies() {
        // initial = U256::MAX, final = 5:
        //  - Add (final - initial) overflows on unsigned subtract (final < initial) -> not applicable
        //  - Sub (initial - final) = 2^256 - 6 -> 32-byte payload, not applicable
        //  - Transform (final itself) = 5 -> 1-byte payload, applicable
        // Only Transform fits, so it must be picked even though it isn't first.
        let max = [0xff_u8; 32];
        let out = run(max, be32(5));
        assert_eq!(out[0], (1u8 << 3) | 3, "Transform with 1-byte payload");
        assert_eq!(&out[1..], &[5u8]);
    }

    #[test]
    fn nothing_strategy_when_all_diffs_too_large() {
        // initial and final are both random-looking 32-byte values that don't compress.
        // We construct values such that:
        // - Add diff = 32 bytes (because the result has top bytes set)
        // - Sub diff = 32 bytes
        // - Transform on final = 32 bytes
        // -> Nothing is the only applicable strategy, emits 33 bytes.
        let initial = [
            0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff,
            0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff,
            0x00, 0xff, 0x00, 0xff,
        ];
        let r#final = [
            0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00,
            0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00,
            0xff, 0x00, 0xff, 0x00,
        ];
        let out = run(initial, r#final);
        assert_eq!(out.len(), 33, "Nothing strategy emits metadata + 32 bytes");
        assert_eq!(out[0], 0, "Nothing metadata = 0");
        assert_eq!(&out[1..], &r#final[..]);
    }

    #[test]
    fn no_change_uses_one_byte() {
        // initial == final → both Add/Sub diffs are 0 (length 0), Transform on final
        // depends on final. For final = 0, Transform also = 0. The metadata is then
        // 1 byte total (metadata) + 0 byte payload. Add is selected first with bytes=0.
        let out = run(be32(42), be32(42));
        assert_eq!(out.len(), 1, "zero-byte payload + 1-byte metadata");
        // Add wins (bytes=0, tag=1) -> metadata = (0<<3)|1 = 1
        assert_eq!(out[0], 1);
    }

    #[test]
    fn length_function_agrees_with_emit_across_corpus() {
        // Sweep a small grid of value pairs and assert
        // optimal_compression_length matches the byte count emitted by
        // optimal_compression. This is the contract pubdata accounting
        // depends on.
        let corpus_lo: [u128; 7] = [0, 1, 255, 256, 1 << 32, 1 << 64, u128::MAX];
        let mut hi_pattern = [0u8; 32];
        hi_pattern[0] = 0xab;
        hi_pattern[15] = 0xcd;
        let extras = [[0u8; 32], hi_pattern, [0xff; 32]];

        for &a in &corpus_lo {
            for &b in &corpus_lo {
                let _ = run(be32(a), be32(b));
            }
        }
        for &a in &extras {
            for &b in &extras {
                let _ = run(a, b);
            }
        }
    }
}
