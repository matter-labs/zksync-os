use airbender_host::Inputs;
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::internal_error;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

pub struct WitnessRecordingOracle<O: IOOracle> {
    inner: O,
    inputs: Inputs,
}

impl<O: IOOracle> WitnessRecordingOracle<O> {
    pub fn new(inner: O) -> Self {
        Self {
            inner,
            inputs: Inputs::new(),
        }
    }

    pub fn into_inputs(self) -> (O, Inputs) {
        (self.inner, self.inputs)
    }
}

impl<O: IOOracle> IOOracle for WitnessRecordingOracle<O> {
    fn query<I: Serialize, R: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<R, InternalError> {
        let response: R = self.inner.query(query_type, input)?;
        self.inputs
            .push(&response)
            .map_err(|_| internal_error!("witness recording failed"))?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use airbender_guest::input::read_with;
    use airbender_guest::transport::MockTransport;

    fn encode_value<T: Serialize>(v: &T) -> Vec<u8> {
        AirbenderCodecV0::encode(v).expect("encode")
    }

    struct FixedOracle {
        values: Vec<Vec<u8>>,
        cursor: usize,
    }

    impl FixedOracle {
        fn new(values: Vec<Vec<u8>>) -> Self {
            Self { values, cursor: 0 }
        }
    }

    impl IOOracle for FixedOracle {
        fn query<I: Serialize, O: DeserializeOwned + Serialize>(
            &mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<O, InternalError> {
            let bytes = &self.values[self.cursor];
            self.cursor += 1;
            AirbenderCodecV0::decode(bytes).map_err(|_| zk_ee::internal_error!("decode failed"))
        }
    }

    use basic_bootloader::bootloader::oracle_types::{
        DivRemResponse, FieldSqrtResponse, ModexpResponse,
    };

    #[test]
    fn roundtrip_mixed_types_including_callable_oracle_responses() {
        let inner = FixedOracle::new(vec![
            encode_value(&42u32),
            encode_value(&DivRemResponse {
                quotient: [1, 2, 3, 4],
            }),
            encode_value(&ModexpResponse {
                quotient: vec![0xAA, 0xBB],
                remainder: vec![0xCC],
            }),
            encode_value(&FieldSqrtResponse {
                result: zk_ee::utils::Bytes32::zero(),
                is_valid: true,
            }),
        ]);
        let mut recorder = WitnessRecordingOracle::new(inner);

        let _: u32 = recorder.query(0x40070000, &()).unwrap();
        let _: DivRemResponse = recorder.query(0x40050030, &(0u32,)).unwrap();
        let _: ModexpResponse = recorder.query(0x40050010, &(0u32,)).unwrap();
        let _: FieldSqrtResponse = recorder.query(0x40050011, &(0u32,)).unwrap();

        let (_, inputs) = recorder.into_inputs();
        let mut transport = MockTransport::new(inputs.words().to_vec());

        let r1: u32 = read_with(&mut transport).unwrap();
        let r2: DivRemResponse = read_with(&mut transport).unwrap();
        let r3: ModexpResponse = read_with(&mut transport).unwrap();
        let r4: FieldSqrtResponse = read_with(&mut transport).unwrap();

        assert_eq!(r1, 42);
        assert_eq!(r2.quotient, [1, 2, 3, 4]);
        assert_eq!(r3.quotient, vec![0xAA, 0xBB]);
        assert_eq!(r3.remainder, vec![0xCC]);
        assert!(r4.is_valid);
    }

    #[test]
    fn roundtrip_recording_and_replay() {
        let inner = FixedOracle::new(vec![
            encode_value(&42u32),
            encode_value(&99u32),
            encode_value(&7u32),
        ]);
        let mut recorder = WitnessRecordingOracle::new(inner);

        let v1: u32 = recorder.query(0, &()).unwrap();
        let v2: u32 = recorder.query(0, &()).unwrap();
        let v3: u32 = recorder.query(0, &()).unwrap();

        assert_eq!((v1, v2, v3), (42, 99, 7));

        let (_inner, inputs) = recorder.into_inputs();
        let witness_words = inputs.words().to_vec();

        let mut transport = MockTransport::new(witness_words);
        let r1: u32 = read_with(&mut transport).unwrap();
        let r2: u32 = read_with(&mut transport).unwrap();
        let r3: u32 = read_with(&mut transport).unwrap();

        assert_eq!((r1, r2, r3), (42, 99, 7));
    }
}
