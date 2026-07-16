---
status: accepted
---

# Email-confirmation watching via IMAP app password

## Context

Layer A (PR #687) auto-tracks job applications submitted through the extension by monitoring form-submission events — it catches every successful submit the user makes via the extension autofill. However, Layer A only sees submits made through the extension itself; it cannot detect applications submitted via the employer's website directly or through other channels (manual browser, saved links, external job boards that bypass the extension).

Layer C corroborates submitted applications by monitoring the user's inbox for confirmation emails — a signal orthogonal to extension activity that confirms an application reached the employer's system. The feature is email-confirmation watching: a user connects their Gmail account to the desktop app, which scans the INBOX locally for application-confirmation emails and notifies the user, offering to auto-track the matched application.

The critical decision is how to access Gmail: OAuth or an app password. This ADR documents the rationale and trade-offs.

## Decision

**Use IMAP + Gmail app password, not OAuth.** The rationale is quantified:

1. **Gmail OAuth (`gmail.readonly` scope) is a restricted scope** — one shared OAuth client requires Google app verification (submission of screenshots, user support processes, privacy policies, terms of service, "intended use" justification) and an annual third-party CASA assessment (~$540–$1,800/yr, weeks-to-months timeline, recurring). A shared _unverified_ client is capped at 100 users lifetime, rendering this path unviable for an open-source app scaling past ~50 users. The path is impractical for this project.

2. **Gmail metadata scope is insufficient** — `gmail.metadata` returns email headers only (no subject, no body), forbids `q=` query-string searching, and forces an expensive `list()` loop to find candidates. Combined with the unverified-client 100-user cap, OAuth is not a viable path.

3. **The sanctioned free OAuth path (BYO client per user, rclone-style)** costs each user ~10-15 min of Google Cloud Console setup (create OAuth credentials, paste them into Settings). This is the friction users avoid by choosing an app.

4. **IMAP app password is the low-friction path** — 16-char alphanumeric string, generated in ~30 seconds from myaccount.google.com/apppasswords (requires 2FA to be enabled). No Cloud Console setup, no OAuth verification, no annual assessment.

**Honest disclosure — the trade-off:** An app password grants **full mail access** (read AND write), not just readonly. We mitigate this with:

- **OS keychain-only storage** — the credential never reaches disk, never crosses the network unencrypted, and is deleted instantly on user revocation (via the Google account page).
- **IMAP read-only use** — the app only issues IMAP SELECT and UID SEARCH commands; it never issues APPEND, COPY, or any write. The backend code is auditable; the API enforces read-only behavior.
- **Instant revocability** — the user can revoke the password at myaccount.google.com/apppasswords with one click, immediately invalidating the app's access, with no further action needed in the app.

**Version 1 = notify + confirm, no auto-write.** Matching company and title from email subjects/bodies is inherently fuzzy (no URL exists in an email) and cannot meet Layer A's exact-URL deduplication bar. The only new power Layer C grants is local mailbox reading. Auto-writing matched applications is deferred behind a separate opt-in under ADR-0009's "observe X, auto-act Y" consequence.

**Zero email-content egress — local heuristics only.** Parsing and matching happen locally on the device. Email content never reaches an AI provider, never reaches any endpoint, and never leaves the device. The only new network call is the TLS connection to imap.gmail.com:993 (or the user's mail host if custom hosts are ever supported), which is backend-owned and fixed in configuration, never user-supplied (per ADR-0012 discipline).

**Storage posture:** The credential lives in the OS keychain (slot `email-imap`). The `EmailWatchStore` (SQLite, `email_watch.db`) is machine-local and deliberately **NOT** a `DataStore` — it holds only watermark/dedupe state (last UID, last check time, seen emails) that should never reach backups or be shared across devices. The store is `Resettable` (cleared on factory reset alongside the keychain credential).

## Considered options

1. **IMAP app password (chosen).** Minimal user friction, instant revocability, mitigated full-access risk via keychain + audit. Trade-off: requires 2FA.
2. **Gmail OAuth with shared unverified client (100-user cap).** Rejected: impractical for scaling; the cap is a hard blocker.
3. **Gmail OAuth with BYO per-user credentials (10-15 min setup per user).** Rejected: defeats the point of an integrated feature; user friction is comparable to a custom mail-host setup flow.
4. **Support only readonly OAuth scopes (gmail.metadata, no full body).** Rejected: no body text = no subject = no confirmation fingerprint extraction possible; the feature doesn't work.
5. **Defer all email watching to v2+; ship Layer A only.** Rejected: user request explicitly asked for Layer C; the information is available and the feature adds clear value.

## Consequences

- **New Rust module family `email_watch/`** holds the store, connector, parser, and poller (PR A: store + connector + IPC; PR B: parser + matcher + poller).
- **New IPC commands** (5-step): `email_watch_status()`, `email_watch_connect(address, appPassword)`, `email_watch_disconnect()`, `email_watch_set_enabled(bool)`, `email_watch_check_now()`. Commands are backend-owned; the renderer never supplies a host/port (per ADR-0012).
- **New Settings UI** section "Email tracking" in Accounts — email and app-password inputs, Connect/Disconnect, enabled toggle, status line (address, last check, "Check now" button), and a consent disclosure (full mailbox access, keychain storage, zero egress, notify-only v1).
- **New egress class** (ADR-0005 class 7): "IMAP connection to the user's own mail provider (opt-in email-confirmation watching; credential user-supplied and OS-keychain-backed; email content never leaves the device)." Must be enumerated in README.md and SECURITY.md.
- **Notification Center integration** (ADR-016): matched emails are pushed as notifications with kind `"email.match"` and a route to the application row. No new NC changes; the feature reuses the existing pattern.
- **Poller pattern** (PR B): spawned from Tauri setup via `tauri::async_runtime::spawn`, follows the `autopilot_scheduler` precedent, respects min-check-interval + failure backoff. Layering: the poller (L2/L3) must mirror `autopilot_scheduler`'s decision on whether to use `R7_ALLOW` (upward shell reach to `commands::notifications::push_and_notify`) or split a separate `email_watch_scheduler` module — deferred to PR B's critic.
- **Provider-host allowlist (future):** if custom mail hosts are ever exposed in UI, require an explicit scheme/port allowlist (refuse plaintext, require TLS/SSL).
- **Honest limits** (documented): INBOX only (filters/spam/archive miss); employer must send a confirmation; fuzzy match may miss renamed titles; Gmail-only v1 (host is data-driven, no UI yet); requires 2FA + app password (Workspace admins can disable app passwords).

## References

- ADR-0005 (egress classes, class 7 lists the new IMAP class).
- ADR-0009 (observe X, auto-act Y consequence; v1 does not auto-write).
- ADR-0012 (backend-owned config, no renderer-supplied endpoints).
- ADR-016 (Notification Center; email.match is the kind, no NC changes needed).
- ADR-027 (diagnostics redaction; email content never logged).
- Store: `apps/desktop/src-tauri/src/email_watch/mod.rs` (`EmailWatchStore`, `Resettable` trait).
- Connector: `apps/desktop/src-tauri/src/email_watch/imap_client.rs` (`validate_connection`).
- IPC: `apps/desktop/src-tauri/src/commands/email_watch.rs` (5 commands).
- IPC contracts: `packages/shared/src/ipc/contracts/emailWatch.ts`.
- Service hooks: `apps/desktop/src/renderer/services/use-email-watch/`.
- Settings UI: `apps/desktop/src/renderer/features/settings/components/accounts/EmailWatchSection/`.
- i18n: `packages/translations/src/locales/en,de/translation.json` (`settings.accounts.emailWatch.*`).
- Keychain: `apps/desktop/src-tauri/src/credentials/mod.rs` (slot `email-imap`).
- Privacy: `docs/SECURITY.md`, `README.md` (egress enumeration).
