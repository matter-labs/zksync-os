//! Case outcomes and how they map onto finding severities.
//!
//! Outcomes are grouped by a coarse signature for shrinking and deduplication.
//! See the README for comparison limits and exit codes.

use serde::{Deserialize, Serialize};

/// The coarse class of a case outcome. Two runs with the same class are "the
/// same kind of finding" for the purposes of shrinking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeClass {
    Ok,
    InternalError,
    /// A divergence in the forced-gas comparison, requiring triage.
    RevmMismatch,
    /// A divergence only in the independent-gas comparison, requiring triage.
    GasScheduleDiff,
    Slow,
    Panic,
}

impl OutcomeClass {
    /// A short, filename-safe tag.
    pub fn tag(self) -> &'static str {
        match self {
            OutcomeClass::Ok => "ok",
            OutcomeClass::InternalError => "internal-error",
            OutcomeClass::RevmMismatch => "revm-mismatch",
            OutcomeClass::GasScheduleDiff => "gas-schedule-diff",
            OutcomeClass::Slow => "slow",
            OutcomeClass::Panic => "panic",
        }
    }
}

/// Finding severity. `None` is not a finding; `S1` is a correctness or crash
/// finding; `S3` is a performance finding or an independent-gas discrepancy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    None,
    S1,
    S3,
}

/// What running one case produced.
///
/// The `kind` tag keeps the JSON self-describing in a case file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Outcome {
    /// Nothing to report. A deployment failure the program itself predicts
    /// (initcode too large, an `0xEF` runtime prefix) is a normal transaction
    /// revert and lands here, not in a finding.
    Ok,
    /// The bootloader returned an internal error for a block instead of a
    /// per-transaction failure. `root_cause` is the classified cause
    /// (`internal`, `runtime`, or `usage`); `detail` is the full debug string.
    InternalError { root_cause: String, detail: String },
    /// State mismatch in the forced-gas comparison. This is a candidate bug;
    /// nested call budgets can still differ between the engines.
    RevmMismatch { detail: String },
    /// Discrepancy seen only with independent gas. This includes return data and
    /// logs, which the forced-gas runner does not compare, so preserve it for triage.
    GasScheduleDiff { detail: String },
    /// A block took longer than `--slow-ms` of wall time.
    Slow { wall_ms: u128, slow_ms: u64 },
    /// The case panicked; caught by `catch_unwind` around the whole case as a
    /// last resort.
    Panic { message: String },
}

impl Outcome {
    pub fn class(&self) -> OutcomeClass {
        match self {
            Outcome::Ok => OutcomeClass::Ok,
            Outcome::InternalError { .. } => OutcomeClass::InternalError,
            Outcome::RevmMismatch { .. } => OutcomeClass::RevmMismatch,
            Outcome::GasScheduleDiff { .. } => OutcomeClass::GasScheduleDiff,
            Outcome::Slow { .. } => OutcomeClass::Slow,
            Outcome::Panic { .. } => OutcomeClass::Panic,
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            Outcome::Ok => Severity::None,
            Outcome::InternalError { .. }
            | Outcome::RevmMismatch { .. }
            | Outcome::Panic { .. } => Severity::S1,
            Outcome::Slow { .. } | Outcome::GasScheduleDiff { .. } => Severity::S3,
        }
    }

    pub fn is_finding(&self) -> bool {
        self.severity() != Severity::None
    }

    /// A finer signature than the class, used by the shrinker (so a reduction
    /// cannot morph one finding into a different one of the same class — e.g. a
    /// state divergence into a per-transaction mismatch) and by run-level dedup
    /// (so a soak reports distinct findings, not thousands of the same one).
    pub fn signature(&self) -> String {
        match self {
            Outcome::Ok => "ok".to_string(),
            Outcome::InternalError { root_cause, .. } => format!("internal:{root_cause}"),
            Outcome::RevmMismatch { detail } => format!("revm:{}", mismatch_kind(detail)),
            Outcome::GasScheduleDiff { .. } => "gas-schedule-diff".to_string(),
            Outcome::Slow { .. } => "slow".to_string(),
            Outcome::Panic { .. } => "panic".to_string(),
        }
    }
}

/// Reduces a REVM mismatch detail to a coarse kind so the shrinker and dedup key
/// on "the same kind of divergence" rather than exact addresses or slot values.
fn mismatch_kind(detail: &str) -> &'static str {
    if detail.contains("revm runner error") {
        "runner-error"
    } else if detail.contains("per-transaction mismatch") && detail.contains("state mismatch") {
        "state+per-tx"
    } else if detail.contains("per-transaction mismatch") {
        "per-tx"
    } else if detail.contains("state mismatch") {
        "state"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_only_mismatches_are_saved_for_triage() {
        let outcome = Outcome::GasScheduleDiff {
            detail: "per-transaction mismatch: return data".into(),
        };
        assert!(outcome.is_finding());
        assert_eq!(outcome.severity(), Severity::S3);
    }
}
