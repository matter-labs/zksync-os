use super::ABIDecodable;

pub trait DataInterface {
    #[allow(clippy::result_unit_err)]
    fn read_u8(&mut self) -> Result<u8, ()>;
    #[allow(clippy::result_unit_err)]
    fn read_u32(&mut self) -> Result<u32, ()>;
    #[allow(clippy::result_unit_err)]
    fn read_u256_le(&mut self, dst: &mut [u64; 4]) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn skip_bytes(&mut self, len: u32) -> Result<(), ()>;
}
pub trait DecoderInterface: DataInterface + Sized + core::fmt::Debug {
    type StructDecoder<'a>: DecoderInterface + 'a
    where
        Self: 'a;
    #[allow(clippy::result_unit_err)]
    fn create_struct_decoder<T: ABIDecodable>(
        &mut self,
        offset: u32,
    ) -> Result<Self::StructDecoder<'_>, ()>;

    type SequenceDecoder<'a>: DecoderInterface + 'a
    where
        Self: 'a;
    #[allow(clippy::result_unit_err)]
    fn create_sequence_decoder<T: ABIDecodable>(
        &mut self,
        len: u32,
    ) -> Result<Self::SequenceDecoder<'_>, ()>;
    #[allow(clippy::result_unit_err)]
    fn decode_field<T: ABIDecodable>(&mut self) -> Result<T, ()>;
}

#[derive(Clone, Copy, Debug)]
pub struct ABIDecodingWalker<'a> {
    pub(crate) start: *const u8,
    pub(crate) current: *const u8,
    pub(crate) expected_end: *const u8,
    pub(crate) absolute_end: *const u8,
    pub(crate) _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> ABIDecodingWalker<'a> {
    pub(crate) fn from_slice(src: &'a [u8]) -> Self {
        let range = src.as_ptr_range();
        let start = range.start;
        let end = range.end;

        Self {
            start,
            current: start,
            expected_end: start,
            absolute_end: end,
            _marker: core::marker::PhantomData,
        }
    }
}

impl DataInterface for ABIDecodingWalker<'_> {
    fn read_u8(&mut self) -> Result<u8, ()> {
        unsafe {
            let t = self.current.add(1);
            if t > self.expected_end {
                return Err(());
            }
            let value = self.current.read();
            self.current = t;

            Ok(value)
        }
    }
    fn read_u32(&mut self) -> Result<u32, ()> {
        unsafe {
            let t = self.current.add(4);
            if t > self.expected_end {
                return Err(());
            }
            let be_bytes = self.current.cast::<[u8; 4]>().read();
            self.current = t;

            Ok(u32::from_be_bytes(be_bytes))
        }
    }
    fn skip_bytes(&mut self, len: u32) -> Result<(), ()> {
        unsafe {
            let t = self.current.add(len as usize);
            if t > self.expected_end {
                return Err(());
            }
            self.current = t;

            Ok(())
        }
    }
    fn read_u256_le(&mut self, _dst: &mut [u64; 4]) -> Result<(), ()> {
        unimplemented!()
        // unsafe {
        //     let t = self.current.add(32);
        //     if t as *const u8 > self.expected_end {
        //         return Err(())
        //     }
        //     let mut result = [0u32; 8];
        //     for dst in result.iter_mut().rev() {
        //         let be_bytes = self.current.cast::<[u8; 4]>().read();
        //         *dst = u32::from_be_bytes(be_bytes);
        //         self.current = self.current.add(4);
        //     }

        //     Ok(result)
        // }
    }
}

impl ABIDecodingWalker<'_> {
    fn create_subdecoder(&self, offset: u32) -> Result<Self, ()> {
        unsafe {
            let t = self.start.add(offset as usize);
            if t > self.absolute_end {
                return Err(());
            }
            Ok(Self {
                start: t,
                current: t,
                expected_end: t,
                absolute_end: self.absolute_end,
                _marker: core::marker::PhantomData,
            })
        }
    }

    fn clone_self(&self) -> Self {
        Self {
            start: self.start,
            current: self.current,
            expected_end: self.expected_end,
            absolute_end: self.absolute_end,
            _marker: core::marker::PhantomData,
        }
    }

    fn add_expected_len(&mut self, offset: u32) -> Result<(), ()> {
        unsafe {
            let t = self.start.add(offset as usize);
            if t > self.absolute_end {
                return Err(());
            }
            self.expected_end = t;

            Ok(())
        }
    }
}

impl DecoderInterface for ABIDecodingWalker<'_> {
    type StructDecoder<'a>
        = ABIDecodingWalker<'a>
    where
        Self: 'a;

    fn create_struct_decoder<T: ABIDecodable>(
        &mut self,
        offset: u32,
    ) -> Result<Self::StructDecoder<'_>, ()> {
        // NOTE: that just works for Vec<T> type by claiming that it's "length" is a minimal readable 32 bytes,
        // and then sequence decoder will do the job
        let mut subdecoder = self.create_subdecoder(offset)?;
        let expected_len = T::head_encoding_size();
        subdecoder.add_expected_len(expected_len)?;

        Ok(subdecoder)
    }

    type SequenceDecoder<'a>
        = ABIDecodingWalker<'a>
    where
        Self: 'a;
    fn create_sequence_decoder<T: ABIDecodable>(
        &mut self,
        len: u32,
    ) -> Result<Self::SequenceDecoder<'_>, ()> {
        // println!("Creating sequence decoder for type {:?} of len {}", std::any::type_name::<T>(), len);
        // we should consider "current" as new "start"
        let mut subdecoder = self.clone_self();
        subdecoder.start = subdecoder.current;
        subdecoder.expected_end = subdecoder.start;
        if T::is_dynamic() {
            subdecoder.add_expected_len(32 * len)?;
        } else {
            let type_size = T::head_encoding_size();
            subdecoder.add_expected_len(type_size * len)?;
        }

        Ok(subdecoder)
    }

    fn decode_field<T: ABIDecodable>(&mut self) -> Result<T, ()> {
        // println!("Trying to decode {:?}", std::any::type_name::<T>());
        let element = if T::is_dynamic() {
            let offset = <u32 as ABIDecodable>::read(self)?;
            // println!("{:?} is dynamic, reading at offset 0x{:x}", std::any::type_name::<T>(), offset);
            // then descent
            let mut encoder = self.create_struct_decoder::<T>(offset)?;

            T::read(&mut encoder)?
        } else {
            T::read(self)?
        };

        Ok(element)
    }
}
