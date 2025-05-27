// we want to have some abstraction over structures that can be diffed,
// and structures that can be rolled back

pub mod impls;

pub trait ByteEncodable {
    fn encoding_size(&self) -> usize;
    fn write_into_slice(&self, dst: &mut [u8]) -> Result<usize, ()>;
}

pub trait Rollbackable {
    type ReadOrTouchRollbackInformation: 'static + Clone + core::fmt::Debug;
    type WriteRollbackInformation: 'static + Clone + core::fmt::Debug;
    type InitValue: 'static + Clone + core::fmt::Debug;
    type Value: 'static
        + Clone
        + core::fmt::Debug
        + core::default::Default
        + core::cmp::PartialEq
        + core::cmp::Eq;
    type AuxData: 'static + Clone + core::fmt::Debug;

    fn create_initial(value: Self::InitValue) -> Self;
    fn touch(
        &'_ mut self,
        extra_data: &Self::AuxData,
    ) -> (
        &'_ <Self as Rollbackable>::Value,
        Self::ReadOrTouchRollbackInformation,
    );
    fn read(
        &'_ mut self,
        extra_data: &Self::AuxData,
    ) -> (
        &'_ <Self as Rollbackable>::Value,
        Self::ReadOrTouchRollbackInformation,
    );
    fn update(
        &mut self,
        update: &Self::Value,
        extra_data: &Self::AuxData,
    ) -> Self::WriteRollbackInformation;
    fn rollback_read(&mut self, rollback: &Self::ReadOrTouchRollbackInformation);
    fn rollback_write(&mut self, rollback: &Self::WriteRollbackInformation);
    fn current_value(&self) -> &Self::Value;
    fn is_used(&self) -> bool;
}

pub trait KeyLikeWithBounds:
    'static + Clone + Copy + core::cmp::Ord + core::cmp::Eq + core::fmt::Debug
{
    type Subspace: 'static + Clone + core::fmt::Debug;
    fn lower_bound(subspace: Self::Subspace) -> Self;
    fn upper_bound(subspace: Self::Subspace) -> Self;
}
