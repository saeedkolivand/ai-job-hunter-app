// Illustrative, generic samples for the AI-Generate option previews.
//
// These are FIXED examples for a fictional candidate ("Jordan Avery") — never
// built from the user's input. They exist so a user can see what each option
// does to the *end result* before spending tokens on a real generation:
//   • styles/tones, document target, and prompt-quality → sample wording (here)
//   • templates → a rendered page image (see ./template-previews) + a caption
//
// Bodies are Markdown, rendered through the same `MarkdownMessage` the finished
// output uses, so a sample reads like a real result. English-only for now
// (illustrative); per-locale sample text is a deliberate follow-up.

import type { TemplateId } from '@/lib/generate';

// ── Template captions ────────────────────────────────────────────────────────
// One-line "best for" shown under each template image. Kept here (not in
// templates.ts) to stay additive — templates.ts is render metadata only.

export const TEMPLATE_CAPTIONS: Record<TemplateId, string> = {
  classic: 'Maximum ATS safety — single column, no color. Safe for every parser.',
  'swiss-minimal': 'Minimalist Manrope with a red accent. Design-adjacent and product roles.',
  academic: 'Serif throughout with ruled headings. Academia, research, and publications.',
  atelier: 'Premium two-column sidebar. Skills-forward; collapses to single column for ATS.',
  meridian: 'Header-forward tinted band, copper accent. Airy, modern professional.',
  throughline: 'Vertical timeline spine. Engineering & product careers with a clear arc.',
  portrait: 'Photo header, two columns. European market and personal-brand résumés.',
  lebenslauf: 'DIN-style tabular CV with photo. German-speaking (DACH) market standard.',
  cadence: 'Letter-spaced modern headings, restrained blue-grey. Premium and parser-safe.',
  regent: 'Executive serif with small-caps headings and a deep burgundy accent. Leadership roles.',
  aria: 'Minimalist two-column with an airy untinted sidebar and photo. Collapses to single column for ATS.',
  saffron: 'Warm serif with a tinted sidebar and ringed photo. Collapses to single column for ATS.',
};
