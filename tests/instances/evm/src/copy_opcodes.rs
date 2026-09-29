//! CODECOPY and RETURNDATACOPY outcomes captured on the original interpreter.

use crate::test_support::{call_tx, new_tester};
use rig::alloy::primitives::{address, B256};
use rig::constants::DEFAULT_BALANCE;
use rig::ruint::aliases::U256;
use rig::{assert_tx_success, testing_signer};

const CODECOPY: u8 = 0x39;
const RETURNDATACOPY: u8 = 0x3e;

fn push(code: &mut Vec<u8>, value: usize) {
    if value == 0 {
        code.push(0x5f);
    } else if value <= u8::MAX as usize {
        code.extend_from_slice(&[0x60, value as u8]);
    } else {
        code.push(0x61);
        code.extend_from_slice(&(value as u16).to_be_bytes());
    }
}

fn record(code: &mut Vec<u8>, next_slot: &mut usize, dst: usize) {
    // GAS must be the first observation after the copy. Both MLOADs stay within
    // memory already touched by a nonzero copy, even at the msize boundary.
    for (opcode, offset) in [
        (0x5a, None),
        (0x59, None),
        (0x51, Some(dst)),
        (0x51, Some(dst - 1)),
    ] {
        if let Some(offset) = offset {
            push(code, offset);
        }
        code.push(opcode);
        push(code, *next_slot);
        code.push(0x55);
        *next_slot += 1;
    }
}

fn copy(code: &mut Vec<u8>, opcode: u8, dst: usize, src: usize, len: usize, slot: &mut usize) {
    push(code, len);
    push(code, src);
    push(code, dst);
    code.push(opcode);
    record(code, slot, dst);
}

fn call_helper(code: &mut Vec<u8>, address: rig::alloy::primitives::Address, input_len: usize) {
    // CALL(out_size=0, out_offset=0, in_size=input_len, in_offset=0, value=0, to, gas).
    push(code, 0);
    push(code, 0);
    push(code, input_len);
    push(code, 0);
    push(code, 0);
    code.push(0x73);
    code.extend_from_slice(address.as_slice());
    code.extend_from_slice(&[0x5a, 0xf1, 0x50]);
}

fn call_failure(code: &mut Vec<u8>, address: rig::alloy::primitives::Address, slot: &mut usize) {
    push(code, 0);
    push(code, 0);
    push(code, 0);
    push(code, 0);
    push(code, 0);
    code.push(0x73);
    code.extend_from_slice(address.as_slice());
    // Exceptional halts consume forwarded gas. Bound each probe's allowance so
    // the outer script can record its result and continue.
    push(code, 50_000);
    code.push(0xf1);
    push(code, *slot);
    code.push(0x55);
    *slot += 1;
    record(code, slot, 64);
}

fn returndata_helper() -> Vec<u8> {
    let mut code = Vec::new();
    for i in 0..4 {
        let word: Vec<u8> = (0..32).map(|j| (i * 32 + j + 1) as u8).collect();
        code.push(0x7f);
        code.extend_from_slice(&word);
        push(&mut code, i * 32);
        code.push(0x52);
    }
    code.push(0x36); // Return as many bytes as were sent as calldata.
    push(&mut code, 0);
    code.push(0xf3);
    code
}

fn main_code(
    helper: rig::alloy::primitives::Address,
    oob: rig::alloy::primitives::Address,
    underflow_code: rig::alloy::primitives::Address,
    underflow_return: rig::alloy::primitives::Address,
) -> (Vec<u8>, usize) {
    let mut code = Vec::new();
    let mut slot = 0;
    let mut source_patches = Vec::new();
    // Heap starts at 128 bytes; grid copies at 64..67 need no growth.
    push(&mut code, 0x5a);
    push(&mut code, 96);
    code.push(0x52);

    for src in 0..4 {
        for dst in 64..68 {
            push(&mut code, 32);
            code.push(0x61);
            source_patches.push((code.len(), 100 - src));
            code.extend_from_slice(&[0, 0]);
            push(&mut code, dst);
            code.push(CODECOPY);
            record(&mut code, &mut slot, dst);
        }
    }
    // Length, exact-end, nine-word-window and growth boundaries.
    for (back, dst, len) in [
        (100, 64, 0),
        (100, 64, 31),
        (100, 64, 33),
        (32, 96, 32),
        (31, 96, 32),
        (33, 96, 32),
        (34, 128, 32),
        (35, 64, 32),
        (36, 65, 32),
        (37, 64, 32),
        (0, 64, 32),
    ] {
        push(&mut code, len);
        code.push(0x61);
        source_patches.push((code.len(), back));
        code.extend_from_slice(&[0, 0]);
        push(&mut code, dst);
        code.push(CODECOPY);
        record(&mut code, &mut slot, dst);
    }
    for returned in [31, 32, 33, 64, 100] {
        call_helper(&mut code, helper, returned);
        match returned {
            31 => {
                copy(&mut code, RETURNDATACOPY, 64, 0, 31, &mut slot);
                copy(&mut code, RETURNDATACOPY, 64, 31, 0, &mut slot);
            }
            32 => copy(&mut code, RETURNDATACOPY, 96, 0, 32, &mut slot),
            33 => copy(&mut code, RETURNDATACOPY, 64, 1, 32, &mut slot),
            64 => {
                for src in 0..4 {
                    for dst in 64..68 {
                        copy(&mut code, RETURNDATACOPY, dst, src, 32, &mut slot);
                    }
                }
                copy(&mut code, RETURNDATACOPY, 128, 0, 33, &mut slot);
            }
            100 => {
                for src in [64, 65, 66, 67, 68] {
                    copy(&mut code, RETURNDATACOPY, 96, src, 32, &mut slot);
                }
            }
            _ => unreachable!(),
        }
    }
    call_failure(&mut code, oob, &mut slot);
    call_failure(&mut code, underflow_code, &mut slot);
    call_failure(&mut code, underflow_return, &mut slot);
    code.push(0x00);
    code.extend((0..100).map(|i| (i * 17 + 3) as u8));
    let code_len = code.len();
    for (position, back) in source_patches {
        code[position..position + 2].copy_from_slice(&((code_len - back) as u16).to_be_bytes());
    }
    (code, slot)
}

