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
    // `/opt/$USER/…` unescaped becomes `/opt/saeed/…` (or `/opt//…` when the
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
    // Inside double quotes bash keeps a backslash literal unless it precedes
    // one of the three characters above, so the Windows case needs no help;
    // "escape everything" is what would break it.
    expect(buildClaudeCodeSnippet(WINDOWS_PATH, 'read')).not.toContain('\\\\');
  });

  it('is null when the path is unknown — never a command with an empty path', () => {
    expect(buildClaudeCodeSnippet(null, 'read')).toBeNull();
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
