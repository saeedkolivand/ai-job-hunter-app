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

- **OS keychain-only storage** — the credential never reaches disk, never crosses the network unencrypted, and is removed on Disconnect or factory reset.
- **IMAP read-only use** — the app only issues IMAP SELECT and UID SEARCH commands; it never issues APPEND, COPY, or any write. Read-only is an application-behavior guarantee (enforced by code review and auditable), not an API-enforced restriction — the password itself grants full mailbox access, as disclosed above.
- **Instant revocability** — the user can revoke the password at myaccount.google.com/apppasswords with one click, immediately invalidating the app's future IMAP logins; the now-inert keychain entry remains local until Disconnect or factory reset removes it.

**Version 1 = notify + confirm, no auto-write; Version 2 slice 1 & 2 adds adjudicable auto-write.** Matching company and title from email subjects/bodies is inherently fuzzy (no URL exists in an email) and cannot meet Layer A's exact-URL deduplication bar. Auto-writing matched applications is deferred behind a separate opt-in under ADR-0009's "observe X, auto-act Y" consequence (v2 realizes this).

## Version 2: Auto-write with adjudication

**v2 slice 1 (shipped):** 4-way email-intent classifier (confirmation | rejection | interview | offer) over a 173-phrase, 7-language corpus that survived a 57% adversarial kill rate. Subject-only fingerprinting cannot separate rejections (which reuse confirmation subject lines from earlier in the thread); the body classifier is the discriminating signal.

**v2 slice 2 (shipped, wired):** Auto-write infrastructure. **A matched intent writes a status change immediately, but always UNCONFIRMED.** The application's timeline renders it as provisional with Accept / Reject buttons. Accept sets `confirmed = 1` on that row (it is written as `0`); Reject appends a reversal event (table stays append-only; the trail shows the email was wrong). If the user changed the status by hand in the meantime, Reject marks the row reviewed (never clobbered). A separate toggle (`auto_write_enabled`, default OFF) gates the write; opt-in is the primary safeguard, and the unconfirmed row with mandatory adjudication is the backstop.

**Auto-write wired status:** `apply_matched_intent` is called from `email_watch_scheduler.rs`, not from the poller — the L0–L3 table enforced by `tests/architecture.rs` puts the `AppHandle` and `ApplicationStore` reach in the scheduler (L2) and keeps the poller (L1) pure. Shipped behavior is therefore v2 auto-write, gated in order by the `auto_write_enabled` toggle (default OFF, opt-in only), then sender provenance, then a decided intent; any of the three failing is a silent no-op.

