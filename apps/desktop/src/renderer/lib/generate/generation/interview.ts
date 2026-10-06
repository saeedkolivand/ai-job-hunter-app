import {
  buildInterviewQuestionsPrompt,
  buildInterviewQuestionsSystemPrompt,
  buildLikelyQuestionsPrompt,
  buildLikelyQuestionsSystemPrompt,
  buildStarFeedbackPrompt,
  buildStarFeedbackSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  resolveMarket,
} from '@ajh/prompts/generate';
import { getLanguageName, toLanguageCode } from '@ajh/shared/language-detection';

import { OUTPUT_LANGUAGES } from '../locales';
import { buildProviderProfile } from '../provider-context';
import { streamGenerate } from './stream';

/**
 * Generate AI-suggested questions the candidate can ASK the interviewer. Routes
 * through the same streaming pipeline as the other generators and the untrusted
 * company-research fence, so web intel only adds context. Pass `companyBrief`
 * (gathered research) so questions cite concrete company/role detail;
 * `seedTopics` biases them (hybrid). Returns the raw delimited text — parse with
 * `parseInterviewQuestions`.
 */
export async function generateInterviewQuestions(params: {
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  companyBrief?: string;
  seedTopics?: string[];
  /** Target interviewers (canonical audience ids) — N questions per audience. */
  audiences?: string[];
  /** Output language: a locale CODE ('de', 'es', …) when it came from the picker,
   *  otherwise whatever the ad detection produced (a code outside the picker's
   *  allowlist, or a language NAME). Overrides `meta.targetLanguage`, and
   *  deliberately does NOT feed `resolveMarket` — the register stays that of the
   *  job's country even when only the output language changes. */
  language?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const {
    resume,
    jobAd,
    meta,
    model,
    companyBrief = '',
    seedTopics = [],
    audiences = [],
    language,
    signal,
    onToken,
  } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  // The prompt wants a human language NAME, streamGenerate wants a locale code.
  // An allowlisted picker code resolves to its English name; anything else (a
  // detected language the picker doesn't offer, e.g. 'nl') goes through
  // `getLanguageName` — 28 codes, degrading to the code itself.
  //
  // The ISO-639-1 SHAPE CHECK is defence-in-depth, not cosmetics: `language` can
  // originate from a scraped ad (ad → extractMetadata → meta.targetLanguage →
  // here), `getLanguageName` returns an unrecognised string verbatim, and the
  // result lands in the prompt as an instruction OUTSIDE the untrusted-input
  // fence. Anything that isn't code-shaped is dropped rather than echoed, which
  // leaves the `meta`-derived note to run instead. Mirrors the same guard on
  // `generateJobAdSummary`. `nl`/`pl`/`pt-br` still pass.
  const lang = language ? OUTPUT_LANGUAGES.find((l) => l.code === language) : undefined;
  const isIsoCode = /^[a-z]{2}(-[a-z]{2})?$/i.test(language ?? '');
  const languageName =
    lang?.englishName ?? (language && isIsoCode ? getLanguageName(language) : undefined);
  // The anti-AI-tell lexicon keys off the CODE, and a language can arrive as a
  // NAME on extractMetadata's regex-fallback path — 'German'.slice(0, 2) is 'ge',
  // which silently misses the curated German lexicon. Normalize once, here.
  const languageCode = toLanguageCode(lang?.code ?? language ?? meta.targetLanguage ?? '');

  const system = buildInterviewQuestionsSystemPrompt(languageCode, profile);
  const user = buildInterviewQuestionsPrompt({
    resume,
    jobAd,
    meta,
    companyBrief,
    seedTopics,
    audiences,
    target: profile,
    market,
    language: languageName,
  });
  // Prose: creative, detector-resistant writing — keyed off `questions`, not
  // `answers` (that key is application answers only; the candidate asks these).
  // `streamGenerate` clamps the code via `safeLocale`, so a language outside the
  // supported set falls back to 'en' for transport only.
  const raw = await streamGenerate(model, system, user, 'questions', 'prose', {
    onToken,
    locale: languageCode || 'en',
    signal,
  });
  return extractPlainText(raw);
}

/**
 * Generate likely questions the CANDIDATE will be ASKED for this role — the
 * mock-interview practice set (distinct from {@link generateInterviewQuestions},
 * where the candidate asks the interviewer). Session-only: nothing produced here
 * is persisted to the aiGenerations aggregate. Returns the raw delimited text —
 * parse with `parseLikelyQuestions`.
 */
export async function generateLikelyInterviewQuestions(params: {
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { resume, jobAd, meta, model, signal, onToken } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });

  const system = buildLikelyQuestionsSystemPrompt(profile);
  const user = buildLikelyQuestionsPrompt({ resume, jobAd, meta, target: profile, market });
  const raw = await streamGenerate(model, system, user, 'questions', 'prose', {
    onToken,
    locale: meta.targetLanguage || 'en',
    signal,
  });
  return extractPlainText(raw);
}

/**
 * Generate STAR-rubric feedback on the candidate's typed practice answer to one
 * likely question — strengths, gaps vs the job ad, STAR completeness, and a
 * tightened rewrite. Session-only. Returns the raw delimited text — parse with
 * `parseStarFeedback`. A written critique, so `prose`, keyed off `questions`
 * alongside the other interview-prep surfaces.
 */
export async function generateStarFeedback(params: {
  question: string;
  answer: string;
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { question, answer, resume, jobAd, meta, model, signal, onToken } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });

  const system = buildStarFeedbackSystemPrompt(profile);
  const user = buildStarFeedbackPrompt({
    question,
    answer,
    resume,
    jobAd,
    meta,
    target: profile,
    market,
  });
  const raw = await streamGenerate(model, system, user, 'questions', 'prose', {
    onToken,
    locale: meta.targetLanguage || 'en',
    signal,
  });
  return extractPlainText(raw);
}
