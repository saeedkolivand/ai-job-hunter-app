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

import { AGENT_RESUME_TEXT_CAP } from '../src/agent-caps.js';
import {
  CONTEXT_WINDOW_DEFAULT,
  CONTEXT_WINDOW_MAX,
  CONTEXT_WINDOW_MIN,
} from '../src/ai-context-window.js';
import {
  EFFORT_TIMEOUT_MULTIPLIER,
  QUALITY_RUN_FIXED_SECS,
  QUALITY_RUN_GENERATION_PASSES,
  STREAM_BASELINE_SECS,
} from '../src/ai-timeouts.js';
import {
  EVENT_CHANNELS,
  PIPELINE_SECTION_EXPERIENCE_PREFIX,
  PIPELINE_SECTION_KEYS_FIXED,
  PIPELINE_STAGE_PHASES,
  PIPELINE_STAGES,
  PIPELINE_STAGES_FREE,
  SECTION_KEY_MAX_LENGTH,
} from '../src/events/index.js';
import { PROVIDER_SLOTS } from '../src/provider-slots.js';
import {
  AGENT_FLOW_KINDS,
  AgentConfirmRequestSchema,
  AgentRunRequestSchema,
  AI_GENERATE_INTENTS,
  AiGenerateRequestSchema,
  AiGenerationSaveSchema,
  AiGenerationUpdateSchema,
  AiStreamChunkSchema,
  ApplicationTrackSchema,
  ApplicationUpdateSchema,
  AutopilotCreateSchema,
  AutopilotUpdateSchema,
  DATE_FILTER_OPTIONS,
  DedupMarkNotDuplicateRequestSchema,
  DiscoverySearchRequestSchema,
  DiscoveryStarRequestSchema,
  DocumentImportRequestSchema,
  EmbedRequestSchema,
  GENERATION_DEPTHS,
  JobEventSchema,
  MatchResumeRequestSchema,
  ReferralUpsertSchema,
  ResumeExtractTextSchema,
  ResumePipelineRegenerateSectionSchema,
  ResumePipelineResolveFabricationSchema,
  ResumePipelineRunSchema,
  ResumeTrimSuggestionsRequestSchema,
  ResumeValidateContentSchema,
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
    structs: [
      { rustName: 'AgentRunRequest', schema: AgentRunRequestSchema },
      { rustName: 'AgentConfirmRequest', schema: AgentConfirmRequestSchema },
    ],
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
      { rustName: 'ResumeValidateContentRequest', schema: ResumeValidateContentSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/resume_pipeline.rs',
    structs: [
      { rustName: 'ResumePipelineRunRequest', schema: ResumePipelineRunSchema },
      {
        rustName: 'ResumePipelineRegenerateSectionRequest',
        schema: ResumePipelineRegenerateSectionSchema,
      },
      {
        rustName: 'ResumePipelineResolveFabricationRequest',
        schema: ResumePipelineResolveFabricationSchema,
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
      {
        rustName: 'ResumeTrimSuggestionsRequest',
        schema: ResumeTrimSuggestionsRequestSchema,
      },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/dedup.rs',
    structs: [
      { rustName: 'DedupMarkNotDuplicateRequest', schema: DedupMarkNotDuplicateRequestSchema },
    ],
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/discovery.rs',
    structs: [
      { rustName: 'DiscoverySearchRequest', schema: DiscoverySearchRequestSchema },
      { rustName: 'DiscoveryStarRequest', schema: DiscoveryStarRequestSchema },
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

/** rustfmt `max_width` — the column no emitted line may exceed. */
const RUST_MAX_WIDTH = 100;
/** rustfmt `array_width` under the default `use_small_heuristics`: the widest an
 *  array literal's CONTENT (items + their `, ` separators, brackets excluded)
 *  may be before the literal is laid out as a block. */
const RUST_ARRAY_WIDTH = 60;

/**
 * Emit `pub const <name>: <ty> = &[…];` in the form rustfmt already agrees with,
 * for ANY number of items — so a generated file stays `cargo fmt --check`-clean
 * and `pnpm gen:ipc:check`-stable however large a vocabulary grows.
 *
 * Verified against stable rustfmt 1.9, which decides in two independent steps:
 *
 * 1. the literal stays HORIZONTAL only while its content fits
 *    {@link RUST_ARRAY_WIDTH} — this is the step the previous per-generator
 *    heuristics missed, and it bites BEFORE {@link RUST_MAX_WIDTH} does;
 * 2. a horizontal literal whose declaration line would exceed
 *    {@link RUST_MAX_WIDTH} moves to its own 4-space-indented line (which always
 *    fits, being at most `RUST_ARRAY_WIDTH + 8` columns).
 *
 * Past `array_width` the literal becomes a BLOCK whose internal layout depends
 * on the widest element (`short_array_element_width_threshold`): rustfmt PACKS
 * short elements several per line and splits longer ones one per line. Rather
 * than re-derive that packing — the guess this helper exists to stop making —
 * the block form is emitted one element per line under `#[rustfmt::skip]`,
 * which rustfmt reproduces verbatim. A one-per-line block WITHOUT the attribute
 * would be repacked and fail `cargo fmt --check`.
 */
function constSliceDecl(name: string, ty: string, items: string[]): string {
  const inline = items.join(', ');
  if (inline.length <= RUST_ARRAY_WIDTH) {
    const singleLine = `pub const ${name}: ${ty} = &[${inline}];`;
    return singleLine.length <= RUST_MAX_WIDTH
      ? singleLine
      : `pub const ${name}: ${ty} =\n    &[${inline}];`;
  }
  return [
    '#[rustfmt::skip]',
    `pub const ${name}: ${ty} = &[`,
    ...items.map((item) => `    ${item},`),
    '];',
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
  // The `pipeline:stage` phase vocabulary rides along in this module because it
  // is part of the SAME contract as the channel name: same shape as
  // `genDateFilters`/`genAiIntents` below — one hand-typed list (the TS const)
  // instead of two, so the Phase-3 Rust emitter and any `RunEventRow.phase`
  // validation check against the vocabulary the renderer's `PipelineStagePhase`
  // is derived from, not a second copy that drifts the first time a phase is
  // added. Payload STRUCTS stay hand-synced (Phase-3 scope); this is the closed
  // vocabulary only.
  const phasesDecl = constSliceDecl(
    'PIPELINE_STAGE_PHASES',
    '&[&str]',
    PIPELINE_STAGE_PHASES.map((p) => JSON.stringify(p))
  );
  // The `sectionKey` grammar rides along for the same reason as the phase
  // vocabulary: it is part of the SAME `pipeline:stage` contract, it is
  // documented as NORMATIVE for the Phase-3 Rust emitter, and the only way a
  // normative bound stays normative is if the Rust side reads it from here
  // instead of re-typing it.
  //
  // The consts alone were NOT enough: the emitted doc told the Phase-3 emitter to
  // `parse` the index as a `u8`, and Rust's `str::parse::<u8>` is LOOSER than the
  // TS grammar — it accepts `+1` and `007`, so `experience:01` would have reached
  // a wire whose vocabulary is supposed to be closed. The guard is therefore
  // generated too (`is_pipeline_section_key`), next to the consts it enforces, so
  // the two sides can't disagree about what "a decimal u8" means.
  const sectionKeysDecl = constSliceDecl(
    'PIPELINE_SECTION_KEYS_FIXED',
    '&[&str]',
    PIPELINE_SECTION_KEYS_FIXED.map((k) => JSON.stringify(k))
  );
  // The stage vocabulary rides along for the same reason the phase vocabulary
  // does — it is part of the SAME `pipeline:stage` contract — plus a second
  // consumer that makes generating it load-bearing rather than tidy: the
  // per-stage model overrides key on these names, so a stage renamed on one
  // side and not the other would silently orphan a user's override.
  const stagesDecl = constSliceDecl(
    'PIPELINE_STAGES',
    '&[&str]',
    PIPELINE_STAGES.map((s) => JSON.stringify(s))
  );
  // The zero-provider-call subset rides along because the override table has to
  // refuse it, and that table lives in an L1 store that cannot reach the L2
  // pipeline the set is derived from.
  const freeStagesDecl = constSliceDecl(
    'PIPELINE_STAGES_FREE',
    '&[&str]',
    PIPELINE_STAGES_FREE.map((s) => JSON.stringify(s))
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/events/index.ts',
    '#![allow(dead_code)]',
    '',
    ...lines,
    '',
    '/// Closed phase vocabulary for a `pipeline:stage` event, in lifecycle order.',
    '/// Source of truth: `PIPELINE_STAGE_PHASES` in',
    '/// packages/shared/src/events/pipeline.ts.',
    phasesDecl,
    '',
    '/// Every stage name the staged résumé pipeline can run, in pipeline order —',
    '/// pinned against `QUALITY_STAGES` (`pipeline/resume/mod.rs`) by',
    '/// `pipeline::resume::test`. Source of truth: `PIPELINE_STAGES` in',
    '/// packages/shared/src/events/pipeline.ts.',
    '///',
    '/// NORMATIVE for `ai_stage_overrides`: a row whose `stage` is not in this',
    '/// slice must be REJECTED at write time and DROPPED at import time — an',
    '/// override on a stage that never runs is a setting the user cannot see the',
    '/// effect of, and a name from a tampered bundle must not become one.',
    stagesDecl,
    '',
    '/// The stages that make NO provider call — pinned against',
    '/// `Pipeline::free_stage_names()` by `pipeline::resume::test`. Source of',
    '/// truth: `PIPELINE_STAGES_FREE` in packages/shared/src/events/pipeline.ts.',
    '///',
    '/// NORMATIVE for `ai_stage_overrides`: a row on one of these must be',
    '/// REJECTED at write time and DROPPED at import time. There is no model to',
    '/// choose (the stage asks none), so the setting would be inert — and a',
    '/// malformed row on a stage that never calls a provider must not be able to',
    '/// fail a whole run at resolve time.',
    freeStagesDecl,
    '',
    "/// Longest a `pipeline:stage` event's `sectionKey` may be, in UTF-16 code",
    '/// units (the unit the TS guard counts). Every LEGAL key is ASCII, so bytes,',
    '/// chars and UTF-16 units agree for anything that could pass the grammar; a',
    '/// byte-length check on a hostile value is only ever STRICTER, and such a',
    '/// value fails the grammar regardless.',
    '///',
    '/// NORMATIVE: an over-length `sectionKey` must be REJECTED, never truncated —',
    '/// a truncated key names a different section.',
    `pub const SECTION_KEY_MAX_LENGTH: usize = ${SECTION_KEY_MAX_LENGTH};`,
    '',
    '/// The `sectionKey` values that carry no index — the fixed half of the closed',
    '/// grammar (`summary` | `skills` | `experience:<u8>` | `projects` |',
    '/// `education`). Source of truth: `PIPELINE_SECTION_KEYS_FIXED` in',
    '/// packages/shared/src/events/pipeline.ts.',
    sectionKeysDecl,
    '',
    '/// Prefix of the indexed half: `experience:` followed by the CANONICAL decimal',
    '/// form of a `u8` — ASCII digits only, no sign, no whitespace, and no leading',
    '/// zeros (`0` itself is legal). Source of truth:',
    '/// `PIPELINE_SECTION_EXPERIENCE_PREFIX` in packages/shared/src/events/pipeline.ts.',
    `pub const PIPELINE_SECTION_EXPERIENCE_PREFIX: &str = ${JSON.stringify(
      PIPELINE_SECTION_EXPERIENCE_PREFIX
    )};`,
    '',
    '/// Runtime guard for a `pipeline:stage` `sectionKey` — the Rust twin of the TS',
    '/// `isPipelineSectionKey`, checked in the same order: length first (so a hostile',
    '/// value is rejected before any further work), then the fixed half, then the',
    '/// indexed half.',
    '///',
    '/// NORMATIVE: a `sectionKey` that fails this must never reach the wire.',
    '///',
    '/// The index is validated as canonical ASCII decimal BEFORE it is parsed,',
    '/// because `str::parse::<u8>` is LOOSER than the grammar: it accepts `+1` and',
    '/// `007`, which the TS regex `^(0|[1-9][0-9]{0,2})$` rejects. A bare parse would',
    '/// let `experience:01` — a second spelling of `experience:1` — onto a wire whose',
    '/// vocabulary is supposed to be closed.',
    'pub fn is_pipeline_section_key(value: &str) -> bool {',
    '    // Bytes, not UTF-16 units: every LEGAL key is ASCII so the two agree on',
    '    // anything that could pass, and a byte count is only ever stricter.',
    '    if value.len() > SECTION_KEY_MAX_LENGTH {',
    '        return false;',
    '    }',
    '    if PIPELINE_SECTION_KEYS_FIXED.contains(&value) {',
    '        return true;',
    '    }',
    '    let Some(index) = value.strip_prefix(PIPELINE_SECTION_EXPERIENCE_PREFIX) else {',
    '        return false;',
    '    };',
    '    if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {',
    '        return false;',
    '    }',
    "    if index.len() > 1 && index.starts_with('0') {",
    '        return false;',
    '    }',
    '    // Only now is a parse safe to trust: it contributes the `<= 255` bound the',
    '    // TS guard applies with `Number(index) <= 255`.',
    '    index.parse::<u8>().is_ok()',
    '}',
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
  const decl = constSliceDecl(
    'DATE_FILTER_OPTIONS',
    '&[&str]',
    DATE_FILTER_OPTIONS.map((t) => JSON.stringify(t))
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/schemas/index.ts',
    '#![allow(dead_code)]',
    '',
    decl,
    '',
  ].join('\n');
}

/** Generate the AI-generation intent vocabulary from the shared
 *  `AI_GENERATE_INTENTS` — same shape as `genDateFilters` above: one
 *  hand-typed literal list (the TS `const`) instead of two, so
 *  `resolve_intent`'s own Rust test (`commands/ai_provider/mod.rs`) can
 *  iterate the SAME vocabulary the wire schema accepts rather than a second,
 *  driftable copy. */
function genAiIntents(): string {
  const decl = constSliceDecl(
    'AI_GENERATE_INTENTS',
    '&[&str]',
    AI_GENERATE_INTENTS.map((t) => JSON.stringify(t))
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/schemas/index.ts',
    '#![allow(dead_code)]',
    '',
    decl,
    '',
  ].join('\n');
}

/** Generate the reasoning-effort → stream-timeout schedule from the shared
 *  `ai-timeouts.ts` — same shape as `genDateFilters`/`genAiIntents` above:
 *  one hand-typed table (the TS constants) instead of two independently
 *  hand-mirrored copies, so `timeouts.rs`'s `stream_deadline` and the
 *  renderer's `computeStreamTimeoutMs` (`renderer/lib/generate/
 *  stream-promise.ts`) can never drift from each other without
 *  `pnpm gen:ipc:check` catching it. */
function genContextWindowBounds(): string {
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/ai-context-window.ts',
    '#![allow(dead_code)]',
    '',
    '/// The smallest `num_ctx` worth sending: below this a window cannot hold a',
    '/// system prompt plus any useful input, so the call is guaranteed to',
    '/// truncate. Generated so the renderer slider and this validator cannot',
    '/// disagree — see the source-of-truth module for the full rationale.',
    `pub const MIN_CONTEXT_WINDOW: u32 = ${CONTEXT_WINDOW_MIN};`,
    '',
    '/// The largest: past this the request stops being a size and becomes an',
    "/// out-of-memory kill of the user's machine, because Ollama allocates",
    '/// `num_ctx` up front.',
    `pub const MAX_CONTEXT_WINDOW: u32 = ${CONTEXT_WINDOW_MAX};`,
    '',
    '/// What a PICKER starts at when the user has set nothing. The backend never',
    '/// substitutes this: an absent window means the provider’s own default.',
    `pub const DEFAULT_CONTEXT_WINDOW: u32 = ${CONTEXT_WINDOW_DEFAULT};`,
    '',
  ].join('\n');
}

function genStreamTimeouts(): string {
  // f64 literals must carry a decimal point (2 → 2.0) — a bare integer
  // literal doesn't type-infer as f64 in Rust (see `rustDefault` above).
  const tableDecl = constSliceDecl(
    'EFFORT_TIMEOUT_MULTIPLIER',
    '&[(&str, f64)]',
    Object.entries(EFFORT_TIMEOUT_MULTIPLIER).map(
      ([tier, mult]) => `("${tier}", ${Number.isInteger(mult) ? `${mult}.0` : mult})`
    )
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/ai-timeouts.ts',
    '#![allow(dead_code)]',
    '',
    `pub const STREAM_BASELINE_SECS: u64 = ${STREAM_BASELINE_SECS};`,
    '',
    '/// Ascending tier order — see the source-of-truth doc comment for why that',
    '/// matters (`max` is the TOP tier, not `xhigh`). Any tier not listed here',
    '/// (including `None`) gets an implicit 1.0 multiplier.',
    tableDecl,
    '',
    "/// The EFFORT-INVARIANT half of one quality-depth run's deadline: every call",
    '/// whose per-call bound is FLAT — the three JSON stages (each allowed one',
    '/// re-ask), the repair fan-out (`max_repair_attempts` rounds ×',
    "/// `MAX_SECTIONS_PER_ROUND` sections), and `humanize`'s up to 2",
    '/// flagged-document calls, all bounded by `timeouts::OLLAMA_COMPLETION`.',
    '/// See `qualityRunDeadlineSecs` in packages/shared/src/ai-timeouts.ts for',
    '/// the full derivation.',
    `pub const QUALITY_RUN_FIXED_SECS: u64 = ${QUALITY_RUN_FIXED_SECS};`,
    '',
    '/// Effort-SCALED whole-document passes one quality run may make: two — the',
    '/// draft, and the cover letter when `includeCoverLetter` is set. The repair',
    '/// rounds and `humanize` are flat-bounded and live in',
    '/// `QUALITY_RUN_FIXED_SECS` instead.',
    `pub const QUALITY_RUN_GENERATION_PASSES: u64 = ${QUALITY_RUN_GENERATION_PASSES};`,
    '',
  ].join('\n');
}

/** Generate the historic `GenerationDepth` vocabulary — same shape as
 *  `genDateFilters`/`genAiIntents`: one hand-typed list (the TS `const`)
 *  instead of two. There is no Rust `GenerationDepth` enum any more (the `max`
 *  depth's own deletion) and no request field to type against it — this is
 *  read-side only now, e.g. `agent_save_pipeline`'s own membership check. */
function genGenerationDepths(): string {
  const decl = constSliceDecl(
    'GENERATION_DEPTHS',
    '&[&str]',
    GENERATION_DEPTHS.map((d) => JSON.stringify(d))
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/schemas/index.ts',
    '#![allow(dead_code)]',
    '',
    '/// Historic `pipeline_runs.depth`/`QualityReport.pipeline` values — closed',
    '/// for READING, not for a request field. The wire request carries no',
    '/// `depth` field; every new run persists the fixed value `quality`. `fast`',
    "/// (the renderer's own deterministic pass) and `max` (a removed staged",
    '/// depth) remain in this constant only so a historic row still round-trips.',
    decl,
    '',
  ].join('\n');
}

/** Generate the agent-flow `kind` vocabulary — same shape as `genGenerationDepths`
 *  above, and load-bearing for the same reason: the Rust `AgentFlow` registry
 *  (`agent::flows::FLOWS`) is keyed on these tokens, so emitting them lets a Rust
 *  test assert the registry covers exactly the vocabulary the wire schema accepts.
 *  Without it, a kind added to the `z.enum` and to no flow would pass `gen:ipc`
 *  and fail at run time as "unknown agent flow" — a shipped dead option. */
function genAgentFlowKinds(): string {
  const decl = constSliceDecl(
    'AGENT_FLOW_KINDS',
    '&[&str]',
    AGENT_FLOW_KINDS.map((k) => JSON.stringify(k))
  );
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/schemas/index.ts',
    '#![allow(dead_code)]',
    '',
    '/// Every `AgentRunRequest.kind` the wire accepts. The FIRST entry is the',
    '/// serde default (`prep_application`), and `agent::flows::FLOWS` must carry',
    '/// exactly one flow per token — pinned by',
    '/// `agent::flows::tests::the_registry_covers_the_whole_wire_vocabulary`.',
    decl,
    '',
  ].join('\n');
}

/** Generate the agent text caps — same shape and same reason as
 *  `genStreamTimeouts`: the Rust fence cap and the renderer's review threshold
 *  are ONE number, and a hand-copied second literal is the drift this generator
 *  exists to stop. */
function genAgentCaps(): string {
  return [
    '// @generated by `pnpm gen:ipc` — DO NOT EDIT BY HAND.',
    '// Source of truth: packages/shared/src/agent-caps.ts',
    '#![allow(dead_code)]',
    '',
    '/// Longest résumé text, in CHARACTERS, that may cross into an agent run:',
    "/// the seed fence's clamp, the longest generation the `improve_resume` flow",
    '/// can read, and the renderer threshold that disables the entry point. See',
    '/// `AGENT_RESUME_TEXT_CAP` in packages/shared/src/agent-caps.ts for why the',
    '/// three have to be one number.',
    `pub const AGENT_RESUME_TEXT_CAP: usize = ${AGENT_RESUME_TEXT_CAP};`,
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
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/ai_intents.rs',
    content: genAiIntents(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/ai_timeouts.rs',
    content: genStreamTimeouts(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/context_window.rs',
    content: genContextWindowBounds(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/generation_depths.rs',
    content: genGenerationDepths(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/agent_flow_kinds.rs',
    content: genAgentFlowKinds(),
  },
  {
    outFile: 'apps/desktop/src-tauri/src/ipc_contracts/agent_caps.rs',
    content: genAgentCaps(),
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
