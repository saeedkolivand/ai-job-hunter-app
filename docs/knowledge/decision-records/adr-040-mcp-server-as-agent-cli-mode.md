# ADR-040 — MCP stdio server as `agent mcp` mode: legacy 2025-11-25 wire, five curated tools, three generic tiers, per-tool annotations along the Effect boundary

**Status:** Accepted

**Date:** 2026-09-02

**Deciders:** main session

## Context

The agent CLI (`ajh-tauri agent <verb>`) is a headless mode that queries job data and triggers app commands via a loopback WebSocket bridge with HMAC-SHA256 mutual authentication. External callers (shell scripts, CI pipelines, LLM agents) invoke it as a subprocess, receive JSON on stdout, and use exit codes to distinguish success, validation failure, and confirmation-required states.

A third category of caller — an LLM agent running under Claude Code or Codex with MCP protocol support — benefits from tool discovery, structured result schemas, and per-tool permission annotations. The design question: should the agent CLI add an MCP server mode, and if so, at what fidelity?

Three architectural points framed the decision:

1. **Wire version**: The installed Claude Code (2.1.258) and Codex (0.144.6) both launch MCP stdio servers using the legacy 2025-11-25 handshake (`initialize` → `server/discover` → `initialized`). The modern 2026-07-28 revision (stateless, per-request `_meta`, named result types) is not yet deployed in production clients. Supporting both would require version negotiation; supporting the modern spec alone would render the server incompatible with deployed clients.

2. **Tool scope**: The agent CLI already offers five curated verbs (`best-matches`, `job`, `profile`, `automations`, and `schema`) plus a generic `call` tier that respects per-command `Effect` classification (Read, Reversible, Irreversible). Exposing these as MCP tools preserves this tier structure and lets a client decide per-tool whether to auto-approve, prompt, or block.

3. **Confirmation ceremony**: ADR-038 §4 defines the proof as anti-hallucination ('possessing it proves the caller actually read the affected record'), not consent. An LLM caller can complete both hops of the ceremony unaided. The server must never auto-resolve the proof (that would collapse it into the one-hop ceremony ADR-038 says 'stops nothing'); human intent gating moves to the MCP client's per-tool permission prompt.

## Decision

**1. Protocol and wire version**: Use the legacy 2025-11-25 JSON-RPC stdio wire, the only spec deployed in Claude Code 2.1.258 and Codex 0.144.6. Answer `server/discover` with plain `-32601` (method not found), the signal the 2025-11-25 spec itself defines as the fallback that triggers initialization. Never advertise the modern 2026-07-28 revision. The modern era is a named follow-up.

**2. Mode, not binary**: Implement MCP as a mode of the existing `ajh-tauri agent` binary, not a new process or Tauri command. The CLI already detects `agent` as the first argument and exits before the GUI (ADR-037 §3); add an `mcp` verb that intercepts `parse_verb` before dispatch, identical to how `--help` does, requiring zero changes to `generate_handler!` or the R8 line-cap test (tests/architecture.rs:690).

**3. Implementation shape**: Hand-rolled JSON-RPC on `serde_json` + `tokio`, not an SDK crate. Both `rmcp` (Rust MCP SDK) and the Node.js SDK require code generation or version-specific schemas; the binary's transitive dependencies (`serde_json`, `tokio`) are already present. The server carries one compact JSON object per line on stdout, reads the input from stdin via `std::io::stdin().lock().lines()` blocking on the main thread, and on `Err` (EPIPE after the client closes the pipe) returns exit code 0, because release is `panic="abort"` and this path runs above `crash_reporting::init`.

**4. Tools along the Effect boundary**: Eight tools total: five curated (`best-matches`, `job`, `profile`, `automations`, `commands`) plus three generic dispatch tiers (`call-read`, `call-reversible`, `call-irreversible`), each carrying MCP tool annotations (`readOnlyHint`/`destructiveHint`) that a client can act on. Rejected: one monolithic `call` tool (every client would classify it as destructive, so Read operations cannot be auto-approved while Irreversible ones are gated). The `commands` tool is local (no bridge call) and returns the policy table filtered by `effect` argument, letting an LLM discover targets without leaving the session.

