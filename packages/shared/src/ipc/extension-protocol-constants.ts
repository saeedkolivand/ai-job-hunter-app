/**
 * Browser-extension ⇄ desktop protocol — zod-free constants & types.
 *
 * Split out from `extension-protocol.ts` so the browser extension can import the
 * wire-message constants and payload types WITHOUT pulling zod into its bundle
 * (zod v4's JIT `Function("")` feature-probe trips AMO's `DANGEROUS_EVAL` lint).
 * The zod schemas in `extension-protocol.ts` are bound to these interfaces so the
 * two can never drift. This module is pure data + types — no zod, no `window`,
 * no Node.
 *
 * ## Protocol v2 — mutual HMAC challenge-response (the token NEVER goes on the wire)
 * v2 replaces the plaintext per-frame token with a mutual challenge-response so
 * neither a passive observer nor a port-squatter ever sees the pairing secret:
 *
 * 1. Extension → `hello  { protocol: 2, clientNonce }`   (no token)
 * 2. Desktop   → `challenge { serverNonce }`
 * 3. Extension → `auth   { proof }`      proof = HMAC-SHA256(token, CLIENT_MSG)
 * 4. Desktop verifies `proof` CONSTANT-TIME → `auth.ok { serverProof }`
 *                                       serverProof = HMAC-SHA256(token, SERVER_MSG)
 * 5. Extension verifies `serverProof` CONSTANT-TIME. Only then is the socket
 *    authenticated; a bad `serverProof` means the peer can't prove it knows the
 *    token (rogue/port-squatter) → the extension sends NO profile/PII.
 * 6. After auth the socket is session-authenticated: subsequent frames
 *    (`import.request`, `profile.get`, …) carry NO token — the desktop authorizes
 *    them by the already-authenticated connection.
 *
 * The HMAC key is the token's raw UTF-8 bytes (the token is 64-char lowercase
 * hex; its 64 ASCII bytes are the key). The message is domain-separated and
 * canonical so both implementations build a byte-identical string — see
 * {@link handshakeMessage}. Force cutover: the desktop rejects any legacy
 * `{ type:'auth', token }` first frame with `update.required` and closes.
 */

/** Current handshake protocol version carried in the `hello` frame. */
export const EXTENSION_PROTOCOL_VERSION = 2;

/**
 * Every wire message `type`. The v2 handshake types are `hello` / `challenge` /
 * `auth` (now carrying `{ proof }`, NOT a token) / `auth.ok`; `update.required`
 * is the force-cutover reply the desktop sends when a connection's first frame
 * is not a valid v2 `hello` (a legacy token `auth`, a missing/older protocol).
 * `import.request` / `import.result` / `profile.get` / `profile.result` /
 * `applied.check` / `applied.result` / `status.update` / `status.result` /
 * `answers.save` / `answers.result` / `match.live` / `match.result` /
 * `answer.assist` / `answer.assist.result` are the post-auth application
 * frames.
 *
 * **Consent-gate boundary** (the rule the remaining post-auth verbs copy): a
 * read-only lookup over the user's own device-local metadata (`applied.check`)
 * needs no desktop opt-in gate; a user-gestured WRITE to the user's own local
 * store on an exact match (`status.update`) needs no gate either — it is a
 * deliberate click, not a passive/background write, exactly like the
 * existing ungated `import.request{applied:true}`; anything returning fresh
 * PII (`profile.get`) or doing billable/egress work requires a
 * desktop-enforced opt-in. `profile.result.extraLinks` (additional labelled
 * link URLs) is fresh PII of exactly the same class as the rest of that
 * payload (`linkedin`/`github`/`website`, …), so it rides the SAME `profile.get`
 * opt-in gate — no separate consent surface, no protocol bump. `answers.save`
 * WRITES freshly-captured page-derived text into the local store — the
 * mirror direction of `profile.get`'s fill (capture vs. fill are the two
 * directions of the one PII-adjacent gesture) — so it rides that SAME
 * assisted-autofill opt-in too, not the read-only/exact-match-write carve-out
 * `applied.check`/`status.update` get. `match.live` ALSO rides that same
 * opt-in (moved out of the ungated bucket): its `gaps` field is effectively a
 * résumé-keyword membership oracle (which of the user's résumé keywords are
 * ABSENT from the posting), the same consent class as `profile.get`'s PII and
 * `answers.suggest`'s past-answer text — a pure local computation is not by
 * itself a reason to skip the gate. The import-time `matchScore` fill
 * (`import.result.matchScore`) stays ungated: it rides the already-consented
 * import gesture and reveals only a single number, never `gaps`.
 *
 * `answer.assist` gets its OWN, SEPARATE opt-in bucket
 * (`extension_ai_assist_optin`) rather than riding the assisted-autofill one
 * above: it is the first BILLABLE-AI verb on the bridge — real provider
 * spend, not just a local computation or a PII-adjacent local read — a
 * materially different consent class that needs its own desktop-enforced
 * gate, checked before any of the question/context is touched.
 */
