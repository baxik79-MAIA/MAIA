use maia_domain::{FreshnessEvidence, FreshnessResult, Run};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FreshnessBlocker {
    EvidenceMismatch,
    MismatchedSource,
    UnverifiableSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreshnessDecision {
    Allowed,
    Blocked(Vec<FreshnessBlocker>),
}

pub fn evaluate_freshness(run: &Run, evidence: &FreshnessEvidence) -> FreshnessDecision {
    if evidence.validate().is_err()
        || evidence.run_id() != run.id()
        || evidence.action_id() != run.action_id()
        || evidence.action_version() != run.action_version()
        || evidence.action_hash() != run.action_hash()
    {
        return FreshnessDecision::Blocked(vec![FreshnessBlocker::EvidenceMismatch]);
    }
    match *evidence.result() {
        FreshnessResult::Matched => FreshnessDecision::Allowed,
        FreshnessResult::Mismatched => {
            FreshnessDecision::Blocked(vec![FreshnessBlocker::MismatchedSource])
        }
        FreshnessResult::Unverifiable => {
            FreshnessDecision::Blocked(vec![FreshnessBlocker::UnverifiableSource])
        }
    }
}
