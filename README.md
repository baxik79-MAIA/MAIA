# MAIA

**Multichannel Automation & Intelligent Assistance** is an early-stage, actively developed, local-first execution-intelligence project. It is not a production-ready assistant or a supported production deployment.

MAIA explores how to move from a signal through context, a commitment/decision/risk, a plan, an authorized action, a run, and an outcome while retaining human control and auditable provenance.

Public history begins with a sanitized open-source baseline of the actual, actively developed MAIA source. Prior private Git history is intentionally unpublished because it contained local operational telemetry; the source was not reconstructed or rewritten from documentation.

## What is in this repository

- A Rust-centered workspace with Python specification and development-governance tooling.
- OS-neutral domain, policy, orchestration, store ports, runtime and executor boundaries, plus a SQLite adapter.
- Local-model runtime adapters, advisory briefing and experimental desktop/host applications.
- Local intelligence health, diagnostics, hypothesis-evidence ledger and recorder components.
- Provider-agnostic consultation contracts and an optional Round Table module for multi-model review. Concrete adapters own provider and credential access.
- Canonical YAML specifications, generated contracts, ADRs and executable architecture dependency guards.

The architectural direction is a **Capability Mesh within a modular monolith**: capabilities own their implementation and state behind explicit contracts. Local-first and CPU-first/office-PC-first are design constraints, not performance benchmarks or a claim that every model runs well on every PC.

Round Table is optional and remains outside MAIA Core. Reasoning assurance never grants action execution permission. No hidden provider or model fallback is allowed.

## Development and deployment boundaries

`DEVELOPMENT_EVOLUTION` permits bounded development-host evolution under the accepted protected-surface rules. `DEPLOYMENT_LOCKED` requires self-improvement capabilities to be absent by construction; it is not a switch that a deployed instance can turn off. A full workspace or all-features development build is **not** a certified deployment artifact.

Connector integrations and wider work automation remain subject to explicit contracts and acceptance work. Historical milestone reports are development evidence, not promises of production readiness. No installation package, support SLA, adoption count or benchmark is claimed here.

## Build and check

Windows is the locally exercised development platform; core contracts are OS-neutral. Use a current stable Rust toolchain with rustfmt and Clippy, Python 3.13, Node.js 24, PyYAML and tzdata. Native Windows Rust builds also need the MSVC build tools. From the repository root:

```text
python -m pip install -r requirements-dev.txt
python tools/spec_guard.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-fail-fast
python -m unittest discover -s tests/spec -p "test*.py"
```

The guard checks YAML and generated-document/domain drift. Node.js runs the RFC 8785 and action-binding reference checks. Cargo may fetch dependencies on a fresh checkout. Add `--offline` to Cargo commands only after dependencies are cached. Do not enable live-provider tests as part of routine validation.

See [Contributing](CONTRIBUTING.md) for the remaining local checks and [Development status](docs/DEVELOPMENT_STATUS.md) for current limitations.

## Read more

- [Architecture](docs/ARCHITECTURE.md) and [canonical specifications](spec/)
- [Security model](docs/SECURITY_MODEL.md) and [vulnerability reporting](SECURITY.md)
- [Open-source scope](docs/OPEN_SOURCE_SCOPE.md) and [roadmap](docs/ROADMAP.md)
- [Governance](GOVERNANCE.md), [maintainers](MAINTAINERS.md) and [code of conduct](CODE_OF_CONDUCT.md)

## License

MAIA project code and documentation are licensed under [Apache-2.0](LICENSE), subject to separately identified third-party rights. External runtimes, models, services and dependencies retain their own licenses and terms; model weights and provider credentials are not included.
