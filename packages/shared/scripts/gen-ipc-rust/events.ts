import {
  EVENT_CHANNELS,
  PIPELINE_SECTION_EXPERIENCE_PREFIX,
  PIPELINE_SECTION_KEYS_FIXED,
  PIPELINE_STAGE_PHASES,
  PIPELINE_STAGES,
  PIPELINE_STAGES_FREE,
  SECTION_KEY_MAX_LENGTH,
} from '../../src/events/index.js';
import { generatedHeader, snakeCase, strSliceDecl } from './rust-literals.js';

/** Generate the event-channel constants module from the shared EVENT_CHANNELS registry. */
export function genEvents(): string {
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
  const phasesDecl = strSliceDecl('PIPELINE_STAGE_PHASES', PIPELINE_STAGE_PHASES);
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
  const sectionKeysDecl = strSliceDecl('PIPELINE_SECTION_KEYS_FIXED', PIPELINE_SECTION_KEYS_FIXED);
  // The stage vocabulary rides along for the same reason the phase vocabulary
  // does — it is part of the SAME `pipeline:stage` contract — plus a second
  // consumer that makes generating it load-bearing rather than tidy: the
  // per-stage model overrides key on these names, so a stage renamed on one
  // side and not the other would silently orphan a user's override.
  const stagesDecl = strSliceDecl('PIPELINE_STAGES', PIPELINE_STAGES);
  // The zero-provider-call subset rides along because the override table has to
  // refuse it, and that table lives in an L1 store that cannot reach the L2
  // pipeline the set is derived from.
  const freeStagesDecl = strSliceDecl('PIPELINE_STAGES_FREE', PIPELINE_STAGES_FREE);
  return [
    ...generatedHeader('packages/shared/src/events/index.ts'),
    ...lines,
    '',
    '/// Closed phase vocabulary for a `pipeline:stage` event, in lifecycle order.',
    '/// Source of truth: `PIPELINE_STAGE_PHASES` in',
    '/// packages/shared/src/events/pipeline.ts.',
    phasesDecl,
    '',
    '/// Every stage name the staged résumé pipeline can run, in pipeline order —',
    '/// pinned against `QUALITY_STAGES` (`pipeline/resume/mod.rs`) by',
    '/// `pipeline::resume::tests::pipeline_stages`. Source of truth: `PIPELINE_STAGES` in',
    '/// packages/shared/src/events/pipeline.ts.',
    '///',
    '/// NORMATIVE for `ai_stage_overrides`: a row whose `stage` is not in this',
    '/// slice must be REJECTED at write time and DROPPED at import time — an',
    '/// override on a stage that never runs is a setting the user cannot see the',
    '/// effect of, and a name from a tampered bundle must not become one.',
    stagesDecl,
    '',
    '/// The stages that make NO provider call — pinned against',
    '/// `Pipeline::free_stage_names()` by `pipeline::resume::tests::pipeline_stages`. Source of',
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
