---
status: accepted
---

# Bridge authentication via mutual HMAC handshake (protocol v2)

## Context

The desktop⇄extension bridge (Feature 2) authenticated every frame with the **pairing token sent in plaintext** — an `auth` frame right after the socket opens, then the same `token` field on every `import.request`. Three layers guarded the channel: the WS `Origin` allowlist, the per-frame token, and the up-front `auth` check.

The weakness is that the token rides the wire. The bridge binds `127.0.0.1` loopback; a **malicious local process that squats the port before the real app starts** becomes the server the extension connects to, receives the `auth` frame, and **harvests a reusable token**. With the extension autofill feature ([ADR 0009](0009-assisted-autofill.md)), that token also grants Contact-Profile read, so the blast radius grew — a `tauri-security-reviewer` finding on that PR flagged the port-squat risk as newly PII-bearing and recommended a proper challenge/HMAC handshake now that PII rides the same auth path. The token comparison was also a plain `!=` (not constant-time).

The threat is bounded (same-account local process, squat-before-bind, per-user `0o600` token file so the squatter can't just read it), but it is real on multi-user machines, and "a reusable secret transmitted in the clear" is the wrong shape for something now gating PII.

## Decision

Replace plaintext-token-per-frame auth with a **mutual HMAC-SHA256 challenge-response handshake (protocol v2)**. The pairing token becomes **only an HMAC key and is never transmitted**.

**Handshake** (v2 envelope is `{type, reqId, payload}` — no `token` field):

1. Extension → `hello { protocol: 2, clientNonce }`
2. Desktop → `challenge { serverNonce }`
3. Extension → `auth { proof }`, `proof = HMAC-SHA256(key=token, "ajh-bridge/v2\n" + "client" + "\n" + serverNonceHex + "\n" + clientNonceHex)`
4. Desktop verifies **constant-time**; on failure it closes **with no reply** (no oracle). On success → `auth.ok { serverProof }` with role `"server"`.
5. Extension verifies `serverProof` constant-time. **This is the mutual half** — if the server cannot prove it knows the token (a port-squatter), the extension aborts and sends **zero PII**. One-way auth would let a rogue server harvest the profile, so mutual is required.
6. After the handshake the socket is an **authenticated session**: `import.request`/`profile.get` carry **no token** and are authorized by the verified connection, not a per-frame secret. The extension sends application frames **only** in the `connected` phase.

**Invariants:** token never on the wire; HMAC over a domain-separated (`role`) canonical message so the client/server proofs are independent (no reflection); fresh CSPRNG nonces (≥16 bytes) per connection (no replay); constant-time verification both sides (Rust `Mac::verify_slice`, TS non-short-circuiting compare); the profile still honors the autofill opt-in gate desktop-side.

**Rollout: force cutover (hard, no dual-support on the desktop).** The desktop **rejects** any legacy plaintext-token frame with an `update.required` reply and close. A new `outdated` connection phase surfaces the version mismatch on both sides ("update the desktop app" / "update the extension"). A **cross-implementation known-answer test vector** pins the Rust (`hmac` crate) and TS (Web Crypto) HMAC canonicalizations byte-for-byte so they can never drift silently.

## Considered options

1. **Mutual HMAC challenge-response, force cutover (chosen).** Token never transmitted; a squatter learns no reusable secret and cannot impersonate the app to obtain PII. Simplest desktop code (one path). Cost: the hard cutover breaks the bridge for any not-yet-updated install until both sides update, and needs clear "update required" UX on both ends.
2. **Dual-support (accept v2 handshake AND legacy plaintext token), deprecate later.** No user breakage during transition, but keeps the plaintext path — and thus the vulnerability — alive until old installs age out, and doubles the auth code + its attack surface. Rejected: the owner chose the stronger immediate posture.
3. **One-way auth (only the client proves knowledge).** Stops token-harvest but NOT a rogue server harvesting the profile (the client would still send PII to a squatter it can't distinguish from the app). Rejected: fails the actual goal (protecting PII), not just the token.
4. **Per-frame MAC instead of session auth.** Re-authenticate every frame with an HMAC. Rejected: unnecessary for a fixed loopback socket whose identity is stable once the handshake verifies it; session auth is simpler with equal security.
5. **OS-level port protection only** (e.g. exclusive bind). Rejected: can't prevent a squatter that binds _before_ the app starts, and doesn't address the plaintext-token-at-rest-in-transit shape.

## Consequences

- **Hard cutover has a transition window.** Until a user updates _both_ the desktop app and the extension, the bridge shows an `outdated`/update prompt and won't connect. The already-published old extension can't be given nicer UX; the new extension gets the clean `outdated` view. Releases should be sequenced with this in mind.
- **`bad_token` now means an unambiguous rogue/mismatched server** (a failed `serverProof`), not "a wrong token inferred from a close." Because the desktop rejects a bad proof by closing with no reply (no oracle), an ambiguous silent close is treated as recoverable `app_not_running`, never as a false token accusation. The `Connection phase` and `Pairing token` glossary terms are updated accordingly.
- **A second `hmac` crate version** (`0.12`, RustCrypto, digest-0.10, paired with the existing `sha2 0.10`) enters the tree alongside the pre-existing Unix-only `hmac 0.13`; tolerated by `deny.toml`'s `multiple-versions = "warn"`, both MIT/Apache and advisory-clean.
- **The pairing UX is unchanged** — the token is still generated on first run, shown in Settings, pasted into the extension, and rotatable; only its _use_ changed (HMAC key, not a transmitted secret).
- **Native-messaging and WS transports** both carry the handshake identically (the native host is a 1:1 relay).

## References

- Protocol: `packages/shared/src/ipc/extension-protocol-constants.ts` + `extension-protocol.ts` (`hello`/`challenge`/`auth`/`auth.ok`/`update.required`, `HANDSHAKE_TEST_VECTOR`, `EXTENSION_PROTOCOL_VERSION`).
- Desktop: `apps/desktop/src-tauri/src/extension_bridge/handshake.rs` (HMAC core, constant-time verify, nonces, KAT), `mod.rs` (`ConnState` state machine).
- Extension: `apps/extension/src/lib/handshake.ts` (Web-Crypto proof, constant-time hex compare), `bridge.ts` (`performHandshake`, `connected`-gated send path, `outdated` phase).
- Related: [ADR 0009](0009-assisted-autofill.md) (the autofill PII path this hardens); `Pairing token` / `Connection phase` in `docs/CONTEXT.md`; `apps/extension/README.md` threat model.
