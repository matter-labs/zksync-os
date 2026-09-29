//! Opcode and precompile facts, one table per fork.
//!
//! The numbers here (opcode bytes, how many stack items each opcode consumes
//! and produces, and how many immediate bytes follow it) are the ground truth
//! the builder uses to keep generated code stack valid, and the same numbers a
//! reader can replay to verify it. They are transcribed from the interpreter's
//! own dispatch table so the generator and the interpreter agree on what
//! exists.
//!
//! Two forks are modeled:
//!
//! * `Osaka` is what this repository executes today: every precompile is
//!   registered and the CLZ opcode is present.
//! * `CancunWith7702` is the older surface a downstream sequencer still runs
//!   (Cancun opcodes plus the EIP-7702 transaction type). It has no CLZ, and
//!   only the precompiles 0x01 to 0x08 plus P256 behave as real precompiles;
//!   the others respond as empty accounts there. We keep this fork so a caller
//!   can emit programs aimed at that surface even though this repository never
//!   runs it.

use serde::{Deserialize, Serialize};

/// The execution surface a program is generated for.
///
/// The fork changes two things the generator cares about: which opcodes are
/// legal (only CLZ differs between these two) and which precompile addresses
/// are backed by a real precompile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fork {
    /// The current surface of this repository.
    Osaka,
    /// Cancun opcodes plus the EIP-7702 transaction type.
    CancunWith7702,
}

// Opcode bytes. Names and values match the interpreter's opcode module so the
// two tables can be diffed against each other by hand.
pub const STOP: u8 = 0x00;
pub const ADD: u8 = 0x01;
pub const MUL: u8 = 0x02;
pub const SUB: u8 = 0x03;
pub const DIV: u8 = 0x04;
pub const SDIV: u8 = 0x05;
pub const MOD: u8 = 0x06;
pub const SMOD: u8 = 0x07;
pub const ADDMOD: u8 = 0x08;
pub const MULMOD: u8 = 0x09;
pub const EXP: u8 = 0x0a;
pub const SIGNEXTEND: u8 = 0x0b;

pub const LT: u8 = 0x10;
pub const GT: u8 = 0x11;
pub const SLT: u8 = 0x12;
pub const SGT: u8 = 0x13;
pub const EQ: u8 = 0x14;
pub const ISZERO: u8 = 0x15;
pub const AND: u8 = 0x16;
pub const OR: u8 = 0x17;
pub const XOR: u8 = 0x18;
pub const NOT: u8 = 0x19;
pub const BYTE: u8 = 0x1a;
pub const SHL: u8 = 0x1b;
pub const SHR: u8 = 0x1c;
pub const SAR: u8 = 0x1d;
/// Count leading zero bits. Present on Osaka only.
pub const CLZ: u8 = 0x1e;

pub const SHA3: u8 = 0x20;

pub const ADDRESS: u8 = 0x30;
pub const BALANCE: u8 = 0x31;
pub const ORIGIN: u8 = 0x32;
pub const CALLER: u8 = 0x33;
pub const CALLVALUE: u8 = 0x34;
pub const CALLDATALOAD: u8 = 0x35;
pub const CALLDATASIZE: u8 = 0x36;
pub const CALLDATACOPY: u8 = 0x37;
pub const CODESIZE: u8 = 0x38;
pub const CODECOPY: u8 = 0x39;
pub const GASPRICE: u8 = 0x3a;
pub const EXTCODESIZE: u8 = 0x3b;
pub const EXTCODECOPY: u8 = 0x3c;
pub const RETURNDATASIZE: u8 = 0x3d;
pub const RETURNDATACOPY: u8 = 0x3e;
pub const EXTCODEHASH: u8 = 0x3f;

pub const BLOCKHASH: u8 = 0x40;
pub const COINBASE: u8 = 0x41;
pub const TIMESTAMP: u8 = 0x42;
pub const NUMBER: u8 = 0x43;
pub const DIFFICULTY: u8 = 0x44;
pub const GASLIMIT: u8 = 0x45;
pub const CHAINID: u8 = 0x46;
pub const SELFBALANCE: u8 = 0x47;
pub const BASEFEE: u8 = 0x48;
pub const BLOBHASH: u8 = 0x49;
pub const BLOBBASEFEE: u8 = 0x4a;

