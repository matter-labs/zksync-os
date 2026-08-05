//!
//! These tests are focused on EVM execution.
//!
#![cfg(test)]

use rig::alloy::consensus::TxLegacy;
use rig::alloy_sol_types::sol;
use rig::alloy_sol_types::SolCall;
use rig::zksync_os_interface::types::ExecutionOutput;
use rig::zksync_os_interface::types::ExecutionResult;
use rig::Chain;
use rig::{
    alloy::primitives::address,
    ruint::aliases::{B160, U256},
};

#[test]
fn test_blockhash() {
    // Check that all the last 256 block hashes are accessible and the previous to that/
    // current are out of range.
    let mut chain = Chain::empty(None);

    // Code of contract used for testing
    //   contract BlockhashTester {
    //     error ChainTooShort(uint256 currentBlock);
    //     error CurrentBlockHashNonZero(bytes32 got);
    //     error OutOfRange257NonZero(bytes32 got);
    //     error Mismatch(uint256 blockNumber, bytes32 got, bytes32 expected);

    //     /// Runs all checks:
    //     /// - blockhash(current) == 0
    //     /// - blockhash(current - 257) == 0
    //     /// - for i in 1..=256: blockhash(current - i) == bytes32(current - i)
    //     /// Reverts with details on the first failure; returns true if all pass.
    //     function checkAll() external view returns (bool) {
    //         uint256 cur = block.number;
    //         // Need to be able to reference (cur - 257)
    //         if (cur < 257) revert ChainTooShort(cur);

    //         // Current block must be 0
    //         {
    //             bytes32 got = blockhash(cur);
    //             if (got != bytes32(0)) revert CurrentBlockHashNonZero(got);
    //         }

    //         // 257th previous is out of range -> 0
    //         {
    //             bytes32 got = blockhash(cur - 257);
    //             if (got != bytes32(0)) revert OutOfRange257NonZero(got);
    //         }

    //         // Previous 256 blocks must equal their block numbers
    //         unchecked {
    //             for (uint256 i = 1; i <= 256; i++) {
    //                 uint256 bn = cur - i;
    //                 bytes32 expected = bytes32(bn);
    //                 bytes32 got = blockhash(bn);
    //                 if (got != expected) revert Mismatch(bn, got, expected);
    //             }
    //         }

    //         return true;
    //     }
    // }
    let bytecode = hex::decode("608060405234801561000f575f5ffd5b5060043610610029575f3560e01c806379a77d7c1461002d575b5f5ffd5b61003561004b565b60405161004291906101d7565b60405180910390f35b5f5f43905061010181101561009757806040517f3ba2072200000000000000000000000000000000000000000000000000000000815260040161008e9190610208565b60405180910390fd5b5f814090505f5f1b81146100e257806040517fe42affce0000000000000000000000000000000000000000000000000000000081526004016100d99190610239565b60405180910390fd5b505f610101826100f2919061027f565b4090505f5f1b811461013b57806040517fa475f65e0000000000000000000000000000000000000000000000000000000081526004016101329190610239565b60405180910390fd5b505f600190505b61010081116101b4575f81830390505f815f1b90505f824090508181146101a4578281836040517f9ba8c2e500000000000000000000000000000000000000000000000000000000815260040161019b939291906102b2565b60405180910390fd5b5050508080600101915050610142565b50600191505090565b5f8115159050919050565b6101d1816101bd565b82525050565b5f6020820190506101ea5f8301846101c8565b92915050565b5f819050919050565b610202816101f0565b82525050565b5f60208201905061021b5f8301846101f9565b92915050565b5f819050919050565b61023381610221565b82525050565b5f60208201905061024c5f83018461022a565b92915050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f610289826101f0565b9150610294836101f0565b92508282039050818111156102ac576102ab610252565b5b92915050565b5f6060820190506102c55f8301866101f9565b6102d2602083018561022a565b6102df604083018461022a565b94935050505056fea2646970667358221220e82914bf0a48f1834867f2e80c5e5c1acae5c38369cb00ae1216972f7cf4936b64736f6c634300081e0033").unwrap();

    sol! { function checkAll() external view returns (bool); }

    let calldata = {
        let call = checkAllCall {};
        call.abi_encode()
    };

    let wallet = chain.random_signer();

    let to = address!("0x1000000000000000000000000000000000000000");

    // Set a reasonable balance that would be sufficient for normal transactions
    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    let tx = rig::utils::sign_and_encode_alloy_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 1000,
            gas_limit: 500_000,
            to: rig::alloy::primitives::TxKind::Call(to),
            value: Default::default(),
            input: calldata.into(),
        },
        &wallet,
    );

    // We set block number to 300, to have > 256 block hashes to query
    let block_number = 300;
    chain.set_last_block_number(block_number - 1);

    let mut block_hashes = [U256::ZERO; 256];
    for i in 0..256 {
        let n = block_number - (256 - i);
        block_hashes[i as usize] = U256::from(n);
    }
    chain.set_block_hashes(block_hashes);

    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    let result = chain.run_block(vec![tx], None, None, Some(run_config));
    assert!(result.tx_results[0].is_ok(),);

    let tx_result = result.tx_results[0].as_ref().unwrap();
    assert!(tx_result.is_success(), "Transaction should be successful");
    // check the output to ensure the contract was called
    match &tx_result.execution_result {
        ExecutionResult::Success(ExecutionOutput::Call(out)) => assert_eq!(
            out,
            &U256::ONE.to_be_bytes_vec(),
            "Output data doesn't match"
        ),
        _ => panic!("Execution result must be a successful call"),
    }
}

