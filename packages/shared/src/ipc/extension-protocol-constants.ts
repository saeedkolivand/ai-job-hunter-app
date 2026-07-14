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
 * `answers.save` / `answers.result` are the post-auth application frames;
 * `match.live` is **reserved** (fixed now so a future build can add a
 * handler without a protocol bump).
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
 * `applied.check`/`status.update` get.
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
  /** RESERVED — live ATS match for the open posting (not yet handled). */
  matchLive: 'match.live',
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
 * popup can confirm WHICH job was imported; `matchScore` is reserved for the
 * future live-match reply. On failure carries `error`.
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
 * the answer came from (omitted when blank); `score` is the matcher's
 * token-Jaccard similarity (0–1, diagnostic only). `salary` is true when the
 * (scanned) question matched a salary keyword — the popup MUST NOT offer
 * "Fill this field" for such a suggestion, only Copy.
 */
export interface ExtensionAnswerSuggestion {
  question: string;
  answer: string;
  sourceCompany?: string;
  sourceTitle?: string;
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
