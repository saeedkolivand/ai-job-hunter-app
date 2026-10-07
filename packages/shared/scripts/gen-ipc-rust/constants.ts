import {
  CONTEXT_WINDOW_DEFAULT,
  CONTEXT_WINDOW_MAX,
  CONTEXT_WINDOW_MIN,
} from '../../src/ai-context-window.js';
import {
  EFFORT_TIMEOUT_MULTIPLIER,
  OLLAMA_COMPLETION_BASELINE_SECS,
  QUALITY_RUN_FIXED_SECS,
  QUALITY_RUN_GENERATION_PASSES,
  QUALITY_RUN_JSON_STAGE_CALLS,
  STREAM_BASELINE_SECS,
  STREAM_CEILING_FACTOR,
} from '../../src/ai-timeouts.js';
import { PROVIDER_SLOTS } from '../../src/provider-slots.js';
import {
  AI_GENERATE_INTENTS,
  BOARD_IDS,
  DATE_FILTER_OPTIONS,
  GENERATION_DEPTHS,
  MATCH_TIER_CUTS,
} from '../../src/schemas/index.js';
import { constSliceDecl, generatedHeader, snakeCase, strSliceDecl } from './rust-literals.js';

const SCHEMAS_SOURCE = 'packages/shared/src/schemas/index.ts';

/** Generate the provider credential-slot constants module from PROVIDER_SLOTS. */
export function genSlots(): string {
  // Const name = SCREAMING_SNAKE of the camelCase key; value = the BARE slot
  // name. The `ai:` keyring namespace is applied Rust-side at read time, so it
  // is intentionally absent from these literals.
  const lines = Object.entries(PROVIDER_SLOTS).map(
    ([key, slot]) => `pub const ${snakeCase(key).toUpperCase()}: &str = ${JSON.stringify(slot)};`
  );
  return [...generatedHeader('packages/shared/src/provider-slots.ts'), ...lines, ''].join('\n');
}

/** Generate the date-filter token list from the shared DATE_FILTER_OPTIONS.
 *
 *  The Rust aggregator match arms (`adzuna_max_days_old` / `jsearch_date_posted`) map each of
 *  these tokens to a provider-specific value, falling through to a default for unknown tokens.
 *  Emitting the canonical list lets a Rust exhaustiveness test fail if a new TS token isn't
 *  handled by both match arms. */
export function genDateFilters(): string {
  return [
    ...generatedHeader(SCHEMAS_SOURCE),
    strSliceDecl('DATE_FILTER_OPTIONS', DATE_FILTER_OPTIONS),
    '',
  ].join('\n');
}

/** Generate the board-id catalog from the shared `BOARD_IDS`.
 *
 *  Unlike the other lists here, the Rust side ALREADY owns an authoritative
 *  copy of this vocabulary — `scraping::boards::SCRAPERS`, the registry the UI
 *  catalog is built from. So this const is not the source of truth; it exists
 *  purely so a Rust test can compare the two lists and fail when they disagree.
 *  They did: `jobicy` was a registered, listed board with translations and was
 *  absent from `BOARD_IDS` entirely, which no check on either side could see. */
export function genBoardIds(): string {
  return [...generatedHeader(SCHEMAS_SOURCE), strSliceDecl('BOARD_IDS', BOARD_IDS), ''].join('\n');
}

/** Generate the AI-generation intent vocabulary from the shared
 *  `AI_GENERATE_INTENTS` — same shape as `genDateFilters` above: one
 *  hand-typed literal list (the TS `const`) instead of two, so
 *  `resolve_intent`'s own Rust test (`commands/ai_provider/tests/chat_and_intent.rs`) can
 *  iterate the SAME vocabulary the wire schema accepts rather than a second,
 *  driftable copy. */
export function genAiIntents(): string {
  return [
    ...generatedHeader(SCHEMAS_SOURCE),
    strSliceDecl('AI_GENERATE_INTENTS', AI_GENERATE_INTENTS),
    '',
  ].join('\n');
}

/** Generate the context-window bounds from the shared `ai-context-window.ts` — one hand-typed
 *  table (the TS constants) instead of two, so the renderer slider and the Rust validator can
 *  never drift from each other without `pnpm gen:ipc:check` catching it. */