#[ignore = "benchmark for native constants"]
#[test]
fn bench_addmod() {
    // Minimal runtime: return addmod(calldata[0..32], calldata[32..64], calldata[64..96])
    let bytecode = hex::decode("6040356020356000350860005260206000f3").unwrap();

    // Helper: pack three U256 as 96-byte calldata (no selector).
    fn encode_3(a: U256, b: U256, m: U256) -> Vec<u8> {
        let mut v = Vec::with_capacity(96);
        v.extend_from_slice(&a.to_be_bytes_vec());
        v.extend_from_slice(&b.to_be_bytes_vec());
        v.extend_from_slice(&m.to_be_bytes_vec());
        v
    }

    // Chain setup
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let to = address!("0x1000000000000000000000000000000000000000");
    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000u64),
    );
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    // Some handy builders for U256
    let shl = |bits: u32| U256::from(1u64) << bits;
    let ones = |bits: u32| (shl(bits) - U256::from(1u64)); // ((1<<bits) - 1)
    let max = ones(256);

    // “Interesting” vectors that tickle 32-bit costs (carry storms, divider sizes, edge cases)
    // Each is (a, b, m)
    let vectors: Vec<(U256, U256, U256, &'static str)> = vec![
        // m = 0 (EVM: addmod(..., 0) == 0)
        (U256::ZERO, U256::ZERO, U256::ZERO, "m=0 all zero"),
        (U256::ZERO, U256::ONE, U256::from(2), "123"),
        (shl(255), shl(255), U256::ZERO, "m=0 with overflow add"),
        // Small modulus (eL=1) — hardest division path per-step count is largest
        (
            shl(255),
            shl(255),
            U256::from(3u64),
            "eL=1, overflow add, tiny m",
        ),
        (ones(200), ones(200), U256::from(2u64), "eL=1, dense a/b"),
        // Power-of-two moduli (nice normalization extremes)
        (shl(255), shl(255), shl(128), "m=2^128, overflow add"),
        (ones(192), ones(191), shl(64), "m=2^64, mixed widths"),
        // Near-mod reduction (no overall add overflow, single subtract)
        {
            let m = (shl(200)) + U256::from(7u64);
            (
                m - U256::from(1u64),
                U256::from(1u64),
                m,
                "no overflow, a=m-1,b=1 → 0",
            )
        },
        // Mid-sized modulus (eL=3)
        {
            let m = (shl(192)) + U256::from(12_345u64);
            (ones(192), ones(192), m, "eL=3, dense, likely overflow")
        },
        // Dense 4-limb modulus (eL=4)
        {
            let m = (shl(255)) + (shl(200)) + U256::from(1u64);
            (ones(256 - 1), U256::from(42u64), m, "eL=4, dense m")
        },
        // Random-ish but deterministic samples
        {
            let m = (shl(250)) + (shl(123)) + U256::from(999_983u64);
            (
                shl(249) + U256::from(777u64),
                shl(180) + U256::from(555u64),
                m,
                "mixed random 1",
            )
        },
        {
            let m = (shl(240)) + U256::from(1_000_003u64);
            (
                shl(239) + ones(100),
                shl(200) + U256::from(123_456u64),
                m,
                "mixed random 2",
            )
        },
        // A) Max carry storm + tiny MS limb, heavy lower limbs
        (
            shl(255) + ones(255),
            shl(255) + ones(255),
            shl(128) + U256::from(1u64),
            "eL=3, ms=1, full carry",
        ),
        (
            max,
            max,
            shl(128) + (shl(64) - U256::from(1u64)) + U256::from(1u64),
            "eL=3, ms=1, limb1 almost full, limb0=1",
        ),
        (
            max,
            max,
            shl(128) + ones(96),
            "eL=3, ms=1, lower 96 bits all 1s",
        ),
        (
            max,
            max,
            shl(128) + (ones(64) << 64) + U256::from(1u64),
            "eL=3, ms=1, limb1 full, limb0=1",
        ),
        // B) MS limb tiny but not 1 (keeps big normalization; different q̂ rounding)
        (max, max, shl(128) + U256::from(3u64), "eL=3, ms=3"),
        (max, max, shl(128) + U256::from(17u64), "eL=3, ms=17"),
        // C) MS limb small; next limb very large → provoke q̂ overestimates/corrections
        (
            max,
            max,
            shl(128) + ((ones(64)) << 64) + (U256::from(0u64)),
            "eL=3, ms=1, limb1=0xffff.., limb0=0",
        ),
        (
            max,
            max,
            shl(128) + ((ones(64)) << 64) + ones(32),
            "eL=3, ms=1, limb1=full, limb0=low ones",
        ),
        (
            max,
            max,
            shl(128) + ((ones(64)) << 64) + (shl(32) - U256::from(1u64)),
            "eL=3, ms=1, limb1=full, limb0≈2^32",
        ),
        // D) MS limb tiny + limb1 tiny but limb0 big → awkward normalization & carries
        (
            max,
            max,
            shl(128) + (U256::from(1u64) << 64) + (ones(64)),
            "eL=3, ms=1, limb1=1, limb0=full",
        ),
        (
            max,
            max,
            shl(128) + (U256::from(2u64) << 64) + (ones(64) - U256::from(1)),
            "eL=3, ms=1, limb1=2, limb0≈full",
        ),
        // E) Same structure but with a,b chosen to skew top two numerator limbs
        (
            shl(255) + ones(200),
            shl(255) + ones(180),
            shl(128) + ((ones(64)) << 64) + U256::from(1u64),
            "eL=3, skewed numerator hi limbs",
        ),
        (
            ones(256),
            shl(255) + ones(254),
            shl(128) + U256::from(1u64),
            "eL=3, mixed carry profile",
        ),
        // F) Control: your known-heavy case (keep for comparison)
        (shl(255), shl(255), shl(128), "baseline: m=2^128"),
        // eL=4 dense, tiny MS limb → big normalization, heavy limb width
        (
            max,
            max,
            (shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(3u64)),
            "eL=4, ms small, dense lower",
        ),
        // eL=2 with tiny MS limb but massive steps
        (
            max,
            max,
            (shl(64) + U256::from(1u64)),
            "eL=2, ms=1, many steps",
        ),
        // --- Keep your baseline for comparison
        (shl(255), shl(255), shl(128), "baseline: m=2^128"),
        // 1) ms limb = 1, lower limbs “hostile”
        (
            max,
            max,
            shl(128) + (ones(64) << 64) + U256::from(1u64),
            "ms=1, limb1=all 1s, limb0=1",
        ),
        (
            max,
            max,
            shl(128) + (ones(64) << 64) + ones(32),
            "ms=1, limb1=all 1s, limb0=low ones",
        ),
        (
            max,
            max,
            shl(128) + (U256::from(1u64) << 64) + ones(64),
            "ms=1, limb1=1, limb0=all 1s",
        ),
        // 2) tiny but varied ms limb (still big normalization)
        (max, max, shl(128) + U256::from(5u64), "ms=5"),
        (max, max, shl(128) + U256::from(9u64), "ms=9"),
        (max, max, shl(128) + U256::from(257u64), "ms=257"),
        (max, max, shl(128) + U256::from(65537u64), "ms=65537"),
        // 3) “triangular” shapes to perturb q̂
        (
            max,
            max,
            shl(128) + (shl(64)) + U256::from(1u64),
            "ms=1, limb1=2^64, limb0=1",
        ),
        (
            max,
            max,
            shl(128) + (shl(64)) + (shl(32) - U256::from(1u64)),
            "ms=1, limb1=2^64, limb0≈2^32",
        ),
        (
            max,
            max,
            shl(128) + (shl(63)) + ones(64),
            "ms=1<<63, limb0=all 1s",
        ),
        // 4) Skew the numerator’s top limbs (still overflow)
        (
            shl(255) + ones(200),
            shl(255) + ones(180),
            shl(128) + (ones(64) << 64) + U256::from(1u64),
            "skewed numerator hi limbs, ms=1, limb1=all 1s",
        ),
        (
            ones(256),
            shl(255) + ones(254),
            shl(128) + U256::from(1u64),
            "different carry profile, ms=1",
        ),
        // 5) Alternating patterns to maximize carry/borrow ripples
        (
            U256::from_be_bytes([0xAA; 32]),
            U256::from_be_bytes([0x55; 32]),
            shl(128) + (ones(64) << 64) + ones(64),
            "alt pattern a/b, ms=1, limb1/0=all 1s",
        ),
        // 6) “Edge” ms limb still tiny, but limb1 zero, limb0 huge
        (
            max,
            max,
            shl(128) + U256::from(0u64) + ones(64),
            "ms=1, limb1=0, limb0=all 1s",
        ),
    ];

    // Make transactions
    let txs: Vec<_> = vectors
        .iter()
        .enumerate()
        .map(|(i, (a, b, m, _tag))| {
            let input = encode_3(*a, *b, *m);
            rig::utils::sign_and_encode_alloy_tx(
                TxLegacy {
                    chain_id: 37u64.into(),
                    nonce: i as u64,
                    gas_price: 1000,
                    gas_limit: 200_000,
                    to: rig::alloy::primitives::TxKind::Call(to),
                    value: Default::default(),
                    input: input.into(),
                },
                &wallet,
            )
        })
        .collect();

    // Run them all in one block
    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    let _result = chain.run_block(txs, None, None, Some(run_config));
}