pub const POP: u8 = 0x50;
pub const MLOAD: u8 = 0x51;
pub const MSTORE: u8 = 0x52;
pub const MSTORE8: u8 = 0x53;
pub const SLOAD: u8 = 0x54;
pub const SSTORE: u8 = 0x55;
pub const JUMP: u8 = 0x56;
pub const JUMPI: u8 = 0x57;
pub const PC: u8 = 0x58;
pub const MSIZE: u8 = 0x59;
pub const GAS: u8 = 0x5a;
pub const JUMPDEST: u8 = 0x5b;
pub const TLOAD: u8 = 0x5c;
pub const TSTORE: u8 = 0x5d;
pub const MCOPY: u8 = 0x5e;
pub const PUSH0: u8 = 0x5f;

// PUSH1 = 0x60 .. PUSH32 = 0x7f. PUSH2 is named explicitly because the initcode
// wrapper and the jump helpers use a two-byte push for offsets and lengths.
pub const PUSH1: u8 = 0x60;
pub const PUSH2: u8 = 0x61;
pub const PUSH32: u8 = 0x7f;

// DUP1 = 0x80 .. DUP16 = 0x8f.
pub const DUP1: u8 = 0x80;
pub const DUP16: u8 = 0x8f;

// SWAP1 = 0x90 .. SWAP16 = 0x9f.
pub const SWAP1: u8 = 0x90;
pub const SWAP16: u8 = 0x9f;

pub const LOG0: u8 = 0xa0;
pub const LOG1: u8 = 0xa1;
pub const LOG2: u8 = 0xa2;
pub const LOG3: u8 = 0xa3;
pub const LOG4: u8 = 0xa4;

pub const CREATE: u8 = 0xf0;
pub const CALL: u8 = 0xf1;
pub const CALLCODE: u8 = 0xf2;
pub const RETURN: u8 = 0xf3;
pub const DELEGATECALL: u8 = 0xf4;
pub const CREATE2: u8 = 0xf5;
pub const STATICCALL: u8 = 0xfa;
pub const REVERT: u8 = 0xfd;
pub const INVALID: u8 = 0xfe;
pub const SELFDESTRUCT: u8 = 0xff;

/// What one opcode does to the stack and how many bytes it carries inline.
///
/// `stack_in` is the number of items that must already be on the stack, and
/// `stack_out` is the number left in their place. For DUP and SWAP this uses
/// the usual accounting convention: DUP with index n reads n items and leaves
/// n+1 (a net gain of one); SWAP with index n reads and leaves n+1 (no net change).
/// Modeling them this way lets a single "pop stack_in, push stack_out" rule
/// cover every opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpSpec {
    pub byte: u8,
    pub name: &'static str,
    pub stack_in: u8,
    pub stack_out: u8,
    /// Bytes of immediate data that follow the opcode. Only the PUSH family is
    /// non-zero (PUSHn carries n bytes).
    pub immediate_bytes: u8,
}

impl OpSpec {
    const fn new(byte: u8, name: &'static str, stack_in: u8, stack_out: u8) -> Self {
        Self {
            byte,
            name,
            stack_in,
            stack_out,
            immediate_bytes: 0,
        }
    }

    const fn with_immediate(mut self, bytes: u8) -> Self {
        self.immediate_bytes = bytes;
        self
    }
}

