import { describe, expect, it } from 'vitest';

import { ts } from '../../../scripts/gen-api-docs.mjs';
import { collectContractInterfaceFields } from './gen-agent-catalogue';

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
