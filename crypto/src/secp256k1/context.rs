#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use core::alloc::{AllocError, Allocator, Layout};

use super::points::{AffineStorage, JacobianConst};

pub(crate) const WINDOW_A: usize = 5;

pub(crate) const WINDOW_G: usize = 10;

pub(crate) const ECMULT_TABLE_SIZE_A: usize = 1 << (WINDOW_A - 2);
pub(crate) const ECMULT_TABLE_SIZE_G: usize = 1 << (WINDOW_G - 2);
pub(crate) const WNAF_BITS: usize = 129;

pub struct ECMultContext {
    pub(crate) pre_g: [AffineStorage; ECMULT_TABLE_SIZE_G],
    pub(crate) pre_g_128: [AffineStorage; ECMULT_TABLE_SIZE_G],
}

#[cfg(feature = "secp256k1-static-context")]
pub(crate) const ECRECOVER_CONTEXT: ECMultContext = ECMultContext::const_new();

impl ECMultContext {
    /// Creates a new context instance on the stack.
    /// Calling this function in runtime is likely to cause stack overflow.
    /// Instead, use `new_in` to create an instance on the heap
    pub const fn const_new() -> Self {
        let mut context = Self {
            pre_g: [AffineStorage::DEFAULT; ECMULT_TABLE_SIZE_G],
            pre_g_128: [AffineStorage::DEFAULT; ECMULT_TABLE_SIZE_G],
        };

        context.initialize();

        context
    }

    const fn compute_table(table: &mut [AffineStorage; ECMULT_TABLE_SIZE_G], gen: &JacobianConst) {
        use const_for::const_for;

        let mut gj = *gen;
        // 1 * G
        table[0] = gj.to_affine_storage_const();

        let g_doubled = gen.double(None);
        // step by 2*G
        let g_doubled = g_doubled.to_affine_const();

        const_for!(i in 1..ECMULT_TABLE_SIZE_G => {
            // += 2 * G
            gj = gj.add_ge(&g_doubled, None);
            // write it down
            table[i] = gj.to_affine_storage_const();
        });
    }

    const fn initialize(&mut self) {
        let mut gj = JacobianConst::GENERATOR;

        Self::compute_table(&mut self.pre_g, &gj);

        use const_for::const_for;
        const_for!(_ in 0..128 => {
            gj = gj.double(None);
        });

        Self::compute_table(&mut self.pre_g_128, &gj);
    }

    #[cfg(feature = "alloc")]
    pub fn new_in<A: Allocator>(allocator: A) -> Result<Box<Self, A>, AllocError> {
        let mut context = unsafe {
            let layout = Layout::new::<ECMultContext>();
            let ptr = allocator.allocate(layout)?.cast::<ECMultContext>().as_ptr();
            let mut this = Box::from_raw_in(ptr, allocator);

            for i in 0..ECMULT_TABLE_SIZE_G {
                this.pre_g[i] = AffineStorage::DEFAULT;
                this.pre_g_128[i] = AffineStorage::DEFAULT;
            }

            this
        };

        context.initialize();

        Ok(context)
    }

    pub fn new_from_raw_unchecked(
        pre_g: [AffineStorage; ECMULT_TABLE_SIZE_G],
        pre_g_128: [AffineStorage; ECMULT_TABLE_SIZE_G],
    ) -> Self {
        Self { pre_g, pre_g_128 }
    }
}