export const EXTENSION_MESSAGE_TYPES = {
  /** Extension → desktop: handshake step 1 — `{ protocol, clientNonce }`, no token. */
  hello: 'hello',
  /** Desktop → extension: handshake step 2 — `{ serverNonce }`. */
  challenge: 'challenge',
  /** Extension → desktop: handshake step 3 — `{ proof }` (HMAC over the client role). */
  auth: 'auth',
  /** Desktop → extension: handshake step 4 — `{ serverProof }` (HMAC over the server role). */
  authOk: 'auth.ok',
  /**
   * Desktop → extension: force-cutover signal. Sent (then the socket closes) when
   * a connection's first frame is not a valid protocol-2 `hello` — e.g. an old
   * extension's legacy `{ type:'auth', token }` frame. The new extension surfaces
   * this (and a never-a-`challenge` outcome) as an `outdated` state.
   */
  updateRequired: 'update.required',
  /** Extension → desktop: import a job (URL mode, or Scan mode with `html`). No token. */
  importRequest: 'import.request',
  /** Desktop → extension: the import outcome (or an `error`). */
  importResult: 'import.result',
  /**
   * Extension → desktop: fetch the user's Contact Profile fresh for assisted
   * autofill; no payload (authed by the already-authenticated session). The
   * desktop returns the profile ONLY when the autofill opt-in is on, else a
   * refusal `error`. The profile is held transiently for the one fill and NEVER
   * persisted client-side.
   */
  profileGet: 'profile.get',
  /** Desktop → extension: the contact profile fields for autofill (or an `error`). */
  profileResult: 'profile.result',
  /**
   * Extension → desktop: "Check fit" — score the résumé against the open
   * posting's captured DOM (Scan mode only, the SAME capture `import.request`
   * uses; no URL-mode network fetch on this path). Keyword-only ALWAYS — see
   * {@link ExtensionMatchLiveResult}'s doc for why the bridge can never honor
   * the app's semantic-scoring setting. Rides the assisted-autofill opt-in
   * (see the module doc's "Consent-gate boundary" section) — a refusal is
   * just another user-facing `error` on {@link ExtensionMatchLiveResult}.
   */
  matchLive: 'match.live',
  /**
   * Desktop → extension: the `match.live` outcome. Like `status.update`, this
   * verb's errors are user-facing — it answers a deliberate click, never a
   * passive background check.
   */
  matchResult: 'match.result',
  /**
   * Extension → desktop: "have I already applied to this URL?" — a pure,
   * read-only lookup keyed by the normalized job url (no fetch, never
   * mutates). No consent gate: this is the user's own metadata, device-local,
   * loopback only.
   */
  appliedCheck: 'applied.check',
  /** Desktop → extension: the `applied.check` outcome (or an `error`). */
  appliedResult: 'applied.result',
  /**
   * Extension → desktop: "mark this URL applied" — a user-gestured WRITE,
   * structurally restricted to the single `saved → applied` transition on an
   * EXACT normalized-url match (see {@link ExtensionStatusUpdateRequest}). No
   * consent gate: a deliberate click writing to the user's own local store,
   * strictly narrower than the already-ungated `import.request{applied:true}`.
   */
  statusUpdate: 'status.update',
  /**
   * Desktop → extension: the `status.update` outcome. UNLIKE
   * `applied.result`, this verb's errors are user-facing — the popup must
   * render `error`, never fold it into a silent no-op (it answers a
   * deliberate click, not a passive background check).
   */
  statusResult: 'status.result',
  /**
   * Extension → desktop: read the auto-track opt-in (Task #22, auto-track
   * Layer A) — no payload. The extension consults this before ARMING the
   * gesture submit-watcher (client-side gate); the desktop is the
   * authoritative gate (it ALSO re-checks the opt-in before honoring an AUTO
   * `status.update` — see {@link ExtensionStatusUpdateRequest}'s `auto`). No
   * consent needed to READ the flag: it is the user's own device-local
   * setting, loopback only.
   */
  autotrackCheck: 'autotrack.check',
  /** Desktop → extension: the `autotrack.check` outcome — `{ enabled }`. */
  autotrackResult: 'autotrack.result',
  /**
   * Extension → desktop: "save my answers from this page" — append the
   * captured `{question, answer}` pairs from the active tab's filled form
   * fields onto the Application matched by (canonicalized + normalized)
   * `url`. No match → a refusal telling the user to import the job first
   * (see {@link ExtensionAnswersSaveResult}); NEVER auto-creates. Rides the
   * assisted-autofill opt-in (same gate as `profile.get`/`fill` — capture is
   * the mirror direction of fill).
   */
  answersSave: 'answers.save',
  /**
   * Desktop → extension: the `answers.save` outcome. Like `status.update`,
   * this verb's errors are user-facing — the popup must render `error`,
   * never fold it into a silent no-op (it answers a deliberate click).
   */
  answersResult: 'answers.result',
  /**
   * Extension → desktop: "suggest answers for this form" — the labels of the
   * active tab's EMPTY candidate fields (questions-mode collector), fuzzy-
   * matched against every stored `ApplicationAnswer` across ALL applications.
   * Pure local Rust (token-Jaccard, no AI, no egress). Rides the SAME
   * assisted-autofill opt-in as `profile.get`/`answers.save`: a suggestion
   * carries the user's own past answer text, the same consent class as
   * profile data.
   */
  answersSuggest: 'answers.suggest',
  /**
   * Desktop → extension: the `answers.suggest` outcome. Like `status.update`,
   * this verb's errors are user-facing — the popup must render `error`,
   * never fold it into a silent no-op (it answers a deliberate click).
   */
  answersSuggestResult: 'answers.suggest.result',
  /**
   * Extension → desktop: "help me answer this question" — the first
   * BILLABLE-AI verb on the bridge (extension roadmap PR 9). `question` is
   * page/user-derived and UNTRUSTED; the desktop fences it in the prompt. A
   * salary-shaped question routes through the shared salary machinery
   * (scraped Application salary + a web-researched market range); any other
   * question gets a grounded, paste-ready draft (job/company context + the
   * default résumé). Rides a SEPARATE opt-in from `profile.get`/
   * `answers.save` (`extension_ai_assist_optin`, default OFF) — this is
   * billable provider spend, a materially different consent class from the
   * local/free verbs above. `searchWeb` (default OFF, mirrors the in-app
   * toggle) additionally fetches web-search reference notes for the question
   * first; the answer generates identically (just without web grounding) if
   * that lookup is unavailable or fails.
   */
  answerAssist: 'answer.assist',
  /**
   * Desktop → extension: the `answer.assist` outcome — a discriminated union
   * so success/failure fields can never mix. `ok:true` carries the finished
   * `draft` (paste-ready prose, copy-only — there is no fill path for AI
   * text) plus `sourced` flags naming which optional context actually
   * grounded it. `ok:false` carries a user-facing `error` (opt-in off / no
   * usable AI provider configured / a malformed request) — like
   * `status.update`, this verb's errors ARE shown to the user, it answers a
   * deliberate click.
   */
  answerAssistResult: 'answer.assist.result',
  /**
   * Desktop → extension: one incremental delta of a streaming reply —
   * `{ delta }`. The envelope's own `reqId` correlates it to the original
   * request (currently only `answer.assist`; additive so a future streaming
   * verb rides the SAME frame family). Zero or more of these precede the
   * verb's own terminal reply (e.g. `answer.assist.result`); accumulate
   * `delta` by `reqId` — a chunk alone is never a complete answer.
   */
  assistChunk: 'assist.chunk',
  /**
   * Desktop → extension: `{ reqId }`-only (no payload) — the stream named by
   * the envelope's `reqId` has ended, success OR failure. A generic,
   * verb-agnostic mux signal: the verb's own terminal reply carries the
   * actual outcome, so a background accumulator can retire its buffer for
   * `reqId` on this frame without parsing every verb's own reply shape. Also
   * the definitive "stop waiting for more chunks" signal for the resilience
   * case (a dropped connection mid-stream never sends this — the client's
   * own transport-close handling is what surfaces "interrupted" in that
   * case, not this frame).
   */
  assistDone: 'assist.done',
  /**
   * Extension → desktop: `{ reqId }`-only — cancel the in-flight stream
   * named by the envelope's `reqId` (e.g. starting a new draft/rewrite
   * supersedes the previous one, mirroring an in-app `AbortController`).
   * Best-effort: the desktop stops driving the provider call early; the
   * per-provider daily-budget charge already made for that call is NOT
   * refunded. No reply is ever sent for a cancelled `reqId` — the caller
   * already stopped listening for it.
   */
  assistCancel: 'assist.cancel',
} as const;

