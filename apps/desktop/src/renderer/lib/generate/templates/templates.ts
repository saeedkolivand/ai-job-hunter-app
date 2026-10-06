// Document templates — the picker's source of truth. The backend renders from a
// canonical Rust `Template` registry keyed by `id`; only the `id` is sent over
// IPC (see `BaseExportRequest.templateId`), so the colour/size fields here are
// display metadata for the picker, kept consistent with the Rust template.
//
// The id set MUST match the Rust `TemplateId` enum (export/types.rs) — a guard
// test pins the two. `TemplateId` itself is the shared IPC contract's union (not
// a hand-synced local copy) so a `tsc` failure surfaces the moment either side
// adds an id the other doesn't know about, instead of silently compiling as a
// subset.
// `LetterLayoutId` gets the same treatment for the same reason: it was a
// hand-synced copy of the shared union, so adding a layout on one side and not
// the other compiled silently as a subset. Sourced from the contract now, which
// is the single registration point the picker and the backend both answer to.
import type { LetterLayoutId, TemplateId } from '@ajh/shared';

import { ATS_TEMPLATES } from './ats-templates';
import { DESIGN_TEMPLATES } from './design-templates';
import type { DocTemplate } from './doc-template';

export type { LetterLayoutId, TemplateId };

/**
 * Ordered letter-layout ids — the picker's option order. `satisfies` checks
 * membership only; completeness is enforced by `LETTER_LAYOUT_DECORATED`
 * (`Record<LetterLayoutId, boolean>`) and the picker's registration test.
 */
export const LETTER_LAYOUT_IDS = [
  'classic',
  'refined',
  'banded',
  'navy',
  'sidebar',
  'monogram',
] as const satisfies readonly LetterLayoutId[];

/** Picker order (the two unfiltered dropdowns render it as-is). */
const TEMPLATE_ORDER = [
  'classic',
  'swiss-minimal',
  'academic',
  'atelier',
  'meridian',
  'throughline',
  'portrait',
  'lebenslauf',
  'cadence',
  'cologne-navy',
  'regent',
  'aria',
  'saffron',
  'jake',
  'awesome',
  'deedy',
] as const satisfies readonly TemplateId[];

// Merging the tier halves keeps tsc failing when a TemplateId has no entry; the
// ordered map below then restores picker order (spread order would put ATS first).
const MERGED: Record<TemplateId, DocTemplate> = { ...ATS_TEMPLATES, ...DESIGN_TEMPLATES };

export const TEMPLATES = Object.fromEntries(TEMPLATE_ORDER.map((id) => [id, MERGED[id]])) as Record<
  TemplateId,
  DocTemplate
>;

/** Stable list of all template ids (kebab-case on the wire). */
export const TEMPLATE_IDS = Object.keys(TEMPLATES) as TemplateId[];

/**
 * Templates with a true two-column layout that collapses to a single column under
 * ATS mode — mirrors the backend `theme::is_two_column`. The ATS toggle + the
 * recommendation auto-apply key off this rather than a hardcoded id.
 */
const TWO_COLUMN_TEMPLATE_IDS = new Set<TemplateId>(['atelier', 'portrait', 'aria', 'saffron']);

export function isTwoColumnTemplate(id: TemplateId): boolean {
  return TWO_COLUMN_TEMPLATE_IDS.has(id);
}

/**
 * Design-tier templates that render a photo — mirrors the Rust template docs
 * (Portrait/Lebenslauf/Aria/Saffron are the "Phase 3b-i / PR4 photo templates").
 * Drives which ATS-mode hint copy is accurate: a design template that is
 * neither two-column nor photo-bearing (Awesome, Deedy) drops decorative
 * accent styling instead of a photo, so it needs its own hint key.
 */
const PHOTO_TEMPLATE_IDS = new Set<TemplateId>(['portrait', 'lebenslauf', 'aria', 'saffron']);

export function isPhotoTemplate(id: TemplateId): boolean {
  return PHOTO_TEMPLATE_IDS.has(id);
}

export type AtsModeHintKey =
  | 'aiGenerate.atsModeHintTwoColumn'
  | 'aiGenerate.atsModeHintPhoto'
  | 'aiGenerate.atsModeHintDecorative';

/**
 * Which ATS-mode hint key accurately describes what the toggle does for this
 * design-tier template: two-column layouts collapse to one column (dropping
 * any photo along the way); a photo-only template just loses the photo;
 * everything else in the design tier (Awesome, Deedy) has no photo and no
 * columns to collapse — it drops decorative accent styling instead. Single
 * source of truth so the two call sites (StepTemplate, GenerationOutput)
 * can't drift out of sync with each other or with what the template actually
 * does under ATS mode.
 */
