# Usage

Task walkthroughs and the agent CLI / MCP server. Install first: see [INSTALLATION.md](INSTALLATION.md).

<details open>
<summary><strong>Generate a tailored résumé / cover letter</strong></summary>

```
1. Open the app → AI Generate
2. Paste your résumé text, or upload a PDF/DOCX/TXT file
3. Paste the job ad text, or upload a job description file
4. Click Continue → the app detects languages, role, company, top requirements
5. Choose a template + style; optionally enable "Research the company"
6. Generate → watch streaming output (with live reasoning) → export as DOCX / PDF / TXT
```

</details>

<details>
<summary><strong>Run Autopilot &amp; answer application questions</strong></summary>

```
1. Autopilot → New → set board, query, location, schedule, filters
2. Run it → found jobs appear, scored and deduped (New badges on fresh results)
3. Open a found job → Tailor:
   • generate a tailored résumé + cover letter (target: Both)
   • pick application questions → get résumé-grounded answers
   • the documents are saved in Documents → Activity; the job stays "Saved"
4. After you submit the application, click "Mark as applied" so the job shows "Applied"
```

</details>

<details>
<summary><strong>Scrape boards &amp; search semantically</strong></summary>

```
1. Jobs → Scrape → select boards (e.g. LinkedIn + Greenhouse + Aggregator)
2. Query + location; for company-scoped ATS boards (Greenhouse, Lever, etc.), enter company slugs
3. Click Start → results stream into the jobs table
4. Ranked search over the scraped postings — keyword by default, with vector similarity and LLM reranking *re-ordering* the results when semantic ranking is enabled
```

**Note:** Company-scoped ATS boards require company slugs instead of free-text keywords. The **Aggregator** (Adzuna + JSearch) replaces direct scraping of anti-bot boards (Indeed, Glassdoor, Xing); free Adzuna keys available at https://developer.adzuna.com. See `docs/knowledge/scraping-domain.md` for the full board list and configuration details.

</details>

<details>
<summary><strong>CLI agent — `ajh-tauri agent <verb>` (headless mode)</strong></summary>

Run commands from your shell or an LLM agent, with the desktop app already running:

```bash
# Find the best matches across all autopilots
ajh-tauri agent best-matches --limit 10

# Get full details for one job by URL
ajh-tauri agent job "https://example.com/job/123"

# Export your profile for autofill (requires opt-in consent)
ajh-tauri agent profile

# List all autopilots and their status
ajh-tauri agent automations

# Generic command dispatch (advanced: read/reversible commands, irreversible with proof)
ajh-tauri agent call "namespace:command" --input '{"key":"value"}'

# Show help
ajh-tauri agent --help
```

**Platform-specific paths:** The CLI is the same binary as the desktop app, no separate install needed. On **Linux** and **Homebrew/macOS** it is already on `PATH`. On **macOS with dmg drag-install**, add the path to your shell profile:

```bash
export PATH="/Applications/AI Job Hunter.app/Contents/MacOS:$PATH"
```

On **Windows**, the NSIS installer adds its per-user install directory to your `PATH`. If `ajh-tauri` still isn't found, invoke it by full path — `~/.ajh-agent/agent.json` carries it as `exePath` (the app rewrites that file on every launch).

**In the app:** Settings → Developer shows this binary's path and ready-to-copy registration commands for Claude Code and Codex with that path already filled in.

**Requirements:** Bridge-backed calls need the app running; `--help`, MCP startup (`initialize`) and the local `commands` tool do not. The CLI communicates over a local loopback bridge with mutual HMAC-SHA256 authentication (the pairing token is used only as an HMAC key and is never sent on the wire). Run `ajh-tauri agent --help` to see all verbs, exit codes, and error sentinels. For the design rationale see <a href="knowledge/decision-records/adr-037-agent-cli-as-binary-mode-thin-client.md" target="_blank" rel="noopener noreferrer">ADR-037</a> and <a href="knowledge/decision-records/adr-038-agent-cli-full-parity-two-tier.md" target="_blank" rel="noopener noreferrer">ADR-038</a>; for the full policy table, use the MCP `commands` tool or read `apps/desktop/src-tauri/src/extension_bridge/agent_cli/policy.rs` (`agent schema` lists only the five curated resources).

**LLM agents (MCP mode):** The CLI can run as a standard MCP stdio server — any MCP client can run it, not just the two documented below:

