use zk_ee::system::errors::internal::InternalError;

/// TODO
pub trait SnapshottableIo {
    type StateSnapshot;

    fn begin_new_tx(&mut self);

    fn start_frame(&mut self) -> Self::StateSnapshot;
    fn finish_frame(
        &mut self,
        rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError>;
}
