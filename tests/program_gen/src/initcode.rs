//! Wrapping a runtime program as initcode that deploys it.
//!
//! A deployment transaction runs initcode, and whatever the initcode RETURNs
//! becomes the account's runtime code. To deploy an already-decided runtime
//! program we prepend a tiny, fixed preamble that copies the trailing runtime
//! bytes into memory and returns them.

use crate::opcodes::{CODECOPY, PUSH1, PUSH2, RETURN};

/// The preamble is always this many bytes, so the runtime always starts at this
/// offset inside the initcode. Keeping the widths fixed means the offset does
/// not depend on the runtime length.
const PREAMBLE_LEN: usize = 14;

/// Wrap a runtime program as initcode that returns it verbatim.
///
/// The preamble is the classic minimal deployer:
///
/// ```text
///   PUSH2 <len>      length of the runtime code
///   PUSH1 <offset>   where the runtime bytes start inside this initcode
///   PUSH1 0x00       destination in memory
///   CODECOPY         memory[0..len] = initcode[offset..offset+len]
///   PUSH2 <len>      length again
///   PUSH1 0x00       memory offset
///   RETURN           return memory[0..len] as the deployed code
///   <runtime bytes...>
/// ```
///
/// A two byte push holds the length because runtime code never reaches 64 KiB
/// (the interpreter caps it far below that), and a one byte push holds the
/// offset because the preamble is only 14 bytes long.
pub fn wrap_runtime_as_initcode(runtime: &[u8]) -> Vec<u8> {
    let len = runtime.len();
    // The length lives in two bytes; runtime code is bounded well under 64 KiB
    // by the caller, so this never truncates.
    let len_hi = (len >> 8) as u8;
    let len_lo = (len & 0xff) as u8;

    let mut initcode = Vec::with_capacity(PREAMBLE_LEN + len);
    initcode.extend_from_slice(&[
        PUSH2,
        len_hi,
        len_lo, // push runtime length
        PUSH1,
        PREAMBLE_LEN as u8, // push offset of the runtime bytes
        PUSH1,
        0x00,     // push memory destination
        CODECOPY, // copy the runtime into memory
        PUSH2,
        len_hi,
        len_lo, // push runtime length again
        PUSH1,
        0x00,   // push memory offset
        RETURN, // return the copied runtime as deployed code
    ]);
    debug_assert_eq!(initcode.len(), PREAMBLE_LEN);
    initcode.extend_from_slice(runtime);
    initcode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preamble_has_the_expected_shape() {
        let runtime = vec![0x00]; // a single STOP
        let initcode = wrap_runtime_as_initcode(&runtime);
        assert_eq!(initcode.len(), PREAMBLE_LEN + 1);
        // Length encoded in both push sites equals the runtime length. The first
        // push holds it at bytes 1 and 2; the second push, after CODECOPY, holds
        // it at bytes 9 and 10.
        assert_eq!(initcode[1], 0x00);
        assert_eq!(initcode[2], 0x01);
        assert_eq!(initcode[9], 0x00);
        assert_eq!(initcode[10], 0x01);
        // The runtime bytes are appended right after the preamble.
        assert_eq!(&initcode[PREAMBLE_LEN..], &runtime[..]);
    }

    #[test]
    fn empty_runtime_wraps_cleanly() {
        let initcode = wrap_runtime_as_initcode(&[]);
        assert_eq!(initcode.len(), PREAMBLE_LEN);
        assert_eq!(initcode[1], 0x00);
        assert_eq!(initcode[2], 0x00);
    }
}
