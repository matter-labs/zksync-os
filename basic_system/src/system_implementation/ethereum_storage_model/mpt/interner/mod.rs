use alloc::boxed::Box;
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use crypto::MiniDigest;

mod ext_impls;

pub use self::ext_impls::ETHMPTInternerExt;

pub trait ByteBuffer {
    fn write_byte(&mut self, byte: u8);
    fn write_slice(&mut self, slice: &[u8]);
}

pub trait WordBuffer {
    fn write_word(&mut self, word: usize);
    fn write_slice(&mut self, slice: &[usize]);
}

impl<T: MiniDigest> ByteBuffer for T {
    fn write_byte(&mut self, byte: u8) {
        self.update(&[byte]);
    }
    fn write_slice(&mut self, slice: &[u8]) {
        self.update(slice);
    }
}

pub trait InterningBuffer<'a>: ByteBuffer {
    fn flush(self) -> &'a [u8];
    fn flush_mut(self) -> &'a mut [u8];
}

pub trait InterningWordBuffer<'a>: WordBuffer {
    fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<usize>];
    #[allow(clippy::missing_safety_doc)]
    unsafe fn set_word_len(&mut self, len: usize);
    fn flush(self) -> &'a [usize];
    fn flush_as_bytes(self, byte_len: usize) -> &'a [u8];
}

impl WordBuffer for () {
    fn write_word(&mut self, _word: usize) {
        unreachable!()
    }
    fn write_slice(&mut self, _slice: &[usize]) {
        unreachable!()
    }
}

impl<'a> InterningWordBuffer<'a> for () {
    fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<usize>] {
        &mut []
    }

    unsafe fn set_word_len(&mut self, _len: usize) {}

    fn flush(self) -> &'a [usize] {
        unreachable!()
    }
    fn flush_as_bytes(self, _byte_len: usize) -> &'a [u8] {
        unreachable!()
    }
}

pub trait InternerCtor<A: Allocator>: 'static {
    type Interner<'a>: Interner<'a>
    where
        Self: 'a,
        A: 'a;

    fn make_interner_with_capacity_in<'a>(byte_capacity: usize, allocator: A) -> Self::Interner<'a>
    where
        A: 'a;
    fn purge<'a, 'b>(interner: Self::Interner<'a>) -> Self::Interner<'b>
    where
        A: 'a + 'b;
}

pub struct BoxInternerCtor {}

impl<A: Allocator> InternerCtor<A> for BoxInternerCtor {
    type Interner<'a>
        = BoxInterner<A>
    where
        A: 'a;

    fn make_interner_with_capacity_in<'a>(byte_capacity: usize, allocator: A) -> Self::Interner<'a>
    where
        A: 'a,
    {
        BoxInterner::with_capacity_in(byte_capacity, allocator)
    }
    fn purge<'a, 'b>(mut interner: Self::Interner<'a>) -> Self::Interner<'b>
    where
        A: 'a + 'b,
    {
        // we own interner, so we can purge it and reinterpret for another lifetime

        // there is no need to drop, just set used length
        interner.used = 0;

        interner
    }
}

pub trait Interner<'a>: 'a {
    const SUPPORTS_WORD_LEVEL_INTERNING: bool;

    type Buffer: InterningBuffer<'a>
    where
        Self: 'a;
    type WordBuffer: InterningWordBuffer<'a>
    where
        Self: 'a;
    fn get_buffer(&'_ mut self, capacity: usize) -> Result<Self::Buffer, ()>;
    fn get_word_buffer(&'_ mut self, word_capacity: usize) -> Result<Self::WordBuffer, ()>;
}

pub struct MaybeUninitByteBuffer<'a> {
    buffer: &'a mut [MaybeUninit<u8>],
    num_written: usize,
}

impl<'a> ByteBuffer for MaybeUninitByteBuffer<'a> {
    fn write_byte(&mut self, byte: u8) {
        self.buffer[self.num_written].write(byte);
        self.num_written += 1;
    }
    fn write_slice(&mut self, slice: &[u8]) {
        self.buffer[self.num_written..][..slice.len()].write_copy_of_slice(slice);
        self.num_written += slice.len();
    }
}

impl<'a> InterningBuffer<'a> for MaybeUninitByteBuffer<'a> {
    fn flush(self) -> &'a [u8] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), self.num_written) }
    }

