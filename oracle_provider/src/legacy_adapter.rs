use zk_ee::oracle::IOOracle;

pub struct LegacyAdapter<O: IOOracle> {
    pub inner: O,
}

impl<O: IOOracle> LegacyAdapter<O> {
    pub fn new(inner: O) -> Self {
        Self { inner }
    }
}
