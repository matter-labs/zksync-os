#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum CallModifier {
    NoModifier = 0,
    Constructor,
    Delegate,
    Static,
    DelegateStatic,
    ZKVMSystem,
    ZKVMSystemStatic,
    EVMCallcode,
    EVMCallcodeStatic,
}
