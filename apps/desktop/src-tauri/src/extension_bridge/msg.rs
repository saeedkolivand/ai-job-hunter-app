//! Wire `type` strings — the Rust mirror of the shared `EXTENSION_MESSAGE_TYPES`
//! in `packages/shared/src/ipc/extension-protocol-constants.ts`. A parity test
//! (`super::test`) pins every constant here to the TS literal, and a uniqueness
//! test pins them distinct, so the two sides can never drift.
//!
//! Constants only — no logic, no imports. Lifted verbatim out of `mod.rs` when
//! adding [`TOKEN_REVOKED`] pushed that module past the R8 hard LOC cap: the
//! protocol table is exactly the self-contained surface that belongs in its own
//! file, so every future lockstep edit lands here instead of in the connection
//! module.

/// Handshake step 1 (extension → desktop): `{ protocol, clientNonce }`. NO
/// token — the proof (step 3) authenticates. Must be the FIRST frame.
pub const HELLO: &str = "hello";
/// Handshake step 2 (desktop → extension): `{ serverNonce }`.
pub const CHALLENGE: &str = "challenge";
/// Handshake step 3 (extension → desktop): `{ proof }` where
/// `proof = HMAC-SHA256(token, CLIENT_MSG)`. The token is NEVER on the wire
/// in v2; the desktop verifies `proof` constant-time (see [`super::handshake`]).
pub const AUTH: &str = "auth";
/// Handshake step 4 (desktop → extension): `{ serverProof }` where
/// `serverProof = HMAC-SHA256(token, SERVER_MSG)` — the desktop proving IT
/// knows the token so the extension can reject a rogue/port-squatting peer.
pub const AUTH_OK: &str = "auth.ok";
/// Force-cutover reply (desktop → extension): sent, then the socket closes,
/// when a connection's first frame is not a valid protocol-2 `hello` (e.g. an
/// old extension's legacy `{type:'auth', token}` frame, or a lower protocol).
pub const UPDATE_REQUIRED: &str = "update.required";
/// Revocation signal (desktop → extension): the pairing this socket
/// authenticated with is dead because the token was rotated (Settings →
/// "Regenerate", or a factory reset). No payload, and no token material —
/// it says only "re-pair".
///
/// Sent ONLY over an ALREADY-AUTHENTICATED session, immediately before that
/// socket closes. NEVER to a mid-handshake/unauthenticated peer: telling
/// one its pairing was revoked would confirm its token had been valid — the
/// exact oracle the failed-handshake path's silent, reply-less close exists
/// to deny (ADR-0010).
///
/// It exists because a rotation otherwise strands the extension: its
/// reconnect fails the proof check and gets that same silent close, which
/// is indistinguishable from a crashed app, so it retries the dead token
/// forever instead of showing its pairing view. An old extension ignores an
/// unknown wire `type`, so sending this needs no protocol-version bump.
pub const TOKEN_REVOKED: &str = "token.revoked";
pub const IMPORT_REQUEST: &str = "import.request";
pub const IMPORT_RESULT: &str = "import.result";
/// Extension → desktop: fetch the contact profile for assisted autofill; no
/// payload (authed by the already-authenticated session). Returned only when
/// the autofill opt-in is on, else a refusal `error`.
pub const PROFILE_GET: &str = "profile.get";
/// Desktop → extension: the contact-profile fields for autofill (or an `error`).
pub const PROFILE_RESULT: &str = "profile.result";
/// Extension → desktop: "Check fit" (Scan mode only; no URL-mode fetch).
/// Keyword-only ALWAYS, opt-in-gated (same class as `profile.get`), and
/// per-connection throttled — see [`super::match_live`]'s module doc.
pub const MATCH_LIVE: &str = "match.live";
/// Desktop → extension: the `match.live` outcome. Like `status.update`,
/// this verb's errors ARE user-facing (a deliberate click).
pub const MATCH_RESULT: &str = "match.result";
/// Extension → desktop: "have I already applied to this URL?" — a pure,
/// read-only lookup against the local `ApplicationStore` keyed by the
/// normalized job url (no fetch, never mutates, no consent gate — this is
/// the user's own metadata, device-local, loopback only).
pub const APPLIED_CHECK: &str = "applied.check";
/// Desktop → extension: the `applied.check` outcome (found + optional
/// application id/status/title/appliedAt), or `{ found: false, error }` on
/// a malformed/empty url.
pub const APPLIED_RESULT: &str = "applied.result";
/// Extension → desktop: "mark this URL applied" — a user-gestured WRITE,
/// structurally restricted to the single `saved → applied` transition on
/// an EXACT normalized-URL-key match. Never any other transition, never a
/// fuzzy match; see [`super::status_update::resolve_status_update`] for
/// the allowlist.
pub const STATUS_UPDATE: &str = "status.update";
/// Desktop → extension: the `status.update` outcome — `{ ok: true,
/// applicationId, status }` on success, `{ ok: false, error }` on a
/// refusal (no match / wrong starting status / unsupported transition) or
/// a malformed request. UNLIKE `applied.result`, this verb's errors ARE
/// user-facing (it answers a deliberate click, not a passive check).
pub const STATUS_RESULT: &str = "status.result";
/// Extension → desktop: read the auto-track opt-in (Task #22, auto-track
/// Layer A) — no payload. The extension consults this before ARMING its
/// gesture submit-watcher (client-side gate). Reading the flag needs no
/// consent (it is the user's own device-local setting); the WRITE it gates
/// (`status.update { auto: true }`) is the enforced boundary.
pub const AUTOTRACK_CHECK: &str = "autotrack.check";
/// Desktop → extension: the `autotrack.check` outcome — `{ enabled }`.
pub const AUTOTRACK_RESULT: &str = "autotrack.result";
/// Extension → desktop: read the assisted-autofill opt-in (Task #30) — no
/// payload. Mirrors `AUTOTRACK_CHECK` exactly (see [`autofill_check`]):
/// the popup auto-runs "Suggest answers" only when this reads `true`, but
/// the real gate stays enforced on `answers.suggest` itself.
pub const AUTOFILL_CHECK: &str = "autofill.check";
/// Desktop → extension: the `autofill.check` outcome — `{ enabled }`.
pub const AUTOFILL_RESULT: &str = "autofill.result";
/// Extension → desktop: "save my answers from this page" — append the
/// captured `{question, answer}` pairs onto the Application matched by
/// (canonicalized + normalized) `url`. No match → a refusal telling the
/// user to import the job first; NEVER auto-creates. Rides the SAME
/// assisted-autofill opt-in as `profile.get` (capture is the mirror
/// direction of fill) — see [`super::answers_save::resolve_answers_save`].
pub const ANSWERS_SAVE: &str = "answers.save";
/// Desktop → extension: the `answers.save` outcome — `{ ok: true,
/// applicationId, saved, skipped, title?, company? }` on success, `{ ok:
/// false, error }` on a refusal (opt-in off / no match / malformed
/// request). Like `status.update`, this verb's errors ARE user-facing.
pub const ANSWERS_RESULT: &str = "answers.result";
/// Extension → desktop: "suggest answers for this form" — fuzzy-match the
/// scanned EMPTY question labels against every stored `ApplicationAnswer`
/// across ALL applications. Rides the SAME assisted-autofill opt-in as
/// `profile.get`/`answers.save` — see
/// [`super::answers_suggest::resolve_answers_suggest`].
pub const ANSWERS_SUGGEST: &str = "answers.suggest";
/// Desktop → extension: the `answers.suggest` outcome — `{ ok: true,
/// suggestions: [...] }` on success, `{ ok: false, error }` on a refusal
/// (opt-in off / malformed request). Like `status.update`, this verb's
/// errors ARE user-facing.
pub const ANSWERS_SUGGEST_RESULT: &str = "answers.suggest.result";
/// Extension → desktop: "help me answer this question" — the first
/// BILLABLE-AI verb on the bridge. `{ question, url?, searchWeb? }`.
/// Gated on the SEPARATE `ai_assist_enabled` opt-in (never the
/// assisted-autofill one) — see [`super::answer_assist`].
pub const ANSWER_ASSIST: &str = "answer.assist";
/// Desktop → extension: the `answer.assist` outcome — `{ ok: true,
/// question, draft, sourced: {web?, brief?, salary?} }` on success,
/// `{ ok: false, error }` on a refusal (opt-in off / no usable AI
/// provider configured / malformed request). Like `status.update`, this
/// verb's errors ARE user-facing.
pub const ANSWER_ASSIST_RESULT: &str = "answer.assist.result";
/// Desktop → extension: one incremental delta of a streaming reply —
/// `{ delta }`. The envelope's own `reqId` correlates it to the original
/// request; additive so a future streaming verb rides the same family —
/// see [`super::answer_assist`]'s streaming doc.
pub const ASSIST_CHUNK: &str = "assist.chunk";
/// Desktop → extension: no payload — the stream named by the envelope's
/// `reqId` has ended (success or failure); the verb's own terminal reply
/// (e.g. `ANSWER_ASSIST_RESULT`) carries the actual outcome. A generic,
/// verb-agnostic mux signal so a background accumulator can retire its
/// buffer for `reqId` without parsing every verb's reply shape.
pub const ASSIST_DONE: &str = "assist.done";
/// Extension → desktop: no payload — cancel the in-flight stream named
/// by the envelope's `reqId` (starting a new draft/rewrite supersedes
/// the previous one). Best-effort, no reply — dispatched against THIS
/// connection's own [`stream::AssistStreamRegistry`], never a global
/// registry (see [`stream`]'s module doc).
pub const ASSIST_CANCEL: &str = "assist.cancel";
/// CLI → desktop: the read-only agent surface (issue #1084 PR 1).
/// `{ resource: "best-matches"|"job"|"profile"|"automations"|"schema", url?,
/// limit? }` — see [`super::agent_read`]'s module doc for the full wire shape
/// per resource. **CLI-agent only, and now actually gated on it**
/// (`super::advance_authenticated` refuses this for any connection whose
/// handshake `Origin` isn't `auth::AGENT_CLI_ORIGIN` — finding #5, security
/// review; before that fix this was true of the extension's OWN code, but
/// unenforced server-side, so any authenticated socket could send it).
/// Unlike every other constant in this file, this one is deliberately NOT
/// mirrored in the shared TS `EXTENSION_MESSAGE_TYPES`
/// (`packages/shared/src/ipc/extension-protocol-constants.ts`) or the
/// Rust↔TS parity test (`super::test::message_type_constants_match_ts`): the
/// browser extension never sends it (the CLI is a separate Rust process
/// speaking this same loopback protocol directly), so there is no
/// browser-vs-desktop drift for `extension-standards`' protocol-lockstep
/// rule to guard against. A future PR that lets the extension itself use
/// this verb would need to add both AND extend the origin gate.
pub const AGENT_QUERY: &str = "agent.query";
/// Desktop → extension/CLI: the `agent.query` outcome — `{ ok: true,
/// resource, data } | { ok: false, resource, error }`. See [`AGENT_QUERY`]'s doc.
pub const AGENT_RESULT: &str = "agent.result";