/** Union of all wire `type` strings. */
export type ExtensionMessageType =
  (typeof EXTENSION_MESSAGE_TYPES)[keyof typeof EXTENSION_MESSAGE_TYPES];

// ── Handshake message canonicalization (byte-identical on both sides) ──────────

/**
 * Domain-separation prefix for the handshake HMAC. Bumped with the protocol
 * version so a v1↔v2 confusion can never produce a matching proof.
 */
export const HANDSHAKE_DOMAIN = 'ajh-bridge/v2';

/** The two HMAC roles: the client proves in step 3, the server proves in step 4. */
export type HandshakeRole = 'client' | 'server';

/**
 * Build the canonical, domain-separated message that both sides HMAC (keyed by
 * the token's raw UTF-8 bytes). Byte-for-byte identical on the Rust and Web
 * Crypto sides — the shared known-answer vector ({@link HANDSHAKE_TEST_VECTOR})
 * pins this so the two canonicalizations can never silently drift:
 *
 * ```text
 * ajh-bridge/v2\n<role>\n<serverNonceHex>\n<clientNonceHex>
 * ```
 *
 * Nonces are lowercase-hex strings (≥16 random bytes, fresh per connection).
 */
export function handshakeMessage(
  role: HandshakeRole,
  serverNonceHex: string,
  clientNonceHex: string
): string {
  return `${HANDSHAKE_DOMAIN}\n${role}\n${serverNonceHex}\n${clientNonceHex}`;
}

