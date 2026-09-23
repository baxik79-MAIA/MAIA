# GENERATED - do not edit

## Risk classes

- `read`: Read existing data within granted scope.
- `analyze`: Transform, summarize, classify or infer without external side effects.
- `draft`: Create unpublished content or draft objects.
- `write_internal`: Modify internal low-risk metadata/task state.
- `send_internal`: Send or post content to recipients inside approved organization boundary.
- `send_external`: Send/post data outside approved organization boundary.
- `destructive`: Delete, cancel, revoke, overwrite or irreversibly modify data.
- `privileged`: Change auth, permissions, policies, integrations, keys or admin-relevant settings.

## Default gates

- `read` -> `allow`
- `analyze` -> `allow`
- `draft` -> `allow`
- `write_internal` -> `policy`
- `send_internal` -> `confirm`
- `send_external` -> `elevated_confirm`
- `destructive` -> `elevated_confirm`
- `privileged` -> `elevated_confirm`

## Gate to ApprovalState mapping

- `allow`: {"result": "not_required"}
- `policy`: {"if_policy_allows_without_confirmation": "not_required", "if_policy_denies": "no_approval_record_execution_blocked", "otherwise": "pending"}
- `confirm`: {"result": "pending"}
- `elevated_confirm`: {"result": "pending"}

## State machines

- `AgentTaskState`: `draft`, `preflight`, `awaiting_approval`, `queued`, `running`, `pausing`, `paused`, `partially_completed`, `completed`, `canceled`, `failed`
- `ActionState`: `planned`, `gated`, `awaiting_approval`, `queued`, `running`, `reconciling`, `retryable_error`, `completed`, `skipped`, `failed`, `canceled`
- `ApprovalState`: `not_required`, `pending`, `approved`, `rejected`, `expired`, `revoked`
- `CommitmentStatus`: `candidate`, `proposed`, `confirmed`, `in_progress`, `fulfilled`, `overdue`, `canceled`, `disputed`
- `ConnectorHealth`: `unconfigured`, `needs_auth`, `connecting`, `healthy`, `degraded`, `rate_limited`, `blocked`, `error`
- `RunState`: `created`, `starting`, `running`, `retryable_error`, `outcome_unknown`, `reconciling`, `completed`, `failed`, `canceled`

## Pause semantics

- **model:** quiescent_non_preemptive
- **task_transition:** running -> pausing -> paused
- **on_pause_request:** ["stop_scheduling_new_actions_or_runs", "request_cancel_only_for_in_flight_runs_whose_contract_is_cancellable", "allow_non_cancellable_in_flight_runs_to_reach_terminal_or_outcome_unknown", "enter_paused_only_when_no_in_flight_run_remains"]
- **action_state_paused_is_intentionally_absent:** true
- **rationale:** A generic Action pause would falsely promise preemption for non-cancellable side effects such as mail.send.
- **in_flight_states:** ["starting", "running", "outcome_unknown", "reconciling"]
- **run_scope:** all_task_runs_including_superseded_plan_action_revisions
- **after_request_forbidden:** ["create_run", "schedule_run", "created_to_starting"]
