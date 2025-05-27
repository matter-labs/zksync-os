use crate::{dev_format, host::*};

use iwasm_specification::host_ops::*;

use iwasm_specification::intx::U256Repr;
use iwasm_specification::sys::HostOpResult;

use iwasm_interpreter::routines::runtime::instantiate::ExecutionResult;
use iwasm_interpreter::routines::runtime::stack_value::StackValue;
use iwasm_interpreter::routines::InterpreterError;
use zk_ee::system::kv_markers::ColdWarmAccessBitmask;
use zk_ee::system::memory::ArrayBuilder;
use zk_ee::system_trait::execution_environment::{
    get_pieces_of_slice, range_for_at, MemoryRegion, MemoryRegionDescription, MemoryRegionType,
};
use zk_ee::system_trait::{EthereumLikeSystem, Regions, SystemFunctions};
use zk_ee::utils::convenience::{storage_read, storage_write};

struct ShortOpParams {
    result_addr: u32, // WASM C ABI struct result.
    operation: ShortHostOp,
    param: u64,
    op1: u32,
    op2: u32,
}

impl ShortOpParams {
    fn init(
        stack_operands: &mut [StackValue],
        num_inputs: usize,
    ) -> Result<Self, InterpreterError> {
        use iwasm_specification::num_traits::FromPrimitive;

        if num_inputs != 5 {
            return Err(dev_format!(
                "Unexpected number of arguments in short_host_op. Expected 5, got {}",
                num_inputs
            )
            .into());
        }
        if stack_operands.len() < num_inputs {
            panic!("Interpreter failed to provide enough stack space");
        }

        let len = stack_operands.len();
        let inputs = &mut stack_operands[(len - num_inputs)..].iter().copied();

        let result_addr = inputs.next().unwrap().as_i32();

        let operation = inputs.next().unwrap().as_i32();

        let r = Self {
            // ABI was checked and source was validated, so we can not fail here
            result_addr: result_addr as u32,
            operation: FromPrimitive::from_i32(operation).ok_or_else(|| {
                dev_format!("Unexpected ShortHostOp value provided {}.", operation)
            })?,
            param: inputs.next().unwrap().as_i64() as u64,
            op1: inputs.next().unwrap().as_i32() as u32,
            op2: inputs.next().unwrap().as_i32() as u32,
        };
        assert!(inputs.next().is_none());

        Ok(r)
    }
}

#[allow(dead_code)]
struct LongOpParams {
    result_addr: u32, // WASM C ABI struct result.
    operation: LongHostOp,
    param: u64,
    op1: u32,
    op2: u32,
    dst1: u32,
    dst2: u32,
}

impl LongOpParams {
    fn init(
        stack_operands: &'_ mut [StackValue],
        num_inputs: usize,
    ) -> Result<Self, InterpreterError> {
        use iwasm_specification::num_traits::FromPrimitive;

        if num_inputs != 7 {
            return Err(dev_format!(
                "Unexpected number of arguments in short_host_op. Expected 7, got {}",
                num_inputs
            )
            .into());
        }

        if stack_operands.len() < num_inputs {
            panic!("Interpreter failed to provide enough stack space");
        }

        let len = stack_operands.len();
        let inputs = &mut stack_operands[(len - num_inputs)..].iter().copied();

        // ABI was checked and source was validated, so we can not fail here
        let result_addr = inputs.next().unwrap().as_i32() as u32;
        let operation = inputs.next().unwrap().as_i32();
        let operation_parameter = inputs.next().unwrap().as_i64() as u64;
        let op1_offset = inputs.next().unwrap().as_i32() as u32;
        let op2_offset = inputs.next().unwrap().as_i32() as u32;
        let dst1_offset = inputs.next().unwrap().as_i32() as u32;
        let dst2_offset = inputs.next().unwrap().as_i32() as u32;

        let r = Self {
            result_addr,
            operation: FromPrimitive::from_i32(operation)
                .ok_or_else(|| dev_format!("Unexpected LongHostOp value provided {}", operation))?,
            param: operation_parameter,
            op1: op1_offset,
            op2: op2_offset,
            dst1: dst1_offset,
            dst2: dst2_offset,
        };
        assert!(inputs.next().is_none());

        Ok(r)
    }
}