export function genContextWindowBounds(): string {
  return [
    ...generatedHeader('packages/shared/src/ai-context-window.ts'),
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

/** Generate the reasoning-effort → stream-timeout schedule from the shared
 *  `ai-timeouts.ts` — same shape as `genDateFilters`/`genAiIntents` above:
 *  one hand-typed table (the TS constants) instead of two independently
 *  hand-mirrored copies, so `timeouts.rs`'s `stream_deadline` and the
 *  renderer's `computeStreamTimeoutMs` (`renderer/lib/generate/
 *  stream-promise.ts`) can never drift from each other without
 *  `pnpm gen:ipc:check` catching it. */
export function genStreamTimeouts(): string {
  // f64 literals must carry a decimal point (2 → 2.0) — a bare integer
  // literal doesn't type-infer as f64 in Rust (see `rustDefault`).
  const tableDecl = constSliceDecl(
    'EFFORT_TIMEOUT_MULTIPLIER',
    '&[(&str, f64)]',
    Object.entries(EFFORT_TIMEOUT_MULTIPLIER).map(
      ([tier, mult]) => `("${tier}", ${Number.isInteger(mult) ? `${mult}.0` : mult})`
    )
  );
  return [
    ...generatedHeader('packages/shared/src/ai-timeouts.ts'),
    `pub const STREAM_BASELINE_SECS: u64 = ${STREAM_BASELINE_SECS};`,
    '',
    '/// Absolute backstop on one provider call, as a multiple of its idle bound —',
    '/// see `timeouts::stream_ceiling`.',
    `pub const STREAM_CEILING_FACTOR: u32 = ${STREAM_CEILING_FACTOR};`,
    '',
    '/// Baseline NON-streaming completion deadline — the local-Ollama analogue',
    '/// of `STREAM_BASELINE_SECS`, scaled the same way by',
    '/// `timeouts::ollama_completion_deadline`. A separate constant from',
    '/// `STREAM_BASELINE_SECS` even though the two share a value today — they',
    '/// bound different operations, so they can drift independently later.',
    `pub const OLLAMA_COMPLETION_BASELINE_SECS: u64 = ${OLLAMA_COMPLETION_BASELINE_SECS};`,
    '',
    '/// Ascending tier order — see the source-of-truth doc comment for why that',
    '/// matters (`max` is the TOP tier, not `xhigh`). Any tier not listed here',
    '/// (including `None`) gets an implicit 1.0 multiplier. Shared by both',
    '/// `stream_deadline` and `ollama_completion_deadline`.',
    tableDecl,
    '',
    '/// How many non-streaming round-trips `analyze_job`/`strategy` make in the',
    '/// WORST case: 2 stages × (1 call + 1 allowed re-ask), each through',
    '/// `Completer::complete_json`. `match_evidence` is deliberately absent: it',
    '/// selects evidence from the source résumé in pure Rust and makes no',
    '/// provider call.',
    `pub const QUALITY_RUN_JSON_STAGE_CALLS: u64 = ${QUALITY_RUN_JSON_STAGE_CALLS};`,
    '',
    "/// The BASELINE-tier share of one quality-depth run's deadline: the repair",
    '/// fan-out (`max_repair_attempts` rounds × `MAX_SECTIONS_PER_ROUND` sections)',
    "/// and `humanize`'s up to 2 flagged-document calls, each bounded by",
    '/// `OLLAMA_COMPLETION_BASELINE_SECS`. `quality_run_deadline` scales it by the',
    "/// effort multiplier — both stages send the run's effort. See",
    '/// `qualityRunDeadlineSecs` in packages/shared/src/ai-timeouts.ts for the full',
    '/// derivation, including why the two JSON stages are NOT in this term.',
    `pub const QUALITY_RUN_FIXED_SECS: u64 = ${QUALITY_RUN_FIXED_SECS};`,
    '',
    '/// Effort-SCALED whole-document passes one quality run may make: two — the',
    '/// draft, and the cover letter when `includeCoverLetter` is set. The repair',
    '/// rounds and `humanize` are non-streaming and live in',
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
export function genGenerationDepths(): string {
  return [
    ...generatedHeader(SCHEMAS_SOURCE),
    '/// Historic `pipeline_runs.depth`/`QualityReport.pipeline` values — closed',
    '/// for READING, not for a request field. The wire request carries no',
    '/// `depth` field; every new run persists the fixed value `quality`. `fast`',
    "/// (the renderer's own deterministic pass) and `max` (a removed staged",
    '/// depth) remain in this constant only so a historic row still round-trips.',
    strSliceDecl('GENERATION_DEPTHS', GENERATION_DEPTHS),
    '',
  ].join('\n');
}

/** Generate the match-score band cut points from the shared `MATCH_TIER_CUTS`
 *  — same shape as `genContextWindowBounds` above: one hand-typed table of
 *  related numeric constants (not a Zod object schema, so it isn't a
 *  `MODULES` entry) flattened to four `pub const`s so
 *  `commands::autopilot::best_matches` can read a qualification bar without
 *  hardcoding it. Source of truth: `MATCH_TIER_CUTS` in
 *  packages/shared/src/schemas/index.ts. */
export function genMatchTiers(): string {
  // A Rust `f64` literal needs a decimal point, and `${55.5}.0` would emit the
  // invalid `55.5.0`. `toFixed(1)` is correct for an integer and a fractional
  // cut point alike.
  const f64 = (n: number) => n.toFixed(1);
  // The four consts below are hand-named so each carries its own doc comment,
  // which means a NEW variant on `MATCH_TIER_CUTS` would be emitted nowhere and
  // silently never reach Rust — `gen:ipc:check` regenerates from this same
  // function, so it would compare the omission against itself and stay green.
  // Assert the shape instead: gaining or losing a variant fails the generator
  // until both this emitter and the Rust reader are updated.
  const variants = Object.keys(MATCH_TIER_CUTS).sort().join(',');
  if (variants !== 'combined,coverage') {
    throw new Error(
      `MATCH_TIER_CUTS variants changed (${variants}) — update genMatchTiers() and the reader in commands/autopilot/best_matches.rs`
    );
  }
  return [
    ...generatedHeader(SCHEMAS_SOURCE),
    "/// `coverage` High cut — the embedding-free keyword-coverage kernel's",
    '/// qualification bar. ponytail: heuristic starting values, not calibrated.',
    `pub const MATCH_TIER_COVERAGE_HIGH: f64 = ${f64(MATCH_TIER_CUTS.coverage.high)};`,
    '',
    '/// `coverage` Medium cut.',
    `pub const MATCH_TIER_COVERAGE_MEDIUM: f64 = ${f64(MATCH_TIER_CUTS.coverage.medium)};`,
    '',
    "/// `combined` High cut — the semantic+ATS kernel's qualification bar.",
    `pub const MATCH_TIER_COMBINED_HIGH: f64 = ${f64(MATCH_TIER_CUTS.combined.high)};`,
    '',
    '/// `combined` Medium cut.',
    `pub const MATCH_TIER_COMBINED_MEDIUM: f64 = ${f64(MATCH_TIER_CUTS.combined.medium)};`,
    '',
  ].join('\n');
}
