pub enum Assert<const COND: bool> {}

pub trait IsTrue {}

impl IsTrue for Assert<true> {}
