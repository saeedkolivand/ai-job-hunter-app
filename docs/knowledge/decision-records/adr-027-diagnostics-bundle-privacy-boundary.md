# ADR-027: Diagnostics-bundle privacy boundary

Last updated: 2026-06-30

**Status:** Accepted

## Context

The diagnostics bundle is written to a user-chosen path and then attached to **public** GitHub
issues as a support artifact. The app-data directory (`app.path().app_data_dir()`) also holds the
SQLite store, résumé / job PII, vector databases, AI provider credentials (via the OS keychain),
and the user's contact profile. Naively zipping the data directory would expose all of that to the
public issue tracker.

A secondary risk: structured log lines and crash-report tokens can carry email addresses (from
contact-profile imports or apply-email generation) and JSON-shaped secrets
(`"api_key":"sk-…"`) that a naive assignment-only redactor (`key=` pattern) does not catch —
because after whitespace-token splitting and brace/quote trimming, the JSON identifier is
`key":` not `key=`.

## Decision

**1. Strict allowlist — never a denylist.**
`build_diagnostics_zip` in `apps/tauri/src-tauri/src/commands/support.rs` builds a fresh zip from
exactly three named entries via explicit `zip.start_file` calls:

- `system-info.txt` — generated at runtime (OS name, OS version, arch, app version, total RAM);
  contains no user PII and needs no redaction pass.
- `crashes.log` — read from `<data_dir>/crashes.log` if it exists as a plain file.
- `logs/<name>` — each plain file under `<data_dir>/logs/`.

No directory walk of the data dir occurs. Any future bundle entry requires an explicit
`zip.start_file` addition AND must be redaction-safe.

**2. Redaction pass on all text before zipping.**
Every line of `crashes.log` and the `logs/` files is tokenised on whitespace and each token is
passed through `redact_token` in `apps/tauri/src-tauri/src/autopilot_helpers/mod.rs`. That
function replaces sensitive tokens with neutral placeholders across five shape classes:

| Shape                                                        | Placeholder             |
| ------------------------------------------------------------ | ----------------------- |
| URL (`://`)                                                  | `<url-redacted>`        |
| Windows / Unix / home path                                   | `<path-redacted>`       |
| Bare IPv4 / host:port                                        | `<host-redacted>`       |
| Credential — assignment (`key=`) **or** JSON-field (`key":`) | `<credential-redacted>` |
| Email (`local@domain.tld`)                                   | `<email-redacted>`      |

The dual `=` / `":` check is deliberate: structured log lines such as `{"api_key":"sk-…"}` are
a single whitespace-delimited token; after brace/quote trimming the identifier is `api_key":`
not `api_key=`, so an assignment-only detector would miss it.

**3. Symlinks skipped.**
Both `crashes.log` and every `logs/` entry are examined with `symlink_metadata()` (which does
_not_ follow symlinks) before reading. A symlink pointing at the SQLite store, a résumé, or any
other sensitive file is skipped silently rather than read and zipped.

**4. Delivery via native save dialog + reveal.**
`support_export_diagnostics` (`commands/support.rs`) accepts a caller-supplied `dest` path
originating from the OS native Save dialog in the `ContactFeedback` renderer component. After a
successful write it emits a success response; the renderer then calls `revealItemInDir` to open
the file manager at the saved location.

## Alternatives rejected

- **Zip data dir with a denylist**: a new sensitive file (future credential cache, new vector DB
  shard) would leak into the public bundle by default. Allowlist-by-construction is the only safe
  posture when the output is a public artifact.
- **Ship logs raw (unredacted)**: violates the Path Privacy rule (no real paths in public
  artifacts) and exposes email addresses and structured-log credentials to any GitHub reader.

## Consequences

- Any file added to the bundle in future **must** be added via an explicit `zip.start_file` call
  in `build_diagnostics_zip` and must be verified as redaction-safe (run through `redact_token`
  or proven PII-free like `system-info.txt`).
- `redact_token` is the last line of defense before a public artifact. Extending it with a new
  PII shape or credential marker is a **security change** and requires a corresponding test in
  `autopilot_helpers/mod.rs` (cf. `json_embedded_email_is_redacted`,
  `json_password_field_is_redacted`).
- `system-info.txt` is generated (not read from disk) and contains only OS metadata; it is
  deliberately excluded from the redaction pass.
