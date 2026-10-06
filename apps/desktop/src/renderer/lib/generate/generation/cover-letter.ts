import {
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  type GenerationMode,
  getLinkMap,
  injectLinksIntoGeneratedText,
  resolveMarket,
} from '@ajh/prompts/generate';

import { usePreferencesStore } from '@/store/preferences-store';

import { buildProviderProfile } from '../provider-context';
import { researchCompany } from './research';
import { streamGenerate } from './stream';

/**
 * Generate the cover letter and surface the company-research brief that informed
 * it. When `opts.researchCompany` is on, a best-effort brief is fetched and folded
 * into the prompt; it is also returned so the caller can persist it on the
 * generation record (the doc card's "Company research" section). `companyBrief` is
 * `''` when research is off or the fetch yields nothing. `text` is the cleaned,
 * link-injected letter.
 */
export async function generateCoverLetter(
  resume: string,
  jobAd: string,
  meta: GenerationMeta,
  mode: GenerationMode,
  model: string,
  onToken: (tok: string) => void,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void,
  opts?: { researchCompany?: boolean; market?: string }
): Promise<{ text: string; companyBrief: string }> {
  const profile = buildProviderProfile(model);

  // Opt-in: fetch a company brief and fold it into the prompt's fit paragraph.
  const companyBrief = opts?.researchCompany
    ? await researchCompany(jobAd, meta.companyName, meta.jobTitle)
    : '';

  // Resolve the cover-letter market from the job's country (decision: job
  // location, not ad language) with an optional manual override; the letter is
  // written in `meta.targetLanguage` but adopts this market's etiquette.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
    override: opts?.market,
  });
  // User-supplied preferences (salary/start date) — stated only where the market
  // expects them (e.g. DACH); never fabricated. From the global settings store.
  const applicant = usePreferencesStore.getState().applicant;
  const tone = usePreferencesStore.getState().outputTone;

  // No external writing-style sample: the résumé is already embedded verbatim in
  // <candidate_resume>, so `hasStyleReference` stays false and the fictional tone
  // exemplar (English-target only) still applies. `hasBrief` mirrors
  // `buildCoverLetterPrompt`'s own derivation (`Boolean(companyBrief.trim())`),
  // so the system prompt only points at <company_research> when the user prompt
  // will actually fence one.
  const system = buildCoverLetterSystemPrompt(
    mode,
    profile,
    tone,
    meta.targetLanguage,
    false,
    Boolean(companyBrief.trim())
  );
  const user = buildCoverLetterPrompt(
    resume,
    jobAd,
    meta,
    mode,
    profile,
    companyBrief,
    market,
    applicant
  );
  // Cover letters are `prose_grounded`, not plain `prose`: "deliberately
  // creative" is a REGISTER argument (the temperature), not a license to drop
  // the traceability guard — the letter asserts real résumé achievements to an
  // employer (the prompt's `LETTER_HONESTY` contract), and presence_penalty
  // pushing toward new topics is exactly the invented-achievement risk that
  // contract exists to prevent.
  const raw = await streamGenerate(model, system, user, 'cover', 'prose_grounded', {
    onToken,
    locale,
    signal,
    onThinking,
  });
  return {
    text: injectLinksIntoGeneratedText(extractPlainText(raw), getLinkMap(resume)),
    companyBrief,
  };
}