pub fn short_host_op<S: EthereumLikeSystem>(
    host: &'_ mut ZkOSHost<'_, S>,
    stack_operands: &'_ mut [StackValue],
    num_inputs: usize,
) -> Result<ExecutionResult, InterpreterError> {
    let params = ShortOpParams::init(stack_operands, num_inputs)?;

    #[cfg(feature = "testing")]
    {
        print!(
            "\x1b[38;5;214mHost:\x1b[0m call received: {:?}",
            params.operation
        );
    }
    let (success, result) = match params.operation {
        ShortHostOp::Revert => {
            let _str_offset = params.op1 as usize;
            let _str_len = params.op2 as usize;

            let _memory = host.system.get_memory_region(MemoryRegionType::Shared);

            #[cfg(feature = "testing")]
            {
                let str = unsafe {
                    std::str::from_raw_parts(_memory.as_ptr().add(_str_offset), _str_len)
                };

                println!();
                println!("--------------------------------------------------------------------------------");
                println!("{}", str);
                println!("--------------------------------------------------------------------------------");
            }

            return Ok(ExecutionResult::Reverted);
        }
        a @ ShortHostOp::ReturnOk => {
            let returndata_offset = params.op1 as usize;
            let returndata_len = params.op2 as usize;
            debug_assert_eq!(host.returndata_region.region_type, MemoryRegionType::Shared);
            host.returndata_region.description = MemoryRegionDescription {
                offset: returndata_offset,
                len: returndata_len,
            };

            #[cfg(feature = "testing")]
            {
                let returndata = host.system.get_memory_region_range(host.returndata_region);
                dbg!(hex::encode(returndata));
            }

            // can save returndata and stop execution
            let result = if a == ShortHostOp::ReturnOk {
                ExecutionResult::Return
            } else {
                ExecutionResult::Reverted
            };
            return Ok(result);
        }
        ShortHostOp::CalldataSelector => {
            let cd = &host.system.get_memory_region_range(host.context.calldata)[..4];

            // We're using it as is - the library expects the number with bytes flipped.
            // Safety: Guaranteed to be 4 initialized bytes due to previous line.
            let s = unsafe { *(cd.as_ptr() as *const _ as *const u32) };

            (true, s)
        }
        ShortHostOp::CalldataSize => {
            let calldata_len = host.context.calldata.len() - 4; // We don't need the selector.

            #[cfg(feature = "testing")]
            {
                print!(" -> 0x{:04x?}", calldata_len);
            }

            if calldata_len % 32 != 0 {
                (false, calldata_len as u32)
            } else {
                (true, calldata_len as u32)
            }
        }
        ShortHostOp::CalldataReadInto => {
            let dst_offset = params.op1 as usize;

            if dst_offset % 8 != 0 {
                return Err(
                    dev_format!("CalldataReadInto destination offset is not aligned.").into(),
                );
            }

            let cd_len = host.context.calldata.len() - 4; // We don't need the selector.
            let cd_offset = host.context.calldata.offset() + 4; // We don't need the selector.

            let Regions { heap, calldata, .. } = host.system.get_memory_regions();

            #[cfg(feature = "testing")]
            {
                print!(" -> Copying 0x{:0x?} calldata bytes", cd_len);
                // print!(" -> {:0x?}", &m_cd[cd_offset .. cd_offset + cd_len]);
            }

            let dst = &mut heap[dst_offset..dst_offset + cd_len];

            dst.copy_from_slice(&calldata[cd_offset..cd_offset + cd_len]);

            (true, 0)
        }
        ShortHostOp::MessageData => 'arm: {
            if params.param != 1 {
                if cfg!(feature = "testing") {
                    panic!("Unexpected param {}", params.param);
                }

                break 'arm (false, 1);
            }

            let dst_offset = params.op1 as usize;

            if dst_offset % 8 != 0 {
                return Err(dev_format!("Destination offset is not aligned.").into());
            }

            let dst = unsafe {
                &mut *host
                    .system
                    .get_memory_region_range_mut(MemoryRegion::shared_for_at::<U256Repr>(
                        dst_offset,
                    )?)
                    .as_mut_ptr()
                    .cast::<U256Repr>()
            };

            let src_limbs = host.context.caller.as_limbs();

            const { assert!(core::mem::size_of::<U256Repr>() == 32) }
            let mut iter = dst.as_u64_le_lsb_limbs_mut().into_iter();

            for i in src_limbs.iter().take(3) {
                // Safety: there are 4 limbs, we're using 3 here.
                unsafe { *iter.next().unwrap_unchecked() = *i };
            }

            // Safety: there's one more limb left.
            unsafe { *iter.next().unwrap_unchecked() = 0 };

            (true, 0)
        }
        ShortHostOp::StorageWrite => {
            let key = host
                .system
                .get_memory_region_range(MemoryRegion::shared_for_at::<U256Repr>(
                    params.op1 as usize,
                )?);

            let val = host
                .system
                .get_memory_region_range(MemoryRegion::shared_for_at::<U256Repr>(
                    params.op2 as usize,
                )?);

            let key = unsafe { &*key.as_ptr().cast::<U256Repr>() };
            let val = unsafe { &*val.as_ptr().cast::<U256Repr>() };

            #[cfg(feature = "testing")]
            {
                print!(" -> writing value {:?} to key {:?}", val, key);
            }

            // Safety: both types are 32 byte arrays.
            #[allow(clippy::missing_transmute_annotations)]
            let key = unsafe { core::mem::transmute(key) };
            #[allow(clippy::missing_transmute_annotations)]
            let val = unsafe { core::mem::transmute(val) };
            let mut bm = ColdWarmAccessBitmask::empty();

            let write_fn = match params.param {
                0 => storage_write::<S, false>,
                1 => storage_write::<S, true>,
                _x => {
                    return Err(dev_format!("Unexpected StorageWrite parameter {}.", _x).into());
                }
            };

            write_fn(
                host.system,
                &mut host.resources,
                &host.context.address,
                key,
                val,
                &mut bm,
            )
            .map_err(|_e| dev_format!("{:?}", _e))?;

            (true, 0)
        }
        ShortHostOp::StorageRead => {
            let key_offset = params.op1 as usize;
            let key = host
                .system
                .get_memory_region_range(MemoryRegion::shared_for_at::<U256Repr>(key_offset)?);

            let key = unsafe { &*key.as_ptr().cast::<U256Repr>() };
            #[cfg(feature = "testing")]
            {
                print!("\n      -> reading at key 0x{:0x?}", key);
            }

            // Safety: both types are 32 byte arrays.
            #[allow(clippy::missing_transmute_annotations)]
            let key = unsafe { core::mem::transmute(key) };

            let mut bm = ColdWarmAccessBitmask::empty();

            let read_fn = match params.param {
                0 => storage_read::<S, false>,
                1 => storage_read::<S, true>,
                _x => {
                    return Err(dev_format!("Unexpected StorageRead parameter {}.", _x).into());
                }
            };

            let result = read_fn(
                host.system,
                &mut host.resources,
                &host.context.address,
                key,
                &mut bm,
            );

            let result = match result {
                #[allow(clippy::missing_transmute_annotations)]
                Ok(r) => unsafe { core::mem::transmute::<_, U256Repr>(r) },
                Err(_) => return Err("Could not read storage".into()),
            };

            #[cfg(feature = "testing")]
            {
                print!("\n      -> returned result 0x{:x?}", result);
                print!("\n     ");
            }

            let dst = host
                .system
                .get_memory_region_range_mut(MemoryRegion::shared_for_at::<U256Repr>(
                    params.op2 as usize,
                )?);

            dst.copy_from_slice(result.as_raw_bytes());

            (true, 0)
        }
        ShortHostOp::HashKeccak256 => {
            let mut buf = ArrayBuilder::default();

            let src = host.system.get_memory_region_range(MemoryRegion {
                region_type: MemoryRegionType::Shared,
                description: MemoryRegionDescription {
                    offset: params.op1 as usize,
                    len: params.param as usize,
                },
            });

            S::SystemFunctions::keccak256(
                src,
                &mut buf,
                host.resources,
                host.system.get_allocator(),
            )
            .map_err(|_| InterpreterError::from(()))?;

            let dst = host
                .system
                .get_memory_region_range_mut(MemoryRegion::shared_for_at::<[u8; 32]>(
                    params.op2 as usize,
                )?);
            dst.copy_from_slice(&buf.build());

            (true, 0)
        }
        _ => return Err(dev_format!("Unsupported operation {:?}", params.operation).into()),
    };

    #[cfg(feature = "testing")]
    {
        println!(" -> success: {}", success);
    }

    let dst = host
        .system
        .get_memory_region_range_mut(MemoryRegion::shared_for_at::<HostOpResult>(
            params.result_addr as usize,
        )?);

    unsafe {
        core::ptr::write(
            dst.as_mut_ptr().cast(),
            HostOpResult {
                success,
                param: result as u64,
            },
        )
    };

    // let len = stack_operands.len();
    //
    // let outputs = &mut stack_operands[(len - num_inputs)..];
    // outputs[0] = StackValue::new_bool(success);
    // outputs[1] = StackValue::new_i64(result as u64 as i64);

    Ok(ExecutionResult::Continue)
}