#[ignore = "benchmark for native constants"]
#[test]
fn bench_mulmod() {
    let bytecode = hex::decode("6040356020356000350960005260206000f3").unwrap();

    // Pack 3×U256 into 96-byte calldata (big-endian limbs).
    fn encode_3(a: U256, b: U256, m: U256) -> Vec<u8> {
        let mut v = Vec::with_capacity(96);
        v.extend_from_slice(&a.to_be_bytes_vec());
        v.extend_from_slice(&b.to_be_bytes_vec());
        v.extend_from_slice(&m.to_be_bytes_vec());
        v
    }

    // Helpers for vectors
    let shl = |bits: u32| U256::from(1u64) << bits;
    let ones = |bits: u32| (shl(bits) - U256::from(1u64));
    let max = ones(256);

    // Byte-pattern helpers that keep limb0 ≠ 0
    let a_01ff = U256::from_be_bytes([0x01; 32]); // all 0x01
    let b_7f = U256::from_be_bytes([0x7F; 32]); // all 0x7F
    let a_ff = U256::from_be_bytes([0xFF; 32]);
    let b_fe = U256::from_be_bytes([0xFE; 32]);
    let a_aa = U256::from_be_bytes([0xAA; 32]);
    let b_55 = U256::from_be_bytes([0x55; 32]);
    let a_11 = U256::from_be_bytes([0x11; 32]);
    let b_77 = U256::from_be_bytes([0x77; 32]);

    // Vectors designed to maximize cycles:
    // - Multiply side: a,b dense and "carry-stormy" (all limbs non-zero).
    // - Divide side: 4-limb modulus with tiny MS limb and jagged lower limbs.
    let vectors: Vec<(U256, U256, U256, &'static str)> = vec![
        // m = 0 special case
        (max, max, U256::ZERO, "m=0 → 0"),
        (U256::ZERO, U256::ONE, U256::from(2), "123"),
        // Dense 4-limb moduli with tiny MS limb (max normalization); a,b dense
        (max, max, shl(192), "eL=4, m=2^192"),
        (max, max, shl(192) + U256::from(1u64), "eL=4, ms=1"),
        (max, max, shl(192) + U256::from(3u64), "eL=4, ms=3"),
        // Hostile lower limbs to trigger corrections / long borrows
        (
            max,
            max,
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(1u64),
            "eL=4, ms=1, limb2/1 all 1s, limb0=1",
        ),
        (
            max,
            max,
            shl(192) + (ones(64) << 128) + U256::from(1u64),
            "eL=4, ms=1, limb2 all 1s, limb1=0, limb0=1",
        ),
        (
            max,
            max,
            shl(192) + (shl(63) << 128) + ones(64),
            "eL=4, ms=1<<63, limb1 all 1s",
        ),
        (
            max,
            max,
            shl(192) + (shl(64)) + (shl(32) - U256::from(1u64)),
            "eL=4, ms=1, limb2=2^64, limb1≈2^32",
        ),
        // Alternating bytes to maximize addmul carry waves
        (
            U256::from_be_bytes([0xFF; 32]),
            U256::from_be_bytes([0xFE; 32]),
            shl(192) + U256::from(1u64),
            "a=FF.., b=FE.., ms=1",
        ),
        (
            U256::from_be_bytes([0xAA; 32]),
            U256::from_be_bytes([0x55; 32]),
            shl(192) + (ones(64) << 128) + ones(64),
            "a/b alternating, limb2/0 all 1s",
        ),
        (
            ones(256),
            ones(256) - U256::from(1u64),
            shl(192) + U256::from(9u64),
            "max*(max-1), ms=9",
        ),
        // Skew the numerator’s hi limbs to perturb qhat repeatedly
        (
            shl(255) + ones(200),
            shl(255) + ones(180),
            shl(192) + (ones(64) << 128) + U256::from(1u64),
            "skewed hi limbs, ms=1, limb2 all 1s",
        ),
        (
            shl(255) + ones(191),
            shl(254) + ones(193),
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(3u64),
            "skewed hi limbs, dense lower, ms=3",
        ),
        // A couple of controls (should be cheaper; useful for comparison plots)
        (max, max, shl(128), "control: eL=3"),
        (
            ones(128),
            ones(128),
            shl(192) + U256::from(1u64),
            "control: a,b only 2 limbs non-zero",
        ),
        // ——— ms=1 with “hostile” mid/low limbs ———
        (
            max,
            max,
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(0xFFFF_FFFF_FFFF_FFFDu64),
            "ms=1, limb2=all1, limb1=all1, limb0=odd-FFFF...FFFD",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + (ones(64) << 128) + (shl(63) << 64) + U256::from(1u64),
            "ms=1, limb2=all1, limb1≈2^63, limb0=1",
        ),
        (
            a_aa,
            b_55,
            shl(192) + (shl(63) << 128) + (ones(64) << 64) + U256::from(3u64),
            "ms≈2^63, limb1=all1, limb0=3",
        ),
        (
            max,
            max,
            shl(192) + (ones(64) << 128) + U256::from(0u64) + (ones(64)),
            "ms=1, limb2=all1, limb1=0, limb0=all1",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + (U256::from(2u64) << 128) + (ones(64) << 64) + U256::from(1u64),
            "ms=1, limb2=2, limb1=all1, limb0=1",
        ),
        (
            max,
            max,
            shl(192) + (U256::from(9u64) << 128) + (shl(63) << 64) + U256::from(0xFFFFu64),
            "ms=1, limb2=9, limb1≈2^63, limb0=0xFFFF",
        ),
        // ——— weird small ms values that often skew q̂ ———
        (max, max, shl(192) + U256::from(5u64), "ms=5 (tiny ms)"),
        (a_ff, b_fe, shl(192) + U256::from(9u64), "ms=9 (tiny ms)"),
        (max, max, shl(192) + U256::from(257u64), "ms=257 (tiny ms)"),
        (
            max,
            max,
            shl(192) + U256::from(65537u64),
            "ms=65537 (tiny ms)",
        ),
        // ——— triangular / jagged moduli ———
        (
            max,
            max,
            shl(192) + (shl(64)) + (shl(32) - U256::from(1u64)),
            "ms=1, limb2=2^64, limb1≈2^32",
        ),
        (
            a_01ff,
            b_7f,
            shl(192) + (shl(64)) + ones(64),
            "ms=1, limb2=2^64, limb1=all1",
        ),
        (
            a_aa,
            b_55,
            shl(192) + (shl(63) << 128) + (shl(32)) + U256::from(1u64),
            "ms≈2^63, limb1≈2^32, limb0=1",
        ),
        // ——— skew the product’s top limbs (still dense) ———
        (
            shl(255) + ones(220),
            shl(255) + ones(219),
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(1u64),
            "skewed hi limbs, ms=1, limb2/1 all1",
        ),
        (
            shl(255) + ones(191),
            shl(254) + ones(193),
            shl(192) + (ones(64) << 128) + (U256::from(1u64) << 64) + U256::from(3u64),
            "skewed hi, limb2 all1, limb1=1, limb0=3",
        ),
        // ——— alternating A/B with odd limb0 (avoid even gcd artifacts) ———
        (
            a_ff,
            b_fe,
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(0xFFFF_FFFF_FFFF_FFFFu64),
            "ms=1, limb2/1 all1, limb0=all1",
        ),
        (
            a_aa,
            b_55,
            shl(192)
                + (U256::from(3u64) << 128)
                + (ones(64) << 64)
                + U256::from(0xDEAD_BEEF_DEAD_BEEFu64),
            "ms=1, limb2=3, limb1 all1, limb0=DEAD..",
        ),
        // ——— extreme carry chains in addmul with “spiky” modulus ———
        (
            max,
            ones(256) - U256::from(1u64),
            shl(192) + (ones(64) << 128) + (shl(63) << 64) + U256::from(0x1u64),
            "max*(max-1), ms=1, limb1≈2^63",
        ),
        (
            a_01ff,
            max,
            shl(192)
                + (U256::from(1u64) << 128)
                + (ones(64))
                + U256::from(0xFFFF_FFFF_FFFF_FFFDu64),
            "a=01.., b=max, limb2=1, limb1=all1, limb0 odd-FFFF...FFFD",
        ),
        // A) ms=1, lower limbs “all ones” variants with odd limb0 (maximize borrow chains)
        (
            max,
            max,
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(0xFFFF_FFFF_FFFF_FFFDu64),
            "ms=1, limb2/1=all1, limb0=FFFF...FFFD",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(0xFFFF_FFFF_FFFF_FFFFu64),
            "ms=1, limb2/1=all1, limb0=all1",
        ),
        (
            a_aa,
            b_55,
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(0xDEAD_BEEF_DEAD_BEEFu64),
            "ms=1, limb2/1=all1, limb0=DEAD..",
        ),
        // B) ms tiny but not 1; limb2/limb1 jagged to skew q̂
        (
            max,
            max,
            shl(192) + U256::from(3u64) + (ones(64) << 128) + (shl(63) << 64),
            "ms=3, limb2=all1, limb1≈2^63",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + U256::from(5u64) + (shl(63) << 128) + ones(64),
            "ms=5, limb2≈2^63, limb1=all1",
        ),
        (
            max,
            max,
            shl(192) + U256::from(257u64) + ((ones(64) - U256::from(1)) << 128) + (ones(64) << 64),
            "ms=257, limb2≈all1-1, limb1=all1",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + U256::from(65537u64) + (shl(62) << 128) + (shl(32) - U256::from(1)),
            "ms=65537, limb2≈2^62, limb1≈2^32-1",
        ),
        // C) “Triangular” moduli: big limb2, small limb1, odd limb0
        (
            max,
            max,
            shl(192) + (shl(64) << 128) + U256::from(1u64) + (ones(64)),
            "ms=1, limb2=2^64, limb1=1, limb0=all1",
        ),
        (
            a_11,
            b_77,
            shl(192)
                + (shl(63) << 128)
                + U256::from(0u64)
                + (U256::from(1u64) << 40)
                + U256::from(1u64),
            "ms≈2^63, limb1=0, limb0=2^40+1",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + (U256::from(2u64) << 128) + (ones(64)) + U256::from(1u64),
            "ms=1, limb2=2, limb1=all1, limb0=1",
        ),
        // D) Skew product’s top limbs to invite 2× corrections after normalization
        (
            shl(255) + ones(220),
            shl(255) + ones(219),
            shl(192) + (ones(64) << 128) + (ones(64) << 64) + U256::from(1u64),
            "skewed hi, limb2/1=all1, ms=1",
        ),
        (
            shl(255) + ones(191),
            shl(254) + ones(193),
            shl(192) + (ones(64) << 128) + (U256::from(1u64) << 64) + U256::from(3u64),
            "skewed hi, limb2=all1, limb1=1, limb0=3",
        ),
        // E) Carry-stormy a,b with odd/even interplay; spiky modulus
        (
            max,
            ones(256) - U256::from(1u64),
            shl(192) + (ones(64) << 128) + (shl(63) << 64) + U256::from(1u64),
            "max*(max-1), ms=1, limb1≈2^63",
        ),
        (
            a_ff,
            b_fe,
            shl(192)
                + ((ones(64) - U256::from(1)) << 128)
                + ((ones(64) - U256::from(1)) << 64)
                + U256::from(0xFFFFu64),
            "ms=1, limb2/1≈all1-1, limb0=FFFF",
        ),
        // F) “Hole” in mid limb to force long borrows
        (
            max,
            max,
            shl(192) + (ones(64) << 128) + U256::from(0u64) + (ones(64) - U256::from(1)),
            "ms=1, limb2=all1, limb1=0, limb0≈all1-1",
        ),
        (
            a_aa,
            b_55,
            shl(192) + (U256::from(1u64) << 128) + U256::from(0u64) + (ones(64)),
            "ms=1, limb2=1, limb1=0, limb0=all1",
        ),
        // G) Power-of-two “almost” with noisy low limbs (keeps normalization big)
        (
            max,
            max,
            shl(192) + (shl(63) << 128) + (shl(63) << 64) + U256::from(1u64),
            "ms≈2^63, limb2≈2^63, limb1=1",
        ),
        (
            a_ff,
            b_fe,
            shl(192) + (shl(1) << 128) + (ones(64) << 64) + U256::from(0x8000_0000_0000_0001u64),
            "ms=2, limb2=2, limb1=all1, limb0=odd hi bit",
        ),
    ];

    // --- Chain setup
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let to = address!("0x1000000000000000000000000000000000000000");

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000u64),
    );
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    // Build txs with unique nonces
    let txs: Vec<_> = vectors
        .iter()
        .enumerate()
        .map(|(i, (a, b, m, _tag))| {
            let input = encode_3(*a, *b, *m);
            rig::utils::sign_and_encode_alloy_tx(
                TxLegacy {
                    chain_id: 37u64.into(),
                    nonce: i as u64,
                    gas_price: 1000,
                    gas_limit: 300_000,
                    to: rig::alloy::primitives::TxKind::Call(to),
                    value: Default::default(),
                    input: input.into(),
                },
                &wallet,
            )
        })
        .collect();

    // Run all calls in one block
    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    let _result = chain.run_block(txs, None, None, Some(run_config));
}

