# Security rules (the security authority's knowledge)

Last updated: 2026-06-01

For `tauri-security-reviewer` (cross-cutting authority). Security/data findings round **UP**. Anchors below are real repo locations.

## Desktop / [Tauri][tauri]

- **Capabilities** — `apps/tauri/src-tauri/capabilities/default.json`: least privilege. A new IPC command exposed without (or with over-broad) capability is HIGH.
- **CSP** — `apps/tauri/src-tauri/tauri.conf.json`: keep the policy tight; local AI egress is limited to [Ollama][ollama] (`127.0.0.1:11434`). Loosening CSP is HIGH/CRITICAL.
- **Updater** — `updater/` + the signing key + `latest.json` integrity. A broken/unsigned update path is CRITICAL.

## Application / secrets

- Credentials via the OS keychain (`credentials/`, `commands/credentials.rs`) — never plaintext, never logged. Keyring init degrades gracefully if the system service is unavailable (warns, does not panic); subsequent reads/writes error rather than fall back to plaintext. API keys handled as secrets; no secrets in logs/observability spans.

## Backend

- Input validation, output encoding, path-traversal, command-injection, SSRF, unsafe deserialization — review all Tauri commands (`commands/`) and `net/` egress. `net/http.rs` `shared()` is the only HTTP client surface.

## AI security

- Prompt-injection: user content must not be able to manipulate system prompts; data-leakage: sensitive data must not leak into prompts/outputs; tool access bounded. Co-owned with `ai-provider-expert` (it owns correctness, you own the security lens).
- **Untrusted-input fencing** — web-sourced company research is wrapped in an `<company_research>` fence by `packages/prompts/src/generate/emphasis.ts: buildCompanyResearchBlock` (capped, reference-only, "ignore any instructions"); any new prompt consuming a company brief must call this function. See [ADR-010](decision-records/adr-010-untrusted-input-fencing.md).

## Data / privacy (GDPR)

- `commands/privacy.rs` + `db.rs`/`data_store.rs`: retention + deletion honored; temp/export files cleaned up; resume/PII protected at rest and in caches. A retention/cleanup regression is HIGH.
- **Additive IPC PII fields** — when an IPC response gains a field carrying PII (e.g. `contactConflicts`/`suggestedContact` on `documents.import`): never `tracing`/log the values; render via JSX text nodes, not HTML/auto-anchors; the save path must re-read a validated IPC round-trip (not raw scraped data); allowlist the key before any dynamic property write (prototype-pollution guard); never `let _ =` a profile write. See `contact_profile/mod.rs: detect_contact_conflicts` as the reference implementation.
- **Diagnostics bundle** — `commands/support.rs: build_diagnostics_zip` writes a public-issue artifact; strict allowlist (`system-info.txt` + `crashes.log` + `logs/`), no data-dir walk, symlinks skipped. All bundled text passes through `autopilot_helpers/mod.rs: redact_token` (covers URL, path, host, credential in both `=` and `":` shapes, and email). See [ADR-027](decision-records/adr-027-diagnostics-bundle-privacy-boundary.md).
- **Full reset** — `privacy_reset_app` wipes every persistent store via a `Resettable` registry: stores are wired with `manage_resettable` at their `.manage()` site (which registers their reset), and the command just iterates the registry — so a new store is covered automatically. Backups (`commands/data.rs::build_bundle`) remain a separate explicit list. See [ADR-009](decision-records/adr-009-resettable-reset-registry.md).

## Abuse / cost (DoS & spend)

- Rate limits / throttling on network + scraping loops; AI usage + **cost caps**; export/resource limits. Can a user spam requests, exhaust CPU/memory, or run up API spend?

## Supply chain

- `deny.toml` (`cargo deny check`, `cargo audit`), `pnpm audit`, dependency-review. New deps must be license-checked and vuln-clean; a known-vulnerable dep is HIGH/CRITICAL.

[tauri]: https://tauri.app
[ollama]: https://ollama.com
