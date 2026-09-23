use maia_domain::{RecipientBoundary, RiskClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientClassificationError {
    EmptyInput,
}

/// Resolves the effective recipient boundary using canonical precedence:
/// external, then unknown, then internal.
pub fn classify_recipient_boundary(
    boundaries: &[RecipientBoundary],
) -> Result<RecipientBoundary, RecipientClassificationError> {
    if boundaries.is_empty() {
        return Err(RecipientClassificationError::EmptyInput);
    }
    if boundaries.contains(&RecipientBoundary::External) {
        return Ok(RecipientBoundary::External);
    }
    if boundaries.contains(&RecipientBoundary::Unknown) {
        return Ok(RecipientBoundary::Unknown);
    }
    Ok(RecipientBoundary::Internal)
}

/// Maps a resolved boundary to risk while preserving explicit destructive or
/// privileged operation floors.
pub const fn risk_for_recipient_boundary(
    boundary: RecipientBoundary,
    operation_floor: RiskClass,
) -> RiskClass {
    match operation_floor {
        RiskClass::Destructive | RiskClass::Privileged => operation_floor,
        _ => match boundary {
            RecipientBoundary::Internal => RiskClass::SendInternal,
            RecipientBoundary::External | RecipientBoundary::Unknown => RiskClass::SendExternal,
        },
    }
}
