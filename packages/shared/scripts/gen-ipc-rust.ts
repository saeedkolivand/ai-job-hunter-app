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

import {
  AiGenerateRequestSchema,
  AiGenerationSaveSchema,
  ApplyStartSchema,
  AutopilotCreateSchema,
  AutopilotUpdateSchema,
  ConversationSaveMessageSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  MatchResumeRequestSchema,
  ResumeExtractTextSchema,
  ScrapeBoardRequestSchema,
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
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/scrape.rs',
    structs: [
      { rustName: 'ScrapeBoardRequest', schema: ScrapeBoardRequestSchema },
      { rustName: 'ScrapeUrlRequest', schema: ScrapeUrlRequestSchema },
    ],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/ai.rs',
    structs: [
      { rustName: 'AiGenerateRequest', schema: AiGenerateRequestSchema },
      { rustName: 'AiEmbedRequest', schema: EmbedRequestSchema },
      { rustName: 'AiGenerationSaveRequest', schema: AiGenerationSaveSchema },
    ],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/documents.rs',
    structs: [
      {
        rustName: 'DocumentsImportRequest',
        schema: DocumentImportRequestSchema,
        fieldOverrides: { bytes: 'Vec<u8>' },
      },
    ],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/resume.rs',
    structs: [
      {
        rustName: 'ResumeExtractTextRequest',
        schema: ResumeExtractTextSchema,
        fieldOverrides: { bytes: 'Vec<u8>' },
      },
    ],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/autopilot.rs',
    structs: [
      { rustName: 'AutopilotCreateRequest', schema: AutopilotCreateSchema },
      { rustName: 'AutopilotUpdateRequest', schema: AutopilotUpdateSchema },
    ],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/apply.rs',
    structs: [{ rustName: 'ApplyStartRequest', schema: ApplyStartSchema }],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/matching.rs',
    structs: [{ rustName: 'MatchResumeRequest', schema: MatchResumeRequestSchema }],
  },
  {
    outFile: 'apps/tauri/src-tauri/src/ipc_contracts/conversations.rs',
    structs: [
      { rustName: 'ConversationSaveMessageRequest', schema: ConversationSaveMessageSchema },
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
    // A default only applies when the field is also required (create-style). In a
    // `.partial()` patch schema a defaulted field is optional → absent means "leave
    // unchanged", so it must be Option, not a forced default value.
    const useDefault = !override && 'default' in prop && required.has(key);

    if (useDefault) {
      const fn = `default_${snakeCase(name)}_${field}`;
      struct.helpers.push(`fn ${fn}() -> ${base} {\n    ${rustDefault(prop, base)}\n}`);
      struct.fields.push(`    #[serde(default = "${fn}")]`);
      struct.fields.push(`    pub ${field}: ${base},`);
    } else if (override || required.has(key)) {
      struct.fields.push(`    pub ${field}: ${base},`);
    } else {
      struct.fields.push(`    pub ${field}: Option<${base}>,`);
    }
  }
}

function renderStruct(s: RustStruct): string {
  return [
    '#[derive(Debug, Deserialize, Serialize)]',
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

const check = process.argv.includes('--check');
let stale = false;

for (const mod of MODULES) {
  const target = join(REPO_ROOT, mod.outFile);
  const next = genModule(mod);
  if (check) {
    let current: string;
    try {
      current = readFileSync(target, 'utf8');
    } catch {
      current = '';
    }
    if (current !== next) {
      stale = true;
      console.error(`✗ stale: ${mod.outFile} — run \`pnpm gen:ipc\``);
    }
  } else {
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, next);
    console.log(`✓ wrote ${mod.outFile}`);
  }
}

if (check && stale) process.exit(1);
if (check) console.log('✓ IPC codegen output is up to date');
