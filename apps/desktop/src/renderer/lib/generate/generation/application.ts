import {
  buildApplicationAnswerPrompt,
  buildApplicationAnswerSystemPrompt,
  buildApplicationEmailPrompt,
  extractPlainText,
  type GenerationMeta,
  resolveMarket,
  type SalaryRange,
} from '@ajh/prompts/generate';

import { usePreferencesStore } from '@/store/preferences-store';

import { buildProviderProfile } from '../provider-context';
import { streamGenerate } from './stream';

/**
 * Generate a single, résumé-grounded answer to one application question. Routes
 * through the same streaming pipeline as résumé/cover-letter generation and the
 * shared grounding contract (no fabrication). Pass `companyBrief` to inform
 * company-context questions; it is fenced as untrusted by the prompt layer.
 * Returns plain text.
 */
export async function generateApplicationAnswer(params: {
  question: string;
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  companyBrief?: string;
  /** Opt-in per-question web-search notes (see `researchAnswer`); fenced
   *  separately from `companyBrief` and never a source of candidate facts. */
  webSearchNotes?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
  /** This question's registry `guidance` (see `ApplicationQuestion.guidance`),
   *  when it has one — absent for user-typed custom questions. */
  guidance?: string;
  /** Web-researched market salary range (salary question only, see
   *  `lookupSalaryRange`); undefined when no lookup ran or it found
   *  nothing reliable. */
  salaryRange?: SalaryRange;
}): Promise<string> {
  const {
    question,
    resume,
    jobAd,
    meta,
    model,
    companyBrief = '',
    webSearchNotes = '',
    signal,
    onToken,
    guidance,
    salaryRange,
  } = params;
  const profile = buildProviderProfile(model);

  // Market drives the answer's register; applicant prefs answer logistics
  // questions (salary/start date/notice/remote) honestly without fabrication.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  const applicant = usePreferencesStore.getState().applicant;
  const tone = usePreferencesStore.getState().outputTone;

  const system = buildApplicationAnswerSystemPrompt(tone, meta.targetLanguage, profile);
  const user = buildApplicationAnswerPrompt({
    question,
    resume,
    jobAd,
    meta,
    companyBrief,
    webSearchNotes,
    target: profile,
    market,
    applicant,
    guidance,
    salaryRange,
  });
  // `prose_grounded`, not plain `prose`: the output asserts factual claims about
  // the candidate to a real employer, so it must stay traceable to the résumé —
  // the adapter drops `presencePenalty` for this intent (it pushes toward new
  // topics, i.e. invented candidate facts).
  const raw = await streamGenerate(model, system, user, 'answers', 'prose_grounded', {
    onToken,
    locale: meta.targetLanguage || 'en',
    signal,
  });
  return extractPlainText(raw);
}

/**
 * Generate a short application email and stream tokens to the caller.
 * Returns the raw output — the caller splits on the first "Subject: " line
 * (see `buildApplicationEmailPrompt` OUTPUT CONTRACT). Mirrors
 * `generateCoverLetter`: same provider config, streaming pipeline, and
 * honesty contract — no new IPC.
 */
export async function generateApplicationEmail(params: {
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  recipientName?: string;
  recipientEmail?: string;
  companyBrief?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const {
    resume,
    jobAd,
    meta,
    model,
    recipientName,
    recipientEmail,
    companyBrief = '',
    signal,
    onToken,
  } = params;
  const profile = buildProviderProfile(model);
  // Same market resolution as the cover letter (job country first, letter
  // language as the fallback): greeting and sign-off follow that market's etiquette.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  const tone = usePreferencesStore.getState().outputTone;
  const { system, user } = buildApplicationEmailPrompt(
    { resume, jobAd, meta, recipientName, recipientEmail, companyBrief, market, tone },
    profile
  );
  // `prose_grounded`: the prompt's contract is the same résumé-grounded honesty
  // application answers have, so it drops `presencePenalty` the same way.
  return streamGenerate(model, system, user, 'cover', 'prose_grounded', {
    onToken,
    locale: meta.targetLanguage ?? 'en',
    signal,
  });
}