**Why default OFF — reversing an earlier decision:** Five security rounds established that sender authentication via email headers **cannot be made sound** by content alone. Two candidate fixes were built and measured against the specific residual (a forged `Authentication-Results` header containing a forged `dmarc=pass` clause via an attacker-controlled RFC 5321 envelope local part echoed verbatim by Gmail's authentic SPF evaluation) — both failed. Closing this residual properly requires independent DNS-backed DKIM/SPF/DMARC re-verification, which is a new dependency and egress surface deliberately absent from this feature's design. Since the sender gate cannot be made sound, auto-write is now opt-in: nobody receives status changes silently, and adjudication remains the mandatory backstop for anyone who turns it on.

**Which applications are eligible:** every application the ladder can act on — any live status, plus a terminal status that is itself an unconfirmed email-derived write. Both the matcher and the ladder read one shared predicate so they cannot disagree, which matters because excluding terminal statuses at the matcher would silently reinstate the absorbing-`Rejected` problem the exception below exists to prevent.

**Interview intent skips intermediate stages:** `Interview` intent (e.g., "we'd like to interview you") advances straight to `Interviewing` from whatever eligible status the application is in, rather than stepping through the ladder.

**Terminal-status absorption with email-write exception:** A `Rejected` or `Accepted` status normally absorbs all further status changes — once terminal, always terminal. Exception: if the current status is itself an unconfirmed email-derived write, a later email-derived intent may supersede it. This exists because attribution is attacker-supplied: a single cold email from an unrecognised sender could otherwise freeze an application `Rejected` forever and silently stop tracking it. A user-set or user-accepted terminal status still absorbs (no exception).

**Sender provenance gate:** An auto-write requires a `WRITE_GATE_DOMAINS` hit on the **visible `From:` header** domain (`compute_write_authorized` reads `Fingerprint::write_gate_domain`, not `domain_hint`, and `parse_header` derives the domain from `message.from()`, not the SMTP envelope) together with an aligned DMARC pass read from the topmost `Authentication-Results` header. `WRITE_GATE_DOMAINS` is deliberately narrower than `SCORE_HINTS` — it drops `linkedin.com` and `indeed.com`, which relay third-party text from their own valid infrastructure. Naming `domain_hint` here would describe the wider scoring list and re-merge the two roles that were split precisely so a scoring heuristic could never double as an authorisation. A cold email from an unrecognised sender never writes, even if it fingerprints and classifies. This is the cheapest sufficient signal that already exists; two wider signals were considered (sender domain matching the company's expected domain, thread linkage via `References`/`In-Reply-To`) and deferred as future work.

**On `host_is_known_to_stamp` (a narrow, documented mitigating signal, not a closed gate):** A recipient mail host's presence in the `Authentication-Results` header proves the host processes and stamps a header of that name; it does NOT prove the host stamps a `dmarc=` clause for every sender domain the message's `From:` claims. DMARC evaluation legitimately produces no `dmarc=` section for a given domain (e.g., the sender domain has no DMARC policy, or the domain the host was asked to evaluate has no policy alignment). An attacker picks such a domain deliberately, supplies a forged `dmarc=pass` clause themselves via an RFC 5321 envelope-echo mechanism inside the same header's SPF comment, and the two are indistinguishable by content alone — real grammar is real grammar. Checking whether a host is "known to stamp" closes only one narrow case: a message with a completely forged `Authentication-Results` header from a host that never stamps anything at all. Two candidate additional checks (verifying the authserv-id or re-checking against DNS) were measured during security rounds and both failed. The residual is documented, opt-in-gated, and mitigated by mandatory adjudication.

**Recall gaps and residuals, honestly stated:**

- English has exactly ONE discriminating confirmation phrase and ONE discriminating offer phrase (vs. 23 rejection phrases). German has ZERO discriminating confirmation phrases. Across 7 languages, 138 discriminating phrases remain, split 9 confirmation / 76 rejection / 30 interview / 23 offer.
- `classify_intent` returning `None` is the safe direction (no write) for unrecognized patterns. Recall on confirmation/offer is thinner than the phrase count alone suggests.
- A stale offer phrase quoted from an older thread message can outrank a current interview phrase, because the tie-break has no positional awareness. Rejection is unaffected (it wins first, position-independently). Adjudication is the mitigation.
- **Sender authentication residual is live and documented** (not closed): A forged `dmarc=pass` clause inside a comment of an attacker-controlled SPF section, echoed from an RFC 5321 envelope local part the attacker supplied to Gmail, cannot be distinguished from a real clause by parsing content alone. Two candidate structural fixes were measured and discarded (authserv-id verification, DNS re-check). Closing this requires DNS-backed re-verification (new dependency, new egress). Mitigations in place: opt-in default (nobody exposed without asking), every write lands unconfirmed (adjudication is mandatory), and cold senders never write (recognized domain + aligned DMARC pass required). An opt-in user who enables auto-write assumes this residual in exchange for convenience — the timeline is where mistakes are caught and reversed.

**Matching normalization:** Phrase matching normalizes whitespace (strips quote markers, joins wrapped lines, collapses whitespace), folds typographic apostrophes (U+2019 → U+0027), and composes NFC (handles NFD accents). Both the haystack (email body) and needles (corpus phrases, compiled at build time) are normalized identically. Every defect (wrapped-line failures, NBSP, curly apostrophes, accented characters) was reproduced as a failing test before the fix.

**Zero email-content egress to external processing — local heuristics only.** The app retrieves email content from the user's own IMAP server (ingress from the user's own mail host), but parsing, matching, and intent classification happen locally on the device. Email content never reaches an AI provider or any external processing endpoint. The TLS connection to imap.gmail.com:993 (or the user's mail host if custom hosts are ever supported) is backend-owned, fixed in configuration, and never user-supplied (per ADR-0012 discipline).

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
- ADR-0009 (observe X, auto-act Y consequence; v2 realizes auto-write).
- ADR-0012 (backend-owned config, no renderer-supplied endpoints).
- ADR-016 (Notification Center; email.match is the kind, no NC changes needed).
- ADR-027 (diagnostics redaction; email content never logged).
- Store: `apps/desktop/src-tauri/src/email_watch/mod.rs` (`EmailWatchStore`, `Resettable` trait).
- Connector: `apps/desktop/src-tauri/src/email_watch/imap_client.rs` (`validate_connection`).
- Parser: `apps/desktop/src-tauri/src/email_watch/parser.rs` (RFC2047/MIME decode, fingerprint).
- Authentication-Results RFC 8601 tokeniser: `apps/desktop/src-tauri/src/email_watch/auth_results.rs` (replaces substring-scanning; safe fail-closed; documented residual on forged clauses via envelope echo).
- Intent classifier: `apps/desktop/src-tauri/src/email_watch/intent.rs` (4-way classifier, normalization, status ladder).
- Phrase corpus: `apps/desktop/src-tauri/src/email_watch/intent_phrases.json` (173-phrase, 7-language compiled asset).
- Auto-write: `apps/desktop/src-tauri/src/email_watch/auto_write.rs` (`apply_matched_intent`, provenance gate, opt-in default, wired).
- Status events: `apps/desktop/src-tauri/src/applications/status_events.rs` (append-only audit trail with `source` and `confirmed` columns).
- IPC: `apps/desktop/src-tauri/src/commands/email_watch.rs` (5 commands).
- IPC contracts: `packages/shared/src/ipc/contracts/emailWatch.ts`.
- Service hooks: `apps/desktop/src/renderer/services/use-email-watch/`.
- Settings UI: `apps/desktop/src/renderer/features/settings/components/accounts/EmailWatchSection/`.
- i18n: `packages/translations/src/locales/en,de/translation.json` (`settings.accounts.emailWatch.*`).
- Keychain: `apps/desktop/src-tauri/src/credentials/mod.rs` (slot `email-imap`).
- Privacy: `docs/SECURITY.md`, `README.md` (egress enumeration).
