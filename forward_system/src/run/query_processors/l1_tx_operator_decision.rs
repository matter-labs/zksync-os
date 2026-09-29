use super::*;
use oracle_provider::{ReadWitnessSource, ZkEENonDeterminismSource};
use std::collections::VecDeque;
use zk_ee::oracle::query_ids::L1_TX_OPERATOR_DECISION_QUERY_ID;
use zk_ee::system::validator::{L1TxOperatorDecision, OracleTxValidator};
use zksync_os_interface::types::L1TxOperatorDecision as InterfaceDecision;

/// The operator's recorded decisions on priority (L1 -> L2) transactions, in
/// block order, collected from the sequencing run's transaction outputs.
///
/// Serves `L1_TX_OPERATOR_DECISION_QUERY_ID` for the `OracleTxValidator` of a
/// witness-generation run (see [`Self::into_witness_validator`]). The bootloader
/// issues one query per included priority transaction when filtering is enabled,
/// so running out of decisions is a wiring bug and panics like the other responders do.
#[derive(Clone, Debug, Default)]
pub struct L1TxOperatorDecisionResponder {
    decisions: VecDeque<L1TxOperatorDecision>,
}

impl L1TxOperatorDecisionResponder {
    /// The validator of a witness-generation run: replays these decisions
    /// through a side oracle whose reads are appended to `witness`'s read log,
    /// interleaved in program order with the block oracle's reads.
    pub fn into_witness_validator(
        self,
        witness: &ReadWitnessSource,
    ) -> OracleTxValidator<ReadWitnessSource> {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(self);
        OracleTxValidator::new(ReadWitnessSource::with_shared_read_items(
            oracle,
            witness.get_read_items(),
        ))
    }
}

impl FromIterator<InterfaceDecision> for L1TxOperatorDecisionResponder {
    fn from_iter<T: IntoIterator<Item = InterfaceDecision>>(decisions: T) -> Self {
        Self {
            decisions: decisions
                .into_iter()
                .map(|decision| match decision {
                    InterfaceDecision::Accepted => L1TxOperatorDecision::Accepted,
                    InterfaceDecision::RejectedBeforeExecution => {
                        L1TxOperatorDecision::RejectedBeforeExecution
                    }
                    InterfaceDecision::RejectedAfterExecution => {
                        L1TxOperatorDecision::RejectedAfterExecution
                    }
                })
                .collect(),
        }
    }
}

impl OracleQueryProcessor for L1TxOperatorDecisionResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![L1_TX_OPERATOR_DECISION_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert_eq!(query_id, L1_TX_OPERATOR_DECISION_QUERY_ID);

        let decision = self
            .decisions
            .pop_front()
            .expect("operator decision queried for more priority transactions than were recorded");
        DynUsizeIterator::from_constructor(decision, UsizeSerializable::iter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oracle_provider::DummyMemorySource;

    #[test]
    fn replays_recorded_decisions_in_order() {
        let mut responder: L1TxOperatorDecisionResponder = [
            InterfaceDecision::RejectedAfterExecution,
            InterfaceDecision::Accepted,
            InterfaceDecision::RejectedBeforeExecution,
        ]
        .into_iter()
        .collect();

        for expected in [
            L1TxOperatorDecision::RejectedAfterExecution,
            L1TxOperatorDecision::Accepted,
            L1TxOperatorDecision::RejectedBeforeExecution,
        ] {
            let mut words = responder.process_buffered_query(
                L1_TX_OPERATOR_DECISION_QUERY_ID,
                vec![],
                &DummyMemorySource,
            );
            let decoded = L1TxOperatorDecision::from_iter(&mut words).unwrap();
            assert_eq!(decoded, expected);
        }
        assert!(responder.decisions.is_empty());
    }

    #[test]
    #[should_panic(expected = "more priority transactions than were recorded")]
    fn panics_when_decisions_run_out() {
        let mut responder = L1TxOperatorDecisionResponder::default();
        let _ = responder.process_buffered_query(
            L1_TX_OPERATOR_DECISION_QUERY_ID,
            vec![],
            &DummyMemorySource,
        );
    }
}
