---
name: security-checklist
description: The security review checklist (risk / validation / abuse / data) with this repo's anchors. Load when reviewing security-sensitive changes — capabilities, IPC, deps, secrets, AI, privacy, rate-limits.
---

# Security checklist

Authoritative: `docs/knowledge/security-rules.md`. Severity bias for security/data findings = round **UP**.

## Risk assessment (every change)

- What assets / user data are affected? What attack surface changes? What abuse opportunities open?

## Validation

- Inputs validated · outputs sanitized · permissions minimized · secrets protected · errors handled securely · logging reviewed (no secrets/PII in logs) · dependencies reviewed.

## Abuse / cost (DoS & spend)

- Rate limits / request throttling present? AI usage + cost caps? Export/resource limits? Can a user spam, exhaust CPU/memory, or run up API spend?

## AI security

- Can user input manipulate system prompts (injection)? Can sensitive data leak into prompts? Can AI reach unintended tools? Is AI output validated?

## Data

- Resume/PII protected · temp files cleaned up · export files secured · local storage/cache secured · retention & deletion honored (GDPR).

## Desktop / supply chain

- `tauri.conf.json` CSP intact (incl. Ollama `127.0.0.1:11434`) · `capabilities/default.json` least-privilege · updater signing key + `latest.json` integrity · `deny.toml` / `cargo audit` / `pnpm audit` clean · new deps license-checked.

## External standards & best-practices (verified 2026-06-19)

> **Re-baseline to current editions:** OWASP Top 10 **2025** (was 2021), ASVS **5.0** (was 4.x), SLSA **v1.2** (was v1.0), LLM Top 10 **2025**, CWE Top 25 **2025**. Cite these, not older ones.

- **OWASP Top 10:2025** (RC Nov 2025 — confirm final before locking ranks) https://owasp.org/Top10/2025/ — **new vs 2021:** `A03 Software Supply-Chain Failures` (expands old A06) and `A10 Mishandling of Exceptional Conditions`; SSRF folded into A01; A02 Security Misconfiguration rose to #2.
- **OWASP ASVS 5.0.0** (2025-05-30) — verify against L1/L2/L3. https://github.com/OWASP/ASVS
- **OWASP LLM Top 10 (2025)** — LLM01 Prompt Injection · LLM02 Sensitive-Info · LLM05 Improper Output Handling · LLM06 Excessive Agency · LLM07 System-Prompt Leakage. https://genai.owasp.org/llm-top-10/
- **Supply chain (now OWASP A03):** target **SLSA Build L3** (https://slsa.dev/spec/v1.2/); commit `pnpm-lock.yaml` + CI `--frozen-lockfile`; `cargo audit`/`cargo deny` vs RUSTSEC daily. ⚠️ **Provenance ≠ safety** — the 2025–26 **Shai-Hulud** npm worm shipped malware with _valid_ SLSA L3 provenance via hijacked OIDC tokens → pin exact versions, `--ignore-scripts`, scope-lock publish tokens. https://unit42.paloaltonetworks.com/npm-supply-chain-attack/
- **Frameworks:** NIST SSDF (SP 800-218) + CISA Secure-by-Design as the baseline.
- **Secrets** → OS keychain (`tauri-plugin-stronghold`/keyring), never config/logs. **Tauri** least-privilege capabilities + strict CSP + minisign-signed updater (any manifest/key compromise = full takeover). https://v2.tauri.app/security/
- **CWE Top 25 (2025-12-11)** — #1 XSS, #2 SQLi, #3 CSRF; watch CWE-284 (Improper Access Control), CWE-639 (authz bypass via user-controlled key), CWE-770 (alloc without limits). https://www.cisa.gov/news-events/alerts/2025/12/11/2025-cwe-top-25-most-dangerous-software-weaknesses

**Common mistakes:** citing Top 10 **2021** / ASVS **4.x** / SLSA **v1.0** (all superseded); treating npm/SLSA provenance as a safety guarantee (Shai-Hulud defeated valid L3); wildcard Tauri capabilities or no CSP ("it's a desktop app"); secrets in `tauri.conf.json`/env/logs; lockfile uncommitted or CI without `--frozen-lockfile`; no `cargo audit`/`cargo deny` gate; ignoring A03/A10 + LLM01/LLM06 for AI features.
