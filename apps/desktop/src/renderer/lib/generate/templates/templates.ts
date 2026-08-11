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
import type { TemplateId } from '@ajh/shared';

export type { TemplateId };

/**
 * Cover-letter **layout** (arrangement only). Mirrors the Rust `LetterLayout`
 * enum (export/types.rs) and the shared contract union
 * (`BaseExportRequest.letterLayoutId`). Layout = composition; the palette + fonts
 * always inherit from the chosen résumé {@link TemplateId}. `classic` is the
 * default — an omitted value renders the pre-layout-picker output.
 */
export type LetterLayoutId = 'classic' | 'refined' | 'banded' | 'navy';

/** Ordered letter-layout ids — the picker's option order + the exhaustiveness pin. */
export const LETTER_LAYOUT_IDS = [
  'classic',
  'refined',
  'banded',
  'navy',
] as const satisfies readonly LetterLayoutId[];

interface DocTemplate {
  id: TemplateId;
  name: string;
  /**
   * ATS-safe vs. design tier — mirrors the Rust `TemplateTier`. Drives the
   * gallery grouping (ATS-Safe / Design sections + badge) and which templates
   * surface the ATS-mode toggle (design layouts drop the photo / linearize).
   */
  tier: 'ats' | 'design';
  // Colors (hex, no #)
  nameColor: string;
  sectionColor: string;
  accentColor: string;
  bodyColor: string;
  dateColor: string;
  emphasisColor: string;
  ruleColor: string;
  // Sizes (pt)
  namePt: number;
  sectionPt: number;
  bodyPt: number;
  // DOCX layout
  marginIn: number;
  lineSpacingDocx: number;
  sectionSpacingBefore: number;
  // Style flags
  nameCentered: boolean;
  sectionAllCaps: boolean;
  sectionStyle: 'ruled-bottom' | 'underline' | 'bold-only';
}

