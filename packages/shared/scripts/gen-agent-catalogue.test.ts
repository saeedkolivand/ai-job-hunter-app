import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import { agentNoteOf, docOf, ts } from '../../../scripts/gen-api-docs.mjs';
import {
  type CatalogueArg,
  type CatalogueEntry,
  collectContractInterfaceFields,
  mergeCatalogueEntry,
} from './gen-agent-catalogue';
import { type FieldSources, toInputJsonSchema } from './gen-agent-catalogue/field-sources';
import { applyOverrides } from './gen-agent-catalogue/schema-overrides';

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

// Issue #1183 F2: the duplicate-call-site guard used to key BOTH the description-equality check
// AND the args-equality check on the bare command name, which forced two namespaces sharing a
// dispatched command (`boards.disconnect`/`linkedin.disconnect` -> `boards_logout`) to publish
// byte-identical TSDoc wording or fail codegen — `linkedin.ts`'s own wording was genericized to
// `boards.ts`'s to satisfy this, degrading `docs/API.md`. The fix keys description-equality on
// `(namespace, command)` instead, while args-equality stays keyed on the bare command (the shape
// `check_input` actually enforces at dispatch really is single-valued).
describe('mergeCatalogueEntry (issue #1183 F2)', () => {
  const arg = (name: string): CatalogueArg => ({ name, required: true, fields: undefined });

  it('does not fail when two namespaces share a command with different descriptions but equal args', () => {
    const entries = new Map<string, CatalogueEntry>();
    const descByNamespaceCommand = new Map<string, string>();
    expect(() => {
      mergeCatalogueEntry(
        entries,
        descByNamespaceCommand,
        'boards',
        'boards_logout',
        'Disconnect a board.',
        [arg('boardId')]
      );
      mergeCatalogueEntry(
        entries,
        descByNamespaceCommand,
        'linkedin',
        'boards_logout',
        'Disconnect and clear LinkedIn session.',
        [arg('boardId')]
      );
    }).not.toThrow();
    // First-namespace-wins: the published description is the FIRST call site's, not the second's
    // — cosmetic only, since nothing downstream validates against it (see the fn's own doc).
    expect(entries.get('boards_logout')?.description).toBe('Disconnect a board.');
  });

  it('still fails when two namespaces share a command with DIFFERING argument shapes', () => {
    const entries = new Map<string, CatalogueEntry>();
    const descByNamespaceCommand = new Map<string, string>();
    mergeCatalogueEntry(
      entries,
      descByNamespaceCommand,
      'boards',
      'boards_logout',
      'Disconnect a board.',
      [arg('boardId')]
    );
    expect(() =>
      mergeCatalogueEntry(
        entries,
        descByNamespaceCommand,
        'linkedin',
        'boards_logout',
        'Disconnect a board.',
        [arg('boardId'), arg('extra')]
      )
    ).toThrow(/DIFFERING argument/);
  });

  it('still fails when the SAME namespace declares two conflicting descriptions for one command', () => {
    const entries = new Map<string, CatalogueEntry>();
    const descByNamespaceCommand = new Map<string, string>();
    mergeCatalogueEntry(
      entries,
      descByNamespaceCommand,
      'boards',
      'boards_logout',
      'Disconnect a board.',
      [arg('boardId')]
    );
    expect(() =>
      mergeCatalogueEntry(
        entries,
        descByNamespaceCommand,
        'boards',
        'boards_logout',
        'Something else entirely.',
        [arg('boardId')]
      )
    ).toThrow(/DIFFERING TSDoc descriptions/);
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

// #1491: the catalogue carries JSON Schema generated from the Zod contracts, and a hand-written
// `@agent` note that `docs/API.md` never shows.
describe('toInputJsonSchema (issue #1491)', () => {
  it('emits nested fields, enums and ranges, with a defaulted field optional (input side)', () => {
    const schema = JSON.parse(
      toInputJsonSchema(
        z.object({
          mode: z.enum(['a', 'b']),
          pages: z.number().int().min(1).max(10).default(2),
          nested: z.object({ tag: z.string().max(5) }),
        })
      )
    );
    expect(schema.$schema).toBeUndefined();
    expect(schema.properties.mode.enum).toEqual(['a', 'b']);
    expect(schema.properties.pages).toMatchObject({ minimum: 1, maximum: 10 });
    expect(schema.properties.nested.properties.tag.maxLength).toBe(5);
    expect(schema.required).toEqual(['mode', 'nested']);
  });

  it('refuses a non-Zod value instead of emitting a guess', () => {
    expect(() => toInputJsonSchema({ type: 'object' })).toThrow(/non-Zod/);
  });
});

describe('@agent note (issue #1491)', () => {
  const src = `interface C {
    /**
     * Short summary.
     *
     * @agent Long note
     *   over two lines.
     */
    a(): void;
  }`;
  const sf = sourceFile('c.ts', src);
  const [member] = (sf.statements[0] as ts.InterfaceDeclaration).members;

  it('is read whole, on one line, for the catalogue', () => {
    expect(agentNoteOf(member, sf)).toBe('Long note over two lines.');
  });

  it('is stripped from the text docs/API.md renders', () => {
    expect(docOf(member, sf)).toBe('Short summary.');
  });
});

// `.refine`s never reach z.toJSONSchema; `resume_pipeline_run.req` re-adds them as `allOf`/`anyOf`.
describe('applyOverrides — resume_pipeline_run rules', () => {
  const sources = (names: string[]) =>
    ({
      zodSchemas: Object.fromEntries(names.map((n) => [n, z.object({ a: z.string().optional() })])),
    }) as unknown as FieldSources;
  const entries = () =>
    new Map([
      ['autopilot_update', { args: [{ name: 'req' }] }],
      ['job_preferences_set', { args: [{ name: 'prefs' }] }],
      ['resume_pipeline_run', { args: [{ name: 'req' }] }],
    ]) as unknown as Map<string, CatalogueEntry>;
  const NAMES = ['AutopilotUpdateSchema', 'JobPreferencesSchema', 'ResumePipelineRunSchema'];

  it('adds a resume source, a posting source and an output rule', () => {
    const map = entries();
    applyOverrides(map, sources(NAMES));
    const schema = JSON.parse(map.get('resume_pipeline_run')?.args[0]?.schema ?? '{}');
    expect(schema.allOf).toHaveLength(3);
    expect(schema.allOf[0].anyOf.map((o: { required: string[] }) => o.required[0])).toEqual([
      'resumeId',
      'resumeText',
    ]);
    expect(schema.allOf[1].anyOf).toHaveLength(3);
    expect(schema.allOf[0].anyOf[0].properties.resumeId.pattern).toBe(String.raw`\S`);
    expect(JSON.parse(map.get('autopilot_update')?.args[0]?.schema ?? '{}').allOf).toBeUndefined();
  });

  it('throws when a row matches no catalogued argument or schema', () => {
    expect(() => applyOverrides(new Map(), sources(NAMES))).toThrow(/matches no catalogued/);
    expect(() => applyOverrides(entries(), sources([]))).toThrow(/is not exported/);
  });
});