    fn flush_mut(self) -> &'a mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer.as_mut_ptr().cast(), self.num_written)
        }
    }
}

pub struct MaybeUninitWordBuffer<'a> {
    buffer: &'a mut [MaybeUninit<usize>],
    num_written: usize,
}

impl<'a> WordBuffer for MaybeUninitWordBuffer<'a> {
    fn write_word(&mut self, word: usize) {
        self.buffer[self.num_written].write(word);
        self.num_written += 1;
    }
    fn write_slice(&mut self, slice: &[usize]) {
        self.buffer[self.num_written..][..slice.len()].write_copy_of_slice(slice);
        self.num_written += slice.len();
    }
}

impl<'a> InterningWordBuffer<'a> for MaybeUninitWordBuffer<'a> {
    fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<usize>] {
        &mut self.buffer[self.num_written..]
    }

    unsafe fn set_word_len(&mut self, len: usize) {
        assert!(len <= self.buffer.len());
        self.num_written = len;
    }

    fn flush_as_bytes(self, byte_len: usize) -> &'a [u8] {
        assert!(byte_len <= self.num_written * core::mem::size_of::<usize>());
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), byte_len) }
    }

    fn flush(self) -> &'a [usize] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), self.num_written) }
    }
}

#[derive(Clone, Debug)]
pub struct BoxInterner<A: Allocator> {
    buffer: Box<[MaybeUninit<usize>], A>,
    used: usize,
}

impl<A: Allocator> BoxInterner<A> {
    pub fn with_capacity_in(byte_capacity: usize, allocator: A) -> Self {
        let word_capacity = byte_capacity.next_multiple_of(core::mem::size_of::<usize>())
            / core::mem::size_of::<usize>();
        Self {
            buffer: Box::new_uninit_slice_in(word_capacity, allocator),
            used: 0,
        }
    }

    // We have mutable reference, so all internet slices would be deconstructed by that time
    pub fn reset(&mut self) {
        self.used = 0;
    }
}

impl<'a, A: Allocator + 'a> Interner<'a> for BoxInterner<A> {
    const SUPPORTS_WORD_LEVEL_INTERNING: bool = true;

    type Buffer
        = MaybeUninitByteBuffer<'a>
    where
        Self: 'a;

    type WordBuffer
        = MaybeUninitWordBuffer<'a>
    where
        Self: 'a;

    fn get_buffer(&'_ mut self, capacity: usize) -> Result<Self::Buffer, ()>
    where
        A: 'a,
    {
        let next_multiple = capacity.next_multiple_of(core::mem::size_of::<usize>());
        let word_capacity = next_multiple / core::mem::size_of::<usize>();
        if self.used + word_capacity > self.buffer.len() {
            return Err(());
        }
        unsafe {
            let to_use = core::slice::from_raw_parts_mut(
                self.buffer.as_mut_ptr().add(self.used).cast(),
                next_multiple,
            );
            self.used += word_capacity;

            Ok(MaybeUninitByteBuffer {
                buffer: to_use,
                num_written: 0,
            })
        }
    }

    fn get_word_buffer(&'_ mut self, word_capacity: usize) -> Result<Self::WordBuffer, ()> {
        if self.used + word_capacity > self.buffer.len() {
            return Err(());
        }
        unsafe {
            let to_use = core::slice::from_raw_parts_mut(
                self.buffer.as_mut_ptr().add(self.used),
                word_capacity,
            );
            self.used += word_capacity;

            Ok(MaybeUninitWordBuffer {
                buffer: to_use,
                num_written: 0,
            })
        }
    }
}
