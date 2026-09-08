import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { ts } from '../../../scripts/gen-api-docs.mjs';
import { collectContractInterfaceFields } from './gen-agent-catalogue';

const HERE = dirname(fileURLToPath(import.meta.url));

// A1-r2-AC-3 MEDIUM: two exported interfaces sharing a name across different `ipc/contracts/*.ts`
// files used to be silently resolved by `Map.set`'s last-wins semantics, so the nested-key
// contract dispatch ENFORCES would depend on `readdirSync` iteration order — the exact hazard the
// sibling multi-call-site guard (`processNamespaceFile`) already `fail()`s on for descriptions and
// argument shapes. Synthetic in-memory sources (`ts.createSourceFile`), never real repo files —
// this guard is about the FUNCTION's own collision logic, not today's (duplicate-free) contract
// tree.
function sourceFile(path: string, text: string): ts.SourceFile {
  return ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true);
}

describe('collectContractInterfaceFields', () => {
  it('resolves a single interface normally', () => {
    const sources = new Map([['a.ts', sourceFile('a.ts', 'export interface Foo { a: string; }')]]);
    const fields = collectContractInterfaceFields(sources);
    expect(fields.get('Foo')).toEqual(['a']);
  });

  it('accepts the same interface name declared twice with IDENTICAL fields (order-independent)', () => {
    const sources = new Map([
      ['a.ts', sourceFile('a.ts', 'export interface Foo { a: string; b: number; }')],
      ['b.ts', sourceFile('b.ts', 'export interface Foo { b: number; a: string; }')],
    ]);
    expect(() => collectContractInterfaceFields(sources)).not.toThrow();
  });

  it('fails on the same interface name declared twice with DIFFERING fields', () => {
    const sources = new Map([
      ['a.ts', sourceFile('a.ts', 'export interface Foo { a: string; b: number; }')],
      ['b.ts', sourceFile('b.ts', 'export interface Foo { a: string; c: boolean; }')],
    ]);
    expect(() => collectContractInterfaceFields(sources)).toThrow(/Foo/);
  });
});

// A1-r3-AC-3 MEDIUM: `gen-agent-catalogue.ts` used to guard its own direct-invocation branch with
// `resolve(process.argv[1]) === fileURLToPath(import.meta.url)` — a comparison that could stop
// matching silently (a symlinked/shimmed invocation, or a Windows drive-letter/short-path
// mismatch) and leave `pnpm gen:agent-catalogue:check` exiting 0 having done nothing, passing CI
// against a stale catalogue. The fix removes that comparison entirely: `main` is exported with no
// self-invocation, and only the separate `gen-agent-catalogue.cli.ts` entry point calls it, the
// same shape `gen-api-docs.cli.mjs` already uses. These two checks pin both halves of that fix so
// a regression to the old pattern fails here rather than only showing up as a silent CI no-op.
describe('CI drift-gate entry point (A1-r3-AC-3)', () => {
  it('has no argv-comparison self-invocation guard left in the generator module', () => {
    const source = readFileSync(resolve(HERE, 'gen-agent-catalogue.ts'), 'utf8');
    expect(source).not.toMatch(/if\s*\(\s*process\.argv\[1\]/);
  });

  it('routes both gen:agent-catalogue package scripts through the .cli.ts wrapper', () => {
    const pkg = JSON.parse(readFileSync(resolve(HERE, '../package.json'), 'utf8')) as {
      scripts: Record<string, string>;
    };
    expect(pkg.scripts['gen:agent-catalogue']).toContain('gen-agent-catalogue.cli.ts');
    expect(pkg.scripts['gen:agent-catalogue:check']).toContain('gen-agent-catalogue.cli.ts');
  });

  // TR-01 MEDIUM: this is the one flag the whole CI drift gate depends on — the generator only
  // refuses to write (exit 1) a stale catalogue when invoked with `--check` (gen-agent-catalogue.ts
  // `const check = process.argv.includes('--check')`). Dropping it from the `:check` script turns
  // CI into a silent no-op that exits 0 and rewrites the catalogue instead of failing on drift.
  it('passes --check to the :check script only, never to the plain generator script', () => {
    const pkg = JSON.parse(readFileSync(resolve(HERE, '../package.json'), 'utf8')) as {
      scripts: Record<string, string>;
    };
    expect(pkg.scripts['gen:agent-catalogue:check']).toContain('--check');
    expect(pkg.scripts['gen:agent-catalogue']).not.toContain('--check');
  });
});
