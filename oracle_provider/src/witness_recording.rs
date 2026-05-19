use zk_ee::internal_error;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

pub struct WitnessRecordingOracle<O: IOOracle> {
    inner: O,
    witness_words: Vec<u32>,
}

impl<O: IOOracle> WitnessRecordingOracle<O> {
    pub fn new(inner: O) -> Self {
        Self {
            inner,
            witness_words: Vec::new(),
        }
    }

    pub fn into_witness(self) -> (O, Vec<u32>) {
        (self.inner, self.witness_words)
    }
}

impl<O: IOOracle> IOOracle for WitnessRecordingOracle<O> {
    fn query<
        I: zk_ee::oracle::WincodeSerialize,
        R: zk_ee::oracle::WincodeDeserialize + zk_ee::oracle::WincodeSerialize,
    >(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<R, InternalError> {
        let response: R = self.inner.query(query_type, input)?;
        let response_bytes = wincode::serialize(&response)
            .map_err(|_| internal_error!("witness recording: wincode serialize failed"))?;
        // Push raw LE u32 words — no framing. WordReader on the guest reads
        // these directly without expecting a length prefix.
        for chunk in response_bytes.chunks(4) {
            let mut buf = [0u8; 4];
            buf[..chunk.len()].copy_from_slice(chunk);
            self.witness_words.push(u32::from_le_bytes(buf));
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use airbender_guest::transport::MockTransport;
    use airbender_guest::word_reader::WordReader;

    fn encode_value<T: wincode::SchemaWrite<wincode::config::DefaultConfig, Src = T>>(
        v: &T,
    ) -> Vec<u8> {
        wincode::serialize(v).expect("encode")
    }

    fn read_wincode<T: zk_ee::oracle::WincodeDeserialize>(
        transport: &mut MockTransport,
    ) -> Result<T, InternalError> {
        let reader = WordReader::new(transport);
        wincode::deserialize_from(reader)
            .map_err(|_| zk_ee::internal_error!("wincode decode failed"))
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
        fn query<
            I: zk_ee::oracle::WincodeSerialize,
            O: zk_ee::oracle::WincodeDeserialize + zk_ee::oracle::WincodeSerialize,
        >(
            &mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<O, InternalError> {
            let bytes = &self.values[self.cursor];
            self.cursor += 1;
            wincode::deserialize(bytes).map_err(|_| zk_ee::internal_error!("decode failed"))
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
        let _: DivRemResponse = recorder.query(0x40050030, &0u32).unwrap();
        let _: ModexpResponse = recorder.query(0x40050010, &0u32).unwrap();
        let _: FieldSqrtResponse = recorder.query(0x40050011, &0u32).unwrap();

        let (_, witness_words) = recorder.into_witness();
        let mut transport = MockTransport::new(witness_words);

        let r1: u32 = read_wincode(&mut transport).unwrap();
        let r2: DivRemResponse = read_wincode(&mut transport).unwrap();
        let r3: ModexpResponse = read_wincode(&mut transport).unwrap();
        let r4: FieldSqrtResponse = read_wincode(&mut transport).unwrap();

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

        let (_inner, witness_words) = recorder.into_witness();

        let mut transport = MockTransport::new(witness_words);
        let r1: u32 = read_wincode(&mut transport).unwrap();
        let r2: u32 = read_wincode(&mut transport).unwrap();
        let r3: u32 = read_wincode(&mut transport).unwrap();

        assert_eq!((r1, r2, r3), (42, 99, 7));
    }
}
