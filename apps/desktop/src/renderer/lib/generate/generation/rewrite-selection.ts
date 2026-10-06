import { buildRewritePrompt, extractPlainText, type RewriteDocType } from '@ajh/prompts/generate';

import {
  buildProviderProfile,
  resolveActiveProvider,
  type TemperatureStep,
} from '../provider-context';
import { deriveRewriteLocale } from '../rewrite';
import { computeStreamTimeoutMs } from '../stream-promise';
import { streamGenerate } from './stream';

/** Per-{@link RewriteDocType} step, for the Ollama temperature override
 *  lookup only — inline rewrite ALWAYS uses `deterministic` intent
 *  regardless of docType: a surgical edit to a selected span ("tighten this
 *  sentence") must not become a high-temperature/detector-resistant rewrite just
 *  because the surrounding document is prose — that would reintroduce
 *  drift/fabrication risk into exactly the span the user is hand-shaping. */
const REWRITE_STEP: Record<RewriteDocType, TemperatureStep> = {
  resume: 'resume',
  'cover-letter': 'cover',
  'application-answer': 'answers',
  email: 'cover',
};

/**
 * Inline AI rewrite of a selected span (F4): builds the grounded rewrite prompt
 * and streams through the shared pipeline (zero per-provider code, no new IPC).
 * The model is instructed to return ONLY the rewritten span; `extractPlainText`
 * strips any stray markdown/thinking the model echoes. Pass `onToken` to stream
 * the rewrite into a preview and `signal` to abort an in-flight rewrite.
 *
 * The output language is derived from the SELECTION
 * ({@link deriveRewriteLocale}), not from the document: a Dutch span inside a
 * document whose `meta.targetLanguage` is `en` came back in English in 12 of 18
 * measured runs. The derived language is what the prompt names, and it is also
 * sent as the transport locale — but the transport can only carry a supported
 * OUTPUT language (`safeLocale` clamps `nl` to `en`), so the PROMPT is the part
 * that actually pins the language.
 */
export async function rewriteSelection(params: {
  selection: string;
  instruction: string;
  before: string;
  after: string;
  docType: RewriteDocType;
  model: string;
  /** FALLBACK document language, used only when the selection's own language
   *  cannot be detected (default 'en'). Pass the generation's
   *  `meta.targetLanguage`. `streamGenerate` clamps the resolved value to a
   *  supported locale via `safeLocale`. */
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    selection,
    instruction,
    before,
    after,
    docType,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);
  // The span's own language wins over the document's; `locale` is the fallback
  // for a span too short/ambiguous to detect.
  const language = deriveRewriteLocale(selection, locale);

  const { system, user } = buildRewritePrompt(
    { selection, instruction, before, after, docType, language },
    profile
  );
  const raw = await streamGenerate(model, system, user, REWRITE_STEP[docType], 'deterministic', {
    onToken,
    locale: language,
    signal,
  });
  return extractPlainText(raw);
}

/**
 * The renderer-side abort bound for ONE inline rewrite, in ms: the shared
 * effort-scaled stream budget ({@link computeStreamTimeoutMs}) for the SAME
 * `effort` {@link rewriteSelection}'s request carries (the active provider's
 * setting — this surface deliberately does not ask for a cheaper tier, since a
 * low tier measured STRICTLY worse at honouring a length limit).
 *
 * Exists because the popover previously hard-coded 60 s, which is BELOW the
 * backend's own deadline for the same request (300 s × the effort multiplier) —
 * inverting the invariant `computeStreamTimeoutMs` exists to hold, so a long
 * reasoning pass was killed client-side while the backend was still streaming.
 */
export function resolveRewriteTimeoutMs(model: string): number {
  const { providerSettings } = resolveActiveProvider(model);
  return computeStreamTimeoutMs(providerSettings?.effort);
}