/// Look up the stack behavior of a single opcode byte.
///
/// Returns `None` for byte values that are not opcodes on any modeled fork
/// (the interpreter treats those as INVALID). A caller that replays a byte
/// stream should stop, as execution would, when it meets such a byte outside
/// of PUSH immediate data.
pub fn lookup(byte: u8) -> Option<OpSpec> {
    // The PUSH, DUP and SWAP families are ranges, so compute them rather than
    // spelling out 64 near-identical rows.
    if (PUSH1..=PUSH32).contains(&byte) {
        let n = byte - PUSH1 + 1;
        return Some(OpSpec::new(byte, "PUSH", 0, 1).with_immediate(n));
    }
    if (DUP1..=DUP16).contains(&byte) {
        let n = byte - DUP1 + 1;
        return Some(OpSpec::new(byte, "DUP", n, n + 1));
    }
    if (SWAP1..=SWAP16).contains(&byte) {
        let n = byte - SWAP1 + 1;
        return Some(OpSpec::new(byte, "SWAP", n + 1, n + 1));
    }

    let spec = match byte {
        STOP => OpSpec::new(STOP, "STOP", 0, 0),
        ADD => OpSpec::new(ADD, "ADD", 2, 1),
        MUL => OpSpec::new(MUL, "MUL", 2, 1),
        SUB => OpSpec::new(SUB, "SUB", 2, 1),
        DIV => OpSpec::new(DIV, "DIV", 2, 1),
        SDIV => OpSpec::new(SDIV, "SDIV", 2, 1),
        MOD => OpSpec::new(MOD, "MOD", 2, 1),
        SMOD => OpSpec::new(SMOD, "SMOD", 2, 1),
        ADDMOD => OpSpec::new(ADDMOD, "ADDMOD", 3, 1),
        MULMOD => OpSpec::new(MULMOD, "MULMOD", 3, 1),
        EXP => OpSpec::new(EXP, "EXP", 2, 1),
        SIGNEXTEND => OpSpec::new(SIGNEXTEND, "SIGNEXTEND", 2, 1),
        LT => OpSpec::new(LT, "LT", 2, 1),
        GT => OpSpec::new(GT, "GT", 2, 1),
        SLT => OpSpec::new(SLT, "SLT", 2, 1),
        SGT => OpSpec::new(SGT, "SGT", 2, 1),
        EQ => OpSpec::new(EQ, "EQ", 2, 1),
        ISZERO => OpSpec::new(ISZERO, "ISZERO", 1, 1),
        AND => OpSpec::new(AND, "AND", 2, 1),
        OR => OpSpec::new(OR, "OR", 2, 1),
        XOR => OpSpec::new(XOR, "XOR", 2, 1),
        NOT => OpSpec::new(NOT, "NOT", 1, 1),
        BYTE => OpSpec::new(BYTE, "BYTE", 2, 1),
        SHL => OpSpec::new(SHL, "SHL", 2, 1),
        SHR => OpSpec::new(SHR, "SHR", 2, 1),
        SAR => OpSpec::new(SAR, "SAR", 2, 1),
        CLZ => OpSpec::new(CLZ, "CLZ", 1, 1),
        SHA3 => OpSpec::new(SHA3, "SHA3", 2, 1),
        ADDRESS => OpSpec::new(ADDRESS, "ADDRESS", 0, 1),
        BALANCE => OpSpec::new(BALANCE, "BALANCE", 1, 1),
        ORIGIN => OpSpec::new(ORIGIN, "ORIGIN", 0, 1),
        CALLER => OpSpec::new(CALLER, "CALLER", 0, 1),
        CALLVALUE => OpSpec::new(CALLVALUE, "CALLVALUE", 0, 1),
        CALLDATALOAD => OpSpec::new(CALLDATALOAD, "CALLDATALOAD", 1, 1),
        CALLDATASIZE => OpSpec::new(CALLDATASIZE, "CALLDATASIZE", 0, 1),
        CALLDATACOPY => OpSpec::new(CALLDATACOPY, "CALLDATACOPY", 3, 0),
        CODESIZE => OpSpec::new(CODESIZE, "CODESIZE", 0, 1),
        CODECOPY => OpSpec::new(CODECOPY, "CODECOPY", 3, 0),
        GASPRICE => OpSpec::new(GASPRICE, "GASPRICE", 0, 1),
        EXTCODESIZE => OpSpec::new(EXTCODESIZE, "EXTCODESIZE", 1, 1),
        EXTCODECOPY => OpSpec::new(EXTCODECOPY, "EXTCODECOPY", 4, 0),
        RETURNDATASIZE => OpSpec::new(RETURNDATASIZE, "RETURNDATASIZE", 0, 1),
        RETURNDATACOPY => OpSpec::new(RETURNDATACOPY, "RETURNDATACOPY", 3, 0),
        EXTCODEHASH => OpSpec::new(EXTCODEHASH, "EXTCODEHASH", 1, 1),
        BLOCKHASH => OpSpec::new(BLOCKHASH, "BLOCKHASH", 1, 1),
        COINBASE => OpSpec::new(COINBASE, "COINBASE", 0, 1),
        TIMESTAMP => OpSpec::new(TIMESTAMP, "TIMESTAMP", 0, 1),
        NUMBER => OpSpec::new(NUMBER, "NUMBER", 0, 1),
        DIFFICULTY => OpSpec::new(DIFFICULTY, "DIFFICULTY", 0, 1),
        GASLIMIT => OpSpec::new(GASLIMIT, "GASLIMIT", 0, 1),
        CHAINID => OpSpec::new(CHAINID, "CHAINID", 0, 1),
        SELFBALANCE => OpSpec::new(SELFBALANCE, "SELFBALANCE", 0, 1),
        BASEFEE => OpSpec::new(BASEFEE, "BASEFEE", 0, 1),
        BLOBHASH => OpSpec::new(BLOBHASH, "BLOBHASH", 1, 1),
        BLOBBASEFEE => OpSpec::new(BLOBBASEFEE, "BLOBBASEFEE", 0, 1),
        POP => OpSpec::new(POP, "POP", 1, 0),
        MLOAD => OpSpec::new(MLOAD, "MLOAD", 1, 1),
        MSTORE => OpSpec::new(MSTORE, "MSTORE", 2, 0),
        MSTORE8 => OpSpec::new(MSTORE8, "MSTORE8", 2, 0),
        SLOAD => OpSpec::new(SLOAD, "SLOAD", 1, 1),
        SSTORE => OpSpec::new(SSTORE, "SSTORE", 2, 0),
        JUMP => OpSpec::new(JUMP, "JUMP", 1, 0),
        JUMPI => OpSpec::new(JUMPI, "JUMPI", 2, 0),
        PC => OpSpec::new(PC, "PC", 0, 1),
        MSIZE => OpSpec::new(MSIZE, "MSIZE", 0, 1),
        GAS => OpSpec::new(GAS, "GAS", 0, 1),
        JUMPDEST => OpSpec::new(JUMPDEST, "JUMPDEST", 0, 0),
        TLOAD => OpSpec::new(TLOAD, "TLOAD", 1, 1),
        TSTORE => OpSpec::new(TSTORE, "TSTORE", 2, 0),
        MCOPY => OpSpec::new(MCOPY, "MCOPY", 3, 0),
        PUSH0 => OpSpec::new(PUSH0, "PUSH0", 0, 1),
        LOG0 => OpSpec::new(LOG0, "LOG0", 2, 0),
        LOG1 => OpSpec::new(LOG1, "LOG1", 3, 0),
        LOG2 => OpSpec::new(LOG2, "LOG2", 4, 0),
        LOG3 => OpSpec::new(LOG3, "LOG3", 5, 0),
        LOG4 => OpSpec::new(LOG4, "LOG4", 6, 0),
        CREATE => OpSpec::new(CREATE, "CREATE", 3, 1),
        CALL => OpSpec::new(CALL, "CALL", 7, 1),
        CALLCODE => OpSpec::new(CALLCODE, "CALLCODE", 7, 1),
        RETURN => OpSpec::new(RETURN, "RETURN", 2, 0),
        DELEGATECALL => OpSpec::new(DELEGATECALL, "DELEGATECALL", 6, 1),
        CREATE2 => OpSpec::new(CREATE2, "CREATE2", 4, 1),
        STATICCALL => OpSpec::new(STATICCALL, "STATICCALL", 6, 1),
        REVERT => OpSpec::new(REVERT, "REVERT", 2, 0),
        INVALID => OpSpec::new(INVALID, "INVALID", 0, 0),
        SELFDESTRUCT => OpSpec::new(SELFDESTRUCT, "SELFDESTRUCT", 1, 0),
        _ => return None,
    };
    Some(spec)
}

