// Direct-invocation entry point for `gen-agent-catalogue.ts` (A1-r3-AC-3 MEDIUM) — see that
// file's own comment above `export async function main`. `pnpm gen:agent-catalogue`/
// `gen:agent-catalogue:check` run THIS file, never `gen-agent-catalogue.ts` itself, so `main()`
// running is a property of the process starting at all, never an `import.meta.url` comparison
// that could silently fail to match. Same shape as `gen-api-docs.cli.mjs`.

import { main, REPO_ROOT } from './gen-agent-catalogue';

try {
  await main();
} catch (error) {
  // Path privacy: print the message alone. Node's default handler prints a
  // stack trace full of absolute paths, and a filesystem error carries one in
  // its message, so the repo root is stripped out of both.
  const message = error instanceof Error ? error.message : String(error);
  const roots = [REPO_ROOT, REPO_ROOT.split('\\').join('/')];
  console.error(roots.reduce((text, root) => text.split(root).join('.'), message));
  process.exitCode = 1;
}
