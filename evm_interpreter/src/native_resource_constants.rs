// Arithmetic operations
pub const ADD_NATIVE_COST: u64 = 40;
pub const MUL_NATIVE_COST: u64 = 40;
pub const SUB_NATIVE_COST: u64 = 40;
pub const DIV_NATIVE_COST: u64 = 270;
pub const SDIV_NATIVE_COST: u64 = 616;
pub const MOD_NATIVE_COST: u64 = 236;
pub const SMOD_NATIVE_COST: u64 = 321;
pub const ADDMOD_NATIVE_COST: u64 = 780;
pub const MULMOD_NATIVE_COST: u64 = 470;
pub const EXP_BASE_NATIVE_COST: u64 = 700;
pub const EXP_PER_BYTE_NATIVE_COST: u64 = 5_000;
pub const SIGNEXTEND_NATIVE_COST: u64 = 270;

// Comparison & bitwise logic
pub const LT_NATIVE_COST: u64 = 80;
pub const GT_NATIVE_COST: u64 = 80;
pub const SLT_NATIVE_COST: u64 = 80;
pub const SGT_NATIVE_COST: u64 = 80;
pub const EQ_NATIVE_COST: u64 = 60;
pub const ISZERO_NATIVE_COST: u64 = 50;
pub const AND_NATIVE_COST: u64 = 70;
pub const OR_NATIVE_COST: u64 = 70;
pub const XOR_NATIVE_COST: u64 = 70;
pub const NOT_NATIVE_COST: u64 = 60;
pub const BYTE_NATIVE_COST: u64 = 70;
pub const SHL_NATIVE_COST: u64 = 160;
pub const SHR_NATIVE_COST: u64 = 150;
pub const SAR_NATIVE_COST: u64 = 288;
pub const CLZ_NATIVE_COST: u64 = 130;

// SHA3
// Cost of the SHA3 opcode wrapper only (heap handling + dispatch); the keccak
// permutation rounds are charged inside the keccak256 system function.
pub const KECCAK256_NATIVE_COST: u64 = 120;

// Environmental
pub const ADDRESS_NATIVE_COST: u64 = 50;
pub const BALANCE_NATIVE_COST: u64 = 60;
pub const SELFBALANCE_NATIVE_COST: u64 = 711;
pub const ORIGIN_NATIVE_COST: u64 = 50;
pub const CHAINID_NATIVE_COST: u64 = 50;
pub const COINBASE_NATIVE_COST: u64 = 60;
pub const TIMESTAMP_NATIVE_COST: u64 = 50;
pub const NUMBER_NATIVE_COST: u64 = 50;
pub const DIFFICULTY_NATIVE_COST: u64 = 60;
pub const CALLER_NATIVE_COST: u64 = 50;
pub const GASLIMIT_NATIVE_COST: u64 = 60;
pub const GAS_NATIVE_COST: u64 = 50;
pub const BLOCKHASH_NATIVE_COST: u64 = 190;
pub const CALLVALUE_NATIVE_COST: u64 = 40;
pub const CALLDATALOAD_NATIVE_COST: u64 = 300;
pub const CALLDATASIZE_NATIVE_COST: u64 = 50;
pub const CALLDATACOPY_NATIVE_COST: u64 = 200;
pub const CODESIZE_NATIVE_COST: u64 = 50;
pub const CODECOPY_NATIVE_COST: u64 = 200;
pub const GASPRICE_NATIVE_COST: u64 = 60;
pub const BASEFEE_NATIVE_COST: u64 = 60;
pub const BLOBHASH_NATIVE_COST: u64 = 172;
pub const BLOBBASEFEE_NATIVE_COST: u64 = 60;
pub const EXTCODESIZE_NATIVE_COST: u64 = 60;
pub const EXTCODECOPY_NATIVE_COST: u64 = 200;
pub const EXTCODEHASH_NATIVE_COST: u64 = 60;
pub const RETURNDATASIZE_NATIVE_COST: u64 = 50;
pub const RETURNDATACOPY_NATIVE_COST: u64 = 200;

// Memory / Stack / Storage / Flow
pub const HEAP_EXPANSION_BASE_NATIVE_COST: u64 = 35;
pub const HEAP_EXPANSION_PER_BYTE_NATIVE_COST: u64 = 1;
pub const MLOAD_NATIVE_COST: u64 = 250;
pub const MSTORE_NATIVE_COST: u64 = 250;
pub const MSTORE8_NATIVE_COST: u64 = 80;
pub const MCOPY_NATIVE_COST: u64 = 200;
pub const COPY_BASE_NATIVE_COST: u64 = 80;
// Per-byte copy cost. The underlying `memcpy` is alignment-dependent on RISC-V:
// ~0.7 cyc/byte when src and dst share alignment, ~1.4+ cyc/byte on the
// misaligned byte/shift path. A single rate cannot distinguish them, so it is
// set to cover the misaligned worst case (over-charging aligned copies).
// Also feeds the L1/L2 intrinsic per-calldata-byte cost (same copy).
pub const COPY_BYTE_NATIVE_COST: u64 = 2;
pub const SLOAD_NATIVE_COST: u64 = 100;
pub const SSTORE_NATIVE_COST: u64 = 100;
pub const TLOAD_NATIVE_COST: u64 = 100;
pub const TSTORE_NATIVE_COST: u64 = 100;
pub const MSIZE_NATIVE_COST: u64 = 50;
pub const JUMP_NATIVE_COST: u64 = 60;
pub const JUMPI_NATIVE_COST: u64 = 100;
pub const PC_NATIVE_COST: u64 = 50;
pub const RETURN_NATIVE_COST: u64 = 70;
pub const REVERT_NATIVE_COST: u64 = 180;
pub const SELFDESTRUCT_NATIVE_COST: u64 = 3080;
pub const POP_NATIVE_COST: u64 = 30;
pub const JUMPDEST_NATIVE_COST: u64 = 30;
pub const CREATE_NATIVE_COST: u64 = 25_000;
pub const CREATE2_NATIVE_COST: u64 = 25_000;
pub const CALL_NATIVE_COST: u64 = 1_500;
pub const CALLCODE_NATIVE_COST: u64 = 1_500;
pub const DELEGATECALL_NATIVE_COST: u64 = 1_500;
pub const STATICCALL_NATIVE_COST: u64 = 1_500;

