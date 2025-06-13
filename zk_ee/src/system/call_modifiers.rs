#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub enum CallModifier {
    #[default]
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
