import type { TemplateId } from '@ajh/shared';

export interface DocTemplate {
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

/** Fields most templates share; an entry only states where it differs. */
const DEFAULTS = {
  sectionPt: 11,
  bodyPt: 10.5,
  nameCentered: false,
  sectionStyle: 'ruled-bottom',
} as const;

export function docTemplate(
  t: Omit<DocTemplate, keyof typeof DEFAULTS> & Partial<Pick<DocTemplate, keyof typeof DEFAULTS>>
): DocTemplate {
  return { ...DEFAULTS, ...t };
}
