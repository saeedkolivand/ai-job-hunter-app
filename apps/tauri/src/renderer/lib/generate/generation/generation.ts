/**
 * LLM generation for Resume + Cover Letter.
 *
 * 1. Extract metadata (JSON — name, role, company, languages, keywords)
 * 2. Generate resume      (streamed text with **keyword** bold markers)
 * 3. Generate cover letter (streamed text with **keyword** bold markers)
 *
 * Generation runs through the backend orchestration pipeline (`ai.generatePipeline`),
 * which streams `ai:stream` deltas under the returned jobId. Export lives in `./export`.
 */

import {
  buildBuilderSystemPrompt,
  buildInterviewResumePrompt,
  type InterviewAnswers,
} from '@ajh/prompts/builder';
import {
  buildApplicationAnswerPrompt,
  buildApplicationAnswerSystemPrompt,
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildMetadataPrompt,
  buildReferralPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  buildRewritePrompt,
  extractPlainText,
  type GenerationMeta,
  type GenerationMode,
  getBodyLinkMap,
  getLinkMap,
  injectLinksIntoGeneratedText,
  type ReferralFormat,
  resolveMarket,
  type RewriteDocType,
  validateMetadata,
} from '@ajh/prompts/generate';
import { detectLanguages } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from '../../app-client';
import { safeLocale } from '../locales';
import {
  buildProviderProfile,
  resolveActiveProvider,
  resolveEffectiveTier,
} from '../provider-context';
import { awaitAiStream } from '../stream-promise';

export type { GenerationMeta, GenerationMode };
export { MODES } from '@ajh/prompts/generate';

// ─── LLM helpers ─────────────────────────────────────────────────────────────

/** One generation step that can carry its own per-model temperature override. */
type TemperatureStep = 'analysis' | 'resume' | 'cover' | 'answers' | 'referral';

/** Effective sampling temperature for one generation step. A user-set per-model,
 *  per-step temperature override (settings → local model limits) wins for that
 *  step; otherwise the per-step default applies. Each step is independent — an
 *  unset step falls back to its default. Override is Ollama-only — cloud/CLI
 *  providers always use the per-step default. */
function resolveTemperature(step: TemperatureStep, stepDefault: number): number {
  const cfg = usePreferencesStore.getState().aiProviderConfig;
  const provider = cfg?.activeProvider ?? 'ollama';
  if (provider !== 'ollama') return stepDefault;
  const model = cfg?.providers?.ollama?.model;
  const override = model
    ? cfg?.providers?.ollama?.modelLimits?.[model]?.temperature?.[step]
    : undefined;
  return override ?? stepDefault;
}

async function streamGenerate(
  model: string,
  system: string,
  user: string,
  onToken: (tok: string) => void,
  temperature = 0.3,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void
): Promise<string> {
  const api = getClient();
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider(model);
  // Per-model generation limits are local (Ollama) only — cloud/CLI providers
  // ignore them, and the backend only applies num_predict/num_ctx for Ollama.
  const localLimits =
    activeProvider === 'ollama' ? providerSettings?.modelLimits?.[activeModel] : undefined;
  // Resume + cover-letter generation runs through the backend orchestration
  // pipeline (a composable Pipeline of stages), not the raw generate command.
  // Same streaming contract: emits `ai:stream` deltas under the returned jobId.
  const res = await api.ai.generatePipeline({
    model: activeModel,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    locale: safeLocale(locale),
    temperature,
    // Always send the active provider — the backend routes strictly and will
    // not fall back to Ollama. baseUrl only applies to OpenAI-compatible servers.
    provider: activeProvider,
    baseUrl: providerSettings?.baseUrl,
    // Reasoning effort for CLI agents that support it (e.g. Codex).
    effort: providerSettings?.effort,
    // Per-model local limits (Ollama) — context window (num_ctx) + max output
    // (num_predict). Omitted (undefined) for cloud/CLI or when unset.
    maxTokens: localLimits?.maxTokens,
    contextWindow: localLimits?.contextWindow,
  });

  return awaitAiStream(api, res.jobId, { onToken, onThinking, signal });
}

// ─── Generation steps ─────────────────────────────────────────────────────────

