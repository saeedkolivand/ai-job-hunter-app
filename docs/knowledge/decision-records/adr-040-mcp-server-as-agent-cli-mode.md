# ADR-040 — MCP stdio server as `agent mcp` mode: legacy 2025-11-25 wire, five curated tools, three generic tiers, per-tool annotations along the Effect boundary

**Status:** Accepted

**Date:** 2026-09-02

**Deciders:** main session

## Context

The agent CLI (`ajh-tauri agent <verb>`) is a headless mode that queries job data and triggers app commands via a loopback WebSocket bridge with HMAC-SHA256 mutual authentication. External callers (shell scripts, CI pipelines, LLM agents) invoke it as a subprocess, receive JSON on stdout, and use exit codes to distinguish success, validation failure, and confirmation-required states.

A third category of caller — an LLM agent running under Claude Code or Codex with MCP protocol support — benefits from tool discovery, structured result schemas, and per-tool permission annotations. The design question: should the agent CLI add an MCP server mode, and if so, at what fidelity?

Three architectural points framed the decision:

1. **Wire version**: The installed Claude Code (2.1.258) and Codex (0.144.6) both launch MCP stdio servers using the legacy 2025-11-25 handshake (`initialize` → `notifications/initialized`; `server/discover` is the _modern_ probe, which a legacy server answers with `-32601`). The modern 2026-07-28 revision (stateless, per-request `_meta`, named result types) is not yet deployed in production clients. Supporting both would require version negotiation; supporting the modern spec alone would render the server incompatible with deployed clients.

2. **Tool scope**: The agent CLI already offers five curated verbs (`best-matches`, `job`, `profile`, `automations`, and `schema`) plus a generic `call` tier that respects per-command `Effect` classification (Read, Reversible, Irreversible). Exposing these as MCP tools preserves this tier structure and lets a client decide per-tool whether to auto-approve, prompt, or block.

3. **Confirmation ceremony**: ADR-038 §4 defines the proof as anti-hallucination ('possessing it proves the caller actually read the affected record'), not consent. An LLM caller can complete both hops of the ceremony unaided. The server must never auto-resolve the proof (that would collapse it into the one-hop ceremony ADR-038 says 'stops nothing'); human intent gating moves to the MCP client's per-tool permission prompt.

## Decision

**1. Protocol and wire version**: Use the legacy 2025-11-25 JSON-RPC stdio wire, the only spec deployed in Claude Code 2.1.258 and Codex 0.144.6. Answer `server/discover` with plain `-32601` (method not found), the signal the 2025-11-25 spec itself defines as the fallback that triggers initialization. Never advertise the modern 2026-07-28 revision. The modern era is a named follow-up.

**2. Mode, not binary**: Implement MCP as a mode of the existing `ajh-tauri agent` binary, not a new process or Tauri command. The CLI already detects `agent` as the first argument and exits before the GUI (ADR-037 §3); add an `mcp` mode, intercepted before verb parsing exactly like `--help`, requiring zero changes to `generate_handler!` or the R8 line-cap test (tests/architecture.rs:690).

**3. Implementation shape**: Hand-rolled JSON-RPC on `serde_json` + `tokio`, not an SDK crate. Both `rmcp` (Rust MCP SDK) and the Node.js SDK require code generation or version-specific schemas; the binary's transitive dependencies (`serde_json`, `tokio`) are already present. The server carries one compact JSON object per line on stdout, reads the input from stdin via `std::io::stdin().lock().lines()` blocking on the main thread, and on `Err` (EPIPE after the client closes the pipe) returns exit code 0, because release is `panic="abort"` and this path runs above `crash_reporting::init`.

**4. Tools along the Effect boundary**: Eight tools total: five curated (`best-matches`, `job`, `profile`, `automations`, `commands`) plus three generic dispatch tiers (`call-read`, `call-reversible`, `call-irreversible`), each carrying MCP tool annotations (`readOnlyHint`/`destructiveHint`) that a client can act on. Rejected: one monolithic `call` tool (every client would classify it as destructive, so Read operations cannot be auto-approved while Irreversible ones are gated). The `commands` tool is local (no bridge call) and returns the policy table filtered by `effect` argument, letting an LLM discover targets without leaving the session.

