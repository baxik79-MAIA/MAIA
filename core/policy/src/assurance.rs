use maia_domain::ApprovalAssurance;

/// Returns whether achieved evidence satisfies the required assurance level.
///
/// The order is the canonical order from `spec/approval.yaml`; this function
/// does not infer a global risk ordering.
pub const fn assurance_satisfies(achieved: ApprovalAssurance, required: ApprovalAssurance) -> bool {
    match required {
        ApprovalAssurance::None => true,
        ApprovalAssurance::Confirm => matches!(
            achieved,
            ApprovalAssurance::Confirm | ApprovalAssurance::ElevatedConfirm
        ),
        ApprovalAssurance::ElevatedConfirm => {
            matches!(achieved, ApprovalAssurance::ElevatedConfirm)
        }
    }
}
