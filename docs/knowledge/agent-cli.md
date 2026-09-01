# Agent CLI (`ajh-tauri agent <verb>`)

Last updated: 2026-09-01

A headless CLI mode of the shipped `ajh-tauri` binary, invoked alongside the running desktop app. Enables external programs (shell scripts, LLM agents, CI pipelines) to query job data, profile fields, and trigger commands without a GUI. The same binary, no separate install.

## Invocation

```bash
ajh-tauri agent <verb> [args]
```

Run `ajh-tauri agent --help` to see the full verb list, argument patterns, and exit codes. `--help` works even if the app is not running.

All verbs except `--help` require **the desktop app to be running**. Replies are always JSON on stdout, whether success or failure.

## Exit codes

Exit codes distinguish four outcomes (see `ajh-tauri agent --help` for the full contract):

- **0** — Success; `{"ok":true,...}` returned
- **1** — App refused the command (rate-limited, validation error, autofill off, etc.); error details in JSON on stdout
- **2** — Round trip failed (app not running, bad CLI usage, connection error) or protocol issue
- **4** — `call` verb only: an `Effect::Irreversible` command needs `--confirm '<value>'`; reply's `detail` field names which **other read command** to call first for the proof value (ADR-038 §4)

The `error` field in exit-code-2 replies carries a fixed sentinel (not a path, URL, or echoed input). A `detail` field may also be present with additional context (e.g., the specific validation error). Full sentinel list in `ajh-tauri agent --help`.

## Binary locations (v0.145.0+)

| Platform                     | Path                                                                                      | On `PATH` by default?            |
| ---------------------------- | ----------------------------------------------------------------------------------------- | -------------------------------- |
| **Linux** (deb/rpm)          | `/usr/bin/ajh-tauri`                                                                      | Yes                              |
| **macOS Homebrew**           | Symlinked via `brew install --cask ai-job-hunter`                                         | Yes                              |
| **macOS dmg drag-install**   | `/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri`                                | No; add to `$PATH` if needed     |
| **Windows** (nsis, per-user) | User's local install directory (use `~/.ajh-agent/agent.json` to locate programmatically) | Not on PATH; invoke by full path |

## Discovery

A program can locate the app's binary and data directory via the pointer file:

```json
~/.ajh-agent/agent.json
{ "exePath": "/path/to/ajh-tauri", "dataDir": "/path/to/app-data" }
```

Written by the app on every launch (idempotent). This is the supported mechanism for automated discovery.

## Design & constraints

- **Mode, not binary**: The CLI is an argv mode of the shipped `ajh-tauri` executable, detected by the first post-binary token being exactly `agent`. It short-circuits before the GUI or single-instance plugin, and exits cleanly.
- **Authentication**: Uses the same loopback WebSocket bridge as the browser extension, with mutual HMAC challenge-response. The pairing token is used only as an HMAC key and is never sent on the wire; both clients reuse the same OS-stored credential.
- **Policy table**: The `call` verb is a generic tier that respects per-command `Effect` classification (Read, Reversible, Irreversible). Irreversible commands require a `--confirm` proof value read from a separate command first (ADR-038 §4). The curated verbs are a separate, simpler tier that predates it.
- **Timeout discipline**: Per-step budgets (handshake, query) + an outer invocation deadline prevent hung/squatting ports from stalling the entire call.
- **Privacy**: The `error` field is always a fixed sentinel from a closed set; paths and pairing tokens never appear in any reply. A `detail` field may be present with human-readable context (e.g., validation errors). Safe for use in LLM agent transcripts.

## See also

- [ADR-037](decision-records/adr-037-agent-cli-as-binary-mode-thin-client.md) — Design rationale (binary mode, loopback bridge, authentication)
- [ADR-038](decision-records/adr-038-agent-cli-full-parity-two-tier.md) — Full policy table and command-dispatch tiers (curated vs. generic)
- `apps/desktop/src-tauri/src/extension_bridge/agent_cli.rs` — Client implementation (verb parsing, exit codes, error sentinels)
- `apps/desktop/src-tauri/src/extension_bridge/agent_call.rs` — Server-side dispatch and policy lookup
