use crate::constants::*;
use crate::routines::memory::*;
use crate::routines::runtime::host::*;
use crate::routines::runtime::stack_value::*;
use crate::routines::InterpreterError;

use super::instantiate::ExecutionResult;

pub fn short_host_op(
    host: &mut TrivialHost<Vec<u8>>,
    stack_operands: &mut [StackValue],
    num_inputs: usize,
) -> Result<ExecutionResult, InterpreterError> {
    if num_inputs != 4 {
        return Err(InterpreterError::new("TODO".to_string()));
    }
    if stack_operands.len() < num_inputs {
        panic!("Interpreter failed to provide enough stack space");
    }
    let len = stack_operands.len();
    let inputs = &stack_operands[(len - num_inputs)..];
    let mut it = inputs.iter().copied();
    // ABI was checked and source was validated, so we can not fail here
    let operation = it.next().unwrap().as_i32() as u32;
    let _operation_parameter = it.next().unwrap().as_i32() as u32;
    let _op0 = it.next().unwrap().as_i32() as u32;
    let _op1 = it.next().unwrap().as_i32() as u32;
    assert!(it.next().is_none());
    // println!("Short host and IO operation = {}", operation);
    // dbg!(operation_parameter);
    // dbg!(op0);
    // dbg!(op1);
    let result = match operation {
        0x01 | 0x02 => {
            // let returndata_len = op1 as usize;
            // let mut buffer = Vec::with_capacity(returndata_len);
            // unsafe {
            //     buffer.set_len(returndata_len);
            // }
            // mem_read_into_slice(memory, &mut buffer, op0)?;
            // println!("{}", hex::encode(&buffer));

            // can save returndata and stop execution
            return Err(InterpreterError::new("??".to_string()));
        }
        0x03 => host.context.len() as u32,
        _ => return Err(InterpreterError::new("??".to_string())),
    };

    let outputs = &mut stack_operands[(len - num_inputs)..];
    outputs[0] = StackValue::new_i32(1);
    outputs[1] = StackValue::new_i32(result as i32);

    Ok(ExecutionResult::Continue)
}