#[test]
fn copy_opcodes_golden() {
    let helper = address!("0000000000000000000000000000000000000c01");
    let main = address!("0000000000000000000000000000000000000c02");
    let oob = address!("0000000000000000000000000000000000000c03");
    let underflow_code = address!("0000000000000000000000000000000000000c04");
    let underflow_return = address!("0000000000000000000000000000000000000c05");
    let signer = testing_signer(0);
    let (code, slots) = main_code(helper, oob, underflow_code, underflow_return);
    let mut oob_code = Vec::new();
    call_helper(&mut oob_code, helper, 31);
    push(&mut oob_code, 32);
    push(&mut oob_code, 0);
    push(&mut oob_code, 0);
    oob_code.push(RETURNDATACOPY);
    let mut tester = new_tester()
        .with_max_tx_gas_limit(20_000_000)
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(helper, &returndata_helper())
        .with_evm_contract(oob, &oob_code)
        .with_evm_contract(underflow_code, &[CODECOPY])
        .with_evm_contract(underflow_return, &[RETURNDATACOPY])
        .with_evm_contract(main, &code);
    let output = tester.execute_block(vec![call_tx(signer, main, 16_000_000)]);
    assert_tx_success!(output, 0);
    let tx = output.tx_results[0].as_ref().unwrap();
    let values: Vec<B256> = (0..slots)
        .map(|slot| {
            output
                .storage_writes
                .iter()
                .find(|write| {
                    write.account == main
                        && write.account_key
                            == B256::from(U256::from(slot as u64).to_be_bytes::<32>())
                })
                .map(|write| write.value)
                .unwrap_or(B256::ZERO)
        })
        .collect();
    // Captured with the unchanged interpreter at a103ec63.
    let mut golden = include_str!("copy_opcodes.golden").lines();
    assert_eq!(
        format!("{} {} {slots}", tx.gas_used, tx.computational_native_used),
        golden.next().unwrap()
    );
    for (slot, value) in values.iter().enumerate() {
        assert_eq!(
            value.to_string(),
            golden
                .next()
                .unwrap_or_else(|| panic!("missing golden slot {slot}")),
            "storage slot {slot}"
        );
    }
    assert!(golden.next().is_none());
}

#[test]
fn copy_offsets_above_u32_do_not_truncate_in_guest() {
    let main = address!("0000000000000000000000000000000000000c06");
    let invalid_destination = address!("0000000000000000000000000000000000000c07");
    let signer = testing_signer(0);

    let mut code = Vec::new();
    push(&mut code, 1);
    push(&mut code, 64);
    code.push(0x52); // MSTORE grows the heap before the guarded copy.
    push(&mut code, 32);
    code.extend_from_slice(&[0x64, 1, 0, 0, 0, 0]); // source = 2^32
    push(&mut code, 64);
    code.push(CODECOPY);
    push(&mut code, 64);
    code.push(0x51); // The generic path zero-fills this word.
    push(&mut code, 0);
    code.push(0x55);
    let mut next_slot = 1;
    call_failure(&mut code, invalid_destination, &mut next_slot);
    code.push(0x00);

    let mut invalid_code = Vec::new();
    push(&mut invalid_code, 0);
    push(&mut invalid_code, 64);
    invalid_code.push(0x52);
    push(&mut invalid_code, 32);
    push(&mut invalid_code, 0);
    invalid_code.extend_from_slice(&[0x64, 1, 0, 0, 0, 64]); // destination = 2^32 + 64
    invalid_code.extend_from_slice(&[CODECOPY, 0x00]);
    invalid_code.resize(64, 0);

    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(main, &code)
        .with_evm_contract(invalid_destination, &invalid_code);
    let output = tester.execute_block(vec![call_tx(signer, main, 1_000_000)]);
    assert_tx_success!(output, 0);
    for slot in [0u64, 1] {
        let value = output
            .storage_writes
            .iter()
            .find(|write| {
                write.account == main
                    && write.account_key == B256::from(U256::from(slot).to_be_bytes::<32>())
            })
            .map(|write| write.value)
            .unwrap_or(B256::ZERO);
        assert_eq!(value, B256::ZERO, "slot {slot}");
    }
}
