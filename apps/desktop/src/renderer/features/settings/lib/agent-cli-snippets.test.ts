/**
 * Quoting is the whole reason these builders exist, so the assertions are on
 * the EXACT strings a user copies — not on "contains the flag". A snippet that
 * is one quote wrong still contains every substring a laxer test would look
 * for, and only fails when it is pasted into a shell or a TOML parser.
 */
import { describe, expect, it } from 'vitest';

import { AGENT_CLI_TIERS, buildClaudeCodeSnippet, buildCodexSnippet } from './agent-cli-snippets';

/** The realistic bad case: an install path with a space in it. */
const WINDOWS_PATH = 'C:\\Users\\demo\\AppData\\Local\\AI Job Hunter\\ajh-tauri.exe';
const MACOS_PATH = '/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri';

/** The read-tier command around the quoted path — split so the escaping tests
 *  can name the escaped word alone without re-spelling the whole line. */
const CLAUDE_PREFIX = 'claude mcp add --scope user ai-job-hunter -- "';
const CLAUDE_SUFFIX = '" agent mcp';

/** The read-tier `claude mcp add …` line for a path whose escaped form is `word`. */
const claudeLine = (word: string) => `${CLAUDE_PREFIX}${word}${CLAUDE_SUFFIX}`;

/** The escaped path out of a built snippet — the bytes a shell actually parses. */
function escapedPath(exePath: string): string {
  const snippet = buildClaudeCodeSnippet(exePath, 'read');
  if (snippet === null) throw new Error('expected a snippet');
  return snippet.slice(CLAUDE_PREFIX.length, snippet.length - CLAUDE_SUFFIX.length);
}

describe('buildClaudeCodeSnippet', () => {
  it('double-quotes a path containing a space, read tier adding no flag', () => {
    expect(buildClaudeCodeSnippet(MACOS_PATH, 'read')).toBe(
      'claude mcp add --scope user ai-job-hunter -- "/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri" agent mcp'
    );
  });

  it('keeps Windows backslashes verbatim inside the double quotes', () => {
    expect(buildClaudeCodeSnippet(WINDOWS_PATH, 'read')).toBe(
      'claude mcp add --scope user ai-job-hunter -- "C:\\Users\\demo\\AppData\\Local\\AI Job Hunter\\ajh-tauri.exe" agent mcp'
    );
  });

  it('names a distinct server per tier and appends that tier flag', () => {
    expect(buildClaudeCodeSnippet(MACOS_PATH, 'reversible')).toBe(
      'claude mcp add --scope user ai-job-hunter-write -- "/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri" agent mcp --allow-reversible'
    );
    expect(buildClaudeCodeSnippet(MACOS_PATH, 'irreversible')).toBe(
      'claude mcp add --scope user ai-job-hunter-unrestricted -- "/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri" agent mcp --allow-irreversible'
    );
  });

  it('escapes a $ so the shell cannot expand part of the path away', () => {
    // `/opt/$USER/…` unescaped becomes `/opt/alice/…` (or `/opt//…` when the
    // variable is unset) by the time `claude` sees it — a server registered at
    // a path that does not exist, with no error at paste time.
    expect(buildClaudeCodeSnippet('/opt/$USER/ajh-tauri', 'read')).toBe(
      'claude mcp add --scope user ai-job-hunter -- "/opt/\\$USER/ajh-tauri" agent mcp'
    );
  });

  it('escapes a backtick so the shell cannot run part of the path as a command', () => {
    expect(buildClaudeCodeSnippet('/opt/a`b`c/ajh-tauri', 'read')).toBe(
      'claude mcp add --scope user ai-job-hunter -- "/opt/a\\`b\\`c/ajh-tauri" agent mcp'
    );
  });

  it('escapes a double quote so the path cannot close its own quoting', () => {
    expect(buildClaudeCodeSnippet('/opt/a"b/ajh-tauri', 'read')).toBe(
      'claude mcp add --scope user ai-job-hunter -- "/opt/a\\"b/ajh-tauri" agent mcp'
    );
  });

  it('leaves backslashes ALONE — doubling them would corrupt every Windows path', () => {
    // Inside double quotes bash keeps a backslash literal unless it precedes a
    // metacharacter (the describe below covers the case where it does), so the
    // Windows case needs no help; "escape everything" is what would break it.
    expect(buildClaudeCodeSnippet(WINDOWS_PATH, 'read')).not.toContain('\\\\');
  });

  it('is null when the path is unknown — never a command with an empty path', () => {
    expect(buildClaudeCodeSnippet(null, 'read')).toBeNull();
  });
});

