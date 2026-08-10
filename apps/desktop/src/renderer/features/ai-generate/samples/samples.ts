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
//
// Values are i18n KEYS, not display text — the render site (StepTemplate)
// resolves them through `t()`. Copy itself lives in translation.json (en/de).

export const TEMPLATE_CAPTIONS: Record<TemplateId, string> = {
  classic: 'aiGenerate.templateCaption.classic',
  'swiss-minimal': 'aiGenerate.templateCaption.swiss-minimal',
  academic: 'aiGenerate.templateCaption.academic',
  atelier: 'aiGenerate.templateCaption.atelier',
  meridian: 'aiGenerate.templateCaption.meridian',
  throughline: 'aiGenerate.templateCaption.throughline',
  portrait: 'aiGenerate.templateCaption.portrait',
  lebenslauf: 'aiGenerate.templateCaption.lebenslauf',
  cadence: 'aiGenerate.templateCaption.cadence',
  regent: 'aiGenerate.templateCaption.regent',
  aria: 'aiGenerate.templateCaption.aria',
  saffron: 'aiGenerate.templateCaption.saffron',
  'cologne-navy': 'aiGenerate.templateCaption.cologne-navy',
  jake: 'aiGenerate.templateCaption.jake',
  awesome: 'aiGenerate.templateCaption.awesome',
  deedy: 'aiGenerate.templateCaption.deedy',
};
