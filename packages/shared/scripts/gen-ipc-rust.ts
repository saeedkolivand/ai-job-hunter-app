/**
 * IPC codegen — Zod schema → Rust request struct.
 *
 * The renderer, the IPC contract, the Rust command, and the TS client are
 * otherwise hand-synced (4 files per capability). This makes the Zod schemas in
 * `src/schemas` the single source of truth for request shapes and emits the
 * matching Rust `Deserialize` structs, so the two can't drift.
 *
 * Run `pnpm gen:ipc` to regenerate, or `pnpm gen:ipc --check` to fail when the
 * committed output is stale (used in CI).
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { z } from 'zod';

import { EVENT_CHANNELS } from '../src/events/index.js';
import { PROVIDER_SLOTS } from '../src/provider-slots.js';
import {
  AgentRunRequestSchema,
  AiGenerateRequestSchema,
  AiGenerationSaveSchema,
  AiGenerationUpdateSchema,
  AiStreamChunkSchema,
  ApplicationTrackSchema,
  ApplicationUpdateSchema,
  AutopilotCreateSchema,
  AutopilotUpdateSchema,
  DATE_FILTER_OPTIONS,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  JobEventSchema,
  MatchResumeBatchRequestSchema,
  MatchResumeRequestSchema,
  ReferralUpsertSchema,
  ResumeExtractTextSchema,
  ScrapeBoardsRequestSchema,
  ScrapeUrlRequestSchema,
} from '../src/schemas/index.js';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, '../../..');

interface StructSpec {
  rustName: string;
  schema: z.ZodType;
  /** Override the Rust type for specific fields (e.g. bytes the JSON Schema can't represent). */
  fieldOverrides?: Record<string, string>;
}

interface ModuleSpec {
  /** Output file, relative to repo root. */
  outFile: string;
  structs: StructSpec[];
}