/**
 * A backslash sitting immediately BEFORE a metacharacter (CodeQL
 * js/incomplete-sanitization).
 *
 * One escaping pass is not enough there: bash consumes the path's own backslash
 * together with the one we add, and the metacharacter it was supposed to
 * neutralise comes back LIVE. Every expectation below is the string bash must
 * receive; the round-trip at the end is the same claim checked against a real
 * shell instead of my reading of the manual.
 */
describe('buildClaudeCodeSnippet escaping around a backslash', () => {
  it('doubles the backslash before a $ instead of feeding the shell an escape it eats', () => {
    // Naive `\$` → `\\$`: bash reads `\\` as one literal backslash and then
    // EXPANDS `$b`. The path must arrive as `a\$b`, so the word has to carry
    // `\\` (the literal backslash) followed by `\$` (the escaped dollar).
    expect(buildClaudeCodeSnippet('a\\$b', 'read')).toBe(claudeLine('a\\\\\\$b'));
  });

  it('doubles the backslash before a backtick — the command-substitution case', () => {
    // The reported one: `C:\` + a backtick opens a command substitution that
    // swallows the rest of the line.
    expect(buildClaudeCodeSnippet('a\\`b', 'read')).toBe(claudeLine('a\\\\\\`b'));
  });

  it('doubles the backslash before a double quote — the path cannot close its word', () => {
    expect(buildClaudeCodeSnippet('a\\"b', 'read')).toBe(claudeLine('a\\\\\\"b'));
  });

  it('carries a doubled backslash through as a doubled backslash', () => {
    // `a\\b` is two literal backslashes. The first precedes a backslash, so it
    // doubles; the second precedes `b` and is already literal.
    expect(buildClaudeCodeSnippet('a\\\\b', 'read')).toBe(claudeLine('a\\\\\\b'));
  });

  it('doubles a TRAILING backslash — the next character is the closing quote', () => {
    // `"C:\dir\"` would escape the quote that ends the word and swallow the
    // rest of the command.
    expect(buildClaudeCodeSnippet('C:\\dir\\', 'read')).toBe(claudeLine('C:\\dir\\\\'));
  });

  it('still leaves an ordinary Windows path completely untouched', () => {
    // The reason this is two passes and not "escape every backslash": the
    // common case has no metacharacter at all, and must survive verbatim for
    // PowerShell and cmd, which do not read backslash as an escape.
    expect(escapedPath(WINDOWS_PATH)).toBe(WINDOWS_PATH);
    expect(escapedPath(MACOS_PATH)).toBe(MACOS_PATH);
  });
});

/**
 * `execFileSync`, loaded at RUNTIME.
 *
 * `apps/desktop` pins `types: ["vite/client"]` — the renderer is not Node and
 * must not typecheck as if it were — so a static `node:child_process` import
 * does not compile here. The specifier is therefore a variable: tsc leaves an
 * unresolvable dynamic import alone, and vitest (which runs on Node) resolves
 * it when the test asks for it. Nothing else in this file is Node-aware.
 */
const NODE_CHILD_PROCESS = 'node:child_process';

type ExecFileSync = (
  file: string,
  args: readonly string[],
  options: { input: string; encoding: 'utf8' }
) => string;

/**
 * Run `script` through bash and return its stdout.
 *
 * The script goes in on STDIN, not argv: on Windows, node re-quotes an argument
 * for CreateProcess and the MSYS runtime re-parses it, which eats exactly the
 * backslashes under test — the harness would fail on correct output.
 */
