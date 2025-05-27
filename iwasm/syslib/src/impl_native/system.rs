// This is used to establish a test environment in an easy manner, and there's a guard against using
// it concurrently.
#[allow(static_mut_refs)]
pub mod env {
    use crate::types::ints::U256BE;
    use alloc::boxed::Box;
    use core::sync::atomic::AtomicUsize;

    static mut CURRENT_ENV: Option<Box<dyn Env>> = Option::None;
    static mut ENV_COUNTER: AtomicUsize = AtomicUsize::new(0);

    pub trait Env {
        fn get_selector(&self) -> u32;
        fn get_calldata(&self) -> &[u8];
        fn storage_read_s(&self, ix: &U256BE) -> U256BE;
        fn storage_write_s(&mut self, ix: &U256BE, v: &U256BE);
    }

    pub fn set(env: Box<dyn Env>) {
        unsafe {
            if ENV_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed) != 0 {
                panic!("Attempting to set environment concurrently. Concurrent environments aren't supported.");
            }
            CURRENT_ENV = Some(env);
        }
    }

    pub fn unset() {
        unsafe { ENV_COUNTER.fetch_sub(1, core::sync::atomic::Ordering::Relaxed) };
    }

    pub(crate) fn access<R, F: FnOnce(&mut dyn Env) -> R>(f: F) -> R {
        let env = unsafe {
            CURRENT_ENV
                .as_mut()
                .expect(
                    "Environment not set. Use `syslib::system::env::set` to set the environment.",
                )
                .as_mut()
        };

        f(env)
    }
}

pub mod calldata {
    use core::mem::MaybeUninit;

    use super::env;

    pub fn selector() -> u32 {
        env::access(|x| x.get_selector())
    }

    pub fn size() -> u32 {
        env::access(|x| x.get_calldata().len()) as u32
    }

    pub fn read_into(tgt: &mut [MaybeUninit<u8>]) {
        env::access(|x| {
            let calldata = x.get_calldata();

            unsafe {
                let calldata: &[MaybeUninit<u8>] = &*(calldata as *const _ as *const _);
                tgt.copy_from_slice(calldata);
            }
        });
    }
}

pub mod storage {
    use crate::types::ints::U256BE;

    use super::env;

    pub fn read_s(ix: &U256BE) -> U256BE {
        env::access(|x| x.storage_read_s(ix))
    }

    pub fn write_s(ix: &U256BE, value: &U256BE) {
        env::access(|x| x.storage_write_s(ix, value))
    }
}

pub mod msg {
    use crate::types::Address;

    pub fn sender() -> Address {
        todo!();
    }
}

pub mod slice {
    use crate::types::ints::U256BE;

    pub fn hash_keccak256(input: &[u8]) -> U256BE {
        todo!();
    }
}
