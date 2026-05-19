use airbender_guest::transport::Transport;
use airbender_guest::word_reader::WordReader;
use core::mem::MaybeUninit;
use zk_ee::internal_error;
use zk_ee::oracle::RawWordReadable;
use zk_ee::system::errors::internal::InternalError;

pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport> ProvingOracle<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Read a fixed-size type by writing raw u32 words directly into the
    /// destination. No wincode, no WordReader, no intermediaries.
    #[inline(always)]
    fn read_raw<O: RawWordReadable>(&mut self) -> O {
        const { assert!(core::mem::size_of::<O>() % 4 == 0) };
        let mut result = MaybeUninit::<O>::uninit();
        let dst = result.as_mut_ptr() as *mut u32;
        let word_count = core::mem::size_of::<O>() / 4;
        for i in 0..word_count {
            unsafe {
                dst.add(i).write(self.transport.read_word());
            }
        }
        unsafe { result.assume_init() }
    }

    /// Read a type via wincode deserialization through WordReader.
    #[inline(always)]
    fn read_wincode<O: zk_ee::oracle::WincodeDeserialize>(&mut self) -> Result<O, InternalError> {
        let reader = WordReader::new(&mut self.transport);
        wincode::deserialize_from(reader)
            .map_err(|_e| internal_error!("proving oracle read failed"))
    }
}

/// Helper trait to dispatch between raw word reads and wincode deserialization.
/// Uses the `RawWordReadable` marker to select the fast path at monomorphization time.
trait ReadDispatch: zk_ee::oracle::WincodeDeserialize + Sized {
    fn read_from<T: Transport>(oracle: &mut ProvingOracle<T>) -> Result<Self, InternalError>;
}

// Default: use wincode
impl<O: zk_ee::oracle::WincodeDeserialize> ReadDispatch for O {
    default fn read_from<T: Transport>(
        oracle: &mut ProvingOracle<T>,
    ) -> Result<Self, InternalError> {
        oracle.read_wincode()
    }
}

// Specialization: use raw word reads for RawWordReadable types
impl<O: zk_ee::oracle::WincodeDeserialize + RawWordReadable> ReadDispatch for O {
    fn read_from<T: Transport>(oracle: &mut ProvingOracle<T>) -> Result<Self, InternalError> {
        Ok(oracle.read_raw())
    }
}

impl<T: Transport + 'static> zk_ee::oracle::IOOracle for ProvingOracle<T> {
    fn query<
        I: zk_ee::oracle::WincodeSerialize,
        O: zk_ee::oracle::WincodeDeserialize + zk_ee::oracle::WincodeSerialize,
    >(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        O::read_from(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airbender_guest::transport::MockTransport;
    use zk_ee::oracle::IOOracle;

    fn encode_value<T: wincode::Serialize<Src = T>>(value: &T) -> Vec<u32> {
        let bytes = wincode::serialize(value).expect("encode");
        let mut words = Vec::new();
        for chunk in bytes.chunks(4) {
            let mut buf = [0u8; 4];
            buf[..chunk.len()].copy_from_slice(chunk);
            words.push(u32::from_le_bytes(buf));
        }
        words
    }

    #[test]
    fn reads_sequential_typed_values() {
        let mut words = Vec::new();
        words.extend(encode_value(&42u32));
        words.extend(encode_value(&true));
        words.extend(encode_value(&0xDEADBEEFu64));
        let mut oracle = ProvingOracle::new(MockTransport::new(words));

        let v1: u32 = oracle.query(0, &()).unwrap();
        let v2: bool = oracle.query(0, &()).unwrap();
        let v3: u64 = oracle.query(0, &()).unwrap();

        assert_eq!(v1, 42);
        assert_eq!(v2, true);
        assert_eq!(v3, 0xDEADBEEF);
    }

    #[test]
    fn ignores_query_type_and_input() {
        let words = encode_value(&99u32);
        let mut oracle = ProvingOracle::new(MockTransport::new(words));

        let result: u32 = oracle.query(0x40070000, &(1u32, 2u32, 3u32)).unwrap();
        assert_eq!(result, 99);
    }
}
