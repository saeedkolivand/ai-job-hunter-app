import {
  buildHelpChatPrompt,
  buildHelpChatSystemPrompt,
  buildJobAdSummaryPrompt,
  buildJobAdSummarySystemPrompt,
  extractPlainText,
  type GenerationMeta,
  hasRenderablePages,
  type HelpChatAppSection,
  type HelpChatEntry,
  type HelpChatTurn,
} from '@ajh/prompts/generate';

import { OUTPUT_LANGUAGES } from '../locales';
import { buildProviderProfile } from '../provider-context';
import { streamGenerate } from './stream';

/**
 * Summarize a single job ad into a short "key notes" digest — résumé-INDEPENDENT
 * (no résumé, no company brief, no scoring). Written in the ad's own language
 * (`meta.targetLanguage`) and returned as concise markdown (bold section labels
 * survive `extractPlainText`).
 */
export async function generateJobAdSummary(params: {
  jobAd: string;
  meta?: GenerationMeta | null;
  model: string;
  language?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { jobAd, meta, model, language, signal, onToken } = params;
  // Nothing to summarize → skip the wasted API call on an empty/whitespace ad.
  if (!jobAd.trim()) return '';
  const profile = buildProviderProfile(model);

  // `language` arrives as a locale CODE ('de', 'es', …) from the picker. The prompt
  // wants a human language NAME; streamGenerate wants a code. Resolve both once from
  // OUTPUT_LANGUAGES (the allowlist) so the name interpolated into the prompt can't
  // be an arbitrary injected string and the locale isn't silently collapsed to 'en'.
  const lang = language ? OUTPUT_LANGUAGES.find((l) => l.code === language) : undefined;

  const system = buildJobAdSummarySystemPrompt(lang?.englishName);
  const user = buildJobAdSummaryPrompt(jobAd, meta, profile, lang?.englishName);
  // A factual digest, not creative writing — `deterministic`, keyed off
  // `analysis` (the same job-ad-analysis surface as `extractMetadata`).
  const raw = await streamGenerate(model, system, user, 'analysis', 'deterministic', {
    onToken,
    locale: lang?.code ?? meta?.targetLanguage ?? 'en',
    signal,
  });
  return extractPlainText(raw);
}

/**
 * Answer one in-app help question from the retrieved help entries plus a
 * read-only glance at the user's own data (ADR-043) — the generation half of
 * the help chat, whose retrieval half is `useHelpSearch`.
 *
 * Mirrors {@link generateJobAdSummary}: same pipeline, same allowlisted-language
 * handling. `prose_grounded` rather than `deterministic`: the output is prose the
 * user will act on, and every claim must be traceable to the entries supplied.
 * `analysis` is the temperature step because this reads and summarizes supplied
 * material rather than writing on the candidate's behalf.
 *
 * The entries and the sidebar page names are the app's own shipped copy
 * (trusted); the glance, history and question are fenced as untrusted by the
 * prompt builder.
 */
export async function generateHelpAnswer(params: {
  question: string;
  entries: HelpChatEntry[];
  /** The sidebar's sections and page names, already translated by the caller. */
  appPages?: HelpChatAppSection[];
  dataGlance?: string;
  history?: HelpChatTurn[];
  model: string;
  /** Locale CODE ('de', 'es', …) — resolved through the OUTPUT_LANGUAGES allowlist. */
  language?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { question, entries, appPages, dataGlance, history, model, language, signal, onToken } =
    params;
  // Nothing to answer → skip the wasted API call on an empty/whitespace question.
  if (!question.trim()) return '';
  const profile = buildProviderProfile(model);

  // Same allowlist resolution as `generateJobAdSummary`: keeps an arbitrary
  // string out of the interpolated instruction.
  const lang = language ? OUTPUT_LANGUAGES.find((l) => l.code === language) : undefined;

  // The system prompt's APP PAGES clauses and the user prompt's APP PAGES block
  // are the same decision, so it is made ONCE here, off the same input — else
  // the rules would name a list the user prompt had not rendered, an instruction
  // to name a page out of nothing.
  const hasAppPages = hasRenderablePages(appPages);
  const system = buildHelpChatSystemPrompt(lang?.englishName, { hasAppPages });
  const user = buildHelpChatPrompt({
    question,
    entries,
    appPages,
    dataGlance,
    history,
    target: profile,
    language: lang?.englishName,
  });
  const raw = await streamGenerate(model, system, user, 'analysis', 'prose_grounded', {
    onToken,
    locale: lang?.code ?? 'en',
    signal,
  });
  return extractPlainText(raw);
}