pub fn long_host_op<S: EthereumLikeSystem>(
    host: &'_ mut ZkOSHost<'_, S>,
    stack_operands: &'_ mut [StackValue],
    num_inputs: usize,
) -> Result<ExecutionResult, InterpreterError> {
    let params = LongOpParams::init(stack_operands, num_inputs)?;

    #[cfg(feature = "testing")]
    {
        print!(
            "\x1b[38;5;214mHost:\x1b[0m call received: {:?}",
            params.operation
        );
    }

    let (_operand_0_size, _operand_1_size, _dst_0_size, _dst_1_size) = match params.operation {
        // LongHostOp::FullWidthMul => {
        //     // FullWidthMul
        //     if operation_parameter == 0 || operation_parameter > 32 {
        //         return Err(().into());
        //     }
        //     let len = operation_parameter as usize;
        //     (len, len, len, len)
        // }
        // LongHostOp::Div => {
        //     // Div
        //     if operation_parameter == 0 || operation_parameter > 32 {
        //         return Err(().into());
        //     }
        //     let len = operation_parameter as usize;
        //     (len, len, len, 0)
        // }
        // LongHostOp::Shl => {
        //     // Shl
        //     let operand_len = (operation_parameter as u8) as u32;
        //     let _shift_len = operation_parameter >> 8;
        //     if operand_len == 0 || operand_len > 32 {
        //         return Err(().into());
        //     }
        //     let len = operand_len as usize;
        //     (len, 0, len, 0)
        // }
        // LongHostOp::Compare => {
        //     // Compare
        //     if operation_parameter == 0 || operation_parameter > 32 {
        //         return Err(().into());
        //     }
        //     let len = operation_parameter as usize;
        //     (len, len, 0, 0)
        // }
        // LongHostOp::CalldataReadU256LE => {
        //     // calldata read u256
        //     (0, 0, 32, 0)
        // }
        // LongHostOp::ExternalOpaqueCall => {
        //     const ABI_LEN: usize = 32 * 3;
        //     let calldata_len = operation_parameter as usize;
        //
        //     (ABI_LEN, calldata_len, 0, 0)
        // }
        LongHostOp::ImmutablesRead => {
            let slice = host.context.immutables;
            let size = slice.len();
            let src = (&slice[0]) as *const _;

            let dst = &mut host.system.get_memory_region_mut(MemoryRegionType::Shared)
                [params.dst1 as usize];
            let dst = dst as *mut _;

            unsafe {
                core::ptr::copy_nonoverlapping(src, dst, size);
            }

            // TODO: return actual data
            (0, 0, 0, 0)
        }
        LongHostOp::IntxFillBytes => {
            let ([src], dst) = get_pieces_of_slice(
                host.system.get_memory_regions().heap,
                [range_for_at::<[u8; 32]>(params.op1 as usize)?],
                range_for_at::<U256Repr>(params.dst1 as usize)?,
            )
            .ok_or(())?;

            use core::mem::size_of;

            const {
                assert!(32 == size_of::<U256Repr>());
            }

            dst.copy_from_slice(src);

            (0, 0, 0, 0)
        }
        LongHostOp::IntxSwapEndianness => match params.param {
            0 => {
                let repr_ptr = host
                    .system
                    .get_memory_region_range_mut(MemoryRegion {
                        region_type: MemoryRegionType::Shared,
                        description: MemoryRegionDescription {
                            offset: params.dst1 as usize,
                            len: core::mem::size_of::<U256Repr>(),
                        },
                    })
                    .as_mut_ptr()
                    .cast::<U256Repr>();

                let dst = unsafe { &mut *repr_ptr };

                dst.swap_endianness_inplace();

                (0, 0, 0, 0)
            }
            1 => {
                let ([src], dst) = get_pieces_of_slice(
                    host.system.get_memory_regions().heap,
                    [range_for_at::<[u8; 32]>(params.op1 as usize)?],
                    range_for_at::<U256Repr>(params.dst1 as usize)?,
                )
                .ok_or(())?;

                let src = unsafe { &*src.as_ptr().cast::<U256Repr>() };
                let dst = unsafe { &mut *dst.as_mut_ptr().cast::<U256Repr>() };

                src.swap_endianness_into(dst);

                (0, 0, 0, 0)
            }
            _ => {
                return Err(dev_format!(
                    "Unexpected operation parameter {} for IntxSwapEndianness.",
                    params.param
                )
                .into())
            }
        },
        LongHostOp::IntxOverflowingAdd => {
            let ([left, right], dst) = get_pieces_of_slice(
                host.system.get_memory_regions().heap,
                [
                    range_for_at::<U256Repr>(params.op1 as usize)?,
                    range_for_at::<U256Repr>(params.op2 as usize)?,
                ],
                range_for_at::<U256Repr>(params.dst1 as usize)?,
            )
            .ok_or(())?;

            let left = unsafe { &*left.as_ptr().cast::<U256Repr>() };
            let right = unsafe { &*right.as_ptr().cast::<U256Repr>() };
            let dst = unsafe { &mut *dst.as_mut_ptr().cast::<U256Repr>() };

            U256Repr::le_add_into(params.param as usize, left, right, dst);

            #[cfg(feature = "testing")]
            {
                println!();
                println!("      -> left:   {:#0x?}", left);
                println!("      -> right:  {:#0x?}", right);
                println!("      -> result: {:#0x?}", dst);
            }

            (0, 0, 0, 0)
        }
        LongHostOp::IntxOverflowingSub => {
            let ([left, right], dst) = get_pieces_of_slice(
                host.system.get_memory_regions().heap,
                [
                    range_for_at::<U256Repr>(params.op1 as usize)?,
                    range_for_at::<U256Repr>(params.op2 as usize)?,
                ],
                range_for_at::<U256Repr>(params.dst1 as usize)?,
            )
            .ok_or(())?;

            let left = unsafe { &*left.as_ptr().cast::<U256Repr>() };
            let right = unsafe { &*right.as_ptr().cast::<U256Repr>() };
            let dst = unsafe { &mut *dst.as_mut_ptr().cast::<U256Repr>() };

            U256Repr::le_sub_into(params.param as usize, left, right, dst);

            #[cfg(feature = "testing")]
            {
                println!();
                println!("      -> left:   {:#0x?}", left);
                println!("      -> right:  {:#0x?}", right);
                println!("      -> result: {:#0x?}", dst);
            }

            (0, 0, 0, 0)
        }
        _ => return Err(dev_format!("Unsupported operation {:?}", params.operation).into()),
    };

    #[cfg(feature = "testing")]
    {
        println!();
    }

    // TODO: why's this here? host doesn't allocate memory. Ensure that this check exists when
    // allocating memory and remove.
    // check sizes
    // let memory_len = host.num_heap_pages() as usize * PAGE_SIZE;
    // if op1_offset + operand_0_size > memory_len
    //     || op2_offset + operand_1_size > memory_len
    //     || dst1_offset + dst_0_size > memory_len
    //     || dst2_offset + dst_1_size > memory_len
    // {
    //     return Err(().into());
    // }

    let dst = host
        .system
        .get_memory_region_range_mut(MemoryRegion::shared_for_at::<HostOpResult>(
            params.result_addr as usize,
        )?);

    unsafe {
        core::ptr::write(
            dst.as_mut_ptr().cast(),
            HostOpResult {
                success: true,
                param: 0,
            },
        )
    };

    // let outputs = &mut stack_operands[(len - num_inputs)..];
    // outputs[0] = StackValue::new_bool(true);
    // outputs[1] = StackValue::new_i64(0 as u64 as i64);

    Ok(ExecutionResult::Continue)

    // outputs[0] = StackValue::new_bool(success);
    // outputs[1] = StackValue::new_i64(result as u64 as i64);

    // let memory = &mut host.heap;
    // let (success, return_value): (bool, u32) = match operation {
    //     5 => {
    //         let a = read_integer_repr(memory, op0_offset, operand_0_size)?;
    //         let b = read_integer_repr(memory, op1_offset, operand_1_size)?;
    //         let mut dst: [u64; 8] = [0u64; 8];
    //         let bit_size = operation_parameter * 8;
    //         full_width_mul(&a, &b, &mut dst, bit_size as usize);
    //         let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    //         if num_top_word_bits != 0 {
    //             num_words += 1;
    //         }
    //         let (low, high) = dst.split_at(num_words as usize);
    //         let high = &high[..(num_words as usize)];
    //         let low = u64_slice_as_u8_slice(&low);

    //         HostHeap::copy_into_memory(memory, &low, dst0_offset as u32)?;
    //         let high = u64_slice_as_u8_slice(&high);
    //         HostHeap::copy_into_memory(memory, &high, dst1_offset as u32)?;

    //         (true, 0)
    //     }
    //     7 => {
    //         // Div
    //         let mut b = read_integer_repr(memory, op1_offset, operand_1_size)?;
    //         if repr_is_zero(&b) {
    //             (true, 0)
    //         } else {
    //             let mut a = read_integer_repr(memory, op0_offset, operand_0_size)?;
    //             let bit_size = operation_parameter * 8;
    //             let (mut num_words, num_top_word_bits) = (bit_size / 64, bit_size % 64);
    //             if num_top_word_bits != 0 {
    //                 num_words += 1;
    //             }
    //             ruint::algorithms::div(
    //                 &mut a[..(num_words as usize)],
    //                 &mut b[..(num_words as usize)],
    //             );

    //             let quotient = u64_slice_as_u8_slice(&a);
    //             HostHeap::copy_into_memory(memory, &quotient, dst0_offset as u32)?;

    //             (true, 0)
    //         }
    //     }
    //     10 => {
    //         let operand_len = (operation_parameter as u8) as u32;
    //         let shift_len = operation_parameter >> 8;
    //         let mut a = read_integer_repr(memory, op0_offset, operand_0_size)?;
    //         print_integer_repr(&a);
    //         let bit_size = operand_len * 8;
    //         let of = integer_repr_overflowing_shl(&mut a, bit_size as usize, shift_len as usize);
    //         print_integer_repr(&a);

    //         (true, of as u32)
    //     }
    //     18 => {
    //         let a = read_integer_repr(memory, op0_offset, operand_0_size)?;
    //         let b = read_integer_repr(memory, op1_offset, operand_1_size)?;
    //         print_integer_repr(&a);
    //         print_integer_repr(&b);
    //         let bit_size = operation_parameter * 8;
    //         let comparison_result = integer_repr_compare(&a, &b, bit_size as usize);

    //         (true, comparison_result as i32 as u32)
    //     }
    //     37 => {
    //         let calldata_offset = operation_parameter;
    //         let calldata_len = host.context.len() as u32;
    //         if calldata_offset >= calldata_len {
    //             // write zeroes
    //             HostHeap::fill_memory(memory, 0, dst0_offset as u32, 32)?;
    //             (true, 0)
    //         } else {
    //             let calldata_end = calldata_offset + 32;
    //             if calldata_end > calldata_len {
    //                 todo!()
    //             } else {
    //                 // trivial case
    //                 let src = &host.context[(calldata_offset as usize)..][..32];
    //                 // we should manage endianness here - machine is LE, but calldata is BE
    //                 let repr = read_repr_from_be_bytes(src);
    //                 let src = u64_slice_as_u8_slice(&repr);
    //                 HostHeap::copy_into_memory(memory, src, dst0_offset as u32)?;
    //                 (true, 0)
    //             }
    //         }
    //     }
    //     _ => return Err(()),
    // };

    // let outputs = &mut stack_operands[(len - num_inputs)..];
    // outputs[0] = StackValue::new_i32(success as u32 as i32);
    // outputs[1] = StackValue::new_i32(return_value as i32);

    // Ok(ExecutionResult::Success)
}
