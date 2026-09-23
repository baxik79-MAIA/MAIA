# Open-source scope

This repository begins MAIA's public open-source history from a sanitized snapshot of the actual committed source. It is an ongoing development baseline, not a reconstructed demonstration or a one-time source dump. The original private history remains private and intact.

Included: Rust workspace source, manifests and lockfile, canonical YAML, ADRs, owned architecture documentation, prompts/locales, synthetic tests, migrations, generators, reviewed diagrams and deterministic CI. Apache-2.0 applies to MAIA-owned contributions; external dependencies retain their own licenses. Models, weights, vendor documentation and external runtimes are not bundled.

Operational reports, provider receipts, private correspondence, personal data, local paths/identities, runtime databases and build outputs are excluded. Synthetic test identifiers do not identify real sessions. Source constants describing normal Windows locations or documented application defaults remain where functionally meaningful.

Future public-safe core development should arrive as focused changes through the public review and validation process. Keep machine-local configuration under ignored `.local/` and runtime records outside tracked source. Review files and metadata before each public commit; never merge, mirror or push private ancestry. Public publication does not imply production or commercial readiness.

Development-only Desk orchestration source is retained for its deterministic guards. It requires explicit `MAIA_DESK_THREAD_ID` configuration and an externally supplied `.local/DeskBridge.ps1`; no private thread, bridge or credential is bundled. Tests supply a synthetic identity and fake transport. These tools are opt-in and outside product authorization.