pub fn long_host_op(
    host: &mut TrivialHost<Vec<u8>>,
    stack_operands: &mut [StackValue],
    num_inputs: usize,
) -> Result<ExecutionResult, InterpreterError> {
    if num_inputs != 6 {
        return Err(InterpreterError::new("Wrong number of inputs".to_string()));
    }
    if stack_operands.len() < num_inputs {
        panic!("Interpreter failed to provide enough stack space");
    }
    let len = stack_operands.len();
    let inputs = &stack_operands[(len - num_inputs)..];
    let mut it = inputs.iter().copied();
    // ABI was checked and source was validated, so we can not fail here
    let operation = it.next().unwrap().as_i32() as u32;
    let operation_parameter = it.next().unwrap().as_i32() as u32;
    let op0_offset = it.next().unwrap().as_i32() as u32 as usize;
    let op1_offset = it.next().unwrap().as_i32() as u32 as usize;
    let dst0_offset = it.next().unwrap().as_i32() as u32 as usize;
    let dst1_offset = it.next().unwrap().as_i32() as u32 as usize;
    debug_assert!(it.next().is_none());
    if op0_offset % 8 != 0 || op1_offset % 8 != 0 || dst0_offset % 8 != 0 || dst1_offset % 8 != 0 {
        return Err(InterpreterError::new("wrong offset mod".to_string()));
    }
    let (operand_0_size, operand_1_size, dst_0_size, dst_1_size) = match operation {
        5 => {
            // FullWidthMul
            if operation_parameter == 0 || operation_parameter > 32 {
                return Err(InterpreterError::new("TODO".to_string()));
            }
            let len = operation_parameter as usize;
            (len, len, len, len)
        }
        7 => {
            // Div
            if operation_parameter == 0 || operation_parameter > 32 {
                return Err(InterpreterError::new("TODO".to_string()));
            }
            let len = operation_parameter as usize;
            (len, len, len, 0)
        }
        10 => {
            // Shl
            let operand_len = (operation_parameter as u8) as u32;
            let _shift_len = operation_parameter >> 8;
            if operand_len == 0 || operand_len > 32 {
                return Err(InterpreterError::new("TODO".to_string()));
            }
            let len = operand_len as usize;
            (len, 0, len, 0)
        }
        18 => {
            // Compare
            if operation_parameter == 0 || operation_parameter > 32 {
                return Err(InterpreterError::new("TODO".to_string()));
            }
            let len = operation_parameter as usize;
            (len, len, 0, 0)
        }
        37 => {
            // calldata read u256
            (0, 0, 32, 0)
        }
        _ => {
            todo!();
        }
    };
    // check sizes
    let memory_len = host.num_heap_pages() as usize * PAGE_SIZE;
    if op0_offset + operand_0_size > memory_len
        || op1_offset + operand_1_size > memory_len
        || dst0_offset + dst_0_size > memory_len
        || dst1_offset + dst_1_size > memory_len
    {
        return Err(InterpreterError::new("TODO".to_string()));
    }
    let memory = &mut host.heap;
    let (success, return_value): (bool, u32) = match operation {
        5 => {
            let a = read_integer_repr(memory, op0_offset, operand_0_size)?;
            let b = read_integer_repr(memory, op1_offset, operand_1_size)?;
            let mut dst: [u64; 8] = [0u64; 8];
            let bit_size = operation_parameter * 8;
            full_width_mul(&a, &b, &mut dst, bit_size as usize);
            let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
            if num_top_word_bits != 0 {
                num_words += 1;
            }
            let (low, high) = dst.split_at(num_words as usize);
            let high = &high[..(num_words as usize)];
            let low = u64_slice_as_u8_slice(low);

            HostHeap::copy_into_memory(memory, low, dst0_offset as u32)?;
            let high = u64_slice_as_u8_slice(high);
            HostHeap::copy_into_memory(memory, high, dst1_offset as u32)?;

            (true, 0)
        }
        7 => {
            // Div
            let mut b = read_integer_repr(memory, op1_offset, operand_1_size)?;
            if repr_is_zero(&b) {
                (true, 0)
            } else {
                let mut a = read_integer_repr(memory, op0_offset, operand_0_size)?;
                let bit_size = operation_parameter * 8;
                let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
                if num_top_word_bits != 0 {
                    num_words += 1;
                }
                ruint::algorithms::div(
                    &mut a[..(num_words as usize)],
                    &mut b[..(num_words as usize)],
                );

                let quotient = u64_slice_as_u8_slice(&a);
                HostHeap::copy_into_memory(memory, quotient, dst0_offset as u32)?;

                (true, 0)
            }
        }
        10 => {
            let operand_len = (operation_parameter as u8) as u32;
            let shift_len = operation_parameter >> 8;
            let mut a = read_integer_repr(memory, op0_offset, operand_0_size)?;
            print_integer_repr(&a);
            let bit_size = operand_len * 8;
            let of = integer_repr_overflowing_shl(&mut a, bit_size as usize, shift_len as usize);
            print_integer_repr(&a);

            (true, of as u32)
        }
        18 => {
            let a = read_integer_repr(memory, op0_offset, operand_0_size)?;
            let b = read_integer_repr(memory, op1_offset, operand_1_size)?;
            print_integer_repr(&a);
            print_integer_repr(&b);
            let bit_size = operation_parameter * 8;
            let comparison_result = integer_repr_compare(&a, &b, bit_size as usize);

            (true, comparison_result as i32 as u32)
        }
        37 => {
            let calldata_offset = operation_parameter;
            let calldata_len = host.context.len() as u32;
            if calldata_offset >= calldata_len {
                // write zeroes
                HostHeap::fill_memory(memory, 0, dst0_offset as u32, 32)?;
                (true, 0)
            } else {
                let calldata_end = calldata_offset + 32;
                if calldata_end > calldata_len {
                    todo!()
                } else {
                    // trivial case
                    let src = &host.context[(calldata_offset as usize)..][..32];
                    // we should manage endianness here - machine is LE, but calldata is BE
                    let repr = read_repr_from_be_bytes(src);
                    let src = u64_slice_as_u8_slice(&repr);
                    HostHeap::copy_into_memory(memory, src, dst0_offset as u32)?;
                    (true, 0)
                }
            }
        }
        _ => return Err(InterpreterError::new("TODO".to_string())),
    };

    let outputs = &mut stack_operands[(len - num_inputs)..];
    outputs[0] = StackValue::new_i32(success as u32 as i32);
    outputs[1] = StackValue::new_i32(return_value as i32);

    Ok(ExecutionResult::Continue)
}

pub fn read_integer_repr(
    memory: &mut Vec<Vec<u8>>,
    offset: usize,
    len: usize,
) -> Result<[u64; 4], ()> {
    let mut repr = [0; 4];
    let dst = unsafe { core::slice::from_raw_parts_mut(repr.as_mut_ptr().cast::<u8>(), len) };
    HostHeap::mem_read_into_slice(memory, dst, offset as u32)?;

    Ok(repr)
}

pub fn integer_repr_overflowing_add(dst: &mut [u64], other: &[u64], bit_size: usize) -> bool {
    let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    if num_top_word_bits != 0 {
        num_words += 1;
    }
    let mut overflow = false;
    for (dst, src) in dst.iter_mut().zip(other.iter()).take(num_words) {
        let (t, of0) = dst.overflowing_add(*src);
        let (t, of1) = t.overflowing_add(overflow as u64);
        *dst = t;
        overflow = of0 | of1;
    }
    // if it's not the full width type then check overflow
    if num_top_word_bits != 0 {
        let top_test_mask = u64::MAX << num_top_word_bits;
        overflow |= (dst[num_words - 1] & top_test_mask) != 0;
    }

    overflow
}

