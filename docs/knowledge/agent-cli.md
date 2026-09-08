# Agent CLI (`ajh-tauri agent <verb>`)

Last updated: 2026-09-08

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
- **2** — No result was delivered: the round trip failed (app not running, bad CLI usage, connection error, protocol issue) or the app refused/discarded the reply. The `error` field names which; a `result_too_large` refusal means the command itself may already have run
- **4** — `call` verb only: an `Effect::Irreversible` command needs `--confirm '<value>'`; reply's `detail` field names which **other read command** to call first for the proof value (ADR-038 §4)

The `error` field in exit-code-2 replies carries a fixed sentinel (not a path, URL, or echoed input). A `detail` field may also be present with additional context (e.g., the specific validation error). `--help` lists the sentinels the CLI synthesizes for itself; the app-side refusal names that reach the same field are the variants of `Refusal` in `agent_call.rs`.

## Binary locations

The CLI is the shipped `ajh-tauri` executable itself, so where it lands — and whether anything puts it on `PATH` — is decided by each platform's packaging, not by this page. Never hardcode a path or assume `PATH`: resolve it at runtime through the pointer file (**Discovery**, below), or read it out of the app at **Settings → Developer**.

Where each platform's truth lives — read the source, it moves with packaging:

- **Windows (NSIS, per-user)** — the installer's PATH hook, `apps/desktop/src-tauri/windows/hooks.nsh`, wired via `bundle.windows.nsis.installerHooks` in `apps/desktop/src-tauri/tauri.conf.json`.
- **Windows (Microsoft Store, MSIX)** — the `windows.appExecutionAlias` extension in `apps/desktop/src-tauri/windows/msix/AppxManifest.xml`. A packaged app cannot edit `PATH`, so the alias is what makes the same command name resolve. Which path the app then publishes for itself is `platform::config::agent_cli_exe_path`, which delegates to `platform::msix::published_exe_path` — including its "publish nothing" answer when a packaged build has no usable alias (the user can switch one off): no pointer file is written at all then, rather than one naming a path nothing can launch.
- **macOS** — the Homebrew cask's `binary` stanza (`Casks/ai-job-hunter.rb`). That stanza _is_ the linking step, so whether an install is reachable by name follows from whether it went through the cask.
- **Linux (deb/rpm)** — Tauri's own bundle layout for the `deb`/`rpm` entries in `bundle.targets` (`apps/desktop/src-tauri/tauri.conf.json`); the repo adds no override.
- **Linux (AppImage)** — nothing is installed; the durable path is the `.AppImage` file the user launched, identified by the `launched_appimage` predicate in `apps/desktop/src-tauri/src/platform/config.rs` — never the transient mount path a process inside the image reports for itself.

## Discovery

A program can locate the app's binary and data directory via the pointer file:

```json
{ "exePath": "…/ajh-tauri", "dataDir": "…/app-data" }
```

Its location (directory and filename both) is owned by `platform::config::agent_pointer_path`; `extension_bridge::register` rewrites it on each launch (idempotent) **only when both that path and the published exe path resolve**. Otherwise the write is skipped, including the MSIX no-alias case above, so a missing pointer file is a normal state to handle rather than an error; the skip conditions are the early returns on `write_agent_pointer` and its caller `register_native_host`, and they are what to read. This is the supported mechanism for automated discovery: resolve the path through that function rather than hardcoding one.