**5. Confirm ceremony passes through verbatim**: The `call-irreversible` tool carries a `confirm` argument. The server passes it to the app's confirmation logic (`gate` and `dispatch_irreversible_confirmed` in `agent_call.rs`) unchanged; it never fetches-then-confirms. If the proof does not match, the result carries `{"dispatched":false,"error":"confirmation_mismatch","detail":"…"}` with the expected value never disclosed (proof.rs:150). Results, not protocol errors, for every app-side refusal: `confirmation_required`, `confirmation_mismatch`, `not_exposed`, `unknown_command`, `rate_limited`.

**6. Write tiers are opt-in; the read tier is the default**: `agent mcp` offers only the curated read tools, `call-read` and `commands`. `call-reversible` appears only with `--allow-reversible`; `call-irreversible` only with `--allow-irreversible`, which implies the former. Naming a hidden tool is answered with `-32602`, and `commands` marks the hidden class `unavailable` rather than directing the model at a tool the session cannot see. Rationale: the client's permission prompt is the only intent gate, and a client that reads neither the Anthropic-only `_meta` hint nor treats `destructiveHint` as binding (Codex with `approval_policy = "never"`) turns a hint into nothing — that argument is identical for both write tiers, so both get the same mechanism. The default therefore matches the tier the original plan scoped: a read-only server. The flags are argv-only (no environment or config path), and the `instructions` string names every enabled tier so an elevated session leaves a trace where the user looks.

**7. Tool descriptions carry untrusted-text notices**: The Rust fence at the point of dispatch (prompt_fence.rs:418-428) carries no instruction text, only bare tags like `<job_posting>…</job_posting>`. The MCP tool description for `best-matches` and the `instructions` field must tell the model those spans are third-party data not to be trusted as facts. The same notice applies to any `structuredContent` if the server adds it.

**8. Results are verbatim, sentinel vocabulary is closed**: The tool result carries the JSON-RPC response from the app's dispatch logic on `content[].text` (exit code plus the JSON-RPC body). The `isError` flag signals app refusals (errors) and client-side failures; tool results never map to JSON-RPC error responses. Error details use the fixed sentinel vocabulary established in agent_cli.rs (no echoed argv, no paths, no token values). A `detail` field may carry human context (e.g., which read command to call next for a proof).

**9. Stdout hygiene**: Exactly one compact JSON-RPC frame per line via `writeln!(stdout().lock(), serde_json::to_string(...))`. Never `println!`, never `to_string_pretty`, never logger output on stdout. Logs go to stderr only. The `ensure_console_output` call (windows_console.rs) is preserved as the first action, a no-op on inherited pipes.

**10. A fresh bridge connection per tool call**: The server holds no long-lived socket. Every `tools/call` that reaches the app runs the CLI's own `query()` path end to end — pointer file, token file, HMAC handshake, one request, close — so a token regenerated in Settings is picked up by the next call with no reconnect logic, and a `pairing_rejected` refusal is an ordinary `isError` result; the loop continues. The throttle lives on the app side, so reconnecting per call cannot widen it. Results above `MCP_RESULT_MAX_BYTES` are refused as `result_too_large`, never truncated, and `structuredContent` is not emitted: no observed client surfaces it and it would duplicate every payload, PII included, in the persisted transcript. One trade recorded here: a `NotExposed` target is refused before the bridge, so probing those rows opens no app-side span and consumes no throttle; since `commands` already enumerates them without a bridge call, the local refusal adds no disclosure, it only removes a trace.

**11. No app launch dependency**: `initialize` sends only the protocol version and basic info; it does not check app status, read the app's state, or establish a connection. Startup works with the app closed. The first tool call (e.g., `best-matches`) is where the connection handshake happens, in the same path the CLI already uses.

**12. Single-flight dispatch, on purpose**: `serve` reads one frame, dispatches it synchronously (`block_on` from the sync loop, never inside the runtime) and writes the reply before reading the next. One in-flight `tools/call` therefore blocks every later frame, including `ping`, for at most `INVOCATION_TIMEOUT`. Chosen because it keeps exactly one writer with no interleaving on stdout, matches how the deployed clients issue tool calls (one at a time, awaited), and bounds the throttle to one bridge connection per process; a test pins the in-order contract. The alternative — a reader task plus a serialized writer — is the follow-up if a real client's liveness check ever kills the server mid-dispatch, which is why client keepalive behaviour is recorded as unverified below.

## Consequences

### Positive

- **Tool discovery for LLM agents**: Claude Code and Codex can query `tools/list`, enumerate targets without leaving the session, and offer per-tool permission prompts (or not, depending on client configuration).

