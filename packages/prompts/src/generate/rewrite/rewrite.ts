/**
 * Inline rewrite of a selected span (F4).
 *
 * Given a selected span plus its surrounding context, ask the model to rewrite
 * ONLY that span per a user instruction, returning the replacement text with no
 * preamble or quoting. Grounded: the surrounding `before`/`after` is supplied so
 * the rewrite stays consistent in tense, voice, person, and the document's style;
 * the model is told never to fabricate facts. Provider-aware via
 * {@link resolveProfile} — like every other builder, it accepts a bare tier or a
 * full provider profile, so it adapts across ollama / cloud / cli with zero
 * per-provider code: the resolved profile sizes how much surrounding context the
 * prompt carries (more for large-context providers, bounded for small models).
 */

import { type PromptTarget, resolveProfile } from '../../provider/index.js';
import {
  ANTI_AI_TELL_LEXICAL,
  ANTI_AI_TELL_PROSE,
  HUMANIZE_LEXICAL,
  HUMANIZE_PROSE,
} from '../natural-voice/index.js';

export type RewriteDocType = 'resume' | 'cover-letter' | 'application-answer' | 'email';

export interface RewriteParams {
  /** The exact selected span the user wants rewritten. */
  selection: string;
  /**
   * The user's free-text instruction (or a preset's instruction).
   *
   * SECURITY: this MUST be user-originated — it is deliberately treated as an
   * instruction the model obeys. Never build it from other users' data, scraped
   * job-ad text, company-research briefs, or any untrusted source; doing so would
   * turn a prompt-injection payload into a live instruction. The grounded
   * before/after/selection context is fenced; the instruction is not.
   */
  instruction: string;
  /** Text immediately preceding the selection — context only, never rewritten. */
  before: string;
  /** Text immediately following the selection — context only, never rewritten. */
  after: string;
  docType: RewriteDocType;
}

/**
 * Typed doc label map — a Record keyed by RewriteDocType so adding a new doc type
 * is a compile-time error here until a label is provided (exhaustiveness).
 */
const DOC_LABELS: Record<RewriteDocType, string> = {
  resume: 'résumé',
  'cover-letter': 'cover letter',
  'application-answer': 'application answer',
  email: 'application email',
};

/**
 * Per-doc-type natural-voice ruleset. A résumé span keeps ATS bullet conventions,
 * so only the lexical word bans apply; a cover-letter span is prose, so the full
 * prose-flow ruleset applies. An application-answer span is short, first-person,
 * honest prose (connected sentences, not ATS bullets), so it takes the same
 * prose ruleset as a cover letter. An application email is short, first-person
 * prose sent to an employer contact, so it takes the same prose ruleset too.
 * Mirrors {@link DOC_LABELS} so a new doc type is a compile-time error until a
 * voice is provided (exhaustiveness).
 */
const DOC_VOICE: Record<RewriteDocType, string> = {
  resume: `${ANTI_AI_TELL_LEXICAL}\n${HUMANIZE_LEXICAL}`,
  'cover-letter': `${ANTI_AI_TELL_PROSE}\n${HUMANIZE_PROSE}`,
  'application-answer': `${ANTI_AI_TELL_PROSE}\n${HUMANIZE_PROSE}`,
  email: `${ANTI_AI_TELL_PROSE}\n${HUMANIZE_PROSE}`,
};

// Hard cap on the selected span itself so a huge selection can't blow a small
// model's context (the selection is echoed verbatim into the prompt). Generous
// enough for any realistic paragraph-level rewrite.
const MAX_SELECTION_CHARS = 4000;

/**
 * Build the rewrite system + user prompt. The system prompt fixes the contract
 * (return only the span, preserve style, no fabrication); the user prompt carries
 * the grounded context, the instruction, and the span to rewrite.
 */
export function buildRewritePrompt(
  params: RewriteParams,
  target: PromptTarget = 'large'
): { system: string; user: string } {
  const { selection, instruction, before, after, docType } = params;
  // Resolve the provider profile and size the surrounding-context budget from it:
  // larger-context providers (cloud / large local) get more grounding for a
  // more consistent rewrite, while small models stay bounded. Derived from the
  // resolved job-ad slice (a comparable "reference text" budget) rather than a
  // fixed constant, so the rewrite participates in the provider abstraction
  // instead of discarding it. Both neighbours share the budget (÷2), floored so
  // even the smallest tier keeps a usable style cue.
  const { jobAdChars } = resolveProfile(target);
  const contextChars = Math.max(400, Math.floor(jobAdChars / 2));

  const doc = DOC_LABELS[docType];
  const span = selection.slice(0, MAX_SELECTION_CHARS);
  const beforeCtx = before.slice(-contextChars);
  const afterCtx = after.slice(0, contextChars);

  const system = `You rewrite a single selected span inside a candidate's ${doc}.

ABSOLUTE RULES (never break these):
1. Rewrite ONLY the text inside <selection>. Treat <before> and <after> as read-only context; never repeat, continue, or alter them.
2. Output the rewritten span ONLY. No preamble, no explanation, no quotation marks, no surrounding context, no labels.
3. Preserve the document's tense, voice, grammatical person, and overall style so the rewrite reads as one continuous piece with the surrounding text.
4. Never fabricate facts: keep every concrete claim (skills, employers, titles, metrics, dates) that the original span asserts. You may rephrase, tighten, or expand wording, but do not invent new facts not present in the span.
5. If the instruction contains an explicit length or count constraint ("max N characters", "under N words", "one sentence", etc.), treat it as a HARD requirement: count the characters or words in your output and trim it to fit before replying. A rewrite that exceeds the stated limit is wrong regardless of quality.
6. Follow the user's instruction. If it conflicts with these rules, keep the rules.
7. Stay in the same language as the selected span.

${DOC_VOICE[docType]}`;

  const user = `<before>
${beforeCtx}
</before>
<selection>
${span}
</selection>
<after>
${afterCtx}
</after>

### INSTRUCTION ###
${instruction}

Rewrite <selection> following the instruction. Output ONLY the rewritten span, nothing else:`;

  return { system, user };
}
