# Contributing to MAIA

MAIA is early-stage software with evolving contracts. Keep changes focused and describe the problem, resulting behavior, validation and material limitations. Use synthetic examples and review your diff for private data before opening a pull request.

## Canonical development order

Read the project handbook and applicable agent instructions. The authority order is `spec/*.yaml`, ADRs, canonical prompts/locales, generated artifacts, explanatory documentation, then implementation. Change canonical YAML before changing a contract; regenerate derived files using the existing generators, never by hand. Preserve human approval, provenance, provider neutrality, CPU-first/local-first constraints and deployment-lock boundaries.

Architecture-sensitive changes need review by the Architecture Owner under the existing collaboration protocol. A reasoning review does not authorize an external action. Public contributors do not need access to private operational transcripts or a particular commercial model to propose a change; maintainers coordinate any required review.

## Validation

Use current stable Rust with rustfmt/Clippy, Python 3.13, Node.js 24 and dependencies from `requirements-dev.txt` (PyYAML and tzdata). The workspace uses Rust edition 2024. Run from the root:

```text
python -m pip install -r requirements-dev.txt
python tools/validate_spec.py
python tools/spec_guard.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
cargo test --workspace --all-features --no-fail-fast
python -m unittest discover -s tests/spec -p "test*.py"
python -m unittest discover -s tools/tests -p "test*.py"
python -m unittest discover -s tools/maia_local_harness/tests -p "test*.py"
python tools/tests/test_continuation_driver.py
python tools/tests/test_night_shift.py
python tools/tests/test_round_table_absent.py
python tools/verify_round_table_absent.py
git diff --check
```

The absence proof builds and tests a temporary workspace with Round Table removed. It uses Cargo offline mode and needs dependencies cached first. `spec_guard.py` writes its generated report. The all-features command tests development-only code and does not produce a deployment-approved build. Do not run ignored/live tests, enable live-provider opt-ins, or supply provider credentials in ordinary CI.

Do not weaken a guard to obtain a green build. Report failures and distinguish pre-existing failures from those introduced by your change. Stage only explicitly reviewed paths; do not include operational records, local model caches or personal configuration.

## Pull requests

Include linked requirements, relevant acceptance coverage and any migration/security impact. Contract, architecture or dependency changes need the corresponding evidence. Keep unrelated edits separate. By intentionally submitting a contribution, you offer it under the project's Apache-2.0 license unless an agreed separate arrangement applies. Only submit material you have the right to contribute.