async function runBash(script: string): Promise<string> {
  const { execFileSync } = (await import(/* @vite-ignore */ NODE_CHILD_PROCESS)) as {
    execFileSync: ExecFileSync;
  };
  return execFileSync('bash', ['-s'], { input: script, encoding: 'utf8' });
}

/**
 * The same claim, measured instead of reasoned: feed the quoted word to a real
 * bash and require the ORIGINAL path back.
 *
 * Skipped when no usable bash is on PATH (a Windows box without Git Bash),
 * because the alternative is a suite that fails on a developer's machine for a
 * reason that has nothing to do with the code. The probe RUNS one rather than
 * looking it up: Windows ships a `bash.exe` shim that exists and then fails
 * when no WSL distribution is installed.
 */
describe('buildClaudeCodeSnippet under a real bash', () => {
  it('round-trips every hostile path back to itself', async ({ skip }) => {
    try {
      if ((await runBash('printf %s ok')) !== 'ok') skip();
    } catch {
      skip();
    }

    const paths = [
      WINDOWS_PATH,
      MACOS_PATH,
      'a\\$b',
      'a\\`b',
      'a\\"b',
      'a\\\\b',
      'C:\\dir\\',
      '/opt/$USER/ajh-tauri',
      '/opt/a`b`c/ajh-tauri',
      '/opt/a"b/ajh-tauri',
    ];
    for (const path of paths) {
      expect(await runBash(`printf %s "${escapedPath(path)}"`)).toBe(path);
    }
  });
});

describe('buildCodexSnippet', () => {
  it('puts a Windows path in a TOML literal string, so no backslash is escaped', () => {
    expect(buildCodexSnippet(WINDOWS_PATH, 'read')).toBe(
      [
        '[mcp_servers.ai-job-hunter]',
        "command = 'C:\\Users\\demo\\AppData\\Local\\AI Job Hunter\\ajh-tauri.exe'",
        'args = ["agent", "mcp"]',
      ].join('\n')
    );
  });

  it('keeps ONE table name across tiers and moves the flag into args', () => {
    expect(buildCodexSnippet(MACOS_PATH, 'reversible')).toBe(
      [
        '[mcp_servers.ai-job-hunter]',
        "command = '/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri'",
        'args = ["agent", "mcp", "--allow-reversible"]',
      ].join('\n')
    );
    expect(buildCodexSnippet(MACOS_PATH, 'irreversible')).toBe(
      [
        '[mcp_servers.ai-job-hunter]',
        "command = '/Applications/AI Job Hunter.app/Contents/MacOS/ajh-tauri'",
        'args = ["agent", "mcp", "--allow-irreversible"]',
      ].join('\n')
    );
  });

  it('falls back to a basic string when the path itself contains a single quote', () => {
    // A TOML literal string has no escapes at all, so `…/O'Brien/…` inside one
    // terminates the string early and the config file will not parse.
    expect(buildCodexSnippet("C:\\Users\\O'Brien\\ajh-tauri.exe", 'read')).toBe(
      [
        '[mcp_servers.ai-job-hunter]',
        'command = "C:\\\\Users\\\\O\'Brien\\\\ajh-tauri.exe"',
        'args = ["agent", "mcp"]',
      ].join('\n')
    );
  });

  it('is null when the path is unknown', () => {
    expect(buildCodexSnippet(null, 'irreversible')).toBeNull();
  });
});

describe('AGENT_CLI_TIERS', () => {
  it('offers read-only first — it is the default the server runs at', () => {
    expect(AGENT_CLI_TIERS).toEqual(['read', 'reversible', 'irreversible']);
  });

  it('produces a distinct pair of snippets for every tier', () => {
    const claude = AGENT_CLI_TIERS.map((tier) => buildClaudeCodeSnippet(MACOS_PATH, tier));
    const codex = AGENT_CLI_TIERS.map((tier) => buildCodexSnippet(MACOS_PATH, tier));
    expect(new Set(claude).size).toBe(AGENT_CLI_TIERS.length);
    expect(new Set(codex).size).toBe(AGENT_CLI_TIERS.length);
  });
});
