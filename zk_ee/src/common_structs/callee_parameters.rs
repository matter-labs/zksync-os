use crate::system::Ergs;

pub struct CalleeParameters<'a> {
    pub next_ee_version: u8,
    pub bytecode: &'a [u8],
    pub bytecode_len: u32,
    pub artifacts_len: u32,
    pub stipend: Option<Ergs>,
}
