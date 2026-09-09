/**
 * Registration snippets for the bundled agent CLI / MCP server, built from the
 * running binary's own path.
 *
 * Pure on purpose: quoting is the only thing that can actually be wrong here
 * (the install path routinely contains a space), and quoting is not observable
 * from a rendered component test. The card in
 * `components/preferences/AgentCliSection` renders whatever these return and
 * copies it verbatim to the clipboard.
 *
 * The server names and flags mirror the project README's CLI-agent section —
 * the same three commands, with the placeholder path substituted for the real
 * one. Nothing here names a release version or claims anything about `PATH`:
 * a full-path command works either way.
 *
 * Claude Code and Codex each take a COMMAND (their own CLI parses it); every
 * other MCP client reads a `mcpServers` JSON block instead, which is why
 * {@link buildGenericMcpSnippet} exists alongside the two command builders.
 *
 * {@link buildCursorDeeplink} and {@link buildVsCodeDeeplink} (roadmap #1146
 * P2) encode that same `mcpServers` entry into each editor's own one-click
 * install URL scheme instead of a paste target — the card opens them via
 * `openExternal`, never by navigating the window to them.
 */

/** Which write tier the generated registration asks the MCP server for. */
export type AgentCliTier = 'read' | 'reversible' | 'irreversible';

/** Tier order as the UI offers it — read-only first, and the default. */
export const AGENT_CLI_TIERS: readonly AgentCliTier[] = ['read', 'reversible', 'irreversible'];

/**
 * Extra argument after `agent mcp` per tier. Read-only adds NOTHING — the
 * server is read-only unless a flag opens a write tier, so an explicit
 * "read-only" flag would suggest a switch that does not exist.
 */
const TIER_FLAG: Record<AgentCliTier, string | null> = {
  read: null,
  reversible: '--allow-reversible',
  irreversible: '--allow-irreversible',
};

/**
 * Claude Code server name per tier. Three distinct names rather than one, so a
 * user who registers the write tier can see which server a call went to (and
 * can keep the read-only one registered alongside it).
 */
const CLAUDE_SERVER_NAME: Record<AgentCliTier, string> = {
  read: 'ai-job-hunter',
  reversible: 'ai-job-hunter-write',
  irreversible: 'ai-job-hunter-unrestricted',
};

/**
 * Codex keeps ONE table name across tiers: `~/.codex/config.toml` keys servers
 * by table header, the tier lives in `args`, and this is the shape the README
 * documents. A second header would be a second server, not a re-registration.
 */
const CODEX_SERVER_NAME = 'ai-job-hunter';

/** `agent mcp` plus the tier's flag, as separate argv words. */
function mcpArgs(tier: AgentCliTier): string[] {
  const flag = TIER_FLAG[tier];
  return flag ? ['agent', 'mcp', flag] : ['agent', 'mcp'];
}

/**
 * `value` as it goes INSIDE a double-quoted bash/zsh word.
 *
 * Between double quotes bash consumes a backslash only when it precedes one of
 * `$`, `` ` ``, `"`, `\` or a newline; everywhere else the backslash stays
 * literal. That asymmetry is the whole function, and it needs TWO passes in
 * this order:
 *
 * 1. **Double every backslash that already precedes one of those five.** Skip
 *    this and the shell eats that backslash together with the escape added
 *    below, handing the metacharacter back live: `C:\` + `` ` `` + `x` naively
 *    escapes to ``C:\\`x``, which bash reads as a literal `\` followed by an
 *    OPEN command substitution. A value's TRAILING backslash counts too — the
 *    character after it is the closing quote the caller writes.
 * 2. **Then escape `` ` ``, `$` and `"` themselves.** Those three are the only
 *    characters that keep a special meaning between double quotes — command
 *    substitution, expansion, and the closing quote. A path under `$HOME/bin`
 *    or a directory literally named `My $Money` would otherwise be EXPANDED,
 *    registering a server at a path that does not exist.
 *
 * Every OTHER backslash is deliberately left alone. Doubling all of them would
 * corrupt the ordinary Windows path — the common case, which has to survive
 * verbatim because PowerShell and cmd do not read backslash as an escape at
 * all.
 *
 * **PowerShell limit:** it also treats `$` and `` ` `` as special inside double
 * quotes, but its escape character is a BACKTICK, not a backslash. One string
 * cannot be correct for both, so this one is correct for bash/zsh. A path
 * containing `$` or a backtick therefore needs hand-editing before it is
 * pasted into PowerShell — rare in an install path, and the alternative is a
 * snippet that is wrong on macOS and Linux in order to be right on Windows.
 */