The `exePath` it publishes is resolved by `platform::config::agent_cli_exe_path` (see [ADR-037](decision-records/adr-037-agent-cli-as-binary-mode-thin-client.md)'s amendment for the AppImage case). For a human rather than a program, the same value is shown in the app at **Settings → Developer**, together with ready-to-copy registration snippets (Claude Code, Codex, and a generic `mcpServers` block for any other client) that already carry the path and the chosen access tier. That card is backed by `commands::system::system_agent_cli_info`, which is renderer-only: it is classified `NotExposed` in `extension_bridge/agent_cli/policy.rs`, so the agent tier cannot call it and an agent discovers the path through the pointer file instead. Card, command and pointer file all read the one resolver, so they cannot disagree.

## Design & constraints

- **Mode, not binary**: The CLI is an argv mode of the shipped `ajh-tauri` executable, detected by the first post-binary token being exactly `agent`. It short-circuits before the GUI or single-instance plugin, and exits cleanly.
- **Authentication**: Uses the same loopback WebSocket bridge as the browser extension, with mutual HMAC challenge-response. The pairing token is used only as an HMAC key and is never sent on the wire; both clients reuse the same OS-stored credential.
- **Policy table**: The `call` verb is a generic tier that respects per-command `Effect` classification (Read, Reversible, Irreversible). Irreversible commands require a `--confirm` proof value read from a separate command first (ADR-038 §4). The curated verbs are a separate, simpler tier that predates it.
- **Frame-cap refusal**: a generic-tier reply too large for the bridge's own WS frame ceiling (`MAX_FRAME_BYTES` in `extension_bridge/mod.rs`) is refused as `result_too_large` rather than written and dropped — a deterministic refusal instead of the `connection_lost` an over-cap frame used to produce, which reads as transient and invites a retry that can never succeed. The dispatcher measures against that same constant instead of defining a second one; the refusal variant and its sentinel live in `agent_call.rs`, with the reasoning on the variant. The MCP mode's own result cap is a separate, smaller constant one hop further out (MCP section below).
- **Paging on the generic tier**: a list command that takes no argument able to narrow its result returns one page — `limit`/`cursor` in, an `items`/`total`/`nextCursor` envelope out — bounded by an audited page-size constant beside the generic tier's reshaping (`agent_call/reshape.rs`). Which rows page, and why that size, live on the constant; a caller discovers it per row from the `commands` tool's returns note, never from a list on this page. The limit-clamp and byte-budget primitives are shared with the curated `found-jobs` resource (`extension_bridge/paging.rs`); the offset-cursor parse there is this tier's alone, because `found-jobs` layers its own issuer-scoped cursor grammar on top (MCP section below).
- **Binary payloads**: where a command's value is bytes rather than JSON text, the agent layer base64-encodes it and states so in an explicit encoding marker on the reply, so a caller decodes on the marker rather than on a guess. The marker's key and the rows it applies to are named beside the same constants in `agent_call/reshape.rs`.
- **Partial preference writes**: `job_preferences_set` merges its body over the stored row rather than replacing it — a key the body omits keeps the stored value, a key present as JSON `null` clears it. The merge sits on the shared Tauri command every caller reaches, the renderer included, not on an agent-only wrapper. The rule, and why it is load-bearing, live on `commands::job_preferences::set_job_preferences`.
- **Timeout discipline**: Per-step budgets (handshake, query) + an outer invocation deadline prevent hung/squatting ports from stalling the entire call.
- **Privacy**: The `error` field of a generic-tier reply is a fixed sentinel from a closed set (the curated tier's throttle refusal is the one prose exception, and predates the MCP mode); neither paths nor pairing tokens appear in any reply. A `detail` field may carry human-readable context. That guarantee covers the _envelope_ only: the `data` of a successful reply is the command's real output and can carry personal data (see the MCP section).

## MCP mode (`agent mcp`)

The agent CLI can run as an MCP (Model Context Protocol) stdio server, exposing the same job queries and command dispatch as discoverable tools for any MCP client (the stdio wire itself is pinned by `tests/mcp_smoke.rs`; Claude Code and Codex are the two the README documents commands for). The Settings → Developer card now generates a generic `mcpServers` JSON block alongside the Claude Code/Codex commands — see the README's "Any MCP client" registration block and `buildGenericMcpSnippet` in `apps/desktop/src/renderer/features/settings/lib/agent-cli-snippets.ts`. It is a mode of the binary, not a verb: `mcp` is intercepted before verb parsing, adds no verb-table row, no Tauri command and no policy row, and reuses the CLI's own bridge path (`query()` in `agent_cli.rs`) for every call — same handshake, same throttle, same sentinels.

**Shape, and where the exact values live** (`apps/desktop/src-tauri/src/extension_bridge/agent_cli/mcp.rs` for the protocol half, `mcp/schemas.rs` for the tool catalogue and `mcp/instructions.rs` for the startup prose, tests beside them):

- **Wire**: the legacy stdio JSON-RPC dialect the installed clients actually send; the accepted versions are `SUPPORTED_VERSIONS`, an unknown request method gets method-not-found, and a notification gets no frame. One compact JSON object per line. `initialize` is static, so startup never depends on the app.
- **Tiers**: read-only by default. Two argv-only launch flags open the write tiers along the `Effect` boundary and resolve into `Tier` (`Tier::from_flags`; the destructive flag implies the reversible one). `parse_launch_args` and its tests pin the flag spellings; `tools()` is the authoritative tool list with descriptions and input schemas. A hidden tier is absent from `tools/list`, naming it is a protocol error, and the `commands` discovery tool marks that class unavailable. `build_instructions` names every enabled tier so an elevated session leaves a trace in the transcript.
- **Refusals are results**: every app-side refusal, the CLI exit code and the confirm hint travel as `isError` text; the server's own sentinels are enumerated by a drift test in `mcp/tests.rs`. A result over the byte cap constant in `mcp.rs` is refused, never truncated. `structuredContent` is not emitted. The startup `instructions` name the error sentinels a client can receive, derived from `ERROR_SENTINELS` in `agent_cli.rs` (the same table behind `agent --help`) instead of a hand-kept second list — `EXPLAINED_IN_PROSE` in `mcp/instructions.rs` is the hand-written skip list that decides which rows the base prose already covers.
- **Confirmation ceremony**: unchanged from ADR-038 §4 — read the proof with `call-read`, pass it back verbatim on `call-irreversible`; the server never resolves a proof itself.
- **Generic-tier `input`**: `call-*` hands `input` to the Tauri handler as its argument map, so it is keyed by that handler's own parameter names exactly as the app's UI sends them — a handler taking one object parameter needs the payload nested under that name, not sent bare. The body fields are the generated request types in `apps/desktop/src-tauri/src/ipc_contracts/` (emitted from the Zod schemas by `pnpm gen:ipc`), but the parameter name exists only in the handler signature under `commands/`; when it is wrong the reply is the `invoke_error` refusal, whose `detail` names the missing key — the retry hint the `input` schema description (`mcp/schemas.rs`) and `instructions` (`mcp/instructions.rs`) both give the model.
- **Throttle and deadline**: the CLI's per-invocation deadline and the global throttle beside `BridgeState` in `agent_read.rs` apply unchanged. Bridge-backed calls are single-flight in input order (exactly one dispatch in flight at any instant, queued calls run strictly FIFO), so the throttle bound remains one bridge connection per process. Local tools and protocol methods answer immediately (ADR-040 §12).
- **Personal data**: any `Read` row that returns user data reaches the client's AI provider and its persisted transcript — the `commands` tool and `POLICY` in `policy.rs` are the list. Third-party posting text is fenced per surface (`fence_description` and `fence_posting_display_fields` in `agent_read.rs`, `fence_found_jobs_description` in `agent_read/found_jobs.rs`; `fence_scraped_fields` on the generic tier) and the server's `instructions` say the spans are untrusted.
- **`found-jobs`** (issue #1115): the one curated resource that paginates rather than returning everything at once — `autopilot_get`/`autopilot_list`/`autopilot_best_matches` cannot enumerate one autopilot's complete `found_jobs` list (unbounded record vs. a cross-autopilot top-N ranking). The cursor is an opaque token bound to the autopilot it was issued for — shape and rejection rules live on `agent_read::found_jobs::parse_found_jobs_cursor`; the page-size derivation and the projected field list live as doc comments on `agent_read::found_jobs::MAX_FOUND_JOBS_LIMIT`/`FoundJobSlice` (its own module, split out under the R8 LOC cap), never restated here.

**See also**: [ADR-040](decision-records/adr-040-mcp-server-as-agent-cli-mode.md) for the decisions and their reasoning.

## See also

- [ADR-037](decision-records/adr-037-agent-cli-as-binary-mode-thin-client.md) — Design rationale (binary mode, loopback bridge, authentication)
- [ADR-038](decision-records/adr-038-agent-cli-full-parity-two-tier.md) — Confirmation ceremony and policy table (curated vs. generic tiers)
- [ADR-040](decision-records/adr-040-mcp-server-as-agent-cli-mode.md) — MCP stdio server mode
- `apps/desktop/src-tauri/src/extension_bridge/agent_cli.rs` — Client implementation (verb parsing, exit codes, error sentinels)
- `apps/desktop/src-tauri/src/extension_bridge/agent_call.rs` — Server-side dispatch and policy lookup
