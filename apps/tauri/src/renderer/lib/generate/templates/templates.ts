// Document templates — colors, sizes, and layout flags consumed by the backend
// DOCX/PDF exporter (`ai.exportAndSave`). Pure data.

export type TemplateId =
  | 'classic'
  | 'modern'
  | 'executive'
  | 'editorial-serif'
  | 'swiss-minimal'
  | 'two-column'
  | 'mono-technical'
  | 'refined-executive'
  | 'academic';

interface DocTemplate {
  id: TemplateId;
  name: string;
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
    sectionStyle: 'underline',
  },

  /** Modern Technical — clean navy, professional, best for tech roles */
  modern: {
    id: 'modern',
    name: 'Modern Technical',
    nameColor: '0D1F3C',
    sectionColor: '0D1F3C',
    accentColor: '1A3A6B',
    bodyColor: '1A1A2E',
    dateColor: '6B6B8A',
    emphasisColor: '0D3D6B',
    ruleColor: 'B8C4DC',
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

  /** Executive — minimalist, charcoal, premium whitespace for senior roles */
  executive: {
    id: 'executive',
    name: 'Executive',
    nameColor: '1C1C1C',
    sectionColor: '2C2C2C',
    accentColor: '444444',
    bodyColor: '2C2C2C',
    dateColor: '808080',
    emphasisColor: '1C1C1C',
    ruleColor: 'CCCCCC',
    namePt: 24,
    sectionPt: 10.5,
    bodyPt: 10.5,
    marginIn: 1.1,
    lineSpacingDocx: 288,
    sectionSpacingBefore: 300,
    nameCentered: true,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Editorial Serif — Source Serif 4 + Inter, deep indigo accent, NYT op-ed character */
  'editorial-serif': {
    id: 'editorial-serif',
    name: 'Editorial Serif',
    nameColor: '1A1A1A',
    sectionColor: '2D2B55',
    accentColor: '2D2B55',
    bodyColor: '1A1A1A',
    dateColor: '5A5A5A',
    emphasisColor: '2D2B55',
    ruleColor: '2D2B55',
    namePt: 22,
    sectionPt: 11,
    bodyPt: 11,
    marginIn: 1.0,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 260,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Swiss Minimal — Manrope, red accent, clean whitespace */
  'swiss-minimal': {
    id: 'swiss-minimal',
    name: 'Swiss Minimal',
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

  /** Two Column — Inter, light sidebar tint */
  'two-column': {
    id: 'two-column',
    name: 'Two Column',
    nameColor: '141414',
    sectionColor: '1E40AF',
    accentColor: '1E40AF',
    bodyColor: '1E1E1E',
    dateColor: '646478',
    emphasisColor: '1E40AF',
    ruleColor: 'B4C8F0',
    namePt: 22,
    sectionPt: 10.5,
    bodyPt: 10,
    marginIn: 0.5,
    lineSpacingDocx: 264,
    sectionSpacingBefore: 200,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'bold-only',
  },

  /** Mono Technical — JetBrains Mono headings, Inter body, cyan accent */
  'mono-technical': {
    id: 'mono-technical',
    name: 'Mono Technical',
    nameColor: '0A0A0A',
    sectionColor: '0096B4',
    accentColor: '00B4D8',
    bodyColor: '1E1E1E',
    dateColor: '647882',
    emphasisColor: '0096B4',
    ruleColor: '00B4D8',
    namePt: 20,
    sectionPt: 10.5,
    bodyPt: 10.5,
    marginIn: 1.0,
    lineSpacingDocx: 276,
    sectionSpacingBefore: 240,
    nameCentered: false,
    sectionAllCaps: true,
    sectionStyle: 'ruled-bottom',
  },

  /** Refined Executive — Playfair Display name, warm gold accent */
  'refined-executive': {
    id: 'refined-executive',
    name: 'Refined Executive',
    nameColor: '141414',
    sectionColor: '645032',
    accentColor: '8B7355',
    bodyColor: '282623',
    dateColor: '78695F',
    emphasisColor: '645032',
    ruleColor: 'C8B9A0',
    namePt: 26,
    sectionPt: 11,
    bodyPt: 10.5,
    marginIn: 1.1,
    lineSpacingDocx: 288,
    sectionSpacingBefore: 300,
    nameCentered: true,
    sectionAllCaps: false,
    sectionStyle: 'ruled-bottom',
  },

  /** Academic — Source Serif 4 throughout, forest green accent */
  academic: {
    id: 'academic',
    name: 'Academic',
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
    sectionStyle: 'underline',
  },
};