const MODULES: ModuleSpec[] = [
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/agent.rs',
    structs: [{ rustName: 'AgentRunRequest', schema: AgentRunRequestSchema }],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/scrape.rs',
    structs: [
      { rustName: 'ScrapeBoardsRequest', schema: ScrapeBoardsRequestSchema },
      { rustName: 'ScrapeUrlRequest', schema: ScrapeUrlRequestSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/ai.rs',
    structs: [
      { rustName: 'AiGenerateRequest', schema: AiGenerateRequestSchema },
      { rustName: 'AiEmbedRequest', schema: EmbedRequestSchema },
      { rustName: 'AiGenerationSaveRequest', schema: AiGenerationSaveSchema },
      { rustName: 'AiGenerationUpdateRequest', schema: AiGenerationUpdateSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/documents.rs',
    structs: [
      {
        rustName: 'DocumentsImportRequest',
        schema: DocumentImportRequestSchema,
        fieldOverrides: { bytes: 'Vec<u8>' },
      },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/resume.rs',
    structs: [
      {
        rustName: 'ResumeExtractTextRequest',
        schema: ResumeExtractTextSchema,
        fieldOverrides: { bytes: 'Vec<u8>' },
      },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/autopilot.rs',
    structs: [
      { rustName: 'AutopilotCreateRequest', schema: AutopilotCreateSchema },
      { rustName: 'AutopilotUpdateRequest', schema: AutopilotUpdateSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/applications.rs',
    structs: [
      { rustName: 'ApplicationTrackRequest', schema: ApplicationTrackSchema },
      { rustName: 'ApplicationUpdateRequest', schema: ApplicationUpdateSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/matching.rs',
    structs: [
      { rustName: 'MatchResumeRequest', schema: MatchResumeRequestSchema },
      { rustName: 'MatchResumeBatchRequest', schema: MatchResumeBatchRequestSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/referrals.rs',
    structs: [{ rustName: 'ReferralUpsertRequest', schema: ReferralUpsertSchema }],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/event_payloads.rs',
    structs: [
      { rustName: 'AiStreamChunk', schema: AiStreamChunkSchema },
      { rustName: 'JobEvent', schema: JobEventSchema },
    ],
  },
];

type JsonSchema = {
  type?: string;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  items?: JsonSchema;
  enum?: unknown[];
  default?: unknown;
  minimum?: number;
  additionalProperties?: unknown;
};

function snakeCase(s: string): string {
  return s.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
}

function pascalCase(s: string): string {
  return s.replace(/(^|[_-])([a-z0-9])/g, (_, __, c) => c.toUpperCase());
}

function singularize(s: string): string {
  return s.endsWith('s') ? s.slice(0, -1) : s;
}

interface RustStruct {
  name: string;
  fields: string[];
  helpers: string[];
}

/** A generated module accumulates structs (parent + nested) and default fns. */
class Emitter {
  readonly structs: RustStruct[] = [];
  private readonly seen = new Set<string>();

  addStruct(name: string): RustStruct | null {
    if (this.seen.has(name)) return null;
    this.seen.add(name);
    const s: RustStruct = { name, fields: [], helpers: [] };
    this.structs.push(s);
    return s;
  }
}

/** Map a JSON Schema property to a Rust type, generating nested structs as needed. */
function rustType(
  prop: JsonSchema,
  ctx: { emitter: Emitter; structName: string; field: string }
): string {
  switch (prop.type) {
    case 'string':
      return 'String';
    case 'boolean':
      return 'bool';
    case 'number':
      return 'f64';
    case 'integer':
      return prop.minimum !== undefined && prop.minimum >= 0 ? 'u32' : 'i64';
    case 'array': {
      const items = prop.items;
      if (items?.type === 'object' && items.properties) {
        const itemName = pascalCase(`${ctx.structName}_${singularize(ctx.field)}`);
        buildStruct(itemName, items, ctx.emitter);
        return `Vec<${itemName}>`;
      }
      const inner = items ? rustType(items, ctx) : 'serde_json::Value';
      return `Vec<${inner}>`;
    }
    case 'object': {
      // record / open map → opaque JSON
      if (!prop.properties) return 'serde_json::Value';
      const nestedName = pascalCase(`${ctx.structName}_${ctx.field}`);
      buildStruct(nestedName, prop, ctx.emitter);
      return nestedName;
    }
    default:
      return 'serde_json::Value';
  }
}

function rustDefault(prop: JsonSchema, ty: string): string {
  if (ty.startsWith('Vec<')) return 'Vec::new()';
  if (ty === 'String') return `${JSON.stringify(prop.default)}.to_string()`;
  if (ty === 'bool') return String(prop.default);
  // f64 literals must carry a decimal point (50 → 50.0).
  if (ty === 'f64' && Number.isInteger(prop.default)) return `${prop.default}.0`;
  return String(prop.default);
}

/**
 * Rust 2018+ keywords (incl. reserved). A snake_cased field that collides with one
 * must be emitted as a raw identifier `r#field` plus a `#[serde(rename = "key")]`
 * carrying the ORIGINAL camelCase key, so the wire shape is unaffected.
 */
const RUST_KEYWORDS = new Set([
  'as',
  'break',
  'const',
  'continue',
  'crate',
  'dyn',
  'else',
  'enum',
  'extern',
  'false',
  'fn',
  'for',
  'if',
  'impl',
  'in',
  'let',
  'loop',
  'match',
  'mod',
  'move',
  'mut',
  'pub',
  'ref',
  'return',
  'self',
  'Self',
  'static',
  'struct',
  'super',
  'trait',
  'true',
  'type',
  'unsafe',
  'use',
  'where',
  'while',
  'async',
  'await',
]);

/**
 * Resolve a snake_cased `field` (derived from the original camelCase `key`) to its
 * Rust identifier and an optional rename attribute. Keyword fields become raw
 * idents `r#field` and gain `#[serde(rename = "<originalKey>")]`. Used by all three
 * field branches so the name/rename routing stays consistent.
 */
function rustFieldName(field: string, key: string): { ident: string; renameAttr: string | null } {
  if (RUST_KEYWORDS.has(field)) {
    return { ident: `r#${field}`, renameAttr: `    #[serde(rename = ${JSON.stringify(key)})]` };
  }
  return { ident: field, renameAttr: null };
}

function buildStruct(
  name: string,
  schema: JsonSchema,
  emitter: Emitter,
  fieldOverrides: Record<string, string> = {}
): void {
  const struct = emitter.addStruct(name);
  if (!struct) return; // already built (dedup)
  const required = new Set(schema.required ?? []);

  for (const [key, prop] of Object.entries(schema.properties ?? {})) {
    const field = snakeCase(key);
    const override = fieldOverrides[key];
    const base = override ?? rustType(prop, { emitter, structName: name, field });
    const { ident, renameAttr } = rustFieldName(field, key);
    // A default only applies when the field is also required (create-style). In a
    // `.partial()` patch schema a defaulted field is optional → absent means "leave
    // unchanged", so it must be Option, not a forced default value.
    const useDefault = !override && 'default' in prop && required.has(key);

    if (useDefault) {
      const fn = `default_${snakeCase(name)}_${field}`;
      // Match rustfmt: a zero-arg signature wider than max_width (100) wraps the
      // empty param list onto its own line, so the generated file stays
      // `cargo fmt --check`-clean as well as `gen:ipc:check`-stable.
      const sig =
        `fn ${fn}() -> ${base} {`.length > 100
          ? `fn ${fn}(\n) -> ${base} {`
          : `fn ${fn}() -> ${base} {`;
      struct.helpers.push(`${sig}\n    ${rustDefault(prop, base)}\n}`);
      struct.fields.push(`    #[serde(default = "${fn}")]`);
      if (renameAttr) struct.fields.push(renameAttr);
      struct.fields.push(`    pub ${ident}: ${base},`);
    } else if (override || required.has(key)) {
      if (renameAttr) struct.fields.push(renameAttr);
      struct.fields.push(`    pub ${ident}: ${base},`);
    } else {
      struct.fields.push(`    #[serde(skip_serializing_if = "Option::is_none")]`);
      if (renameAttr) struct.fields.push(renameAttr);
      struct.fields.push(`    pub ${ident}: Option<${base}>,`);
    }
  }
}

function renderStruct(s: RustStruct): string {
  return [
    '#[derive(Debug, Clone, Deserialize, Serialize)]',
    '#[serde(rename_all = "camelCase")]',
    // IPC DTO: not every field is read on the Rust side.
    '#[allow(dead_code)]',
    `pub struct ${s.name} {`,
    ...s.fields,
    '}',
  ].join('\n');
}

function genModule(mod: ModuleSpec): string {
  const emitter = new Emitter();
  for (const spec of mod.structs) {
    const schema = z.toJSONSchema(spec.schema, { unrepresentable: 'any' }) as JsonSchema;
    if (schema.type !== 'object') {
      throw new Error(`${spec.rustName}: only object schemas are supported`);
    }
    buildStruct(spec.rustName, schema, emitter, spec.fieldOverrides);
  }

  const structs = emitter.structs.map(renderStruct);
  const helpers = emitter.structs.flatMap((s) => s.helpers);
  const body = [...structs, ...helpers].join('\n\n');

  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/schemas/index.ts',
    '',
    'use serde::{Deserialize, Serialize};',
    '',
    body,
    '',
  ].join('\n');
}

/** Generate the event-channel constants module from the shared EVENT_CHANNELS registry. */
function genEvents(): string {
  const lines: string[] = [];
  // Const name = SCREAMING_SNAKE of `<wire-namespace>_<key>`. The wire namespace
  // (the segment before `:` in the wire string) is the prefix — it can differ from
  // the registry key (e.g. the `shortcuts` namespace emits `shortcut:…`).
  for (const channels of Object.values(EVENT_CHANNELS)) {
    for (const [key, wire] of Object.entries(channels as Record<string, string>)) {
      const wireNs = wire.split(':')[0] ?? wire;
      const name = `${snakeCase(wireNs)}_${snakeCase(key)}`.toUpperCase();
      lines.push(`pub const ${name}: &str = ${JSON.stringify(wire)};`);
    }
  }
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/events/index.ts',
    '#![allow(dead_code)]',
    '',
    ...lines,
    '',
  ].join('\n');
}

/** Generate the provider credential-slot constants module from PROVIDER_SLOTS. */
function genSlots(): string {
  // Const name = SCREAMING_SNAKE of the camelCase key; value = the BARE slot
  // name. The `ai:` keyring namespace is applied Rust-side at read time, so it
  // is intentionally absent from these literals.
  const lines = Object.entries(PROVIDER_SLOTS).map(
    ([key, slot]) => `pub const ${snakeCase(key).toUpperCase()}: &str = ${JSON.stringify(slot)};`
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/provider-slots.ts',
    '#![allow(dead_code)]',
    '',
    ...lines,
    '',
  ].join('\n');
}

/** Generate the date-filter token list from the shared DATE_FILTER_OPTIONS. */
function genDateFilters(): string {
  // The Rust aggregator match arms (`adzuna_max_days_old` / `jsearch_date_posted`)
  // map each of these tokens to a provider-specific value, falling through to a
  // default for unknown tokens. Emitting the canonical list lets a Rust
  // exhaustiveness test fail if a new TS token isn't handled by both match arms.
  const list = DATE_FILTER_OPTIONS.map((t) => JSON.stringify(t)).join(', ');
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/schemas/index.ts',
    '#![allow(dead_code)]',
    '',
    `pub const DATE_FILTER_OPTIONS: &[&str] = &[${list}];`,
    '',
  ].join('\n');
}

const check = process.argv.includes('--check');
let stale = false;

// Unified output list: the Zod-derived struct modules plus the event-channel
// constants module (different source of truth: src/events/), written/checked by
// the same logic so `pnpm gen:ipc[:check]` covers both.
const outputs: { outFile: string; content: string }[] = [
  ...MODULES.map((mod) => ({ outFile: mod.outFile, content: genModule(mod) })),
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/events.rs',
    content: genEvents(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/provider_slots.rs',
    content: genSlots(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/date_filters.rs',
    content: genDateFilters(),
  },
];

for (const { outFile, content: next } of outputs) {
  const target = join(REPO_ROOT, outFile);
  if (check) {
    let current: string;
    try {
      current = readFileSync(target, 'utf8');
    } catch {
      current = '';
    }
    if (current !== next) {
      stale = true;
      console.error(`✗ stale: ${outFile} — run \`pnpm gen:ipc\``);
    }
  } else {
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, next);
    console.log(`✓ wrote ${outFile}`);
  }
}

if (check && stale) process.exit(1);
if (check) console.log('✓ IPC codegen output is up to date');
