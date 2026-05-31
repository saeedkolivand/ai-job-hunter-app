# Security rules (the security authority's knowledge)

For `tauri-security-reviewer` (cross-cutting authority). Security/data findings round **UP**. Anchors below are real repo locations.

## Desktop / Tauri

- **Capabilities** — `apps/tauri/src-tauri/capabilities/default.json`: least privilege. A new IPC command exposed without (or with over-broad) capability is HIGH.
- **CSP** — `apps/tauri/src-tauri/tauri.conf.json`: keep the policy tight; local AI egress is limited to Ollama (`127.0.0.1:11434`). Loosening CSP is HIGH/CRITICAL.
- **Updater** — `updater/` + the signing key + `latest.json` integrity. A broken/unsigned update path is CRITICAL.

## Application / secrets

- Credentials via the OS keychain (`credentials/`, `commands/credentials.rs`) — never plaintext, never logged. API keys handled as secrets; no secrets in logs/observability spans.

## Backend

- Input validation, output encoding, path-traversal, command-injection, SSRF, unsafe deserialization — review all Tauri commands (`commands/`) and `net/` egress. `net/http.rs` `shared()` is the only HTTP client surface.

## AI security

- Prompt-injection: user content must not be able to manipulate system prompts; data-leakage: sensitive data must not leak into prompts/outputs; tool access bounded. Co-owned with `ai-provider-expert` (it owns correctness, you own the security lens).

## Data / privacy (GDPR)

- `commands/privacy.rs` + `db.rs`/`data_store.rs`: retention + deletion honored; temp/export files cleaned up; resume/PII protected at rest and in caches. A retention/cleanup regression is HIGH.

## Abuse / cost (DoS & spend)

- Rate limits / throttling on network + scraping loops; AI usage + **cost caps**; export/resource limits. Can a user spam requests, exhaust CPU/memory, or run up API spend?

## Supply chain

- `deny.toml` (`cargo deny check`, `cargo audit`), `pnpm audit`, dependency-review. New deps must be license-checked and vuln-clean; a known-vulnerable dep is HIGH/CRITICAL.
