use airbender_codec::{AirbenderCodec, AirbenderCodecV1, CodecError};
use airbender_guest::transport::Transport;
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::internal_error;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

const STACK_BUF_SIZE: usize = 512;

pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport> ProvingOracle<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    fn read_value<O: DeserializeOwned>(&mut self) -> Result<O, CodecError> {
        let len = self.transport.read_word() as usize;
        let words_needed = (len + 3) / 4;

        if len <= STACK_BUF_SIZE {
            let mut buf = [0u8; STACK_BUF_SIZE];
            let mut offset = 0;
            for _ in 0..words_needed {
                let word_bytes = self.transport.read_word().to_le_bytes();
                let to_copy = (len - offset).min(4);
                buf[offset..offset + to_copy].copy_from_slice(&word_bytes[..to_copy]);
                offset += to_copy;
            }
            AirbenderCodecV1::decode(&buf[..len])
        } else {
            let mut bytes = alloc::vec![0u8; len];
            let mut offset = 0;
            for _ in 0..words_needed {
                let word_bytes = self.transport.read_word().to_le_bytes();
                let to_copy = (len - offset).min(4);
                bytes[offset..offset + to_copy].copy_from_slice(&word_bytes[..to_copy]);
                offset += to_copy;
            }
            AirbenderCodecV1::decode(&bytes)
        }
    }
}

impl<T: Transport + 'static> IOOracle for ProvingOracle<T> {
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        self.read_value()
            .map_err(|_e| internal_error!("proving oracle read failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airbender_codec::{AirbenderCodec, AirbenderCodecV1};
    use airbender_core::wire::frame_words_from_bytes;
    use airbender_guest::transport::MockTransport;

    fn encode_value<T: serde::Serialize>(value: &T) -> Vec<u32> {
        let bytes = AirbenderCodecV1::encode(value).expect("encode");
        frame_words_from_bytes(&bytes).expect("frame")
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
