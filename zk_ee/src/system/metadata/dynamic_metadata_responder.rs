pub trait MetadataRequest: 'static + Sized {
    type Input: 'static + Copy; // no drop
    type Output: 'static + Copy; // no drop
}

pub trait DynamicMetadataResponder {
    #[inline(always)]
    fn can_respond<M: MetadataRequest>() -> bool {
        false
    }
    // For optimization purposes we may want some bookkeeping here
    fn get_metadata_with_bookkeeping<M: MetadataRequest>(&mut self, _input: M::Input) -> M::Output {
        unreachable!("ability to query metadata should be pre-checked");
    }

    fn cast_input<M: MetadataRequest, U: MetadataRequest>(input: M::Input) -> U::Input {
        assert_eq!(core::any::TypeId::of::<M>(), core::any::TypeId::of::<U>());

        unsafe { core::ptr::read((&input as *const M::Input).cast::<U::Input>()) }
    }

    fn cast_output<M: MetadataRequest, U: MetadataRequest>(output: M::Output) -> U::Output {
        assert_eq!(core::any::TypeId::of::<M>(), core::any::TypeId::of::<U>());

        unsafe { core::ptr::read((&output as *const M::Output).cast::<U::Output>()) }
    }
}

struct EmptyMetadata;
pub struct MetadataCollection<T, U> {
    first: T,
    #[allow(dead_code)]
    second: U,
}

impl DynamicMetadataResponder for () {
    #[inline(always)]
    fn can_respond<M: MetadataRequest>() -> bool {
        false
    }
    fn get_metadata_with_bookkeeping<M: MetadataRequest>(&mut self, _input: M::Input) -> M::Output {
        unreachable!("ability to query metadata should be pre-checked");
    }
}

impl<T: DynamicMetadataResponder> DynamicMetadataResponder
    for MetadataCollection<T, EmptyMetadata>
{
    #[inline(always)]
    fn can_respond<M: MetadataRequest>() -> bool {
        <T as DynamicMetadataResponder>::can_respond::<M>()
    }
    fn get_metadata_with_bookkeeping<M: MetadataRequest>(&mut self, input: M::Input) -> M::Output {
        if <T as DynamicMetadataResponder>::can_respond::<M>() {
            self.first.get_metadata_with_bookkeeping::<M>(input)
        } else {
            unreachable!("ability to query metadata should be pre-checked");
        }
    }
}

impl<T: DynamicMetadataResponder> MetadataCollection<T, EmptyMetadata> {
    pub fn initial(first: T) -> Self {
        Self {
            first,
            second: EmptyMetadata,
        }
    }

    pub fn add_responder<U: DynamicMetadataResponder>(
        self,
        next_responder: U,
    ) -> MetadataCollection<MetadataCollection<T, U>, EmptyMetadata> {
        MetadataCollection {
            first: MetadataCollection {
                first: self.first,
                second: next_responder,
            },
            second: EmptyMetadata,
        }
    }
}
