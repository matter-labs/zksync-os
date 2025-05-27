/// Represents that fat pointer to be used through the FFI boundary. It is only ever read through the
/// FFI when compiled to WASM and usize length is 4.
#[repr(C)]
pub struct SliceRef<TWord> {
    pub ptr: TWord,
    pub len: TWord,
}

impl<TWord> SliceRef<TWord> {
    // #[cfg(target_pointer_width = "32")]
    // pub fn from_ref<T>(rf: &[T]) -> SliceRef<u32> {
    //
    //     SliceRef {
    //         ptr: rf as *const _ as *const () as usize as u32,
    //         len: rf.len() as u32,
    //     }
    // }
    //
    //
    // #[cfg(target_pointer_width = "64")]
    pub fn from_ref<T>(rf: &[T]) -> SliceRef<usize> {
        SliceRef {
            ptr: rf as *const _ as *const () as usize,
            len: rf.len(),
        }
    }
}

impl SliceRef<usize> {
    #[allow(clippy::should_implement_trait)]
    pub fn as_ref(&self) -> &[u8] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

#[repr(C)]
pub struct HostOpResult {
    pub success: bool,
    pub param: u64,
}

impl From<HostOpResult> for (bool, u64) {
    fn from(value: HostOpResult) -> Self {
        (value.success, value.param)
    }
}
