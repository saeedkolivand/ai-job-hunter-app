import { z } from 'zod';

import { snakeCase } from './rust-literals.js';

export interface StructSpec {
  rustName: string;
  schema: z.ZodType;
  /** Override the Rust type for specific fields (e.g. bytes the JSON Schema can't represent). */
  fieldOverrides?: Record<string, string>;
}

export interface ModuleSpec {
  /** Output file, relative to repo root. */
  outFile: string;
  structs: StructSpec[];
}

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

function pascalCase(s: string): string {
  return s.replace(/(^|[_-])([a-z0-9])/g, (_, __, c) => c.toUpperCase());
}

function singularize(s: string): string {
  // `-ies` before the bare `-s` rule: `entries` must yield `Entry`, not `Entrie`.
  // Only after a CONSONANT, which is the actual English rule: `movies`/`series`
  // are `-y + s`, not `-y → -ies`, so the blanket form turned them into `Movy`
  // and `Sery`. Only reached for an ARRAY-OF-OBJECT field, whose item struct is
  // named after it (see `rustType`), so this only ever renames generated item
  // structs.
  if (/[^aeiou]ies$/.test(s)) return `${s.slice(0, -3)}y`;
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

export function genModule(mod: ModuleSpec): string {
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