/**
 * Cross-implementation known-answer vector for the handshake HMAC. Asserted by a
 * TS test (Web Crypto) AND a Rust test (`extension_bridge::handshake`) against
 * the SAME expected proofs — if the two byte-canonicalizations ever drift, one
 * side's KAT fails loudly instead of the handshake silently never matching.
 *
 * `proof = HMAC-SHA256(key = utf8(token), msg = handshakeMessage(role, serverNonce, clientNonce))`,
 * lowercase hex. The token here is a fixed 64-char hex string (its 64 ASCII bytes
 * are the key).
 */
export const HANDSHAKE_TEST_VECTOR = {
  token: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  clientNonce: '00112233445566778899aabbccddeeff',
  serverNonce: 'ffeeddccbbaa99887766554433221100',
  clientProof: 'fe16f06234473b154c4e96d43bd25c603975cb2584f950d0d4f495edc5c44f1a',
  serverProof: '75c05269902c14d97ee61a05f4c9dbf812c532735b836665da250b19ce405831',
} as const;

// ── Payload shapes ─────────────────────────────────────────────────────────────

/** `hello` payload — the handshake opener. `protocol` gates the force cutover. */
export interface ExtensionHelloPayload {
  protocol: number;
  /** Fresh client nonce, lowercase hex (≥16 bytes). */
  clientNonce: string;
}