pub fn integer_repr_overflowing_sub(dst: &mut [u64], other: &[u64], bit_size: usize) -> bool {
    let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    if num_top_word_bits != 0 {
        num_words += 1;
    }
    let mut overflow = false;
    for (dst, src) in dst.iter_mut().zip(other.iter()).take(num_words) {
        let (t, of1) = dst.overflowing_sub(overflow as u64);
        let (t, of0) = t.overflowing_sub(*src);
        *dst = t;
        overflow = of0 | of1;
    }

    overflow
}

pub fn integer_repr_overflowing_shl(dst: &mut [u64], bit_size: usize, shift_amount: usize) -> bool {
    let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    if num_top_word_bits != 0 {
        num_words += 1;
    }
    if shift_amount >= bit_size {
        let mut overflow = false;
        for dst in dst.iter_mut().take(num_words) {
            overflow |= *dst != 0;
            *dst = 0;
        }
        return overflow;
    }

    let mask = if num_top_word_bits == 0 {
        u64::MAX
    } else {
        u64::MAX >> (64 - num_top_word_bits)
    };

    let (shift_limbs, shift_bits) = (shift_amount / 64, shift_amount % 64);
    if shift_bits == 0 {
        // Check for overflow
        let mut overflow = false;
        for i in (num_words - shift_limbs)..num_words {
            overflow |= dst[i] != 0;
        }
        if dst[num_words - shift_limbs - 1] > mask {
            overflow = true;
        }

        // Shift
        for i in (shift_limbs..num_words).rev() {
            dst[i] = dst[i - shift_limbs];
        }
        for i in 0..shift_limbs {
            dst[i] = 0;
        }
        dst[num_words - 1] &= mask;
        return overflow;
    }

    // Check for overflow
    let mut overflow = false;
    for i in (num_words - shift_limbs)..num_words {
        overflow |= dst[i] != 0;
    }
    if dst[num_words - shift_limbs - 1] >> (64 - shift_bits) != 0 {
        overflow = true;
    }
    if dst[num_words - shift_limbs - 1] << shift_bits > mask {
        overflow = true;
    }

    // Shift
    for i in (shift_limbs + 1..num_words).rev() {
        dst[i] = dst[i - shift_limbs] << shift_bits;
        dst[i] |= dst[i - shift_limbs - 1] >> (64 - shift_bits);
    }
    dst[shift_limbs] = dst[0] << shift_bits;
    for i in 0..shift_limbs {
        dst[i] = 0;
    }
    dst[num_words - 1] &= mask;

    overflow
}

#[inline(always)]
unsafe fn assume(cond: bool) {
    if !cond {
        core::hint::unreachable_unchecked()
    }
}

pub fn integer_repr_compare(a: &[u64], b: &[u64], bit_size: usize) -> core::cmp::Ordering {
    let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    if num_top_word_bits != 0 {
        num_words += 1;
    }

    for (a, b) in a.iter().zip(b.iter()).take(num_words).rev() {
        // start from higher words
        match a.cmp(b) {
            core::cmp::Ordering::Greater => return core::cmp::Ordering::Greater,
            core::cmp::Ordering::Less => return core::cmp::Ordering::Less,
            core::cmp::Ordering::Equal => {}
        }
    }

    core::cmp::Ordering::Equal
}

pub fn full_width_mul(a: &[u64], b: &[u64], dst: &mut [u64], bit_size: usize) {
    let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    if num_top_word_bits != 0 {
        num_words += 1;
    }
    unsafe {
        assume(num_words <= a.len());
        assume(num_words <= b.len());
    }
    for (a_idx, a) in a.iter().enumerate().take(num_words) {
        let mut overflow = 0u64;
        for (b_idx, b) in b.iter().enumerate().take(num_words) {
            let dst = &mut dst[a_idx + b_idx];
            let t = (*a as u128) * (*b as u128) + (overflow as u128) * (*dst as u128); // can not overflow
            let low = t as u64;
            overflow = (t >> 64) as u64;
            *dst = low;
        }
        dst[a_idx + num_words] = overflow;
    }

    // adjust so high and low take exactly num_words
    if num_top_word_bits != 0 {
        todo!();
    }
}

fn u64_slice_as_u8_slice(src: &[u64]) -> &[u8] {
    let (ptr, len) = (src.as_ptr(), src.len());
    let len = len * 8;
    unsafe { &*core::ptr::slice_from_raw_parts(ptr.cast::<u8>(), len) }
}

fn read_repr_from_be_bytes(src: &[u8]) -> [u64; 4] {
    let mut result = [0u64; 4];
    for (chunk, dst) in src.rchunks(8).zip(result.iter_mut()) {
        let mut buffer = [0u8; 8];
        buffer.copy_from_slice(chunk);
        *dst = u64::from_be_bytes(buffer);
    }

    result
}

fn repr_is_zero(src: &[u64]) -> bool {
    src.iter().all(|el| *el == 0)
}

fn print_integer_repr(_repr: &[u64; 4]) {

    // let mut dst = String::with_capacity(2 + 4 * 16);
    // use std::fmt::Write;
    // write!(&mut dst, "0x");
    // for word in repr.iter().rev() {
    //     write!(&mut dst, "{:016x}", word);
    // }
    // println!("{}", dst);
}
