use super::ABIEncodable;

pub trait BufferInterface {
    #[allow(clippy::result_unit_err)]
    fn write_u8(&mut self, value: u8) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn write_u32(&mut self, value: u32) -> Result<(), ()>;
    // // fn copy(&mut self, offset: u32, len: u32, src: &[u8]) -> Result<(), ()>;
}

pub trait EncoderInterface: BufferInterface + Sized + core::fmt::Debug {
    type StructEncoder<'a>: EncoderInterface + 'a
    where
        Self: 'a;
    #[allow(clippy::result_unit_err)]
    fn create_struct_encoder<T: ABIEncodable>(&mut self) -> Result<Self::StructEncoder<'_>, ()>;

    type SequenceEncoder<'a>: EncoderInterface + 'a
    where
        Self: 'a;
    #[allow(clippy::result_unit_err)]
    fn create_sequence_encoder<T: ABIEncodable>(
        &mut self,
        len: u32,
    ) -> Result<Self::SequenceEncoder<'_>, ()>;

    #[allow(clippy::result_unit_err)]
    fn encode_field<T: ABIEncodable>(&mut self, element: &T) -> Result<u32, ()>;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ABIEncodingWalker<'a> {
    pub(crate) start: *mut u8,
    pub(crate) current: *mut u8,
    pub(crate) absolute_end: *const u8,
    pub(crate) tail_offset: usize,
    pub(crate) _marker: core::marker::PhantomData<&'a ()>,
}

impl ABIEncodingWalker<'_> {
    pub(crate) fn from_ptr_and_buffer_size(start: *mut u8, buffer_size: u32) -> Self {
        unsafe {
            let end = start.add(buffer_size as usize);

            Self {
                start,
                current: start,
                tail_offset: 0,
                absolute_end: end.cast_const(),
                _marker: core::marker::PhantomData,
            }
        }
    }
}

impl BufferInterface for ABIEncodingWalker<'_> {
    fn write_u8(&mut self, value: u8) -> Result<(), ()> {
        unsafe {
            let t = self.current.add(1);
            if t as *const u8 > self.absolute_end {
                return Err(());
            }
            self.current.write(value);
            self.current = t;

            Ok(())
        }
    }
    fn write_u32(&mut self, value: u32) -> Result<(), ()> {
        unsafe {
            let t = self.current.add(4);
            if t as *const u8 > self.absolute_end {
                return Err(());
            }
            self.current.cast::<[u8; 4]>().write(value.to_be_bytes());
            self.current = t;

            Ok(())
        }
    }
}

impl ABIEncodingWalker<'_> {
    pub(crate) fn add_tail_offset(&mut self, offset: u32) -> Result<(), ()> {
        unsafe {
            self.tail_offset += offset as usize;
            let t = self.start.add(self.tail_offset);
            if t.cast_const() > self.absolute_end {
                return Err(());
            }

            Ok(())
        }
    }

    fn create_subencoder(&self) -> Result<Self, ()> {
        unsafe {
            let t = self.start.add(self.tail_offset);
            if t.cast_const() > self.absolute_end {
                return Err(());
            }
            Ok(Self {
                start: t,
                current: t,
                tail_offset: 0,
                absolute_end: self.absolute_end,
                _marker: core::marker::PhantomData,
            })
        }
    }

    fn clone_self(&self) -> Self {
        Self {
            start: self.start,
            current: self.current,
            tail_offset: self.tail_offset,
            absolute_end: self.absolute_end,
            _marker: core::marker::PhantomData,
        }
    }
}

impl EncoderInterface for ABIEncodingWalker<'_> {
    type StructEncoder<'a>
        = ABIEncodingWalker<'a>
    where
        Self: 'a;

    fn create_struct_encoder<T: ABIEncodable>(&mut self) -> Result<Self::StructEncoder<'_>, ()> {
        // println!("Creating struct encoder for {:?}", std::any::type_name::<T>());
        let subencoder = if T::is_dynamic() {
            let mut subencoder = self.create_subencoder()?;
            let initial_offset = T::head_encoding_size();
            subencoder.add_tail_offset(initial_offset)?;

            subencoder
        } else {
            // we are good as is
            self.clone_self()
        };

        Ok(subencoder)
    }

    type SequenceEncoder<'a>
        = ABIEncodingWalker<'a>
    where
        Self: 'a;
    fn create_sequence_encoder<T: ABIEncodable>(
        &mut self,
        len: u32,
    ) -> Result<Self::StructEncoder<'_>, ()> {
        // println!("Creating sequence encoder for {:?} for len {}", std::any::type_name::<T>(), len);
        let mut subencoder = self.create_subencoder()?;
        if T::is_dynamic() {
            subencoder.add_tail_offset(32 * len)?;
        } else {
            let initial_offset = T::head_encoding_size();
            subencoder.add_tail_offset(initial_offset * len)?;
        }

        Ok(subencoder)
    }

    fn encode_field<T: ABIEncodable>(&mut self, element: &T) -> Result<u32, ()> {
        // println!("Encoding {:?}", std::any::type_name::<T>());
        // if T is static then we continue to write is as-if it was a parent frame,
        // otherwise we need to adjust tail offset that will be used
        let written = if T::is_dynamic() {
            let current_offset = self.tail_offset as u32;
            // manually write it down
            <u32 as ABIEncodable>::write(&current_offset, self)?;
            // then descent
            let mut encoder = self.create_struct_encoder::<T>()?;
            let total_written = element.write(&mut encoder)?;
            let adjustment = if T::is_dynamic() {
                0
            } else {
                encoder.tail_offset
            };
            self.add_tail_offset(adjustment as u32)?;

            32 + total_written
        } else {
            element.write(self)?
        };

        Ok(written)
    }
}