/** `challenge` payload — the server's fresh nonce, lowercase hex (≥16 bytes). */
export interface ExtensionChallengePayload {
  serverNonce: string;
}

/** `auth` payload — the client's proof; the token is NEVER on the wire in v2. */
export interface ExtensionAuthPayload {
  /** HMAC-SHA256(token, CLIENT_MSG) as lowercase hex. */
  proof: string;
}

/** `auth.ok` payload — the server's proof the extension verifies before trusting. */
export interface ExtensionAuthOkPayload {
  /** HMAC-SHA256(token, SERVER_MSG) as lowercase hex. */
  serverProof: string;
}

/**
 * `import.request` payload. `html` present ⇒ Scan mode (the extension supplies
 * the authenticated DOM); absent ⇒ URL mode (the desktop fetches + scrapes).
 * `applied` flags the job as already applied (Saved origin otherwise → `saved`).
 */
export interface ExtensionImportRequest {
  url: string;
  html?: string;
  applied?: boolean;
}

/**
 * `import.result` payload. On success carries the created/merged
 * `applicationId` + its `status`, plus the parsed `title`/`company` so the
 * popup can confirm WHICH job was imported. `matchScore` is a best-effort
 * keyword-only ATS score (0–100) against the user's default/most-recent
 * résumé — see {@link ExtensionMatchLiveResult}'s doc for why it is always
 * keyword-only. It is OMITTED (not `0`/`null`) when scoring failed for any
 * reason (no résumé saved yet, unusable posting text, a scoring timeout) —
 * the import itself always succeeds regardless of whether this field is
 * present. UNLIKE `match.live`, this field is never gated on the
 * assisted-autofill opt-in: it rides the already-consented import gesture
 * and reveals only a single number, never the `gaps` list. On failure
 * carries `error`.
 */
export interface ExtensionImportResult {
  applicationId?: string;
  status?: string;
  title?: string;
  company?: string;
  matchScore?: number;
  error?: string;
  /** True when the import was saved but the page could not be fully parsed (stub posting). */
  partial?: boolean;
}

/**
 * `profile.result` payload. On success carries the user's Contact Profile fields
 * for assisted autofill (each optional — a sparse profile is normal); `location`
 * is the profile's single free-text location string (the desktop resolves it from
 * `ContactProfile.location.default`). On refusal (autofill opt-in off) or failure
 * carries `error`. These fields are transient: the extension uses them for the one
 * fill and never writes them to `chrome.storage`.
 *
 * `extraLinks` is fresh PII-over-the-wire exactly like the rest of this payload
 * (a link URL, same as `linkedin`/`github`/`website`) — it rides the SAME
 * autofill opt-in gate; there is no separate consent surface for it. Additive
 * and optional: an old extension that has never heard of the key ignores it,
 * and an old desktop simply never sends it, so there is no protocol bump. The
 * desktop caps it at 10 entries and drops any entry with an empty label or a
 * non-`http(s)` url before it is ever sent.
 */
export interface ExtensionProfileResult {
  fullName?: string;
  email?: string;
  phone?: string;
  location?: string;
  linkedin?: string;
  github?: string;
  website?: string;
  extraLinks?: { label: string; url: string }[];
  /** Refusal (autofill disabled) or failure reason; present ⇒ no fields were sent. */
  error?: string;
}

/**
 * `applied.check` payload — the active tab's URL to look up. Read-only: this
 * never fetches or creates anything, it only asks "does an Application already
 * exist for this url?".
 */
export interface ExtensionAppliedCheckRequest {
  url: string;
}

/**
 * `applied.result` payload. `found` is always present; the rest are populated
 * only when an Application already exists for the (canonicalized + normalized)
 * url. `status` is the lowercase `ApplicationStatus` id (`saved`, `applied`,
 * …). `appliedAt` is epoch ms (matches `Application.applied_at`) — the popup
 * formats it locally. `error` carries a malformed-request or lookup failure;
 * the popup treats ANY error the same as "not found" (renders nothing).
 */
