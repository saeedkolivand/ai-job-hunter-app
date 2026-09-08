import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

// Issue #1183 F3, same guard `gen-agent-catalogue.test.ts`'s "CI drift-gate entry point
// (A1-r3-AC-3)" describe block already pins for its own sibling generator. `gen-api-docs.mjs`
// used to guard its own direct-invocation branch with an `import.meta.url` vs
// `pathToFileURL(process.argv[1]).href` comparison — a comparison that could stop matching
// silently (a symlinked/shimmed invocation, or a Windows drive-letter/short-path mismatch) and
// leave `pnpm gen:api:check` exiting 0 having done nothing, passing CI against a stale
// docs/API.md. The fix removes that comparison entirely: `main` is exported with no
// self-invocation, and only the separate `gen-api-docs.cli.mjs` entry point calls it (see that
// file's own doc comment). These checks pin both halves of that fix so a regression to the old
// pattern fails here rather than only showing up as a silent CI no-op — the ONE fix in this PR
// that shipped with no test at all before this one.
describe('CI drift-gate entry point (issue #1183 F3)', () => {
  it('has no argv/import.meta.url self-invocation guard left in the generator module', () => {
    const source = readFileSync(new URL('./gen-api-docs.mjs', import.meta.url), 'utf8');
    // The removed line's exact shape was `import.meta.url === pathToFileURL(process.argv[1])
    // .href` — matched narrowly on the `===` comparison, not a bare mention of
    // `pathToFileURL(process.argv[1])`, since this file's own doc comment (describing the fix
    // historically) names that exact call in prose without reintroducing the comparison.
    expect(source).not.toMatch(/import\.meta\.url\s*===\s*pathToFileURL/);
    expect(source).not.toMatch(/const isMain\s*=/);
  });

  it('routes both gen:api package scripts through the .cli.mjs wrapper', () => {
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
    expect(pkg.scripts['gen:api']).toContain('gen-api-docs.cli.mjs');
    expect(pkg.scripts['gen:api:check']).toContain('gen-api-docs.cli.mjs');
  });
});