export async function extractMetadata(
  resume: string,
  jobAd: string,
  model: string,
  locale = 'en'
): Promise<GenerationMeta> {
  // Detect languages client-side
  const clientSideDetection = detectLanguages(resume, jobAd);

  const profile = buildProviderProfile(model);

  const { system, user } = buildMetadataPrompt(resume, jobAd, profile);
  try {
    // Analysis carries its own per-model temperature override (user's chosen design).
    const raw = await streamGenerate(
      model,
      system,
      user,
      () => {},
      resolveTemperature('analysis', 0.15),
      locale
    );
    const meta = validateMetadata(raw);
    if (meta) {
      // Override with client-side detection
      return {
        ...meta,
        resumeLanguage: clientSideDetection.resumeName,
        jobAdLanguage: clientSideDetection.jobAdName,
        mismatch: clientSideDetection.mismatch,
      };
    }
  } catch {
    /* fall through */
  }

  const nameMatch = resume.match(/^([A-Z][a-z]+ [A-Z][a-z]+(?:\s[A-Z][a-z]+)?)/m);
  const titleMatch = jobAd.match(/(?:position|role|title|job)[:\s]+([^\n]+)/i);
  const companyMatch = jobAd.match(/(?:at|@|company|employer|firm)[:\s]+([^\n,]+)/i);
  return {
    candidateName: nameMatch?.[1] ?? '',
    jobTitle: titleMatch?.[1]?.trim() ?? '',
    companyName: companyMatch?.[1]?.trim() ?? '',
    resumeLanguage: clientSideDetection.resumeName,
    jobAdLanguage: clientSideDetection.jobAdName,
    mismatch: clientSideDetection.mismatch,
    targetLanguage: clientSideDetection.resumeName,
    topRequirements: [],
  };
}

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

  const system = buildResumeSystemPrompt(mode, profile);
  const user = buildResumePrompt(resume, jobAd, meta, mode, profile);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    resolveTemperature('resume', 0.3),
    locale,
    signal,
    onThinking
  );
  // Contact links go on the header line; body links (projects/publications, #18)
  // are re-attached to their own items anywhere in the body.
  return injectLinksIntoGeneratedText(
    extractPlainText(raw),
    getLinkMap(resume),
    getBodyLinkMap(resume)
  );
}

/**
 * Resume Builder synthesis (#1 / B9): build a from-scratch résumé from structured
 * interview answers in a SINGLE streamed pass. Mirrors {@link generateResume} —
 * same provider config, effective tier, and streaming pipeline (so it works for
 * every provider with zero per-provider code and adds NO new IPC) — but uses the
 * builder prompts grounded on `<interview_answers>` instead of a base résumé + job
 * ad. Provided links are kept inline by the prompt, so no link-map injection is
 * needed (there is no source résumé to parse). Returns plain text.
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
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    resolveTemperature('resume', 0.3),
    locale,
    signal,
    onThinking
  );
  return extractPlainText(raw);
}

/**
 * Best-effort company research for the cover-letter "fit" paragraph. Routes
 * through the backend enricher — the active provider's own web search +
 * synthesis, cached. Any failure or a provider that can't search yields '' so
 * the cover letter still generates. The returned brief is untrusted reference
 * text — the prompt fences it.
 */
export async function researchCompany(
  jobAd: string,
  model: string,
  company?: string
): Promise<string> {
  try {
    const { activeProvider, providerSettings } = resolveActiveProvider(model);
    const res = await getClient().ai.researchCompany({
      jobAd,
      // The AI-extracted company name is far more reliable than the backend's
      // heuristic job-ad scan (which can grab a tagline), so send it when known.
      company: company?.trim() || undefined,
      provider: activeProvider,
      model: providerSettings?.model || model,
      baseUrl: providerSettings?.baseUrl,
    });
    return res?.brief ?? '';
  } catch {
    return '';
  }
}

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
  const { activeModel, activeProvider } = resolveActiveProvider(model);
  const tier = resolveEffectiveTier(activeModel, activeProvider);
  const profile = buildProviderProfile(model);

  // Opt-in: fetch a company brief and fold it into the prompt's fit paragraph.
  const companyBrief = opts?.researchCompany
    ? await researchCompany(jobAd, model, meta.companyName)
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

  const system = buildCoverLetterSystemPrompt(mode, profile);
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
  // Cover letters are prose: a little more temperature loosens the phrasing so it
  // reads human, not mechanical. Small models stay lower to limit drift. A
  // per-model override (if set) wins over this tier-based default.
  const stepDefault = tier === 'small' ? 0.4 : 0.55;
  const temperature = resolveTemperature('cover', stepDefault);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    temperature,
    locale,
    signal,
    onThinking
  );
  return {
    text: injectLinksIntoGeneratedText(extractPlainText(raw), getLinkMap(resume)),
    companyBrief,
  };
}