function shellDoubleQuoted(value: string): string {
  return value.replace(/\\(?=[`$"\\\n]|$)/g, '\\\\').replace(/[`$"]/g, '\\$&');
}

/**
 * The `claude mcp add …` command for one tier, or `null` when the path is
 * unknown (rendering the command with an empty path would produce a line that
 * looks copyable and silently registers nothing).
 *
 * The path is wrapped in DOUBLE quotes: the default install directory contains
 * a space on Windows and macOS, and a double-quoted Windows path survives
 * bash, PowerShell and cmd alike — a single-quoted one does not survive cmd.
 * What the quotes do NOT neutralise is escaped by {@link shellDoubleQuoted}.
 */
export function buildClaudeCodeSnippet(exePath: string | null, tier: AgentCliTier): string | null {
  if (!exePath) return null;
  return [
    'claude mcp add --scope user',
    CLAUDE_SERVER_NAME[tier],
    '--',
    `"${shellDoubleQuoted(exePath)}"`,
    ...mcpArgs(tier),
  ].join(' ');
}

/**
 * The `~/.codex/config.toml` block for one tier, or `null` when the path is
 * unknown.
 *
 * `command` is a TOML LITERAL string (single quotes): a literal string has no
 * escape sequences at all, which is exactly what a Windows path full of
 * backslashes needs — `"C:\Users\…"` in a TOML BASIC string would be read as
 * the escapes `\U`/`\…` and rejected by the parser.
 */
export function buildCodexSnippet(exePath: string | null, tier: AgentCliTier): string | null {
  if (!exePath) return null;
  const args = mcpArgs(tier)
    .map((arg) => `"${arg}"`)
    .join(', ');
  return [
    `[mcp_servers.${CODEX_SERVER_NAME}]`,
    `command = ${tomlString(exePath)}`,
    `args = [${args}]`,
  ].join('\n');
}

/**
 * `value` as a TOML string — literal by default, basic when it has to be.
 *
 * A literal string cannot contain a single quote and has no way to escape one,
 * so a path under a home directory like `O'Brien` has to fall back to a basic
 * string with its backslashes and quotes escaped. Rare, but the failure is an
 * unparseable config file the user pasted from us, which is worse than the
 * three lines it costs to get right.
 */
function tomlString(value: string): string {
  if (!value.includes("'")) return `'${value}'`;
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

/**
 * The `{name, command, args}` triple every non-command-line registration is
 * built from — the generic `mcpServers` block below and both deeplink
 * builders read this SAME shape, so a tier's server name or flag can never
 * drift between the three (roadmap #1146 P2).
 */
function serverEntry(
  exePath: string,
  tier: AgentCliTier
): { name: string; command: string; args: string[] } {
  return { name: CLAUDE_SERVER_NAME[tier], command: exePath, args: mcpArgs(tier) };
}

/**
 * The generic `mcpServers` JSON block for one tier, or `null` when the path is
 * unknown — the shape Claude Desktop, Cursor, Windsurf, Gemini CLI,
 * LM Studio, Jan and most other MCP clients read verbatim from their own
 * config file. Same server name and args as {@link buildClaudeCodeSnippet}
 * (reused, not re-derived), so the two never drift apart.
 *
 * `JSON.stringify` does the escaping — a Windows path with backslashes and
 * spaces is just a JSON string, and round-trips through `JSON.parse` to the
 * exact path with no hand-written quoting rules to get wrong.
 */
export function buildGenericMcpSnippet(exePath: string | null, tier: AgentCliTier): string | null {
  if (!exePath) return null;
  const { name, command, args } = serverEntry(exePath, tier);
  return JSON.stringify({ mcpServers: { [name]: { command, args } } }, null, 2);
}

/**
 * UTF-8 safe base64, without the deprecated `unescape`/`escape` pair: encode
 * to bytes first, THEN base64 the byte string. `btoa` alone throws on any
 * code point above U+00FF, and an install path can contain one.
 */
function utf8ToBase64(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/**
 * Cursor's one-click MCP install link (cursor.com/docs/context/mcp/install-links):
 * `cursor://anysphere.cursor-deeplink/mcp/install?name=<server name>&config=<base64 JSON>`.
 * The `config` payload is ONLY `{command, args}` — Cursor reads the server
 * name from the query parameter, not from inside the encoded object.
 *
 * The base64 text is ALSO percent-encoded: raw base64 can contain `+`, `/`
 * and `=`, and a `+` inside a query string is read back as a space by the
 * standard `URLSearchParams` decode most link handlers use — silently
 * corrupting the payload rather than failing to parse.
 */
export function buildCursorDeeplink(exePath: string | null, tier: AgentCliTier): string | null {
  if (!exePath) return null;
  const { name, command, args } = serverEntry(exePath, tier);
  const config = utf8ToBase64(JSON.stringify({ command, args }));
  return `cursor://anysphere.cursor-deeplink/mcp/install?name=${encodeURIComponent(name)}&config=${encodeURIComponent(config)}`;
}

/**
 * VS Code's stable MCP install link: `vscode:mcp/install?<url-encoded JSON>`.
 * Unlike Cursor's, `name` lives INSIDE the encoded payload alongside
 * `command`/`args`, not as a separate query parameter.
 */
export function buildVsCodeDeeplink(exePath: string | null, tier: AgentCliTier): string | null {
  if (!exePath) return null;
  const entry = serverEntry(exePath, tier);
  return `vscode:mcp/install?${encodeURIComponent(JSON.stringify(entry))}`;
}