/// Whether an opcode is legal on a given fork.
///
/// Every opcode this crate knows about exists on Osaka. The only difference on
/// the older fork is that CLZ was not yet defined there, so it decodes as an
/// invalid opcode instead.
pub fn is_available(byte: u8, fork: Fork) -> bool {
    if lookup(byte).is_none() {
        return false;
    }
    match fork {
        Fork::Osaka => true,
        Fork::CancunWith7702 => byte != CLZ,
    }
}

/// A precompile: its conventional name and the low bytes of its 20 byte
/// address (all the high bytes are zero for every precompile we model).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Precompile {
    pub name: &'static str,
    pub address_low: u16,
}

// Osaka registers the full range: the nine classic precompiles, the KZG point
// evaluation precompile, the seven BLS12-381 precompiles at 0x0b to 0x11, and
// P256 verification at 0x100.
const OSAKA_PRECOMPILES: &[Precompile] = &[
    Precompile {
        name: "ecrecover",
        address_low: 0x01,
    },
    Precompile {
        name: "sha256",
        address_low: 0x02,
    },
    Precompile {
        name: "ripemd160",
        address_low: 0x03,
    },
    Precompile {
        name: "identity",
        address_low: 0x04,
    },
    Precompile {
        name: "modexp",
        address_low: 0x05,
    },
    Precompile {
        name: "ecadd",
        address_low: 0x06,
    },
    Precompile {
        name: "ecmul",
        address_low: 0x07,
    },
    Precompile {
        name: "ecpairing",
        address_low: 0x08,
    },
    Precompile {
        name: "blake2f",
        address_low: 0x09,
    },
    Precompile {
        name: "point_evaluation",
        address_low: 0x0a,
    },
    Precompile {
        name: "bls12_g1add",
        address_low: 0x0b,
    },
    Precompile {
        name: "bls12_g1msm",
        address_low: 0x0c,
    },
    Precompile {
        name: "bls12_g2add",
        address_low: 0x0d,
    },
    Precompile {
        name: "bls12_g2msm",
        address_low: 0x0e,
    },
    Precompile {
        name: "bls12_pairing_check",
        address_low: 0x0f,
    },
    Precompile {
        name: "bls12_map_fp_to_g1",
        address_low: 0x10,
    },
    Precompile {
        name: "bls12_map_fp2_to_g2",
        address_low: 0x11,
    },
    Precompile {
        name: "p256_verify",
        address_low: 0x100,
    },
];

