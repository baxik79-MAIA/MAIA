use maia_domain::{
    Action, Approval, ApprovalId, Attempt, ConnectorProfileId, DomainError, ModelProfileId, Run,
    RunId, Sha256Hex, Version,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSpec {
    pub run_id: RunId,
    pub action_id: maia_domain::ActionId,
    pub attempt: Attempt,
    pub requested_connector_id: Option<ConnectorProfileId>,
    pub requested_model_id: Option<ModelProfileId>,
    pub action_version: Version,
    pub action_hash: Sha256Hex,
    pub approval_id: ApprovalId,
    pub approval_version: Version,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunBinding<'a> {
    pub action_id: &'a maia_domain::ActionId,
    pub action_version: &'a Version,
    pub action_hash: &'a Sha256Hex,
    pub approval_id: &'a ApprovalId,
    pub approval_version: &'a Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunBuildError {
    Domain(DomainError),
    ApprovalDoesNotBindAction,
}

impl From<DomainError> for RunBuildError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

pub fn build_run_spec(
    run_id: RunId,
    action: &Action,
    action_hash: Sha256Hex,
    approval: &Approval,
    attempt: Attempt,
    requested_model_id: Option<ModelProfileId>,
) -> Result<RunSpec, RunBuildError> {
    if approval.action_id() != action.id()
        || approval.action_version() != action.version()
        || approval.action_hash() != &action_hash
    {
        return Err(RunBuildError::ApprovalDoesNotBindAction);
    }
    Ok(RunSpec {
        run_id,
        action_id: action.id().clone(),
        attempt,
        requested_connector_id: action.connector_profile_id().clone(),
        requested_model_id,
        action_version: *action.version(),
        action_hash,
        approval_id: approval.id().clone(),
        approval_version: *approval.version(),
    })
}

pub fn create_run(spec: RunSpec) -> Result<Run, RunBuildError> {
    Ok(Run::new(
        spec.run_id,
        Version::new(1)?,
        spec.action_id,
        spec.attempt,
        spec.requested_connector_id,
        None,
        spec.requested_model_id,
        None,
        maia_domain::RunState::Created,
        maia_domain::OutcomeCertainty::NotApplicable,
        None,
        None,
        None,
        spec.action_version,
        spec.action_hash,
        spec.approval_id,
        spec.approval_version,
    )?)
}

pub fn validate_run_binding(run: &Run, expected: RunBinding<'_>) -> bool {
    run.action_id() == expected.action_id
        && run.action_version() == expected.action_version
        && run.action_hash() == expected.action_hash
        && run.approval_id() == expected.approval_id
        && run.approval_version() == expected.approval_version
}
