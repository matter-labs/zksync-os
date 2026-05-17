use airbender_guest::input::read_with;
use airbender_guest::transport::Transport;
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::internal_error;
use zk_ee::oracle::serde_oracle::SerdeIOOracle;
use zk_ee::system::errors::internal::InternalError;

pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport> ProvingOracle<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: Transport + 'static> SerdeIOOracle for ProvingOracle<T> {
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        read_with::<O>(&mut self.transport)
            .map_err(|_e| internal_error!("proving oracle read failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use airbender_core::wire::frame_words_from_bytes;
    use airbender_guest::transport::MockTransport;

    fn encode_value<T: serde::Serialize>(value: &T) -> Vec<u32> {
        let bytes = AirbenderCodecV0::encode(value).expect("encode");
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
