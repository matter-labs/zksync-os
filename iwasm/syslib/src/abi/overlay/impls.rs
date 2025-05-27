use core::{cell::UnsafeCell, mem::MaybeUninit, ptr};

use iwasm_specification::intx::U256Repr;
use syslib_derive::{tuple_overlay_derive, tuple_overlay_derive_bulk};

use crate::{
    abi::{
        overlay::{Cdr, OverlaidData},
        Encodable, Encoder, EncodingError, Overlaid,
    },
    qol::UnsafeCellEx,
    types::uintx::{size_bound, Assert, IntX, IsTrue, BE, LE},
};

#[repr(transparent)]
pub struct UnsafeUninit<T> {
    pub(crate) inner: UnsafeCell<MaybeUninit<T>>,
}

impl<T> UnsafeUninit<T> {
    unsafe fn as_ref(&self) -> &T {
        &*(self as *const _ as *const _)
    }
}

impl<T> Clone for UnsafeUninit<T> {
    /// This is a safe clone, that doesn't assume the state of the decoding of the value.
    fn clone(&self) -> Self {
        Self {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl<T> Default for UnsafeUninit<T> {
    fn default() -> Self {
        Self {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

impl Encodable for bool {
    const ENCODED_SLOTS_FIT: usize = 32;
    const ENCODED_SLOTS_FIT_BYTES: usize = 1;
    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodingError> {
        encoder.write_sized::<{ u32::ENCODED_SLOTS_FIT_BYTES }, _>(|dst| {
            let dst = dst.push_u32(0)?;
            let dst = dst.push_u32(0)?;
            let dst = dst.push_u32(0)?;
            let dst = dst.push_u32(0)?;
            let dst = dst.push_u32(0)?;
            let dst = dst.push_u32(0)?;
            let dst = dst.push_u32(0)?;
            dst.push_u32(*self as u32).map(|_| ())
        })
    }

    fn encoded_size(&self) -> usize {
        32
    }
}

impl Encodable for u32 {
    // This is a hasty implementation, this value should be 4 and properly accounted for.
    const ENCODED_SLOTS_FIT_BYTES: usize = 32;
    const ENCODED_SLOTS_FIT: usize = 1;
    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodingError> {
        encoder.write_sized::<{ u32::ENCODED_SLOTS_FIT_BYTES }, _>(|dst| {
            dst.push_u32(self.to_be()).map(|_| ())
        })
    }

    fn encoded_size(&self) -> usize {
        32
    }
}

impl Overlaid for u32 {
    type Reflection = ();
    type Deref = u32;

    const IS_INDIRECT: bool = false;

    fn to_deref(cdr: &super::Cdr<Self>) -> &Self::Deref {
        let (data_ptr, mut flag) = cdr.data.as_mut_pair();

        let data = unsafe { &mut *data_ptr };

        if !flag.is_set() {
            let decoded = data.read_u32_be();

            data.write_u32_ne(decoded);

            flag.set();
        }

        unsafe { data.as_u32() }
    }

    fn decode(cdr: &Cdr<Self>) -> Self {
        *Self::to_deref(cdr)
    }

    fn reflection_uninit() -> Self::Reflection {}
}

impl<const N: usize> Encodable for IntX<N, LE> {
    const ENCODED_SLOTS_FIT_BYTES: usize = 32;
    const ENCODED_SLOTS_FIT: usize = 1;
    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodingError>
    where
        [(); Self::ENCODED_SLOTS_FIT_BYTES]:,
    {
        encoder.write_sized::<32, _>(|res| {
            let (_res, dst) = res.slice_for::<32>();

            self.repr.write_into(dst).map_err(|_| EncodingError {})
        })

        // Ok(())
    }

    fn encoded_size(&self) -> usize {
        32
    }
}

impl<const N: usize> Encodable for IntX<N, BE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    const ENCODED_SLOTS_FIT: usize = 1;
    const ENCODED_SLOTS_FIT_BYTES: usize = 32;

    fn encoded_size(&self) -> usize {
        32
    }

    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodingError> {
        encoder.write_sized::<32, _>(|res| {
            let (_res, dst) = res.slice_for::<32>();

            MaybeUninit::copy_from_slice(dst, self.as_bytes());

            Ok(())
        })
    }
}

impl<const N: usize> Overlaid for &IntX<N, LE> {
    type Reflection = UnsafeUninit<Self>;
    type Deref = Self;

    const IS_INDIRECT: bool = false;

    // For now we're dereferencing to a ref.
    // TODO: Benchmark performance for copying the actual value. Dereferencing to value is cleaner.
    fn to_deref(cdr: &Cdr<Self>) -> &Self::Deref {
        let (data_ptr, mut flag) = cdr.data.as_mut_pair();

        let data = unsafe { &mut *data_ptr };

        if !flag.is_set() {
            // TODO: safety comments
            let repr = unsafe { data.as_u256_repr_mut() };
            repr.swap_endianness_inplace();
            let int = unsafe { &*(repr as *mut U256Repr as *mut IntX<N, LE>) };

            let r = unsafe { cdr.reflection.inner.u_deref_mut().write(int) };
            flag.set();

            return r;
        }

        // cdr.reflection.inner.u_deref().
        unsafe { cdr.reflection.as_ref() }
    }

    fn decode(_cdr: &Cdr<Self>) -> Self {
        // *Self::to_deref(cdr)
        todo!("obsolete?")
    }

    fn reflection_uninit() -> Self::Reflection {
        UnsafeUninit {
            inner: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    //
    // fn encode<N>(&self, encoder: &mut Encoder) -> Result<(), EncodingError>
    //     where [(); N]:
    // {
    //
    //     let mut dst = encoder.reserve::<{ size_of::<IntX<N, LE>>() }>();
    //     for l in self.repr.as_u32_be_lsb_limbs() {
    //         dst.
    //     }
    // }
}

impl Encodable for &str {
    const ENCODED_SLOTS_FIT_BYTES: usize = 32;
    const ENCODED_SLOTS_FIT: usize = 1;
    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodingError>
    where
        [(); core::mem::size_of::<Self>()]:,
    {
        let enc_size = self.encoded_size();

        unsafe {
            encoder.write_unsized(enc_size, |dst| {
                let dst_len = dst.len();
                // First, zero-out the last slot, because it's going to be partially filled most of
                // the times.
                let last_slot = dst[(dst_len - 32)..]
                    .as_mut_ptr()
                    .cast::<[MaybeUninit<usize>; 32 / core::mem::size_of::<usize>()]>();
                let last_slot = &mut *last_slot;

                for x in last_slot {
                    x.write(0);
                }

                // Now write the string bytes.
                let src = self.as_bytes();
                let len = src.len();
                // Safety: `dst` len equals `enc_size` which is not smaller than self bytes len.
                ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr().cast(), len);

                // This is the original code, but it requires a super-new nightly (for the .cast() to work)
                //  (that has other bugs - see rust-toolchain.toml).
                //self.as_bytes()
                //    .clone_to_uninit(dst[..self.bytes().len()].as_mut_ptr().cast());

                // Every byte of the `dst` has been written at this point, as only the last slot may
                // be partially filled.

                Ok(())
            })
        }
    }

    fn encoded_size(&self) -> usize {
        let bytes = self.len();

        let rem = bytes % 32;

        bytes + 32 - rem
    }
}

impl<'a> Overlaid for &'a str {
    type Reflection = ();
    type Deref = &'a str;

    const IS_INDIRECT: bool = true;

    fn to_deref(cdr: &super::Cdr<Self>) -> &Self::Deref {
        let (data_ptr, mut flag) = cdr.data.as_mut_pair();

        unsafe {
            let len_slot = super::AbiSlot::instantiate(data_ptr);

            if !flag.is_set() {
                let len = len_slot.read_u32_be();

                let bytes_ptr = data_ptr.add(1).cast();

                let str_ref = core::str::from_utf8(&*core::ptr::slice_from_raw_parts(
                    bytes_ptr,
                    len as usize,
                ))
                .unwrap();

                len_slot.write_raw(str_ref);

                flag.set();
            }

            len_slot.as_raw::<&str>()
        }
    }

    fn decode(cdr: &Cdr<Self>) -> Self {
        Self::to_deref(cdr)
    }

    fn reflection_uninit() -> Self::Reflection {}
}

// impl<T1, T2> Encodable for (T1, T2)
//     where
//     T1: Encodable,
//     T2: Encodable,
// {
//     type Reflection = UnsafeUninit<(Cdr<T1>, Cdr<T2>)>;
//     type Deref = (Cdr<T1>, Cdr<T2>);
//
//     const IS_INDIRECT: bool = true;
//
//     fn to_deref(cdr: &super::Cdr<Self>) -> &Self::Deref {
//         let f1 = unsafe { Cdr::new_with_offset(cdr.data.base_ptr, cdr.data.ix + 0, cdr.data.ix) };
//         let f2 = unsafe { Cdr::new_with_offset(cdr.data.base_ptr, cdr.data.ix + 1, cdr.data.ix) };
//
//         unsafe {
//             cdr.reflection.inner.u_deref_mut().write(
//                 (f1, f2)
//             )
//         }
//     }
//
//     fn decode(cdr: &Cdr<Self>) -> Self {
//         let (v1, v2) = Self::to_deref(cdr);
//
//         let r = (T1::decode(v1), T2::decode(v2));
//
//         r
//     }
//
//     fn reflection_uninit() -> Self::Reflection {
//         UnsafeUninit { inner: UnsafeCell::new(MaybeUninit::uninit()) }
//     }
// }
//
pub struct TupleDerefContainer<T> {
    data: OverlaidData,
    value: UnsafeCell<MaybeUninit<T>>,
}
//
// impl<T1, T2> core::ops::Deref for TupleDerefContainer<T1, T2>
//     where
//     T1: Encodable,
//     T2: Encodable,
// {
//     type Target = (T1, T2);
//
//     fn deref(&self) -> &Self::Target {
//         let r = unsafe { (
//             Cdr::<T1>::new_with_offset(self.data.base_ptr, self.data.ix + 0, self.data.ix).to(|x| T1::decode(&x)),
//             Cdr::<T2>::new_with_offset(self.data.base_ptr, self.data.ix + 1, self.data.ix).to(|x| T2::decode(&x)),
//
//         ) };
//
//         unsafe { self.value.u_deref_mut().write(r) }
//     }
// }
//
// impl<T1, T2> Cdr<(T1, T2)>
//     where
//     T1: Encodable,
//     T2: Encodable,
// {
//     pub fn as_deref(&self) -> TupleDerefContainer<T1, T2> {
//         TupleDerefContainer { data: self.data, value: UnsafeCell::new(MaybeUninit::uninit()) }
//     }
// }

macro_rules! tuple_overlay_impl {
    ($($T:ident),+) => {


        impl<$($T),+> Overlaid for ($($T,)+)
            where
                $($T: Overlaid,)+
        {
            type Reflection = UnsafeUninit<($(Cdr<$T>,)+)>;
            type Deref = ($(Cdr<$T>,)+);

            const IS_INDIRECT: bool = true;

            #[allow(non_snake_case)]
            #[allow(unused_assignments)]
            fn to_deref(cdr: &super::Cdr<Self>) -> &Self::Deref {
                let mut ix = 0;

                $(
                    let $T = unsafe {
                       Cdr::new_with_offset(cdr.data.base_ptr, cdr.data.ix + ix, cdr.data.ix)
                    };
                    ix += 1;
                )+

                unsafe {
                    cdr.reflection.inner.u_deref_mut().write(($($T,)+))
                }
            }

            #[allow(non_snake_case)]
            fn decode(cdr: &Cdr<Self>) -> Self {
                let ($($T,)+) = Self::to_deref(cdr);

                let r = ($($T::decode($T),)+);

                r
            }

            fn reflection_uninit() -> Self::Reflection {
                UnsafeUninit { inner: UnsafeCell::new(MaybeUninit::uninit()) }
            }

        }

        // pub struct TupleDerefContainer<$($T),+> {
        //     data: OverlaidData,
        //     value: UnsafeCell<MaybeUninit<($($T),+)>>
        // }

        impl<$($T),+> core::ops::Deref for TupleDerefContainer<($($T,)+)>
            where
            $($T: Overlaid,)+
        {
            type Target = ($($T,)+);

            #[allow(non_snake_case)]
            #[allow(unused_assignments)]
            fn deref(&self) -> &Self::Target {
                use crate::qol::PipeOp;

                let mut ix = 0;

                $(
                    let $T = unsafe { Cdr::<$T>::new_with_offset(
                        self.data.base_ptr,
                        self.data.ix + ix,
                        self.data.ix).to(|x| $T::decode(&x))
                    };

                    ix += 1;
                )+

                let r = ($($T,)+);

                unsafe { self.value.u_deref_mut().write(r) }
            }
        }

        impl<$($T),+> Cdr<($($T,)+)>
            where
            $($T: Overlaid,)+
        {
            pub fn as_deref(&self) -> TupleDerefContainer<($($T,)+)> {
                TupleDerefContainer { data: self.data, value: UnsafeCell::new(MaybeUninit::uninit()) }
            }
        }
    };
}

tuple_overlay_derive_bulk!(4);
