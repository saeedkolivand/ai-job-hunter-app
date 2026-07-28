#!/usr/bin/env node
// PreToolUse(Bash) — when a grep/find-style command is about to run and a graphify graph exists,
// nudge toward `graphify query/explain` (scoped subgraph) instead of scanning raw files.
// Cross-platform, fast, non-blocking: any error → exit 0.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

let p = {};
try {
  if (!process.stdin.isTTY) p = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
} catch {}
try {
  const cmd = (p.tool_input && p.tool_input.command) || '';
  const cwd = p.cwd || process.cwd();
  // once per session — repeating the same ~350B nudge on every grep-ish Bash call
  // costs tokens dozens of times per search-heavy session for zero new information
  const sid = String(p.session_id || 'nosession').replace(/[^\w-]/g, '');
  const marker = path.join(os.tmpdir(), `graphify-hint-${sid}`);
  if (fs.existsSync(marker)) process.exit(0);
  const hasGraph = fs.existsSync(path.join(cwd, 'graphify-out', 'graph.json'));
  if (hasGraph && /\b(grep|rg|ripgrep|find|fd|ack|ag)\b/.test(cmd)) {
    try {
      // 'wx' = exclusive create: never follows a pre-planted symlink in the shared
      // tmp dir, and closes the TOCTOU between the existsSync check and this write
      fs.writeFileSync(marker, '1', { flag: 'wx', mode: 0o600 });
    } catch {}
    process.stdout.write(
      JSON.stringify({
        hookSpecificOutput: {
          hookEventName: 'PreToolUse',
          additionalContext:
            'graphify: a knowledge graph exists at graphify-out/. For focused codebase questions prefer `graphify query "<question>"` / `graphify explain "<concept>"` / `graphify path "<A>" "<B>"` (scoped subgraph, usually much smaller than grepping raw files). Read GRAPH_REPORT.md only for broad architecture context.',
        },
      })
    );
  }
} catch {}
process.exit(0);
