use core::mem::MaybeUninit;

use overlay::Cdr;

use super::*;

pub mod codable_common;
pub mod decoder;
pub mod encoder;
pub mod impls;
pub mod overlay;

use self::decoder::*;
use self::encoder::*;

const ABI_SLOT_SIZE: usize = 32;

#[derive(Debug)]
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    pub fn new(size: usize) -> Self {
        Self {
            buf: Vec::with_capacity(size),
        }
    }

    pub fn write_sized<
        const N: usize,
        F: FnOnce(Reservation<{ N }>) -> Result<(), EncodingError>,
    >(
        &mut self,
        f: F,
    ) -> Result<(), EncodingError> {
        // assert!(self.buf.capacity() >= self.buf.len() + size_of::<T>());

        // TODO: safety comment
        let reserve_ptr = unsafe {
            self.buf
                .as_mut_ptr()
                .add(self.buf.len())
                .cast::<[MaybeUninit<u8>; N]>()
        };

        let reserve = unsafe { &mut *reserve_ptr };

        let reservation = Reservation::new(reserve);

        f(reservation)?;

        unsafe { self.buf.set_len(self.buf.len() + N) };

        Ok(())
    }

    /// # Safety
    ///
    /// TODO: add docs
    pub unsafe fn write_unsized<F: FnOnce(&mut [MaybeUninit<u8>]) -> Result<(), EncodingError>>(
        &mut self,
        size: usize,
        f: F,
    ) -> Result<(), EncodingError> {
        let reserve = self.buf.spare_capacity_mut();

        assert!(reserve.len() >= size);

        let reserve = &mut reserve[..size];

        f(reserve)?;

        self.buf.set_len(self.buf.len() + size);

        Ok(())
    }

    pub fn finalize(self) -> &'static [u8] {
        self.buf.leak()
    }
}

pub struct Reservation<'a, const N: usize> {
    // encoder: &'a mut Encoder,
    bytes: &'a mut [MaybeUninit<u8>; N],
}

impl<'a, const N: usize> Reservation<'a, N> {
    pub fn new(bytes: &'a mut [MaybeUninit<u8>; N]) -> Self {
        Self { bytes }
    }
    pub fn push_u32(self, value: u32) -> Result<Reservation<'a, { N - 4 }>, EncodingError> {
        let (res, slice) = self.slice_for::<4>();

        unsafe { slice.as_mut_ptr().cast::<u32>().write(value) };

        Ok(res)
    }

    /// In const generic expressions feature current implementation the compiler requires that
    /// expressions in the type declaration and the usage be identical. This means that it's not
    /// possible to use `size_of::<T>` in a call for a function that declares a `[(); N - M]` where
    /// M is the parameter for the size.
    pub fn slice_for<const M: usize>(
        self,
    ) -> (Reservation<'a, { N - M }>, &'a mut [MaybeUninit<u8>; M])
// where [(); N - M]:
    {
        // Safety: N - M >= 0
        let (l, r) = unsafe { self.bytes.split_at_mut_unchecked(M) };

        (
            Reservation {
                // encoder: self.encoder,
                bytes: r.try_into().unwrap(),
            },
            l.try_into().unwrap(),
        )
    }
}

pub type EncodingResult = Result<(), EncodingError>;

pub struct EncodingError {}

pub trait Encodable: Sized {
    /// Amount of slots this encodable needs to be encoded.
    const ENCODED_SLOTS_FIT: usize;
    /// Amount of bytes this encodable needs to be encoded.
    const ENCODED_SLOTS_FIT_BYTES: usize;

    fn encoded_size(&self) -> usize;

    fn encode(&self, encoder: &mut Encoder) -> Result<(), EncodingError>
    where
        [(); Self::ENCODED_SLOTS_FIT_BYTES]:;
}

pub trait Overlaid: Sized {
    type Reflection: Clone + Default;
    const IS_INDIRECT: bool;

    /// The target type for `Deref` trait.
    /// - For values it's the type of the value.
    /// - For composites it's a type that itself is `Deref`. This is because the composites are
    ///     decoded onto stack, so we only want to reserve the target location if needed.
    type Deref;

    fn to_deref(cdr: &Cdr<Self>) -> &Self::Deref;
    fn decode(cdr: &Cdr<Self>) -> Self;
    fn reflection_uninit() -> Self::Reflection;
}

pub enum EncodedSize {
    Const(usize),
    Dyn,
}

pub trait ABICodableCommon: Sized {
    fn is_dynamic() -> bool {
        false
    }
    fn head_encoding_size() -> u32;
}

pub(crate) trait ABIDecodableBase: Sized {
    #[allow(clippy::result_unit_err)]
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()>;
}

pub trait ABIDecodable: ABICodableCommon {
    #[allow(clippy::result_unit_err)]
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()>;
}

impl<T: ABIDecodableBase + ABICodableCommon> ABIDecodable for T {
    #[allow(clippy::result_unit_err)]
    fn read<I: DecoderInterface>(interface: &mut I) -> Result<Self, ()> {
        <T as ABIDecodableBase>::read(interface)
    }
}

pub(crate) trait ABIEncodableBase: Sized {
    #[allow(clippy::result_unit_err)]
    fn write<B: EncoderInterface>(&self, interface: &mut B) -> Result<u32, ()>;
}

pub trait ABIEncodable: ABICodableCommon {
    fn full_encoding_size(&self) -> u32;
    #[allow(clippy::result_unit_err)]
    fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()>;
}

impl<T: ABIEncodableBase + ABICodableCommon> ABIEncodable for T {
    fn full_encoding_size(&self) -> u32 {
        32
    }
    #[allow(clippy::result_unit_err)]
    fn write<I: EncoderInterface>(&self, interface: &mut I) -> Result<u32, ()> {
        self.write(interface)
    }
}

#[allow(clippy::result_unit_err)]
pub fn abi_encode_to_vec<T: ABIEncodable>(element: &T) -> Result<Vec<u8>, ()> {
    let buffer_size = element.full_encoding_size();
    let mut dst: Vec<u8> = Vec::with_capacity(buffer_size as usize);
    let mut interface = ABIEncodingWalker::from_ptr_and_buffer_size(dst.as_mut_ptr(), buffer_size);
    let mut subencoder = interface.create_struct_encoder::<T>()?;
    let total_written = element.write(&mut subencoder)?;
    if total_written != buffer_size {
        return Err(());
    }
    unsafe {
        dst.set_len(buffer_size as usize);
    }
    Ok(dst)
}

#[allow(clippy::result_unit_err)]
pub fn abi_decode_from_bytes<T: ABIDecodable>(src: &[u8]) -> Result<T, ()> {
    let mut interface = ABIDecodingWalker::from_slice(src);
    let mut subdecoder = interface.create_struct_decoder::<T>(0)?;
    let value = T::read(&mut subdecoder)?;
    if subdecoder.current != subdecoder.expected_end {
        return Err(());
    }

    Ok(value)
}

#[allow(clippy::result_unit_err)]
pub fn abi_decode_from_interface<T: ABIDecodable, I: DecoderInterface>(
    interface: &mut I,
) -> Result<T, ()> {
    let mut subdecoder = interface.create_struct_decoder::<T>(0)?;
    let value = T::read(&mut subdecoder)?;

    Ok(value)
}
