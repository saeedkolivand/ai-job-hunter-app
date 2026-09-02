# Agent CLI (`ajh-tauri agent <verb>`)

Last updated: 2026-09-02

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

| Platform                     | Path                                                                                      | On `PATH` by default?                                                              |
| ---------------------------- | ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| **Linux** (deb/rpm)          | `/usr/bin/ajh-tauri`                                                                      | Yes                                                                                |
| **macOS Homebrew**           | Symlinked via `brew install --cask ai-job-hunter`                                         | Yes                                                                                |
| **macOS dmg drag-install**   | `/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri`                                | No; add to `$PATH` if needed                                                       |
| **Windows** (nsis, per-user) | User's local install directory (use `~/.ajh-agent/agent.json` to locate programmatically) | Yes, from the release after v0.145.0 (NSIS hook); before that, invoke by full path |

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
- **Privacy**: The `error` field of a generic-tier reply is a fixed sentinel from a closed set (the curated tier's throttle refusal is the one prose exception, and predates the MCP mode); neither paths nor pairing tokens appear in any reply. A `detail` field may carry human-readable context. That guarantee covers the _envelope_ only: the `data` of a successful reply is the command's real output and can carry personal data (see the MCP section).

## MCP mode (`agent mcp`)

The agent CLI can run as an MCP (Model Context Protocol) stdio server, exposing job queries and command dispatch as discoverable tools for LLM agents such as Claude Code and Codex. `ajh-tauri agent mcp` starts a **read-only** server. Two argv-only flags open the write tiers along the `Effect` boundary: `--allow-reversible` adds undoable state changes, `--allow-irreversible` additionally adds destructive commands and AI spend (and implies the former). The flags are the only way to enable a tier — no environment variable or config path — and the server's `instructions` string names every enabled tier so an elevated session leaves a trace in the client transcript.

**Protocol**: Uses the legacy 2025-11-25 JSON-RPC stdio wire. Each request and response is one compact JSON object per line (never pretty-printed).

**Tools**: curated read-only tools plus one generic dispatch tool per `Effect` class; the authoritative list, descriptions and input schemas are `tools()` in `apps/desktop/src-tauri/src/extension_bridge/agent_cli/mcp.rs`. A tier hidden by the launch flags is absent from `tools/list`, naming it is a protocol error rather than a refusal, and the `commands` discovery tool marks that class `unavailable` instead of pointing at a tool the session cannot see.

**Confirmation ceremony**: Commands marked `Irreversible` require a two-hop ceremony (ADR-038 §4). The `call-read` tool reads the target value; the `call-irreversible` tool passes it back via the `confirm` argument. The proof is passed **verbatim**, including any `<job_posting>…</job_posting>` wrapper and embedded newlines; fenced values JSON-escape cleanly on the stdio line.

**Timeout and rate limits**: Each tool call runs under the CLI's own per-invocation deadline. The global throttle shared by the CLI and the agent-query tier applies unchanged (limits live beside `BridgeState` in `agent_read.rs`); refusals arrive as `rate_limited` tool results and the server never retries. A result larger than `MCP_RESULT_MAX_BYTES` (in `mcp.rs`) is refused as `result_too_large` rather than truncated, because a truncated JSON payload would be worse for the model than none.

**Tool results carry personal data**: The `profile` tool and `call-read` targets such as `documents_get_text`, `applications_list` and `email_watch_status` return PII; `best-matches` returns job postings. Results are sent to the MCP client's AI provider and persisted in its transcript. Use these tools only when the user explicitly asked for that data.

**See also**: [ADR-040](decision-records/adr-040-mcp-server-as-agent-cli-mode.md) for the complete design and wire contract.

## See also

- [ADR-037](decision-records/adr-037-agent-cli-as-binary-mode-thin-client.md) — Design rationale (binary mode, loopback bridge, authentication)
- [ADR-038](decision-records/adr-038-agent-cli-full-parity-two-tier.md) — Confirmation ceremony and policy table (curated vs. generic tiers)
- [ADR-040](decision-records/adr-040-mcp-server-as-agent-cli-mode.md) — MCP stdio server mode
- `apps/desktop/src-tauri/src/extension_bridge/agent_cli.rs` — Client implementation (verb parsing, exit codes, error sentinels)
- `apps/desktop/src-tauri/src/extension_bridge/agent_call.rs` — Server-side dispatch and policy lookup
