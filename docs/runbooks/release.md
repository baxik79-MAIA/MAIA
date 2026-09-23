# Runbook: Release

1. Validate every YAML spec and generated contract.
2. Run unit, connector-contract, E2E, migration and security suites.
3. Run dependency audit and secret scan.
4. Refresh external source snapshot for Microsoft/MCP/Viktor benchmark references.
5. Build installer/service packages.
6. Verify signed artifacts and clean-machine first run.
7. Test upgrade from previous supported schema with backup/rollback.
8. Render current master documentation and inspect layout.
9. Tag release only when target acceptance vector is green.
