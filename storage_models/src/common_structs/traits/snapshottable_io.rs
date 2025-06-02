/// TODO
pub trait SnapshottableIo {
    type TxStats;
    type StateSnapshot;

    fn begin_new_tx(&mut self);
    fn tx_stats(&self) -> Self::TxStats;

    fn start_frame(&mut self) -> Self::StateSnapshot;
    fn finish_frame(&mut self, rollback_handle: Option<&Self::StateSnapshot>);
}
