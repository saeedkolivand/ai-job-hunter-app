import { expect, it } from 'vitest';

import type { GenerationMeta } from '../modes/index.js';
import { antiAiTellLexical, antiAiTellProse } from './natural-voice.js';

/**
 * Regression tests for the centralized anti-AI-tell ruleset (natural-voice.ts)
 * and its wiring into every generation-prompt surface.
 *
 * Invariants under test:
 *  1. DASH-FREE CONSTANTS  — neither exported constant contains an em- or en-dash
 *                            (bans AND the positive HUMANIZE_* blocks).
 *  2. COMPOSITION          — PROSE is a strict superset of LEXICAL; the em-dash ban
 *                            is the distinguishing PROSE-only addition.
 *  3. PROSE SURFACES       — cover-letter, referral, and application-questions system
 *                            prompts carry the ruleset (bans + HUMANIZE_PROSE) and
 *                            are dash-free, at every depth they support (brief /
 *                            task / full).
 *  4. COVER-LETTER EXEMPLAR— the COVER_LETTER_TONE_EXEMPLAR embedded in the full
 *                            system prompt is itself dash-free.
 *  5. RESUME CONTRAST      — resume system prompt carries LEXICAL + HUMANIZE_LEXICAL
 *                            but not the prose em-dash-ban line or any
 *                            prose-imperfection marker; its date-range en-dash
 *                            convention is preserved.
 *  6. REWRITE ROUTING      — docType=cover_letter/application-answer gets PROSE +
 *                            HUMANIZE_PROSE; docType=resume gets LEXICAL +
 *                            HUMANIZE_LEXICAL only (prose em-dash-ban absent).
 *  7. TONE DIRECTIVE       — each output tone maps to its own directive; creative
 *                            stays bounded; the tone param reaches the resume,
 *                            cover-letter, and application-answer system prompts.
 *  8. LANGUAGE-AWARE LEXICON — antiAiTellLexical/antiAiTellProse('de') return a
 *                            curated German lexicon with the English ban-list
 *                            absent; a generic locale (e.g. 'fr') gets a
 *                            language-referencing directive; 'en' is unchanged.
 *                            The language param reaches the resume, cover-letter,
 *                            and application-answer system prompts.
 *  9. STYLE REFERENCE      — an optional styleReference renders a fenced,
 *                            neutralized <style_reference> block with an
 *                            ignore-instructions directive; the cover-letter
 *                            fictional exemplar is dropped when a reference is
 *                            present and falls back (English-target only) when
 *                            absent. When no styleReference is given, the
 *                            prompt instead renders a zero-token voice
 *                            directive pointing at the résumé already embedded
 *                            in <candidate_resume>, rather than duplicating it.
 * 10. FORCED SPECIFICS     — the cover-letter system prompt requires concrete
 *                            resume/job-ad-grounded specifics and a non-generic
 *                            opening hook.
 * 14. CATALOG SHAPE        — every lexicon array is unique, lowercase, trimmed,
 *                            dash-free and apostrophe-free (the matcher is a
 *                            literal comparison, so a curly-apostrophe document
 *                            would silently miss an ASCII-apostrophe entry).
 * 15. NO-AI-SLOP TIERING   — the `no-ai-slop` catalog's résumé-plausible words
 *                            and construction rules reach the PROMPT and stay
 *                            OUT of the validated arrays; the high-precision
 *                            fixed phrases are in both; German gained no
 *                            translated English tell.
 * 16. DEPTH-AWARE TIER     — `brief` carries exactly the bans a deterministic
 *                            validator check verifies; the judgement/
 *                            construction rules are `full`/`task` only. Every
 *                            validated lexicon entry is still spelled out at
 *                            BRIEF depth (or the checker would be stricter
 *                            than the instruction it verifies).
 */

// `antiAiTellLexical()`/`antiAiTellProse()` default to English — calling them
// with no argument is the exact equivalent of the old `ANTI_AI_TELL_LEXICAL`/
// `ANTI_AI_TELL_PROSE` constants they replaced.
export const ANTI_AI_TELL_LEXICAL = antiAiTellLexical();
export const ANTI_AI_TELL_PROSE = antiAiTellProse();

// ─── stable phrase anchors ────────────────────────────────────────────────────
// These are phrases in the current source that uniquely identify a block.
// Anchored to *concepts* in the rule text, not whitespace/punctuation, so minor
// rephrasing doesn't break the tests but removal of the rule does.

/** A phrase stable enough to identify the LEXICAL block is present. */
export const LEXICAL_ANCHOR = 'Drop AI-vocabulary';
/** The em-dash hard-ban line — present in PROSE only, never in LEXICAL alone. */
export const PROSE_EMDASH_BAN = 'EM-DASH HARD BAN';
/** A phrase stable enough to identify the positive HUMANIZE_LEXICAL block. */
export const HUMANIZE_LEXICAL_ANCHOR = 'BULLET VARIETY';
/** A phrase stable enough to identify the positive HUMANIZE_PROSE block. */
export const HUMANIZE_PROSE_ANCHOR = 'CADENCE';

// ─── DepthTargets: one PromptTarget value per resolved depth ─────────────────
// cover-letter supports brief / task / full (see cover-letter.ts + provider/index.ts).
//   'small'  → depth 'brief'  (ollama, tier small)
//   {kind:'cli'} → depth 'task'
//   'large'  → depth 'full'   (ollama, tier large)
export const BRIEF_TARGET = 'small' as const;
export const TASK_TARGET = { kind: 'cli' } as const;
export const FULL_TARGET = 'large' as const;

// A minimal resume so prompt builders don't throw on empty input.
export const STUB_RESUME =
  'Jane Dev\nSenior Engineer\njane@example.com\nSkills: TypeScript, React\n';

export const STYLE_META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'Jane Dev',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  targetLanguage: 'en',
  topRequirements: [],
};

/** The five invariants every PROSE-ruleset system prompt carries (bans, HUMANIZE_PROSE, dash-free). */
export function itCarriesProseRuleset(systemPrompt: () => string): void {
  it('system prompt carries the LEXICAL-ban anchor', () => {
    expect(systemPrompt()).toContain(LEXICAL_ANCHOR);
  });

  it('system prompt carries the PROSE em-dash-ban line', () => {
    expect(systemPrompt()).toContain(PROSE_EMDASH_BAN);
  });

  it('system prompt carries the positive HUMANIZE_PROSE anchor', () => {
    expect(systemPrompt()).toContain(HUMANIZE_PROSE_ANCHOR);
  });

  it('assembled system prompt has no em-dash (—)', () => {
    expect(systemPrompt()).not.toMatch(/—/);
  });

  it('assembled system prompt has no en-dash (–)', () => {
    expect(systemPrompt()).not.toMatch(/–/);
  });
}