#[test]
fn bench_signextend() {
    // ------------------------------------------------------------
    // Minimal runtime: return signextend(k, x) for calldata = k|x (big-endian)
    //
    // 6020 35   // x := calldataload(0x20)
    // 6000 35   // k := calldataload(0x00)
    // 0b        // SIGNEXTEND(k, x)
    // 6000 52   // mstore(0, result)
    // 6020 6000 f3 // return(0, 32)
    // ------------------------------------------------------------
    let bytecode = hex::decode("6020356000350b60005260206000f3").unwrap();

    // Pack 2×U256 into 64-byte calldata (k then x), big-endian words.
    fn encode_2(k: U256, x: U256) -> Vec<u8> {
        let mut v = Vec::with_capacity(64);
        v.extend_from_slice(&k.to_be_bytes_vec());
        v.extend_from_slice(&x.to_be_bytes_vec());
        v
    }

    // Helpers
    let shl = |b: u32| U256::from(1u64) << b;
    let at_byte = |byte_index: u32, val: u64| -> U256 { U256::from(val) << (8 * byte_index) };

    // Craft vectors: (k, x, tag)
    // We hit boundaries, large k, and cases with noisy higher bits.
    let vectors: Vec<(U256, U256, &'static str)> = vec![
        // k >= 32: no change
        (
            U256::from(32u64),
            U256::from(0x1234_5678u64),
            "k=32 → unchanged",
        ),
        (
            U256::from(255u64),
            (shl(255) | U256::from(0xAAu64)),
            "k=255 → unchanged",
        ),
        // k = 0 (sign bit at bit 7)
        (U256::from(0u64), at_byte(0, 0x80), "k=0, x=0x80 → negative"),
        (U256::from(0u64), at_byte(0, 0x7F), "k=0, x=0x7F → positive"),
        // with noisy high bits (should be zeroed/filled appropriately)
        (
            U256::from(0u64),
            (shl(200) | at_byte(0, 0x80)),
            "k=0, x has hi bits, negative",
        ),
        (
            U256::from(0u64),
            (shl(200) | at_byte(0, 0x7F)),
            "k=0, x has hi bits, positive",
        ),
        // k = 1 (sign bit at bit 15)
        (
            U256::from(1u64),
            at_byte(1, 0x80),
            "k=1, x=0x80<<8 → negative",
        ),
        (
            U256::from(1u64),
            at_byte(1, 0x7F) | at_byte(0, 0xFF),
            "k=1, positive with low noise",
        ),
        // k = 2 (sign bit at bit 23)
        (
            U256::from(2u64),
            at_byte(2, 0x80) | at_byte(0, 0xAA),
            "k=2, negative w/ noise",
        ),
        (
            U256::from(2u64),
            at_byte(2, 0x7F) | at_byte(1, 0xFF),
            "k=2, positive w/ noise",
        ),
        // k = 31 (highest meaningful; sign at bit 255)
        (U256::from(31u64), shl(255), "k=31, top sign bit set"),
        (U256::from(31u64), shl(254), "k=31, top sign bit clear"),
        // with extra lower bits present
        (
            U256::from(31u64),
            (shl(255) | at_byte(30, 0xFF) | at_byte(0, 0x01)),
            "k=31, negative w/ lows",
        ),
        (
            U256::from(31u64),
            (shl(254) | at_byte(30, 0xFF) | at_byte(0, 0x01)),
            "k=31, positive w/ lows",
        ),
    ];

    // --- Chain setup + deploy
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();
    let to = address!("0x1000000000000000000000000000000000000000");

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000u64),
    );
    chain.set_evm_bytecode(B160::from_be_bytes(to.into_array()), &bytecode);

    // Build signed txs (unique nonces)
    let txs: Vec<_> = vectors
        .iter()
        .enumerate()
        .map(|(i, (k, x, _tag))| {
            let input = encode_2(*k, *x);
            rig::utils::sign_and_encode_alloy_tx(
                TxLegacy {
                    chain_id: 37u64.into(),
                    nonce: i as u64,
                    gas_price: 1000,
                    gas_limit: 80_000,
                    to: rig::alloy::primitives::TxKind::Call(to),
                    value: Default::default(),
                    input: input.into(),
                },
                &wallet,
            )
        })
        .collect();

    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    let _result = chain.run_block(txs, None, None, Some(run_config));
}