export interface ExtensionAppliedCheckResult {
  found: boolean;
  applicationId?: string;
  status?: string;
  title?: string;
  appliedAt?: number;
  error?: string;
}

/**
 * `status.update` payload — mark a `saved` Application `applied` on an exact
 * URL-key match. `to` is narrowed to the literal `'applied'` in both this
 * schema and the Rust re-validation: the verb is structurally incapable of
 * any other transition (no allowlist to bypass — there is only one value).
 */
export interface ExtensionStatusUpdateRequest {
  url: string;
  to: 'applied';
  /**
   * True marks this an AUTOMATED write from the gesture submit-watcher (Task
   * #22, auto-track Layer A), not a deliberate popup click. The desktop honors
   * an `auto` write ONLY when the auto-track opt-in is on — defense-in-depth:
   * the extension also gates arming client-side, but a compromised extension
   * must not auto-write `applied` without the user's opt-in. Absent/false is
   * the ordinary user-clicked "Mark as applied", ungated exactly as before.
   */
  auto?: boolean;
}

/**
 * `autotrack.result` payload — whether the desktop-enforced auto-track opt-in
 * (Task #22) is currently on. The extension reads it before arming the gesture
 * submit-watcher; a malformed/absent reply is treated as `false` (OFF, the safe
 * default) client-side.
 */
export interface ExtensionAutotrackResult {
  enabled: boolean;
}

/**
 * `status.update` payload — a discriminated union so a reply can never mix
 * success and failure fields: `ok:true` always carries the updated
 * `applicationId` + `status` (the literal `'applied'`); `ok:false` always
 * carries a user-facing `error` (no match / the row's status was not `saved`
 * / a malformed request). UNLIKE {@link ExtensionAppliedCheckResult}, this
 * verb's errors ARE shown to the user — it answers a deliberate click, not a
 * passive background check.
 */
export type ExtensionStatusUpdateResult =
  { ok: true; applicationId: string; status: 'applied' } | { ok: false; error: string };

/** One captured application-form question/answer pair (page-derived, UNTRUSTED
 *  text) — see {@link ExtensionAnswersSaveRequest}. */
export interface ExtensionAnswerPair {
  question: string;
  answer: string;
}

/**
 * `answers.save` payload — the active tab's url plus the captured
 * `{question, answer}` pairs to append onto the matched Application. Untrusted
 * page-derived content: the desktop clamps each field's byte length and the
 * array's entry count at the store boundary (client-side validation here is
 * shape-only).
 */
export interface ExtensionAnswersSaveRequest {
  url: string;
  answers: ExtensionAnswerPair[];
}

/**
 * `answers.save` payload — a discriminated union so a reply can never mix
 * success and failure fields: `ok:true` carries the matched `applicationId`,
 * `saved` (newly-added count) and `skipped` (dedup-dropped count), plus the
 * OPTIONAL `title`/`company` of the matched Application — included on the
 * wire (unlike {@link ExtensionStatusUpdateResult}, which keeps them
 * notification-only) because the handler already loaded the Application row
 * for this verb, so surfacing them here is the smaller change than threading
 * the popup's separately-fetched `applied.check` state through to this
 * confirmation. `ok:false` always carries a user-facing `error` (autofill
 * opt-in off / no match — import the job first / a malformed request).
 * UNLIKE {@link ExtensionAppliedCheckResult}, this verb's errors ARE shown to
 * the user — it answers a deliberate click, not a passive background check.
 */
export type ExtensionAnswersSaveResult =
  | {
      ok: true;
      applicationId: string;
      saved: number;
      skipped: number;
      title?: string;
      company?: string;
    }
  | { ok: false; error: string };

/**
 * `answers.suggest` payload — the (client-capped) labels of the active tab's
 * EMPTY candidate fields to fuzzy-match. Untrusted page-derived content: the
 * desktop clamps each entry's byte length and the array's entry count at the
 * matcher boundary (client-side validation here is shape-only).
 */
