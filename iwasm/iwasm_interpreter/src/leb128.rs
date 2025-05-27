// We want only limited support of encoding/decoding
pub struct LEB128;

impl LEB128 {
    const MAX_BYTES_FOR_U64: usize = 10;
    const MAX_BYTES_FOR_U32: usize = 5;
    const MAX_BYTES_FOR_S33: usize = 5;
    const MAX_BYTES_FOR_S32: usize = 5;
    const MAX_BYTES_FOR_S64: usize = 10;

    pub fn consume_decode_signed(
        src: &[u8],
        size: u32,
        max_encoding_size: usize,
    ) -> Result<(i64, usize), usize> {
        let mut result = 0i64;
        let mut shift = 0u32;

        let mut done = false;
        let mut consumed = 0;
        let mut last_byte = 0u8;
        for b in src.iter().take(max_encoding_size).copied() {
            consumed += 1;
            last_byte = b;
            let body = (b & 0x7f) as i64;
            result |= body << shift;
            shift += 7;
            if b & 0x80 == 0 {
                done = true;
                break;
            }
        }

        if shift < size && last_byte & 0x40 != 0 {
            result |= -1i64 << shift;
        }

        if done {
            Ok((result, consumed))
        } else {
            Err(consumed)
        }
    }

    pub fn consume_decode_s33(src: &[u8]) -> Result<(i64, usize), usize> {
        Self::consume_decode_signed(src, 33, Self::MAX_BYTES_FOR_S33)
    }

    pub fn consume_decode_s32(src: &[u8]) -> Result<(i32, usize), usize> {
        Self::consume_decode_signed(src, 32, Self::MAX_BYTES_FOR_S32).map(|el| (el.0 as i32, el.1))
    }

    pub fn consume_decode_s64(src: &[u8]) -> Result<(i64, usize), usize> {
        Self::consume_decode_signed(src, 64, Self::MAX_BYTES_FOR_S64)
    }

    pub fn consume_decode_u64(src: &[u8]) -> Result<(u64, usize), usize> {
        let mut result = 0u64;
        let mut done = false;
        let mut shift = 0u32;
        let mut consumed = 0;
        for b in src.iter().take(Self::MAX_BYTES_FOR_U64).copied() {
            consumed += 1;
            let body = (b & !0x80) as u64;
            result += body << shift;
            if b & 0x80 == 0 {
                done = true;
                break;
            } else {
                shift += 7;
            }
        }

        if done {
            Ok((result, consumed))
        } else {
            Err(consumed)
        }
    }

    pub fn consume_decode_u32(src: &[u8]) -> Result<(u32, usize), usize> {
        let mut result = 0u32;
        let mut done = false;
        let mut shift = 0u32;
        let mut consumed = 0;
        for b in src.iter().take(Self::MAX_BYTES_FOR_U32).copied() {
            consumed += 1;
            let body = (b & !0x80) as u32;
            result += body << shift;
            if b & 0x80 == 0 {
                done = true;
                break;
            } else {
                shift += 7;
            }
        }

        if done {
            Ok((result, consumed))
        } else {
            Err(consumed)
        }
    }
}
