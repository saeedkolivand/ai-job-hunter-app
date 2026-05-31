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
