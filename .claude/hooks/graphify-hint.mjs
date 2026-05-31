#!/usr/bin/env node
// PreToolUse(Bash) — when a grep/find-style command is about to run and a graphify graph exists,
// nudge toward `graphify query/explain` (scoped subgraph) instead of scanning raw files.
// Cross-platform, fast, non-blocking: any error → exit 0.
import fs from 'node:fs';
import path from 'node:path';

let p = {};
try {
  if (!process.stdin.isTTY) p = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
} catch {}
try {
  const cmd = (p.tool_input && p.tool_input.command) || '';
  const cwd = p.cwd || process.cwd();
  const hasGraph = fs.existsSync(path.join(cwd, 'graphify-out', 'graph.json'));
  if (hasGraph && /\b(grep|rg|ripgrep|find|fd|ack|ag)\b/.test(cmd)) {
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