/**
 * Generate a single, résumé-grounded answer to one application question. Routes
 * through the same streaming pipeline as résumé/cover-letter generation (so it
 * works for every provider with zero per-provider code) and the shared grounding
 * contract (no fabrication). Pass `companyBrief` to inform company-context
 * questions; it is fenced as untrusted by the prompt layer. Returns plain text.
 */
export async function generateApplicationAnswer(params: {
  question: string;
  resume: string;
  jobAd: string;
  meta: GenerationMeta;
  model: string;
  companyBrief?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<string> {
  const { question, resume, jobAd, meta, model, companyBrief = '', signal, onToken } = params;
  const profile = buildProviderProfile(model);

  // Market drives the answer's register; applicant prefs answer logistics
  // questions (salary/start date/notice/remote) honestly without fabrication.
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });
  const applicant = usePreferencesStore.getState().applicant;

  const system = buildApplicationAnswerSystemPrompt();
  const user = buildApplicationAnswerPrompt({
    question,
    resume,
    jobAd,
    meta,
    companyBrief,
    target: profile,
    market,
    applicant,
  });
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    resolveTemperature('answers', 0.3),
    meta.targetLanguage || 'en',
    signal
  );
  return extractPlainText(raw);
}

/**
 * Inline AI rewrite of a selected span (F4). Mirrors {@link generateApplicationAnswer}:
 * reads the active provider config, computes the effective prompt tier, builds the
 * grounded rewrite prompt, and streams through the shared pipeline — so it works
 * for every provider with zero per-provider code and adds NO new IPC. The model is
 * instructed to return ONLY the rewritten span; `extractPlainText` strips any
 * stray markdown/thinking the model echoes. Pass `onToken` to stream the rewrite
 * into a preview and `signal` to abort an in-flight rewrite.
 */
export async function rewriteSelection(params: {
  selection: string;
  instruction: string;
  before: string;
  after: string;
  docType: RewriteDocType;
  model: string;
  /** Document language so the rewrite streams in the right locale (default 'en').
   *  Pass the generation's `meta.targetLanguage`. `streamGenerate` clamps it to a
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

  const { system, user } = buildRewritePrompt(
    { selection, instruction, before, after, docType },
    profile
  );
  const raw = await streamGenerate(model, system, user, onToken ?? (() => {}), 0.3, locale, signal);
  return extractPlainText(raw);
}

/**
 * Draft a single manual referral message (F3a) for the SELECTED format only —
 * one LLM call per format, never all three eagerly. Mirrors
 * {@link generateApplicationAnswer}: reads the active provider config, computes the
 * effective prompt tier, builds the grounded referral prompt, and streams through
 * the shared pipeline — so it works for every provider with zero per-provider code
 * and adds NO new IPC. The person's details are user-typed (no LinkedIn fetch).
 * `extractPlainText` strips any stray markdown/thinking the model echoes; the
 * connection-note ≤300 cap is enforced in the prompt and re-checked by the UI.
 */
export async function generateReferral(params: {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  format: ReferralFormat;
  /** Hard char cap for the body (defaults to 300 for connection notes). */
  charLimit?: number;
  model: string;
  /** Message language so it streams in the right locale (default 'en'). */
  locale?: string;
  onToken?: (tok: string) => void;
  signal?: AbortSignal;
}): Promise<string> {
  const {
    personName,
    personRole,
    companyName,
    jobTitle,
    resume,
    format,
    charLimit,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildReferralPrompt(
    { personName, personRole, companyName, jobTitle, resume, format, charLimit },
    profile
  );
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    resolveTemperature('referral', 0.4),
    locale,
    signal
  );
  return extractPlainText(raw);
}