// Push
pub const PUSH0_NATIVE_COST: u64 = 40;
pub const PUSH1_NATIVE_COST: u64 = 50;
pub const PUSH2_NATIVE_COST: u64 = 60;
// PUSH3..=PUSH8 use the specialized `push_small` path (payload decoded as a
// single u64), so they are far cheaper than the generic PUSH9+ path below.
pub const PUSH3_NATIVE_COST: u64 = 60;
pub const PUSH4_NATIVE_COST: u64 = 60;
pub const PUSH5_NATIVE_COST: u64 = 70;
pub const PUSH6_NATIVE_COST: u64 = 70;
pub const PUSH7_NATIVE_COST: u64 = 70;
pub const PUSH8_NATIVE_COST: u64 = 70;
pub const PUSH9_NATIVE_COST: u64 = 194;
pub const PUSH10_NATIVE_COST: u64 = 201;
pub const PUSH11_NATIVE_COST: u64 = 206;
pub const PUSH12_NATIVE_COST: u64 = 204;
pub const PUSH13_NATIVE_COST: u64 = 200;
pub const PUSH14_NATIVE_COST: u64 = 200;
pub const PUSH15_NATIVE_COST: u64 = 210;
pub const PUSH16_NATIVE_COST: u64 = 210;
pub const PUSH17_NATIVE_COST: u64 = 220;
pub const PUSH18_NATIVE_COST: u64 = 236;
pub const PUSH19_NATIVE_COST: u64 = 230;
pub const PUSH20_NATIVE_COST: u64 = 220;
pub const PUSH21_NATIVE_COST: u64 = 250;
pub const PUSH22_NATIVE_COST: u64 = 240;
pub const PUSH23_NATIVE_COST: u64 = 257;
pub const PUSH24_NATIVE_COST: u64 = 240;
pub const PUSH25_NATIVE_COST: u64 = 270;
pub const PUSH26_NATIVE_COST: u64 = 280;
pub const PUSH27_NATIVE_COST: u64 = 280;
pub const PUSH28_NATIVE_COST: u64 = 287;
pub const PUSH29_NATIVE_COST: u64 = 290;
pub const PUSH30_NATIVE_COST: u64 = 300;
pub const PUSH31_NATIVE_COST: u64 = 300;
pub const PUSH32_NATIVE_COST: u64 = 290;
pub const PUSH_NATIVE_COSTS: [u64; 33] = [
    PUSH0_NATIVE_COST,
    PUSH1_NATIVE_COST,
    PUSH2_NATIVE_COST,
    PUSH3_NATIVE_COST,
    PUSH4_NATIVE_COST,
    PUSH5_NATIVE_COST,
    PUSH6_NATIVE_COST,
    PUSH7_NATIVE_COST,
    PUSH8_NATIVE_COST,
    PUSH9_NATIVE_COST,
    PUSH10_NATIVE_COST,
    PUSH11_NATIVE_COST,
    PUSH12_NATIVE_COST,
    PUSH13_NATIVE_COST,
    PUSH14_NATIVE_COST,
    PUSH15_NATIVE_COST,
    PUSH16_NATIVE_COST,
    PUSH17_NATIVE_COST,
    PUSH18_NATIVE_COST,
    PUSH19_NATIVE_COST,
    PUSH20_NATIVE_COST,
    PUSH21_NATIVE_COST,
    PUSH22_NATIVE_COST,
    PUSH23_NATIVE_COST,
    PUSH24_NATIVE_COST,
    PUSH25_NATIVE_COST,
    PUSH26_NATIVE_COST,
    PUSH27_NATIVE_COST,
    PUSH28_NATIVE_COST,
    PUSH29_NATIVE_COST,
    PUSH30_NATIVE_COST,
    PUSH31_NATIVE_COST,
    PUSH32_NATIVE_COST,
];

// Dup - same for all
pub const DUP_NATIVE_COST: u64 = 50;

// Swap - same for all
pub const SWAP_NATIVE_COST: u64 = 60;

// Log
pub const LOG_NATIVE_COST: u64 = 50;

pub const STEP_NATIVE_COST: u64 = 20;

// Cost of bytecode preprocessing per byte
pub const BYTECODE_PREPROCESSING_BYTE_NATIVE_COST: u64 = 6;
