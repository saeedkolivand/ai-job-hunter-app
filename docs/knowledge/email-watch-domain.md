# Email-watch domain (IMAP confirmation-email polling)

Last updated: 2026-07-17 (task #23 PR B: email-watch poller, parser, matcher, and scheduler)

Owned by `extension-author` (frontend settings UI) and `rust-backend-author` (IMAP polling loop); security co-reviewed by `tauri-security-reviewer`.

## Purpose

Complement to extension submit-watch (Layer A, PR #687): listen for **application confirmation emails** on IMAP-enabled personal inboxes and produce a Notification Center card routing to the Application for user confirmation. Polling-based, fully local (zero content egress to AI providers or external services), and v1 **notify-only** (never auto-writes — the user manually confirms the status change via the existing stage-picker UI).

See [ADR-0013](../adr/0013-email-confirmation-watching.md) for the OAuth-vs-app-password economics, zero-content-egress guarantee, and why v1 is notify-only.

## Architecture overview

New module family: `apps/desktop/src-tauri/src/email_watch/` + `email_watch_scheduler.rs` (L2, mirrors `autopilot_scheduler`).

| Module                     | LOC  | Purpose                                                                                                                                                                           |
| -------------------------- | ---- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`                   | 302  | `EmailWatchStore` (SQLite `email_watch.db`). Account + seen-dedupe tables. Resettable.                                                                                            |
| `imap_client.rs`           | 250+ | TLS IMAP connector (raw socket + `native-tls`). Validates credentials; fetches headers/bodies PEEK-only. No marking read.                                                         |
| `parser.rs`                | 577  | RFC2047 subject decode, MIME text/plain extraction, confirmation-email fingerprinting (EN+DE), company/title candidate extraction. Pure fns, no I/O.                              |
| `matcher.rs`               | 318  | Fuzzy company+title token-Jaccard scoring vs saved Applications. Computes best match or `None` (ambiguous). Pure fns.                                                             |
| `poller.rs`                | 220+ | Tick orchestration: fetch headers, fingerprint, body-fetch candidates, extract, match. Returns outcomes (matched app IDs + UIDs for deduping).                                    |
| `email_watch_scheduler.rs` | 307  | L2 module: spawns from Tauri setup. ~15 min interval, exponential backoff on failure, RunGuard concurrent-run protection, invokes parser/matcher/poller, calls `push_and_notify`. |

**IPC contract** (`commands/email_watch.rs`, 5 commands): `status()`, `connect()`, `disconnect()`, `set_enabled()`, `check_now()`. See `packages/shared/src/ipc/contracts/emailWatch.ts`.

## Read-only IMAP posture

- **`EXAMINE INBOX`** instead of `SELECT` — server-enforced read-only (defense-in-depth; this crate also never issues STORE/APPEND/COPY).
- **`BODY.PEEK[...]`** on all fetches — never sets `\Seen` flag; no side-effect on the user's mailbox.
- **No writes to mailbox or marked messages** — the only write is a local SQLite `seen` table (dedupe cache).
- **Password validation via LOGIN + SELECT/EXAMINE + logout** — no writes are tested; the session is disposable.

## Credential handling

- **No persistent password** — IMAP app password enters via the UI, is validated by real `LOGIN` + `EXAMINE INBOX` (or returns an error immediately), and **never enters SQLite**. Validated once per `email_watch_connect()` IPC call.
- **Keychain-only** — a successful connection stores the address + host in SQLite, the password in OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service). The credential persists by design so the poller survives app restarts; it is cleared only on explicit `email_watch_disconnect` or factory reset.
- **Host prefilled, data-driven** — v1 defaults to `imap.gmail.com:993`; host/port are stored in the account row so future provider auto-config can be data-driven (code change not needed for new providers).
- **Honest disclosure** — an app password grants **full mailbox access** (not readonly over the wire, only our IMAP session is readonly via EXAMINE). Stored in the OS keychain, never in SQLite or app logs.

## Watermark and UIDVALIDITY contract

IMAP mailbox identity changes (renumbering, account migration) are tracked via **UIDVALIDITY** (per-folder unique ID generation counter):

- **On first connect**: store `uidvalidity` + `last_uid`.
- **On each tick**: compare the fresh `uidvalidity` from an EXAMINE response against the stored one.
  - **Same** (and watermark exists): server-side search is UID-bounded (`UID SEARCH SINCE <date> UID <last_uid+1>:*` — RFC 3501 §6.4.4 keys are ANDed) to fetch only headers newer than the watermark. Client-side filter also applied.
  - **Different** (renumbered): mailbox was recreated/renumbered. `reset_on_uidvalidity_change` atomically resets `last_uid` to `NULL` AND clears the entire `seen` table (old UIDs are meaningless under the new UIDVALIDITY). Next tick fetches a rolling ~30-day window unbounded by UID.
- **Watermark seed after UIDVALIDITY-change** — the post-tick `advance_last_uid` must be seeded from the **post-tick store state** (which may have been reset by `reset_on_uidvalidity_change`), not the pre-tick snapshot (a stale seed after a renumbering would silently suppress all future matches).

**Critical invariant**: a stale UID bound after a UIDVALIDITY flip is a correctness hazard — UIDs are relative to the current generation, and carrying a high UID from an old generation into a new one risks permanently suppressing real future matches.

## Seen-dedupe semantics

The `seen` table (`uid PK, matched_app_id, ts`) deduplicates per-mailbox:

- **Per fetch**: all headers returned by `UID SEARCH` are marked `seen` (whether they fingerprint/match or not), so a re-poll after a failure never re-fetches or re-matches the same batch.
- **Matched records**: only matched outcomes are handed to `push_and_notify`; non-matching headers are still marked seen (silently consumed).
- **UID-only identity** — the `uid` column is the primary key within a generation (UIDVALIDITY). After a UIDVALIDITY change, the old seen rows are deleted wholesale, and the new generation's UIDs start fresh.

## Fingerprinting: confirmation-email patterns (subject-driven, domain-hinted)

**Subject regex families** (the ONLY gate for a candidate):

```
EN:
  - "thank(?:s| you)(?: you)? for (?:applying|your application)"
  - "we(?:'ve| have)? received your application"
  - "application (?:confirmation|received)"
  - "received your application"

DE:
  - "ihr(?:e|er) bewerbung"
  - "eingangsbestätigung"
  - "deine bewerbung"
```

Each regex requires an **explicit trailing-boundary alternation** (`$`/punctuation/continuation words like EN `{was|is|has|will}` and DE `{ist|war|wurde}`) outside the capture group to prevent greedy/lazy captures from swallowing trailing sentence continuations (e.g., "Acme Corp was received" mistakenly captures "Acme Corp was received" instead of "Acme Corp").

**Domain hints** (Boost-only, never gate):

- Verified: `greenhouse.io`, `greenhouse-mail.io`
- Folklore (hints only, no enforcement): `lever`, `workday`, `linkedin`, `indeed`

Hints boost the match score by 0.05 but never gate a candidate. A mismatch cannot be rescued by a domain hint.

**Metadata extraction** (candidate words):

- Subject + first 500-byte body snippet + sender display-name (fallback)
- Company token and job title, extracted via 4 per-language regexes (EN title+company, EN company-only, DE title+company, DE company-only)
- ATS-suffix stripping on sender name (e.g., "Acme Corp Careers" → "Acme Corp")

## Matching: fuzzy company + title scoring

**Matcher** (`matcher.rs`, pure fn):

1. **Company threshold (hard gate)**: company tokens from email must achieve ≥ 0.5 Jaccard overlap with a saved Application's company name. Zero overlap = immediate reject.
2. **Stopword normalization** (both sides): lowercase + split on non-alphanum + drop generics (`inc, llc, gmbh, corp, ltd, co, company, the, and, und, ag, kg, se`). "Acme Corp" / "Acme, Inc." / "Acme GmbH" all normalize to `{acme}`.
3. **Boosts** (applied after the company threshold):
   - `domain_hint` if sender domain is verified/hinted (+0.05)
   - `title_boost` if title tokens overlap with any saved Application at this company (×0.1 weight on title Jaccard)
4. **Ambiguous ties** — if the top two candidates have equal scores, return `None` (never guess; notify when confident).
5. **Filters to saved Applications only** — `ApplicationStore::list()` returns all records; matcher considers only those with `status == saved`.

**Tuning constants**:

- `COMPANY_THRESHOLD = 0.5` — two genuinely different company names cannot both cross this bar even with both boosts maxed. Ensures precision.
- `DOMAIN_HINT_BOOST = 0.05` — a verified sender domain is a weak tie-breaker, never a true gateway.
- `TITLE_BOOST_WEIGHT = 0.1` — title overlap contributes 10% of the Jaccard score to the final tally. Named consts so they are easy to retune post-launch once real match-quality data exists.

## Honest limits (document-to-user)

1. **INBOX-only** — the watcher scans INBOX exclusively; filters, spam folders, and archived mail are invisible.
2. **Employer must send** — no emails arrive, no match (confirmation email is not generated or never reaches the mailbox). This is not a workaround for manual status entry.
3. **Fuzzy company+title matching precision limits**:
   - A company "Johnson" matched against a saved application at "Johnson and Johnson" will extract only partial overlap (single token shared).
   - An extracted title ("Engineer") that shares a token with the WRONG saved role at the same company (e.g., "Software Engineer" vs "Backend Developer") is ambiguous; the matcher cannot distinguish which role the email is really about.
   - Both cases are documented in tests as known precision limits under notify-only; they become **unsolvable correctness hazards the moment auto-write exists**, requiring a body negative-signal check ("unfortunately", "not moving forward") as a gating precondition.
4. **Gmail v1** — IMAP host is Gmail-specific. Future versions can be data-driven (add `imap.outlook.com`, `imap.yahoo.com`, etc., without code changes), but v1 is branded Gmail-only.
5. **Requires 2FA + app password** — standard Google Account requirement. Workspace admins can disable app passwords, blocking the feature on managed accounts.
6. **Rejection emails fingerprint as confirmations** — a rejection email with subject "Your application to Acme" matches the fingerprint (contains the literal phrase). This is **acceptable under notify-only** (user confirms via the UI, no auto-write). Any future auto-write must include a body negative-signal check before marking `applied`.

## Notification + confirm flow

- **Trigger**: an email matches the fingerprint AND passes the company-threshold gate and any boosts.
- **Notification** (always): `push_and_notify(app, NewNotification{ kind: "email.match", title: "Possible application confirmation", body: "{title} · {company}", route: {to: "/applications", search: {highlight: <appId>}} }, OsBanner::WhenUnfocused)`. The OS banner fires out-of-focus; in-focus, the Notification Center card appears.
- **No auto-write**: the user clicks the notification card → desktop navigates to the Application row and highlights it. User manually confirms via the stage-picker UI ("Applied", "Rejected", etc.).
- **Dedupe**: if the same message ID or UID is already in the `seen` table, it is skipped (no duplicate notifications).

## Scheduler (L2, mirrors autopilot_scheduler)

`email_watch_scheduler.rs`:

- **Startup**: `start(app)` spawns via `tauri::async_runtime::spawn` (never bare `tokio::spawn`), with a 10 s grace period before the first check.
- **Interval**: 60 s internal tick sweep; `is_due` gates on elapsed time since the last check (success or failure).
- **Base interval**: `BASE_CHECK_INTERVAL = 15 min` (900 s). Every tick, compare elapsed time against the current backoff interval.
- **Backoff**: on failure, backoff doubles per attempt (capped at `MAX_BACKOFF = 2 h`), resetting on success. The backoff counter is in-memory (no SQLite persistence); a restart begins at the base interval.
- **Concurrency guard** (`RunGuard`): only one `run_check` executes at a time (process-global `AtomicBool`, RAII-released). A concurrent caller refuses immediately with a rate-limit sentinel (never queues or waits).
- **`run_check_inner(app) -> AppResult<EmailWatchStatus>`**: the shared real fetch+parse+match+notify pass, called by both the scheduler's due-gated tick AND by the `email_watch_check_now` IPC command (manual user trigger). Every exit path (success, skip, error) stamps `last_check_ms` before returning (so consecutive failures backoff, not just successes).
- **Fetch bounds** (defense against DoS on a huge inbox):
  - `MAX_HEADERS_PER_TICK = 200` — stops at 200 headers, oldest-uid-first. Remainder picked up next tick.
  - `MAX_BODY_BYTES = 200_000` — cap before MIME parsing (large attachments are skipped).
  - `SUBJECT_MAX_BYTES = 500` — bound via `safe_prefix` helper before regex matching.

## Privacy discipline

- **No content logging** — diagnostics bundles redact logs, and the `redact_token` helper doesn't cover arbitrary body text. **Solution**: never log subjects, senders, or body snippets anywhere. Only log:
  - IMAP host/port (data-driven configuration, no PII).
  - Error kind labels (`io`, `bad-response`, `no-response`, etc.) via the `error_kind` helper function, never raw error `Display`.
  - `JoinError` task-panic detail (no content).
- **Test fixtures synthetic-only** — all parser/matcher test cases use synthetic subject/sender/company/title combinations, never real company names or PII.

## New IPC commands

| Command                   | In                       | Out                                                          | Notes                                              |
| ------------------------- | ------------------------ | ------------------------------------------------------------ | -------------------------------------------------- |
| `email_watch_status`      | —                        | `{connected, address?, enabled, lastCheckAt?, lastMatchAt?}` | Query current state; no setup needed.              |
| `email_watch_connect`     | `{address, appPassword}` | `{connected, address, enabled, lastCheckAt?, lastMatchAt?}`  | Validate + store account; password never returned. |
| `email_watch_disconnect`  | —                        | `{…status…}`                                                 | Remove credential + clear account row.             |
| `email_watch_set_enabled` | `bool`                   | `{…status…}`                                                 | Toggle the polling schedule on/off.                |
| `email_watch_check_now`   | —                        | `{…status…}`                                                 | Manual tick (rate-limited 60 s min-gap).           |

See `packages/shared/src/ipc/contracts/emailWatch.ts` for the wire shapes.

## Dependencies and security review

- **`mail-parser` 0.11.5** (Apache-2.0 OR MIT) — RFC2047 subject decode, MIME text/plain extraction. Cached build (one compile per workspace).
- **`imap` 3.0.0-alpha.15** (Apache-2.0 OR MIT) — IMAP protocol. Pre-1.0 alpha, pinned exactly (`=3.0.0-alpha.15`) because credential handling is on this path.
- **`native-tls` 0.2** (MIT) — TLS connector for the manual socket path. Reuses the same OS stack (`reqwest` already depends on it).
- **TLS backend rationale** — started with `imap`'s `rustls-tls` feature, but it bridges through `rustls-connector` (pinned `^0.19`) to `rustls 0.22`/`webpki 0.102.8`, which have 4 live RUSTSEC advisories with no update path. Switched to `native-tls` instead (same per-OS stack `reqwest` already uses). See the `Cargo.toml` comment.

## Related documents

- [ADR-0013](../adr/0013-email-confirmation-watching.md) — Design decision: IMAP vs OAuth, app-password honesty, notify-only posture, zero-content-egress guarantee, new egress class.
- [Extension domain (Layer C, extension-domain.md)](./extension-domain.md) — How Layer A (submit-watch) complements Layer C (email-watch).
- [Notification Center (notification-center.md)](./notification-center.md) — How `push_and_notify` routes intents.