export const TEMPLATES: Record<TemplateId, DocTemplate> = {
  /** ATS Classic — maximum compatibility, no color, safe for all ATS parsers */
  classic: {
    id: 'classic',
    name: 'ATS Classic',
    tier: 'ats',
    nameColor: '111111',
    sectionColor: '111111',
    accentColor: '222222',
    bodyColor: '222222',
    dateColor: '555555',
    emphasisColor: '000000',
    ruleColor: 'AAAAAA',
    namePt: 20,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 1.0,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Swiss Minimal — Manrope, red accent, clean whitespace */
  'swiss-minimal': {
    id: 'swiss-minimal',
    name: 'Swiss Minimal',
    tier: 'ats',
    nameColor: '141414',
    sectionColor: '141414',
    accentColor: 'E63946',
    bodyColor: '282828',
    dateColor: '787878',
    emphasisColor: '141414',
    ruleColor: 'E63946',
    namePt: 22,
    sectionPt: 10.5,
    bodyPt: 10.5,
    marginIn: 1.15,
    lineSpacingDocx: 299,
    sectionSpacingBefore: 320,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'bold-only',
  },

  /** Academic — Source Serif 4 throughout, forest green accent, ruled headings */
  academic: {
    id: 'academic',
    name: 'Academic',
    tier: 'ats',
    nameColor: '141E1E',
    sectionColor: '1B4332',
    accentColor: '1B4332',
    bodyColor: '1E1E1E',
    dateColor: '5A6E64',
    emphasisColor: '1B4332',
    ruleColor: '649678',
    namePt: 20,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.85,
    lineSpacingDocx: 252,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Atelier — premium two-column, full-height sidebar rail, slate-indigo accent */
  atelier: {
    id: 'atelier',
    name: 'Atelier',
    tier: 'design',
    nameColor: '16143A',
    sectionColor: '4A4580',
    accentColor: '4A4580',
    bodyColor: '1E1C32',
    dateColor: '6E69AB',
    emphasisColor: '4A4580',
    ruleColor: '4A4580',
    namePt: 22,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.55,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Meridian — header-forward tinted band, copper accent, airy single column */
  meridian: {
    id: 'meridian',
    name: 'Meridian',
    tier: 'ats',
    nameColor: '2A2A2A',
    sectionColor: 'A0522D',
    accentColor: 'A0522D',
    bodyColor: '1E1E1E',
    dateColor: '7A6A5A',
    emphasisColor: 'A0522D',
    ruleColor: 'A0522D',
    namePt: 26,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.9,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Throughline — vertical timeline spine, forest-teal accent */
  throughline: {
    id: 'throughline',
    name: 'Throughline',
    tier: 'ats',
    nameColor: '141E1E',
    sectionColor: '1A5C52',
    accentColor: '1A5C52',
    bodyColor: '1E1E1E',
    dateColor: '5A6E64',
    emphasisColor: '1A5C52',
    ruleColor: '1A5C52',
    namePt: 22,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 1.0,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Portrait — circular photo, name/title right, slate-teal accent (two-column) */
  portrait: {
    id: 'portrait',
    name: 'Portrait',
    tier: 'design',
    nameColor: '16303A',
    sectionColor: '2A6478',
    accentColor: '2A6478',
    bodyColor: '1E1E28',
    dateColor: '5A7A88',
    emphasisColor: '2A6478',
    ruleColor: '2A6478',
    namePt: 24,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.55,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Lebenslauf — DACH DIN-style tabular CV, photo top-right, formal slate accent */
  lebenslauf: {
    id: 'lebenslauf',
    name: 'Lebenslauf (DACH)',
    tier: 'design',
    nameColor: '1E1E28',
    sectionColor: '3D4F6B',
    accentColor: '3D4F6B',
    bodyColor: '1E1E1E',
    dateColor: '5A6478',
    emphasisColor: '3D4F6B',
    ruleColor: '3D4F6B',
    namePt: 22,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.9,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Cadence — Inter, large 28pt name, blue-grey accent, letter-spaced all-caps ruled headings, underlined links */
  cadence: {
    id: 'cadence',
    name: 'Cadence',
    tier: 'ats',
    nameColor: '1A1A1A',
    sectionColor: '1A1A1A',
    accentColor: '4A6785',
    bodyColor: '2B2B2B',
    dateColor: '6B6B6B',
    emphasisColor: '4A6785',
    ruleColor: '4A6785',
    namePt: 28,
    sectionPt: 10.5,
    bodyPt: 10,
    marginIn: 0.8,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /**
   * Cologne Navy — centred tracked-caps navy header, rule-underlined uppercase
   * headings, blue company names, right-aligned italic dates. Carlito (via the
   * Calibri family, which resolves to the bundled Carlito faces).
   */
  'cologne-navy': {
    id: 'cologne-navy',
    name: 'Cologne Navy',
    tier: 'ats',
    nameColor: '1F3864',
    sectionColor: '1F3864',
    accentColor: '1F5C99',
    bodyColor: '1A1A1A',
    dateColor: '4A4A4A',
    emphasisColor: '1F5C99',
    ruleColor: '1F3864',
    namePt: 20.8,
    sectionPt: 9.5,
    bodyPt: 10,
    marginIn: 0.55,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 240,
    nameCentered: true,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Regent — Source Serif 4, deep burgundy accent + rose rule, serif small-caps headings, executive */
  regent: {
    id: 'regent',
    name: 'Regent',
    tier: 'ats',
    nameColor: '2A2A2E',
    sectionColor: '6E1E2B',
    accentColor: '6E1E2B',
    bodyColor: '26262A',
    dateColor: '7A6A6E',
    emphasisColor: '6E1E2B',
    ruleColor: 'C9A9AE',
    namePt: 26,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.9,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 280,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Aria — minimalist design two-column, untinted right sidebar, photo top-right, slate accent, 30pt Manrope name */
  aria: {
    id: 'aria',
    name: 'Aria',
    tier: 'design',
    nameColor: '111111',
    sectionColor: '1A1A1A',
    accentColor: '46505C',
    bodyColor: '2A2A2A',
    dateColor: '7A7A7A',
    emphasisColor: '46505C',
    ruleColor: 'D6D9DD',
    namePt: 30,
    sectionPt: 10.5,
    bodyPt: 10,
    marginIn: 0.6,
    lineSpacingDocx: 288,
    sectionSpacingBefore: 320,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Saffron — warm design two-column, tinted left sidebar, ringed circular photo, terracotta accent, Source Serif 4 small-caps */
  saffron: {
    id: 'saffron',
    name: 'Saffron',
    tier: 'design',
    nameColor: '3A2E28',
    sectionColor: 'A85A3E',
    accentColor: 'A85A3E',
    bodyColor: '302A26',
    dateColor: '8A7A6E',
    emphasisColor: 'A85A3E',
    ruleColor: 'E2C9B4',
    namePt: 24,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.55,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Jake — after "Jake's Resume": ultra-minimal single column, centred name, thin ruled headings, compact entry lines */
  jake: {
    id: 'jake',
    name: 'Jake',
    tier: 'ats',
    nameColor: '111111',
    sectionColor: '111111',
    accentColor: '111111',
    bodyColor: '222222',
    dateColor: '555555',
    emphasisColor: '111111',
    ruleColor: 'AAAAAA',
    namePt: 24,
    sectionPt: 11,
    bodyPt: 10,
    marginIn: 0.6,
    lineSpacingDocx: 240,
    sectionSpacingBefore: 200,
    nameCentered: true,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Awesome — after Awesome-CV: thin accent-tinted header band, accent-bar section markers, crimson accent */
  awesome: {
    id: 'awesome',
    name: 'Awesome',
    tier: 'design',
    nameColor: '1A1A1A',
    sectionColor: '1A1A1A',
    accentColor: 'C41E3A',
    bodyColor: '222222',
    dateColor: '6E6E6E',
    emphasisColor: 'C41E3A',
    ruleColor: 'C41E3A',
    namePt: 24,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 0.7,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Deedy — modern single-column Deedy revision: bold name block with accent surname, cobalt accent */
  deedy: {
    id: 'deedy',
    name: 'Deedy',
    tier: 'design',
    nameColor: '1A1A1A',
    sectionColor: '1A1A1A',
    accentColor: '1E4FB3',
    bodyColor: '222222',
    dateColor: '787878',
    emphasisColor: '1E4FB3',
    ruleColor: 'C8C8C8',
    namePt: 27,
    sectionPt: 11.5,
    bodyPt: 10.5,
    marginIn: 0.75,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 320,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },
};

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