export function atsModeHintKey(id: TemplateId): AtsModeHintKey {
  if (isTwoColumnTemplate(id)) return 'aiGenerate.atsModeHintTwoColumn';
  if (isPhotoTemplate(id)) return 'aiGenerate.atsModeHintPhoto';
  return 'aiGenerate.atsModeHintDecorative';
}

/**
 * Design-tier templates (photo / two-column / visually rich) — mirrors the Rust
 * `TemplateTier::Design`. Drives the gallery's Design section and the ATS-mode
 * toggle gate: design layouts drop the photo and/or linearize under ATS mode,
 * so the toggle is surfaced for all of them (incl. single-column-with-photo
 * templates like Lebenslauf that `isTwoColumnTemplate` deliberately excludes).
 */
export function isDesignTier(id: TemplateId): boolean {
  return TEMPLATES[id].tier === 'design';
}

/**
 * Whether ATS mode visibly changes each letter layout — i.e. whether the layout
 * carries a decoration the `.typ` drops when `data.opts.ats` is true. Mirrors the
 * `ats` gates in the letter templates one-for-one:
 *
 * - `banded` — the accent band across the top (`letter_banded.typ`).
 * - `sidebar` — the tinted contact rail in the widened left margin (`letter_sidebar.typ`).
 * - `monogram` — the initials tile, which extraction reads as two characters of
 *   noise in front of the candidate's own name ("JS Jane Smith") (`letter_monogram.typ`).
 * - `classic` / `refined` / `navy` — plain text and rules only; their `.typ`
 *   files contain no `ats` gate, so the toggle would be a no-op (and a lie) there.
 *
 * A `Record<LetterLayoutId, boolean>` rather than an id Set: adding a layout id
 * fails `tsc` until someone decides whether it degrades, which is the same
 * discipline `LETTER_LAYOUT_IDS` applies to the picker.
 */
const LETTER_LAYOUT_DECORATED = {
  classic: false,
  refined: false,
  banded: true,
  navy: false,
  sidebar: true,
  monogram: true,
} as const satisfies Record<LetterLayoutId, boolean>;

/**
 * True when the chosen letter layout has a decoration ATS mode drops, so the
 * ATS toggle is worth surfacing on a cover-letter surface. `undefined` (the
 * backend default, `classic`) is not decorated.
 */
export function isDecoratedLetterLayout(id: LetterLayoutId | undefined): boolean {
  return id !== undefined && LETTER_LAYOUT_DECORATED[id];
}

/**
 * Whether picking `templateId` should clear a sticky `atsMode`.
 *
 * `atsMode` is ONE per-export flag covering every document in the run (each
 * export request carries a single document, so the résumé and the letter each
 * receive it on their own request). It is a documented no-op for ATS-tier
 * résumé templates (`single_column.typ`: "data.opts.ats — ATS flag (no-op for
 * single column)"), which is why picking one used to clear the flag: nothing
 * could act on it, so leaving it set was invisible state.
 *
 * That stopped being true once the letter renderer started reading the same flag
 * (`data.opts.ats`): under an ATS-tier résumé template a decorated letter is the
 * ONLY thing the flag still drives, and clearing it took away the letter's only
 * off switch. So: clear only when nothing left in the export can degrade.
 *
 * The decision is "does ANY document in this export still read the flag?", one
 * reader per parameter — never a per-site pile of booleans.
 *
 * @param letterAtsApplies caller's answer to "does this export include a cover
 * letter whose layout is decorated?" — each surface phrases it differently
 * (`activeOut === 'cover'`, `target !== 'resume'`, a prop), so it is passed in
 * rather than derived here. Résumé-only surfaces omit it.
 * @param resumeInRun whether the export includes a résumé at all. `false` in a
 * cover-ONLY run, where `templateId` still names a template (it supplies the
 * letter's palette) but no résumé is rendered from it — so a design-tier id must
 * NOT keep the flag alive. Getting this wrong stranded exactly one case: cover
 * target + Atelier + Monogram, ATS on, switch to Classic → the flag stuck → the
 * next Monogram came back silently pre-ATS'd. Defaults to `true`, the
 * résumé-bearing shape every other caller has.
 */
export function shouldClearAtsMode(
  templateId: TemplateId,
  letterAtsApplies = false,
  resumeInRun = true
): boolean {
  const resumeReadsFlag = resumeInRun && isDesignTier(templateId);
  return !resumeReadsFlag && !letterAtsApplies;
}
