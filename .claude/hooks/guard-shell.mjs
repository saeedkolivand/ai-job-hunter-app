#!/usr/bin/env node
// PreToolUse(Bash|PowerShell): deterministic fleet guardrails, so a rule doesn't
// depend on reaching the agent's prompt. Exit 2 rejects the call; stderr goes to Claude.
//  - Nobody stops the owner's running app (ajh-tauri).
//  - Subagents never change git state: the main session commits by path.
//  - Read-only critics (no Edit/Write in their `tools:`) never write files from the shell.
// ponytail: regexes catch the habitual forms (taskkill, sed -i, git commit), not a
// determined bypass; shell redirection isn't matched (too many `->`/`=>` false hits).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// Dev builds run as `ajh-tauri` on every OS; installed bundles use the product name
// ("AI Job Hunter" on macOS, `ai-job-hunter` on Linux) or the bundle id.
const APP = String.raw`(ajh-tauri|ai[- _]?job[- _]?hunter|com\.ajh\.desktop)`;
const STOPS_APP = new RegExp(
  String.raw`\b(taskkill|kill|pkill|killall|wmic|Stop-Process|spps)\b[^|;&\n]*${APP}` +
    String.raw`|${APP}[^;&\n]*\|\s*(xargs\s+(-\S+\s+)*)?(kill|Stop-Process|spps)\b` +
    String.raw`|osascript\b[^;&\n]*\bquit\b[^;&\n]*${APP}`,
  'i'
);
const GIT_MUTATION =
  /\bgit\s+(-c\s+\S+\s+)*(commit|push|add|stash|checkout|switch|reset|restore|rebase|merge|cherry-pick|revert|clean|am|apply|worktree\s+(add|remove|prune)|branch\s+-[dDmM])\b/;
const SHELL_WRITE =
  /\b(sed|perl)\s+(-\w+\s+)*-\w*i|\b(Set-Content|Add-Content|Out-File|New-Item|Remove-Item|Move-Item|Copy-Item)\b|(^|[;&|]\s*)(rm|mv|cp|tee|truncate)\s/;

/** The rejection message for a shell command, or null to allow it. */
export function verdict(command, agentType, isCritic) {
  if (STOPS_APP.test(command)) return "Blocked: never stop the owner's running app (ajh-tauri).";
  if (!agentType) return null;
  if (GIT_MUTATION.test(command))
    return 'Blocked: subagents never change git state; leave the work uncommitted for the main session.';
  if (isCritic && SHELL_WRITE.test(command))
    return 'Blocked: critics are read-only; report the change as a finding instead of writing files.';
  return null;
}

/** A repo agent whose `tools:` grants neither Edit nor Write is a read-only critic. */
export function isCriticAgent(agentType, dir = process.env.CLAUDE_PROJECT_DIR ?? process.cwd()) {
  try {
    const md = readFileSync(join(dir, '.claude', 'agents', `${agentType}.md`), 'utf8');
    return !/^tools:.*\b(Edit|Write)\b/m.test(md);
  } catch {
    return false; // built-in or plugin agent: git guard only
  }
}

if (process.argv[1]?.endsWith('guard-shell.mjs')) {
  let raw = '';
  for await (const chunk of process.stdin) raw += chunk;
  const { tool_input, agent_type } = JSON.parse(raw || '{}');
  const reason = verdict(
    tool_input?.command ?? '',
    agent_type,
    Boolean(agent_type) && isCriticAgent(agent_type)
  );
  if (reason) {
    console.error(reason);
    process.exit(2);
  }
}
