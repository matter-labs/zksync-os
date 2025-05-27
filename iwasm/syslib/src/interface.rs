use iwasm_specification::host_ops::LongHostOp;

use super::types::uintx::*;
use crate::abi::decoder::*;

pub trait SystemInterface: Sized {
    fn return_ok(result: &[u8]) -> !;

    fn terminate_execution(reason: &'static str) -> ! {
        panic!("{}", reason)
    }

    /***** intX *****/

    fn uintx_new<const N: usize>() -> IntX<Self, N>
    where
        Assert<{ size_bound(N) }>: IsTrue
    {
        IntX::<Self, N> {
            repr: U256Repr([0; 4]),
            phantom: core::marker::PhantomData,
        }
    }

    fn is_zero<const N: usize>(operand: &IntX<Self, N>) -> bool
    where
        Assert<{ size_bound(N) }>: IsTrue
    {
        let (success, overflow) = unsafe {
            crate::long_host_op(
                LongHostOp::OverflowingAdd as u32,
                N as u64,
                operand.repr.0.as_ptr(),
                core::ptr::null(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        };
        if success == false {
            Self::terminate_execution("host function failure");
        }

        overflow != 0
    }

    fn uintx_overflowing_add_assign<const N: usize> (
        dst: &mut IntX<Self, N>,
        other: &IntX<Self, N>,
    ) -> bool
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_overflowing_sub_assign<const N: usize>(
        dst: &mut IntX<Self, N>,
        other: &IntX<Self, N>,
    ) -> bool
    where Assert<{ size_bound(N) }>: IsTrue;
    
    fn uintx_overflowing_shl_assign<const N: usize>(
        dst: &mut IntX<Self, N>,
        rhs: u32) -> bool
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_overflowing_shr_assign<const N: usize>(
        dst: &mut IntX<Self, N>,
        rhs: u32) -> bool
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_widening_mul<const N: usize>(
        dst: &mut IntX<Self, N>,
        other: &IntX<Self, N>,
    ) -> IntX<Self, N>
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_unsigned_div<const N: usize>(
        dst: &mut IntX<Self, N>,
        other: &IntX<Self, N>)
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_unsigned_rem<const N: usize>(
        dst: &mut IntX<Self, N>,
        other: &IntX<Self, N>)
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_unsigned_div_rem<const N: usize>(
        dst: &IntX<Self, N>,
        other: &IntX<Self, N>,
    ) -> (IntX<Self, N>, IntX<Self, N>)
    where Assert<{ size_bound(N) }>: IsTrue;

    fn uintx_unsigned_compare<const N: usize>(
        dst: &IntX<Self, N>,
        other: &IntX<Self, N>,
    ) -> core::cmp::Ordering
    where Assert<{ size_bound(N) }>: IsTrue;

    /***** Storage/Data *****/

    fn return_immutables<T: Clone>(value: &T);
    fn load_immutables<T>() -> T;
    fn sstore(key: &IntX<Self, 32>, value: &IntX<Self, 32>);
    fn sload(key: &IntX<Self, 32>) -> IntX<Self, 32>;

    type Calldata: DecoderInterface;
    type Returndata: DecoderInterface;

    fn calldata_interface() -> Self::Calldata;
}

// use crate::{abi::*, B160, U256};

// #[derive(Clone)]
// pub struct CallParameters {
//     pub destination: B160,
//     pub ergs: U256,
//     pub nominal_token_value: U256,
// }
//
// impl ABICodableCommon for CallParameters {
//     fn is_dynamic() -> bool {
//         false
//     }
//     fn head_encoding_size() -> u32 {
//         <U256 as ABICodableCommon>::head_encoding_size() * 3
//     }
// }
//
// impl ABIEncodable for CallParameters {
//     fn full_encoding_size(&self) -> u32 {
//         <Self as ABICodableCommon>::head_encoding_size()
//     }
//     fn write<I: encoder::EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
//         let mut total_written = 0;
//         total_written += self.destination.write(interface)?;
//         total_written += self.ergs.write(interface)?;
//         total_written += self.nominal_token_value.write(interface)?;
//
//         Ok(total_written)
//     }
// }
//
// impl ABIDecodable for CallParameters {
//     fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
//         let destination = interface.decode_field()?;
//         let ergs = interface.decode_field()?;
//         let nominal_token_value = interface.decode_field()?;
//
//         let new = Self {
//             destination,
//             ergs,
//             nominal_token_value,
//         };
//
//         Ok(new)
//     }
// }
