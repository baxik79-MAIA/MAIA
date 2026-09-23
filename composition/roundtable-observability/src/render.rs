//! M0.15.4 — plain-text rendering of the observability model.
//!
//! Pure functions from model to `String`. No I/O, no clock, no environment, so
//! every surface (a CLI today, something else later) shows the same words and a
//! test can pin them. Wording lives here rather than in each surface so that
//! "not installed", "empty" and "broken" cannot drift apart between them.

use crate::{
    Availability, ContributionOutcome, ObservabilityState, SessionListEntry, SessionPanel,
    UnavailableReason,
};

/// What the operator should do about a state, or an explicit statement that
/// nothing is wrong. A bare label leaves "not installed" reading like a fault.
fn guidance(availability: &Availability) -> &'static str {
    match availability {
        Availability::Available => "",
        Availability::NotInstalled => {
            "This is a supported configuration, not a fault. MAIA runs without the Round Table."
        }
        Availability::Unavailable(UnavailableReason::HistoryUnreachable) => {
            "Action needed: the history location exists in this build but cannot be read. \
             This is not the same as having no sessions."
        }
        Availability::Unavailable(UnavailableReason::HistoryCorrupt) => {
            "Action needed: session records are present but none can be read. \
             This is not the same as having no sessions."
        }
    }
}

/// The session list, or an honest account of why there is none.
pub fn render_state(state: &ObservabilityState) -> String {
    let mut out = format!("{}\n", state.availability.label());
    let note = guidance(&state.availability);
    if !note.is_empty() {
        out.push_str(note);
        out.push('\n');
    }
    if !state.availability.is_available() {
        return out;
    }
    if state.sessions.is_empty() {
        out.push_str("No sessions have been recorded yet.\n");
        return out;
    }
    out.push_str(&format!("{} session(s)\n", state.sessions.len()));
    for s in &state.sessions {
        out.push_str(&render_entry(s));
    }
    out
}

fn render_entry(s: &SessionListEntry) -> String {
    let leader = s.leader.as_deref().unwrap_or("none");
    format!(
        "  {}  [{}]  {:?}  leader: {}  {}\n",
        s.id,
        s.status(),
        s.assurance,
        leader,
        s.subject
    )
}

/// One session in full.
pub fn render_panel(p: &SessionPanel) -> String {
    let mut out = String::new();
    out.push_str(&format!("Session {}\n", p.entry.id));
    out.push_str(&format!("Status: {}\n", p.entry.status()));
    out.push_str(&format!("Assurance: {:?}\n", p.entry.assurance));
    out.push_str(&format!("Subject: {}\n", p.entry.subject));
    out.push_str(&format!("Prompt: {}\n", p.prompt));
    out.push_str(&format!("Quorum: {}\n", p.quorum.headline()));

    match &p.leader {
        Some(l) => out.push_str(&format!(
            "Leader: {} ({}) — {}\n",
            l.participant_id,
            l.provider,
            l.basis()
        )),
        None => out.push_str("Leader: none\n"),
    }

    out.push_str("Contributions:\n");
    if p.contributions.is_empty() {
        out.push_str("  (none recorded)\n");
    }
    for c in &p.contributions {
        out.push_str(&format!(
            "  #{} {} ({}) — {}\n",
            c.sequence,
            c.participant_id,
            c.provider,
            c.outcome.label()
        ));
        out.push_str(&format!(
            "      model configured: {}; model served: {}; request id: {}; {} ms; {}\n",
            c.model_configured,
            c.model_served.display(),
            c.provider_request_id.display(),
            c.duration_ms,
            if c.first_round_isolated {
                "first round isolated"
            } else {
                "not isolated"
            },
        ));
    }
    let failed = p
        .contributions
        .iter()
        .filter(|c| matches!(c.outcome, ContributionOutcome::Failed(_)))
        .count();
    if failed > 0 {
        out.push_str(&format!("  {failed} contribution(s) did not respond.\n"));
    }

    if p.disagreements.is_empty() {
        out.push_str("Disagreements: none recorded\n");
    } else {
        out.push_str("Disagreements:\n");
        for d in &p.disagreements {
            out.push_str(&format!("  - {d}\n"));
        }
    }

    match &p.conclusion {
        Some(c) => out.push_str(&format!("Conclusion: {c}\n")),
        None => out.push_str("Conclusion: none — this session did not reach a decision\n"),
    }

    // Shown, never assumed. If the invariant ever broke, a viewer that stayed
    // silent would be hiding the most important fact on the page.
    if p.execution_authority {
        out.push_str("EXECUTION AUTHORITY IS SET ON THIS SESSION — INVARIANT VIOLATION\n");
    } else {
        out.push_str("Execution authority: none. This is advice only; nothing acts on it.\n");
    }
    out
}

/// A single session that could not be shown.
pub fn render_missing(session_id: &str) -> String {
    format!("No session with id `{session_id}` was found.\n")
}

/// History could not be read while looking for one session.
pub fn render_unavailable(availability: &Availability) -> String {
    let mut out = format!("{}\n", availability.label());
    let note = guidance(availability);
    if !note.is_empty() {
        out.push_str(note);
        out.push('\n');
    }
    out
}
