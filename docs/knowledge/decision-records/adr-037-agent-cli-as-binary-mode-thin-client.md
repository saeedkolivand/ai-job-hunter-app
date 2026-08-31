# ADR-037 — Agent CLI is a mode of the shipped binary and a thin client over the loopback bridge

**Status:** Accepted

**Date:** 2026-08-31

**Deciders:** repo owner, main session

## Context

Issue #1084 asked for a localhost-only, token-gated read-only HTTP API plus an `openapi.json`, because
a Tauri desktop app is a native window an AI agent cannot read or drive — agents have browser
automation, not window automation.

An HTTP listener would have added a port, a CORS policy, an origin allowlist and a token file on disk,
and every one of those is a surface to secure. An agent that can run shell commands needs none of them.

Two independent questions then had to be answered: **where the CLI lives** (a second binary, or a mode
of the existing one) and **where it gets data** (the app's stores, or the running app).

## Decision

**A mode of the existing `ajh-tauri` binary**, selected by an argv sentinel in `main.rs`, and a **thin
client over the extension bridge** that already exists. The app must be running.

1. **Not a second `[[bin]]`.** The release upload globs read only `target/release/bundle/**`
   (`.github/workflows/release.yml`), so a second binary would ship to nobody. The exe is already
   installed and already registered in the native-messaging manifests.

2. **The sentinel's position is load-bearing in both directions.** It sits BELOW the native-host
   short-circuit and ABOVE `ajh_tauri::run()`: `run()` forks the minidump supervisor as its first act,
   and the single-instance plugin would otherwise hand the CLI's argv to the running GUI, pop its
   window, and exit having printed nothing.

3. **A thin client, never a second reader of the stores.** Query logic stays in one place, and the app
   writes a pointer file (`exePath` from `current_exe()`, `dataDir`) on every launch, on
   `register_native_host`'s existing best-effort lifecycle — the binary is not on `PATH` on Windows,
   macOS or Linux AppImage.

4. **Every payload of these five Resources is an allowlist projection**, nested types included, and
   third-party scraped text goes through `prompt_fence`
   ([ADR-010](adr-010-untrusted-input-fencing.md)). This guarantee is scoped to the **curated tier**:
   [ADR-038](adr-038-agent-cli-full-parity-two-tier.md) later adds a generic tier that deliberately
   returns records raw, and fencing is the only half that carries over to it.

## Consequences

### Positive

- **Zero distribution work.** No bundler, installer, updater or CI change: it _is_ the binary every
  installer already contains.
- **No new attack surface.** No port, listener, credential or capability entry; it reuses the bridge's
  v2 mutual HMAC handshake ([ADR-0010](0010-bridge-hmac-handshake.md)).
- **Leak-resistant by construction.** A projection type cannot express the forbidden fields, so a new
  PII field cannot ride out on an existing resource.

### Tradeoffs

- **The app must be running.** With it closed the CLI exits non-zero with a parseable error. This was
  chosen deliberately over reading the stores.
- **Windows has no console in release.** `windows_subsystem = "windows"` with `panic = "abort"` means an
  interactive run has a NULL stdout handle and `println!` aborts silently, so the CLI probes
  `GetStdHandle` first and attaches only when there is nothing inherited.

### Why reading the stores directly was rejected

It is not read-only, on evidence rather than principle. `ApplicationStore::open` unconditionally runs
`link_orphaned_generations` + `backfill_from_generations`, which open `ai_generations.db` read-write,
run `ALTER TABLE … ADD COLUMN`, and **create `Application` rows**. Worse, `run_migrations` reads
`user_version` outside any transaction, so two processes at the same version both apply migration N+1;
the loser gets "duplicate column name", and if the loser is the app, `lib.rs` swallows it as non-fatal
and boots with **no `ApplicationStore`** — the user's entire application tracker silently reads empty.

The data directory also holds ~15 separate SQLite files plus JSON stores, so a second reader would
reimplement the store layer, and `AJH_DATA_DIR` never escapes the app process anyway.
