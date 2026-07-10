// Document templates — the picker's source of truth. The backend renders from a
// canonical Rust `Template` registry keyed by `id`; only the `id` is sent over
// IPC (see `BaseExportRequest.templateId`), so the colour/size fields here are
// display metadata for the picker, kept consistent with the Rust template.
//
// The set MUST match the Rust `TemplateId` enum (export/types.rs) and the shared
// contract union (packages/shared/.../documents.ts) — a guard test pins all three.

export type TemplateId =
  | 'classic'
  | 'swiss-minimal'
  | 'academic'
  | 'atelier'
  | 'meridian'
  | 'throughline'
  | 'portrait'
  | 'lebenslauf';

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
};

/** Stable list of all template ids (kebab-case on the wire). */
export const TEMPLATE_IDS = Object.keys(TEMPLATES) as TemplateId[];

/**
 * Templates with a true two-column layout that collapses to a single column under
 * ATS mode — mirrors the backend `theme::is_two_column`. The ATS toggle + the
 * recommendation auto-apply key off this rather than a hardcoded id.
 */
const TWO_COLUMN_TEMPLATE_IDS = new Set<TemplateId>(['atelier', 'portrait']);

export function isTwoColumnTemplate(id: TemplateId): boolean {
  return TWO_COLUMN_TEMPLATE_IDS.has(id);
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