export interface ExtensionAnswersSuggestRequest {
  questions: string[];
}

/**
 * One matched suggestion — `sourceCompany`/`sourceTitle` name the Application
 * the answer came from (omitted when blank); `sourceQuestion` is the matched
 * candidate's ORIGINAL (raw) question text, always present — the popup shows
 * it as "answered as: '…'" so a cross-question match (two questions similar
 * enough on filler words to cross the matcher's threshold but about
 * different things) is visually self-evident rather than silent; `score` is
 * the matcher's token-Jaccard similarity (0–1, diagnostic only). `salary` is
 * true when EITHER the scanned `question` OR `sourceQuestion` matched a
 * salary keyword — the popup MUST NOT offer "Fill this field" for such a
 * suggestion, only Copy.
 */
export interface ExtensionAnswerSuggestion {
  question: string;
  answer: string;
  sourceCompany?: string;
  sourceTitle?: string;
  sourceQuestion: string;
  score: number;
  salary: boolean;
}

/**
 * `answers.suggest` payload — a discriminated union so a reply can never mix
 * success and failure fields: `ok:true` carries at most 20
 * {@link ExtensionAnswerSuggestion} entries (one per matched question);
 * `ok:false` always carries a user-facing `error` (autofill opt-in off / a
 * malformed request). UNLIKE {@link ExtensionAppliedCheckResult}, this verb's
 * errors ARE shown to the user — it answers a deliberate click, not a
 * passive background check.
 */
export type ExtensionAnswersSuggestResult =
  { ok: true; suggestions: ExtensionAnswerSuggestion[] } | { ok: false; error: string };

/**
 * The 5 quick-action rewrite presets (extension PR 11) — mirrors the in-app
 * `RewritePopover`'s `PRESETS` ids exactly (`shorten`/`expand`/`rephrase`/
 * `impact`/`grammar`); the instruction TEXT each maps to is ported to Rust
 * (see `extension_bridge::answer_rewrite`'s preset map — source of truth:
 * `packages/prompts/src/generate/rewrite/rewrite.ts` + `RewritePopover.tsx`).
 */
export type ExtensionRewritePreset = 'shorten' | 'expand' | 'rephrase' | 'impact' | 'grammar';

/**
 * `answer.assist` payload — a pasted/picked application question to draft an
 * answer for, OR (PR 11, `mode: 'rewrite'`) an existing answer to rewrite.
 *
 * **Draft mode** (`mode` omitted or `'draft'` — back-compat default):
 * `question` is page/user-derived and UNTRUSTED (fenced by the desktop's
 * prompt layer, never treated as an instruction). `url` (the active tab's
 * url, when known) lets the desktop resolve grounding context from the
 * matching Application (job description / company brief / scraped salary) —
 * absent or unmatched falls back to generic grounding (résumé only).
 * `searchWeb` (default OFF, mirrors the in-app toggle) opts into fetching
 * web-search reference notes for the question BEFORE drafting; the answer
 * still generates (without web grounding) if that lookup is unavailable or
 * fails — this can never block the draft.
 *
 * **Rewrite mode** (`mode: 'rewrite'`): a PURE TEXT TRANSFORM of
 * `existingAnswer` (the text already typed into a picked form field) per
 * `preset` (one of {@link ExtensionRewritePreset}, server-resolved to its
 * instruction) or a free-text `instruction` — mirrors the in-app
 * `RewritePopover`, which transforms a selection, not a document-grounded
 * generation. Unlike draft mode, this NEVER pulls résumé/job/company/salary
 * grounding and NEVER routes through the web-search lookup (`searchWeb` is
 * ignored). `existingAnswer`/`instruction` are page/user-derived and
 * UNTRUSTED (fenced the same way as `question`) — `existingAnswer` is
 * additionally PII-adjacent (the user's own past answer): sent transiently,
 * never persisted. `question` is still required/echoed for reply
 * correlation but is NOT fed into the rewrite prompt itself.
 */
export interface ExtensionAnswerAssistRequest {
  question: string;
  url?: string;
  searchWeb?: boolean;
  mode?: 'draft' | 'rewrite';
  existingAnswer?: string;
  preset?: ExtensionRewritePreset;
  instruction?: string;
}

