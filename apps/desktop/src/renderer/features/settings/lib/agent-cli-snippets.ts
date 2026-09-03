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
 * The `claude mcp add …` command for one tier, or `null` when the path is
 * unknown (rendering the command with an empty path would produce a line that
 * looks copyable and silently registers nothing).
 *
 * The path is wrapped in DOUBLE quotes: the default install directory contains
 * a space on Windows and macOS, and a double-quoted Windows path survives
 * bash, PowerShell and cmd alike — a single-quoted one does not survive cmd.
 */
export function buildClaudeCodeSnippet(exePath: string | null, tier: AgentCliTier): string | null {
  if (!exePath) return null;
  return [
    'claude mcp add --scope user',
    CLAUDE_SERVER_NAME[tier],
    '--',
    `"${exePath}"`,
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
