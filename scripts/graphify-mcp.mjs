#!/usr/bin/env node
// Portable stdio launcher for the graphify MCP server.
//
// graphify-out/ is gitignored (per-machine), so this resolves the local Python
// interpreter recorded in graphify-out/.graphify_python at runtime and execs
// `python -m graphify.serve graphify-out/graph.json`, piping stdio straight
// through (MCP speaks JSON-RPC over stdin/stdout). Keeping the interpreter path
// out of the committed config avoids leaking a home path and stays portable —
// every machine resolves its own interpreter. Referenced by .mcp.json.
//
// On a checkout without a built graph (no graphify-out/), it exits non-zero with
// a hint; Claude Code surfaces that as a failed-to-connect MCP server, which is
// non-fatal — run `graphify update .` to build the graph first.
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const interpreterFile = join(root, 'graphify-out', '.graphify_python');
const graph = join(root, 'graphify-out', 'graph.json');

if (!existsSync(interpreterFile) || !existsSync(graph)) {
  process.stderr.write(
    '[graphify-mcp] graphify-out/ not found — run `graphify update .` to build the graph first.\n'
  );
  process.exit(1);
}

const python = readFileSync(interpreterFile, 'utf8').trim();
const child = spawn(python, ['-m', 'graphify.serve', graph], { stdio: 'inherit' });

child.on('error', (err) => {
  process.stderr.write(`[graphify-mcp] failed to start: ${err.message}\n`);
  process.exit(1);
});
child.on('exit', (code) => process.exit(code ?? 0));
