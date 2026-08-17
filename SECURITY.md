# Security Policy

Thanks for helping keep **AI Job Hunter** and its users safe.

## Supported versions

AI Job Hunter ships frequently from `main`. Security fixes land in the **latest release**; please reproduce on the most recent version from the [Releases](https://github.com/saeedkolivand/ai-job-hunter-app/releases) page (or current `main`) before reporting.

| Version        | Supported           |
| -------------- | ------------------- |
| Latest release | ✅                  |
| Older releases | ❌ (please upgrade) |

## Reporting a vulnerability

**Please do not open a public issue, PR, or discussion for security problems.**

Report privately through one of:

1. **GitHub Private Vulnerability Reporting** (preferred): open the repository's **[Security → Report a vulnerability](https://github.com/saeedkolivand/ai-job-hunter-app/security/advisories/new)** form. This keeps the report private until a fix is ready.
2. **Email**: `saeedkolivand1997@gmail.com` with the subject `SECURITY: ai-job-hunter`.

Please include:

- A clear description and the impact (what an attacker can do).
- Steps to reproduce, a proof-of-concept, and affected version / OS.
- Any relevant logs or configuration, **with secrets redacted**.

### What to expect

- **Acknowledgement** within ~5 business days.
- An assessment and, if accepted, a fix targeted at the next release.
- Coordinated disclosure: we'll agree on timing with you and credit you in the release notes unless you prefer to stay anonymous.

## Scope

**In scope**: the desktop application and this repository, including the Rust core and IPC surface (`#[tauri::command]` handlers), credential storage, AI-provider request handling and prompt construction, browser automation / scraping, document import/OCR, the local data stores, the auto-updater, and the build/dependency supply chain.

**Out of scope**

- Vulnerabilities in **third-party AI providers** (OpenAI, Anthropic, Google, Ollama, LM Studio, CLI agents) or **job boards**: report those to the respective vendor.
- Issues that require a **already-compromised machine** or physical access to the user's device.
- Findings against your **own** configured infrastructure (e.g. your self-hosted OpenAI-compatible server).
- Missing hardening that has no demonstrated exploit (e.g. "best-practice" suggestions without impact).

## Security posture

AI Job Hunter is built local-first to minimize attack surface:

- **Local-first**: your jobs, résumés, generations, applications, and tracked job data live in a local database on your machine; there is **no app-operated backend** collecting them, and no behavioural analytics. **Two things happen automatically, without you asking:** the app checks for updates (GitHub, 10 s after launch then every 4 h, transmitting only your app version and OS/architecture), and sends crash reports to Sentry (error + stack trace, OS, app version) when the app breaks — the latter is on by default, asked at first run, revocable in Settings → Privacy, and redacted of paths/links/hosts/e-mails/credentials before transmission. The app's other outbound calls go to services you configure or invoke: the AI provider you configure (which receives the résumé and job text you ask it to generate from), job-board scraping (including the Adzuna, JSearch/RapidAPI, Jooble, Apify and freehire aggregator tiers — freehire being the one that needs no key, tried last), an optional opt-in web search (the AI provider's own, or Exa when a key is stored), user-typed location autocomplete (Photon, with offline fallback), opt-in company-logo enrichment (Clearbit, default **off**), opt-in email-confirmation watching via IMAP (credential user-supplied and OS-keychain-backed; email content never leaves the device), and your optional profile import from GitHub or LinkedIn. Neither the update check nor the crash report ever includes your documents or job data, and the browser extension is excluded entirely from all reporting (see [ADR 0005](docs/knowledge/decision-records/0005-network-egress-privacy-boundary.md)).
- **Credentials in the OS keychain**: API keys and board credentials are stored encrypted via the OS keychain, never in plain text or config files.
- **Untrusted external content is fenced**: web-sourced company research is wrapped in an untrusted block before reaching any prompt (reference-only; the model is told to ignore embedded instructions), and AI output is treated as untrusted by the renderer.
- **Strict IPC boundary**: the renderer talks to the Rust core only through a typed, validated contract; request shapes are generated from shared schemas.
- **Supply-chain gates**: `cargo-deny` and `cargo-machete` run in CI alongside the lint/type/test/clippy gates.

Thank you for reporting responsibly. 🙏