**5. Confirm ceremony passes through verbatim**: The `call-irreversible` tool carries a `confirm` argument. The server passes it to the app's confirmation logic (agent_call.rs:728-751) unchanged; it never fetches-then-confirms. If the proof does not match, the result carries `{"dispatched":false,"error":"confirmation_mismatch","detail":"…"}` with the expected value never disclosed (proof.rs:150). Results, not protocol errors, for every app-side refusal: `confirmation_required`, `confirmation_mismatch`, `not_exposed`, `unknown_command`, `rate_limited`.

**6. Irreversible tools are opt-in**: By default, `agent mcp` omits `call-irreversible` from the `tools/list` response and rejects any attempt to call it with `-32602` (invalid params). Launching with `--allow-irreversible` includes it. This server-side gate applies to every client; the per-tool permission prompt that Claude Code provides is a second gate on top.

**7. Tool descriptions carry untrusted-text notices**: The Rust fence at the point of dispatch (prompt_fence.rs:418-428) carries no instruction text, only bare tags like `<job_posting>…</job_posting>`. The MCP tool description for `best-matches` and the `instructions` field must tell the model those spans are third-party data not to be trusted as facts. The same notice applies to any `structuredContent` if the server adds it.

**8. Results are verbatim, sentinel vocabulary is closed**: The tool result carries the JSON-RPC response from the app's dispatch logic on `content[].text` (exit code plus the JSON-RPC body). The `isError` flag signals app refusals (errors) and client-side failures; tool results never map to JSON-RPC error responses. Error details use the fixed sentinel vocabulary established in agent_cli.rs (no echoed argv, no paths, no token values). A `detail` field may carry human context (e.g., which read command to call next for a proof).

**9. Stdout hygiene**: Exactly one compact JSON-RPC frame per line via `writeln!(stdout().lock(), serde_json::to_string(...))`. Never `println!`, never `to_string_pretty`, never logger output on stdout. Logs go to stderr only. The `ensure_console_output` call (windows_console.rs) is preserved as the first action, a no-op on inherited pipes.

**10. Token freshness on reconnect**: The MCP server holds a single WebSocket connection across multiple tool calls. When reconnecting (e.g., after Settings → Regenerate Token), it re-reads the token file from disk and attempts HMAC auth with the fresh key; a `pairing_rejected` refusal triggers immediate exit (no retry loop). This preserves the freshness guarantee the CLI nominally offers (token read every invocation).

**11. No app launch dependency**: `initialize` sends only the protocol version and basic info; it does not check app status, read the app's state, or establish a connection. Startup works with the app closed. The first tool call (e.g., `best-matches`) is where the connection handshake happens, in the same path the CLI already uses.

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

- **Confirmation ceremony not enforced server-side for all clients**: Claude Code implements `_meta["anthropic/requiresUserInteraction"]` and can force a prompt; Codex does not read this field. On Codex with an `approval_policy` of `never`, an LLM can reach Irreversible rows (and their own approval) by completing the ceremony alone, with no human gate between the model and the action. The `--allow-irreversible` flag and this ADR's documentation make that explicit.

- **Best-matches title/company/location are unfenced in the MCP surface**: The `best-matches` tool returns up to 50 job postings; the fields title, company, and location are raw strings (not wrapped in the same `<job_posting>…</job_posting>` fence the app uses in other contexts). The tool description names them as third-party data.

- **Tool results carry PII to the client**: Results leave the machine and land in the MCP client's cloud provider and persisted transcript. The `profile` tool returns contact info; `documents_get_text` returns the full résumé; `applications_list` and similar return application records. The server carries no redaction (per ADR-038 §3, the generic tier is raw). This is an explicit consequence of MCP design, not a new risk.

### Verified behaviors

The 2025-11-25 JSON-RPC dialect is supported by:

- Claude Code 2.1.258 (uses the TypeScript SDK, which implements the 2025-11-25 handshake and falls back to `-32601` on `server/discover` for legacy servers)
- Codex 0.144.6 (implements the spec directly in Go, same handshake, same fallback)

Both clients pass `initialize` params containing `protocolVersion: "2025-11-25"` and expect a static response that names no async connection or app state. Both re-use the same `USERPROFILE`/`HOME` environment variables the CLI depends on to locate the pointer file.

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