<details>
<summary>Claude Code (recommended)</summary>

Use `claude mcp add --scope user` to register the server (user scope = per-machine, runs only when you authorize):

```bash
claude mcp add --scope user ai-job-hunter -- "/path/to/ajh-tauri" agent mcp
```

On Linux/macOS with Homebrew, `ajh-tauri` is already on `PATH`. On Windows and macOS dmg, substitute the `exePath` from `~/.ajh-agent/agent.json` — keep the double quotes, because the default install directory contains a space on both platforms and an unquoted path registers a server that never connects. The in-app Settings → Developer card generates the same command already quoted (`shellDoubleQuoted` in `apps/desktop/src/renderer/features/settings/lib/agent-cli-snippets.ts`).

By default the server is **read-only**: the model can search postings, read your profile and automations, and enumerate the command table. Two launch flags open the write tiers, each a superset of the last:

```bash
# undoable state changes — track an application, edit your profile, import a document
claude mcp add --scope user ai-job-hunter-write -- "/path/to/ajh-tauri" agent mcp --allow-reversible
# additionally: destructive actions and AI spend — delete documents, remove provider keys, run generation against your API budget
claude mcp add --scope user ai-job-hunter-unrestricted -- "/path/to/ajh-tauri" agent mcp --allow-irreversible
```

The flags only decide which tools the model can _see_; every write still goes through the app's own policy table and, for destructive commands, the confirm ceremony (ADR-038).

</details>

<details>
<summary>Codex</summary>

Add to your `~/.codex/config.toml` (or create it):

```toml
[mcp_servers.ai-job-hunter]
command = 'ajh-tauri'
args = ["agent", "mcp"]
```

If `ajh-tauri` is not on your `PATH`, set `command` to the `exePath` from `~/.ajh-agent/agent.json` — keep it single-quoted, because a TOML literal string has no escape sequences and a Windows path is full of backslashes (in a double-quoted basic string they read as invalid escapes and the whole file stops parsing) — unless the path contains an apostrophe, in which case copy the Settings card's output verbatim (it falls back to an escaped basic string). The Settings → Developer card emits the correctly-quoted form for you (`tomlString` in `apps/desktop/src/renderer/features/settings/lib/agent-cli-snippets.ts`). The same `--allow-reversible` / `--allow-irreversible` flags go in `args`.

</details>

<details>
<summary>Any MCP client</summary>

Most other MCP clients (Claude Desktop, Cursor, Windsurf, Gemini CLI, LM Studio, Jan, etc.) read a `mcpServers` JSON block from their own config file rather than taking a CLI command:

```json
{
  "mcpServers": {
    "ai-job-hunter": {
      "command": "/path/to/ajh-tauri",
      "args": ["agent", "mcp"]
    }
  }
}
```

The same tier flags as above go at the end of `args` — `--allow-reversible` (server name `ai-job-hunter-write`) or `--allow-irreversible` (server name `ai-job-hunter-unrestricted`). The Settings → Developer card copies this block with your real path already filled in — on Windows the backslashes must be doubled (`C:\\Users\\…`) — or just copy the block from the Settings card, which does it for you.

</details>

**URL-only clients:** `agent mcp --http <port>` runs the same server over MCP Streamable HTTP instead of stdio, for a client that only takes a URL rather than launching a subprocess. It binds `127.0.0.1` only — no flag can name a different host, so there is nothing to configure there — and prints one `{"transport":"http","url":"...","token":"..."}` line to stdout, once, before it starts serving; that token is never written to a file, so a client that loses it has to relaunch the server. Every `POST /mcp` needs `Authorization: Bearer <token>`, and any request carrying an `Origin` header is refused outright (this is for local scripts and agent frameworks, never a browser page's `fetch`). It answers one JSON-RPC request per `POST` — no streaming, no sessions, `GET`/`DELETE` both `405` — and shuts down the same way the stdio mode does, on Ctrl-C or stdin EOF.

**Tool results are sent to the model and stored in the client's transcript.** Queries like `profile` and `documents_get_text` return PII; the model can access job postings, your résumé, and application records. Use these tools only when you ask the agent to retrieve that data.

**Important:** The first argument to the MCP server must be `agent` (e.g., `agent mcp`), or the desktop app launches instead.

</details>