/**
 * `answer.assist` payload — a discriminated union so a reply can never mix
 * success and failure fields: `ok:true` carries the original `question`
 * (echoed so the popup can confirm which draft this is, in case of a fast
 * follow-up submit) and the finished `draft` (paste-ready prose — COPY ONLY,
 * there is no fill path for AI-generated text), plus `sourced` naming which
 * optional context actually grounded it (`web` — the opt-in search notes were
 * fetched and non-empty; `brief` — a cached company brief from the matched
 * Application was used; `salary` — this question routed the salary-shaped
 * path). Unchanged by rewrite mode (PR 11) — the rewritten text rides the
 * SAME `draft` field, and every `sourced` flag is always `false` (rewrite is
 * a pure text transform, never grounded in résumé/job/company/salary).
 * `ok:false` always carries a user-facing `error` (the ai-assist opt-in is
 * off / no usable AI provider is configured / a malformed request) — like
 * {@link ExtensionStatusUpdateResult}, this verb's errors ARE shown to the
 * user, it answers a deliberate click.
 */
export type ExtensionAnswerAssistResult =
  | {
      ok: true;
      question: string;
      draft: string;
      sourced: { web?: boolean; brief?: boolean; salary?: boolean };
    }
  | { ok: false; error: string };

/**
 * `match.live` payload — the user-clicked "Check fit". `html` is the SAME
 * Scan-mode DOM capture `import.request` sends (required here, not optional:
 * there is no URL-mode network-fetch fallback for this verb, so the scoring
 * path never adds egress beyond the DOM the extension already captured).
 */
export interface ExtensionMatchLiveRequest {
  url: string;
  html: string;
}

/**
 * `match.live` payload — a discriminated union so a reply can never mix
 * success and failure fields: `ok:true` carries the weighted `combined`
 * score, the keyword-coverage `ats` sub-score, up to 8 missing-keyword
 * `gaps`, the scored résumé's `resumeName` (so the popup can confirm WHICH
 * résumé was used), and `scoreSource`. `ok:false` carries a user-facing
 * `error` (the assisted-autofill opt-in is off / no résumé saved yet / the
 * page couldn't be read / too many requests in quick succession / a
 * malformed request) — like {@link ExtensionStatusUpdateResult}, this verb's
 * errors ARE shown to the user, it answers a deliberate click, not a passive
 * check.
 *
 * `semantic`/`scoreSource: 'combined'` are wire-reserved but NEVER populated
 * by the current desktop implementation: the app's semantic-scoring setting
 * lives ONLY in the renderer's `preferences-store` (persisted to the
 * webview's `localStorage`), which the Rust extension-bridge has no read
 * access to — so the extension's live-match path is unconditionally
 * keyword-only today (`scoreSource` is always `'keyword'`). These fields are
 * reserved so a future PR that gives the bridge a Rust-readable version of
 * that setting doesn't need a protocol bump.
 */
export type ExtensionMatchLiveResult =
  | {
      ok: true;
      combined: number;
      ats: number;
      semantic?: number;
      gaps: string[];
      resumeName: string;
      scoreSource: 'keyword' | 'combined';
    }
  | { ok: false; error: string };

/**
 * `assist.chunk` payload — one incremental delta of a streaming reply. The
 * envelope's `reqId` names which in-flight request this chunk belongs to;
 * this payload carries only the text itself.
 */
export interface ExtensionAssistChunkPayload {
  delta: string;
}

/**
 * The transport envelope every frame is wrapped in. `payload` is left as
 * unknown here (each `type` narrows it via its own payload schema) so a single
 * envelope schema validates the frame shell before the handler dispatches.
 *
 * v2 removed the `token` field entirely: the handshake authenticates the socket
 * (session auth), so no frame ever carries the pairing secret.
 */
export interface ExtensionEnvelope {
  type: ExtensionMessageType;
  /** Caller-chosen correlation id echoed back on the matching reply. */
  reqId: string;
  payload: unknown;
}
