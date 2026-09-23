//! Pure policy composition and execution authorization semantics.
//!
//! This crate consumes already-resolved runtime facts. It does not resolve
//! policy, authenticate actors, read a store, or perform any side effect.
#![forbid(unsafe_code)]

mod assurance;
mod authorization;
mod binding;
mod composition;
mod recipient;

pub use assurance::assurance_satisfies;
pub use authorization::{
    AuthorizationBlocker, AuthorizationBlockerSet, AuthorizationDecision,
    CurrentAuthorizationFacts, authorize_execution,
};
pub use binding::{
    BindingValidityBlocker, BindingValidityBlockerSet, BindingValidityFacts, BindingValidityReport,
    check_binding_validity,
};
pub use composition::{PolicyLayers, compose_layers, compose_policy_decisions};
pub use recipient::{
    RecipientClassificationError, classify_recipient_boundary, risk_for_recipient_boundary,
};