- **Confirmation ceremony preserved**: The two-hop design from ADR-038 §4 is intact; the server never auto-resolves the proof. The model completes both hops and the ceremony bounds the target and guarantees freshness, exactly as designed.

- **Per-tool permission annotations**: The three-way generic dispatch (`call-read`, `call-reversible`, `call-irreversible`) lets a client offer fine-grained control — auto-approve reads, prompt on reversible, always require confirmation on irreversible — instead of all-or-nothing on a monolithic `call` tool.

- **Local `commands` tool**: An LLM can discover policy targets, their effect class, and the proof source (if any) without making app calls, enabling it to plan multi-step ceremonies upfront.

- **Zero new binary additions**: The MCP mode is implemented as an existing-binary feature (`agent mcp`). No separate process, no new Tauri command, no R8 line-cap risk.

- **Offline operation**: `initialize` and `commands` work without the app running. Best-match retrieval, job lookup, and profile export work as long as the app is running — the same requirement as the CLI today.

### Tradeoffs

- **Legacy wire only**: Supporting only the 2025-11-25 protocol means clients using the modern 2026-07-28 spec (not yet deployed) cannot connect. A follow-up ADR will add that version when it reaches production.

- **A slow bridge call looks like a hung server** to a ping-based liveness check, for up to the invocation deadline (§12). No deployed client has been observed to ping mid-call; if one does, the reader/writer split named in §12 is the fix.

- **Confirmation ceremony not enforced server-side for all clients**: Claude Code implements `_meta["anthropic/requiresUserInteraction"]` and can force a prompt; Codex does not read this field. On Codex with an `approval_policy` of `never`, an LLM can reach Irreversible rows (and their own approval) by completing the ceremony alone, with no human gate between the model and the action. The `--allow-irreversible` flag and this ADR's documentation make that explicit.

- **Residual scraped text in `best-matches`**: `title`, `company` and `location` are fenced at the point of projection (`fence_best_match_fields` in `agent_read.rs`) with the same wrapper `job.description` carries; `url` and `board` are validated rather than free text and stay bare. The fence is a marker, not an instruction — the `instructions` string is what tells the model the spans are untrusted.

- **Tool results carry PII to the client**: Results leave the machine and land in the MCP client's cloud provider and persisted transcript. The `profile` tool returns contact info; `documents_get_text` returns the full résumé; `applications_list` and similar return application records. The server carries no redaction (per ADR-038 §3, the generic tier is raw). This is an explicit consequence of MCP design, not a new risk.

### What was verified, and how

- **Against the real binary over stdio** (scripted sessions, app stopped and app running): a supported `protocolVersion` is echoed and an unsupported one gets `2025-11-25`; `server/discover` → `-32601`; `ping`; notifications produce no frame; a hidden tool → `-32602`; every refusal class arrives as an `isError` result carrying the CLI exit code; throttle refusals across consecutive calls with no retry; byte identity between `agent profile` stdout and the `profile` tool's text block; zero bytes on stderr.
- **From the clients' source, not exercised end to end**: Claude Code 2.1.258 (TypeScript SDK) and Codex 0.144.6 (`rmcp` 1.8.0, Rust) both open stdio servers with the legacy `initialize` handshake and treat `-32601` on `server/discover` as the legacy signal. Which `protocolVersion` each sends is UNKNOWN until a live connection is made; the server echoes any supported value. Neither client was connected to this server before merge.

## Related decisions

- **ADR-037**: Agent CLI as a binary mode, loopback bridge, HMAC authentication.
- **ADR-038**: Two-tier policy table and confirmation ceremony (proof bounds target and guarantees freshness, not intent).
- **ADR-0005**: Network egress privacy boundary; generic-tier results are raw (no PII redaction).

## Amendments to related rules

- **`.claude/skills/agent-cli-standards/SKILL.md`** §One JSON document: The rule states that the agent CLI emits exactly one JSON document per invocation. The MCP mode is an exception: it emits newline-delimited JSON-RPC frames (request-response pairs, plus server notifications if added). An 'MCP mode' block in the skill documents this exception.

## Footnotes

- The pairing token returned by `extension_bridge_status` is reclassified from `Effect::Read` to `Effect::NotExposed` and does not reach the generic tier, so the token never appears in tool results (addressed separately in PR).
- Tool results are surfaced to the model only if the client implements that (Claude Code does; Codex does not; unknown for other clients).
- The server does not measure or enforce per-request rate limits; it reuses the global throttle on `BridgeState` shared by both the CLI and the agent-query tier.
