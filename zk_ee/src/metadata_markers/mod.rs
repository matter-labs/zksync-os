//! Minimal, type-directed metadata query system.
//!
//! Goals:
//! - Keep the set of metadata requests small and explicit.
//! - Use TypeId to route requests without large enums or dynamic strings.
//!
//! Note: TypeId does not need to be stable across compilation boundaries. All
//! dispatch happens within a single compiled binary.

pub mod basic_metadata;
// pub mod block_hashes_cache;
pub mod system_metadata;

/// A typed metadata query.
///
/// Each request defines an input and output that are Copy and 'static so
/// responders can forward values by value without drop semantics.
pub trait MetadataRequest: 'static + Sized {
    type Input: 'static + Copy;
    type Output: 'static + Copy;
}

/// Trait for components that can answer a subset of MetadataRequest types.
///
/// Callers must first check `can_respond::<M>()` before invoking
/// `get_metadata_with_bookkeeping::<M>(...)`. Implementations may update
/// internal counters or caches while serving the request.
pub trait DynamicMetadataResponder {
    /// Advertise whether this responder can serve request type M.
    #[inline(always)]
    fn can_respond<M: MetadataRequest>() -> bool {
        false
    }

    /// Serve the request and optionally update bookkeeping.
    ///
    /// Panics if `can_respond::<M>()` is false. Callers are expected to pre-check.
    fn get_metadata_with_bookkeeping<M: MetadataRequest>(&mut self, _input: M::Input) -> M::Output {
        unreachable!("ability to query metadata should be pre-checked");
    }

    /// Internal helper: reinterpret-cast input between the same logical request type.
    ///
    /// Safety: this asserts that `M` and `U` are the same request type (by TypeId).
    /// Under that invariant, their associated Input types are identical, so bit-cast is sound.
    fn cast_input<M: MetadataRequest, U: MetadataRequest>(input: M::Input) -> U::Input {
        assert_eq!(core::any::TypeId::of::<M>(), core::any::TypeId::of::<U>());
        // SAFETY: proven identical request type ⇒ identical associated Input type.
        unsafe { core::ptr::read((&input as *const M::Input).cast::<U::Input>()) }
    }

    /// Internal helper: reinterpret-cast output between the same logical request type.
    ///
    /// Safety: same reasoning as in `cast_input`.
    fn cast_output<M: MetadataRequest, U: MetadataRequest>(output: M::Output) -> U::Output {
        assert_eq!(core::any::TypeId::of::<M>(), core::any::TypeId::of::<U>());
        // SAFETY: proven identical request type ⇒ identical associated Output type.
        unsafe { core::ptr::read((&output as *const M::Output).cast::<U::Output>()) }
    }
}

/// Unit type: a responder that never answers anything.
impl DynamicMetadataResponder for () {
    #[inline(always)]
    fn can_respond<M: MetadataRequest>() -> bool {
        false
    }
    fn get_metadata_with_bookkeeping<M: MetadataRequest>(&mut self, _input: M::Input) -> M::Output {
        unreachable!("ability to query metadata should be pre-checked");
    }
}
