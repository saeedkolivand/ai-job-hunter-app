import {
  buildBuilderSystemPrompt,
  buildInterviewResumePrompt,
  type InterviewAnswers,
} from '@ajh/prompts/builder';
import {
  buildResumePrompt,
  buildResumeSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  type GenerationMode,
  getBodyLinkMap,
  getLinkMap,
  injectLinksIntoGeneratedText,
} from '@ajh/prompts/generate';

import { usePreferencesStore } from '@/store/preferences-store';

import { buildProviderProfile } from '../provider-context';
import { seedHeaderFromContactProfile } from './header-seed';
import { streamGenerate } from './stream';

export async function generateResume(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const profile = buildProviderProfile(model);
  const tone = usePreferencesStore.getState().outputTone;

  const system = buildResumeSystemPrompt(mode, profile, tone, meta.targetLanguage);
  const user = buildResumePrompt(resume, jobAd, meta, mode, profile);
  // Résumé generation is `deterministic`: exact, non-creative output — the
  // exact job-ad keyword repetition ATS matching needs must survive verbatim.
  const raw = await streamGenerate(model, system, user, 'resume', 'deterministic', {
    onToken,
    locale,
    signal,
    onThinking,
  });
  // Contact links go on the header line; body links (projects/publications, #18)
  // are re-attached to their own items anywhere in the body.
  const injected = injectLinksIntoGeneratedText(
    extractPlainText(raw),
    getLinkMap(resume),
    getBodyLinkMap(resume)
  );

  // H: seed the profile's own header (name + contact line) into the text now,
  // AFTER link injection, so it wins over whatever contact-line links that step
  // wrote — its contact-line pass is simply overwritten here.
  return seedHeaderFromContactProfile(injected, meta, locale, signal);
}

/**
 * Resume Builder synthesis (#1 / B9): build a from-scratch résumé from structured
 * interview answers in a SINGLE streamed pass. Mirrors {@link generateResume}
 * (same provider config, streaming pipeline, no new IPC) but uses the builder
 * prompts grounded on `<interview_answers>` instead of a base résumé + job ad.
 * Provided links are kept inline by the prompt, so no link-map injection is
 * needed. Header-seeded exactly like {@link generateResume} (H).
 */
export async function synthesizeResume(
  answers: InterviewAnswers,
  meta: GenerationMeta,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const profile = buildProviderProfile(model);

  const system = buildBuilderSystemPrompt(profile);
  const user = buildInterviewResumePrompt(answers, meta);
  const raw = await streamGenerate(model, system, user, 'resume', 'deterministic', {
    onToken,
    locale,
    signal,
    onThinking,
  });
  return seedHeaderFromContactProfile(extractPlainText(raw), meta, locale, signal);
}