// The older fork backs only the first eight precompiles and P256. Calls to the
// other addresses land on empty accounts there, so we do not treat them as
// precompiles for that fork.
const CANCUN_PRECOMPILES: &[Precompile] = &[
    Precompile {
        name: "ecrecover",
        address_low: 0x01,
    },
    Precompile {
        name: "sha256",
        address_low: 0x02,
    },
    Precompile {
        name: "ripemd160",
        address_low: 0x03,
    },
    Precompile {
        name: "identity",
        address_low: 0x04,
    },
    Precompile {
        name: "modexp",
        address_low: 0x05,
    },
    Precompile {
        name: "ecadd",
        address_low: 0x06,
    },
    Precompile {
        name: "ecmul",
        address_low: 0x07,
    },
    Precompile {
        name: "ecpairing",
        address_low: 0x08,
    },
    Precompile {
        name: "p256_verify",
        address_low: 0x100,
    },
];

/// The precompiles that are real precompiles on this fork.
pub fn precompiles(fork: Fork) -> &'static [Precompile] {
    match fork {
        Fork::Osaka => OSAKA_PRECOMPILES,
        Fork::CancunWith7702 => CANCUN_PRECOMPILES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_family_reports_its_immediate_width() {
        for n in 1u8..=32 {
            let byte = PUSH1 + (n - 1);
            let spec = lookup(byte).unwrap();
            assert_eq!(spec.immediate_bytes, n);
            assert_eq!(spec.stack_in, 0);
            assert_eq!(spec.stack_out, 1);
        }
    }

    #[test]
    fn dup_and_swap_accounting_matches_convention() {
        // DUP3 needs three items and leaves four; SWAP3 needs four and leaves
        // four.
        let dup3 = lookup(DUP1 + 2).unwrap();
        assert_eq!((dup3.stack_in, dup3.stack_out), (3, 4));
        let swap3 = lookup(SWAP1 + 2).unwrap();
        assert_eq!((swap3.stack_in, swap3.stack_out), (4, 4));
    }

    #[test]
    fn clz_is_osaka_only() {
        assert!(is_available(CLZ, Fork::Osaka));
        assert!(!is_available(CLZ, Fork::CancunWith7702));
    }

    #[test]
    fn undefined_bytes_have_no_spec() {
        // 0x0c is a hole in the opcode space on both forks.
        assert!(lookup(0x0c).is_none());
        assert!(!is_available(0x0c, Fork::Osaka));
    }

    #[test]
    fn osaka_has_more_precompiles_than_the_older_fork() {
        assert_eq!(precompiles(Fork::Osaka).len(), 18);
        assert_eq!(precompiles(Fork::CancunWith7702).len(), 9);
    }
}
