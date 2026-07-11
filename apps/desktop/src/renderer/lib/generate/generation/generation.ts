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
  buildApplicationEmailPrompt,
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildGitHubProjectsPrompt,
  buildGitHubProjectsSystemPrompt,
  buildInterviewQuestionsPrompt,
  buildInterviewQuestionsSystemPrompt,
  buildJobAdSummaryPrompt,
  buildJobAdSummarySystemPrompt,
  buildMetadataPrompt,
  buildReferralImprovePrompt,
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
  parseGitHubProjects,
  type ReferralFormat,
  resolveMarket,
  type RewriteDocType,
  type SalaryRange,
  validateMetadata,
} from '@ajh/prompts/generate';
import type { GitHubRepo } from '@ajh/shared';
import { detectLanguages } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from '../../app-client';
import { OUTPUT_LANGUAGES, safeLocale } from '../locales';
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

/** Effective sampling parameters for one generation step. */
interface SamplingParams {
  temperature: number;
  topP?: number;
  frequencyPenalty?: number;
  presencePenalty?: number;
  repeatPenalty?: number;
}

// ponytail: detector-resistance sampling knobs. RAID (ACL 2024) found that
// random sampling + repetition/frequency penalties drop AI-detector accuracy
// by up to 38 points — today the app only plumbs temperature. Applied ONLY to
// PROSE generation surfaces (cover letter, application answers, email,
// referral, interview); resume/analysis/inline-rewrite stay excluded because
// frequency/presence penalties would suppress the exact job-ad keyword
// repetition ATS keyword-matching needs. NOTE: on Anthropic's extended-thinking
// path these knobs are a near-no-op — `top_p` is dropped and temperature is
// forced to 1.0 (the API rejects `top_p` alongside `thinking`), and Anthropic
// has no frequency/presence/repeat penalty params at all — don't assume this
// set is "active" there.
const PROSE_SAMPLING = {
  topP: 0.95,
  frequencyPenalty: 0.3,
  presencePenalty: 0.2,
  repeatPenalty: 1.15,
} as const;

/** Generalizes {@link resolveTemperature} into a per-step sampling resolver:
 *  the temperature override lookup is unchanged, and `prose: true` layers on
 *  the shared {@link PROSE_SAMPLING} penalty set for detector-resistant steps.
 *  `overrides` lets one surface tune a specific knob (e.g. drop a penalty, or
 *  tighten topP for a drift-prone small model) without forking the shared set. */
function resolveSampling(
  step: TemperatureStep,
  temperatureDefault: number,
  prose = false,
  overrides?: Partial<Omit<SamplingParams, 'temperature'>>
): SamplingParams {
  const temperature = resolveTemperature(step, temperatureDefault);
  return prose ? { temperature, ...PROSE_SAMPLING, ...overrides } : { temperature };
}

async function streamGenerate(
  model: string,
  system: string,
  user: string,
  onToken: (tok: string) => void,
  temperature = 0.3,
  locale = 'en',
  signal?: AbortSignal,
  onThinking?: (tok: string) => void,
  sampling?: Omit<SamplingParams, 'temperature'>
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
    // Detector-resistance sampling knobs — present only for prose steps that
    // opted in (see PROSE_SAMPLING); omitted (undefined) everywhere else.
    topP: sampling?.topP,
    frequencyPenalty: sampling?.frequencyPenalty,
    presencePenalty: sampling?.presencePenalty,
    repeatPenalty: sampling?.repeatPenalty,
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
  const tone = usePreferencesStore.getState().outputTone;

  const system = buildResumeSystemPrompt(mode, profile, tone, meta.targetLanguage);
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
 * Best-effort, per-question web-search reference notes for an application
 * answer — opt-in sibling of {@link researchCompany}, scoped to a single
 * question's topic (combines it with the role + company for relevance)
 * rather than a general company overview. Any failure or a provider that
 * can't search degrades to `''` so the answer still generates exactly as
 * without web search — this call must never block or fail generation.
 */
export async function researchAnswer(
  question: string,
  role: string,
  company: string,
  model: string
): Promise<string> {
  try {
    const { activeProvider, providerSettings } = resolveActiveProvider(model);
    const res = await getClient().ai.researchAnswer({
      question,
      role: role.trim() || undefined,
      company: company.trim() || undefined,
      provider: activeProvider,
      model: providerSettings?.model || model,
      baseUrl: providerSettings?.baseUrl,
    });
    return res ?? '';
  } catch {
    return '';
  }
}

/**
 * Best-effort web-grounded market salary-range lookup for the salary
 * application question (C2). Routes through the backend enricher — the active
 * provider's own web search, validated and cached. Any failure, timeout, or a
 * provider that can't search yields `undefined` so the salary answer always
 * falls back to the C1 applicant-preference-only grounding — this call must
 * never block or fail the answer.
 */
export async function lookupSalaryRange(
  role: string,
  company: string,
  location: string,
  model: string,
  /** ISO-3166 alpha-2 job country, when known — grounds the researched currency. */
  country?: string,
  /** Authoritative ISO-4217 currency for `country` (resolve via `countryToCurrency`
   *  from `@ajh/prompts/generate`); omitted falls back to today's unconstrained
   *  "local currency for that location" behavior. */
  currency?: string
): Promise<SalaryRange | undefined> {
  try {
    const { activeProvider, providerSettings } = resolveActiveProvider(model);
    const res = await getClient().ai.lookupSalary({
      role,
      company: company.trim() || undefined,
      location: location.trim() || undefined,
      country: country?.trim() || undefined,
      currency: currency?.trim() || undefined,
      provider: activeProvider,
      model: providerSettings?.model || model,
      baseUrl: providerSettings?.baseUrl,
    });
    return res ?? undefined;
  } catch {
    return undefined;
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
  const tone = usePreferencesStore.getState().outputTone;

  // The candidate's own résumé text doubles as a writing-style reference (their
  // real vocabulary/register), replacing the fictional tone exemplar — see
  // buildCoverLetterSystemPrompt's `hasStyleReference` + buildStyleReferenceBlock.
  const styleReference = resume;
  const system = buildCoverLetterSystemPrompt(
    mode,
    profile,
    tone,
    meta.targetLanguage,
    Boolean(styleReference.trim())
  );
  const user = buildCoverLetterPrompt(
    resume,
    jobAd,
    meta,
    mode,
    profile,
    companyBrief,
    market,
    applicant,
    styleReference
  );
  // Cover letters are prose: more temperature + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) loosens the phrasing so it reads human, not
  // mechanical, and resists AI-detector fingerprinting. Small models stay lower
  // to limit drift (raised proportionally from the previous 0.4/0.55 split). A
  // per-model override (if set) wins over this tier-based default. Small local
  // models (7-8B) also compound drift when the full topP randomness stacks with
  // repeatPenalty, so tighten topP for the small tier only; large stays at the
  // shared PROSE_SAMPLING default.
  const stepDefault = tier === 'small' ? 0.58 : 0.8;
  const sampling = resolveSampling(
    'cover',
    stepDefault,
    true,
    tier === 'small' ? { topP: 0.9 } : undefined
  );
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken,
    sampling.temperature,
    locale,
    signal,
    onThinking,
    sampling
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
  /** Opt-in per-question web-search notes (see {@link researchAnswer}); fenced
   *  separately from `companyBrief` and never a source of candidate facts. */
  webSearchNotes?: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
  /** This question's registry `guidance` (see `ApplicationQuestion.guidance`),
   *  when it has one — absent for user-typed custom questions. */
  guidance?: string;
  /** Web-researched market salary range (salary question only, see
   *  {@link lookupSalaryRange}); undefined when no lookup ran or it found
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

  const system = buildApplicationAnswerSystemPrompt(tone, meta.targetLanguage);
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
    // The candidate's own résumé doubles as a writing-style reference (their
    // real vocabulary/register) — see buildStyleReferenceBlock.
    styleReference: resume,
  });
  // Application answers are prose but résumé-grounded (no-fabrication surface):
  // keep topP/frequencyPenalty/repeatPenalty for detector resistance, but drop
  // presencePenalty (it pushes toward new topics, which risks factual drift
  // here) and use a lower temperature than the freer prose surfaces (cover
  // letter, referral) to keep answers traceable to the résumé.
  const sampling = resolveSampling('answers', 0.5, true, { presencePenalty: undefined });
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling
  );
  return extractPlainText(raw);
}

/**
 * Summarize a single job ad into a short "key notes" digest — résumé-INDEPENDENT
 * (no résumé, no company brief, no scoring). Routes through the same streaming
 * pipeline as the other generators (zero per-provider code), at low temperature.
 * The digest is written in the ad's own language (`meta.targetLanguage`) and
 * returned as concise markdown (bold section labels survive `extractPlainText`).
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
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    resolveTemperature('answers', 0.3),
    lang?.code ?? meta?.targetLanguage ?? 'en',
    signal
  );
  return extractPlainText(raw);
}

/**
 * Generate AI-suggested questions the candidate can ASK the interviewer. Routes
 * through the same streaming pipeline as the other generators (zero per-provider
 * code) and the untrusted company-research fence, so web intel only adds context.
 * Pass `companyBrief` (gathered research) so questions cite concrete company/role
 * detail; `seedTopics` biases them (hybrid). Returns the raw delimited text —
 * parse with `parseInterviewQuestions`.
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
    signal,
    onToken,
  } = params;
  const profile = buildProviderProfile(model);
  const market = resolveMarket({
    jobCountry: meta.jobCountry,
    targetLanguage: meta.targetLanguage,
  });

  const system = buildInterviewQuestionsSystemPrompt();
  const user = buildInterviewQuestionsPrompt({
    resume,
    jobAd,
    meta,
    companyBrief,
    seedTopics,
    audiences,
    target: profile,
    market,
  });
  // Interview questions are prose: keep the existing 0.5 temperature default,
  // adding only the shared detector-resistance penalty set (see PROSE_SAMPLING).
  const sampling = resolveSampling('answers', 0.5, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage || 'en',
    signal,
    undefined,
    sampling
  );
  return extractPlainText(raw);
}

/** A résumé-ready project entry produced from one GitHub repo. Exactly the shape
 *  the resume builder's `projects` field array appends. `link` is the repo's
 *  canonical URL, re-attached verbatim post-parse — NEVER written by the AI. */
export interface GeneratedGitHubProject {
  name: string;
  description: string;
  link: string;
}

/** De-slug a repo name for the offline fallback title ("my-cool-app" → "My Cool App"). */
function deslugRepoName(name: string): string {
  return name
    .replace(/[-_./]+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Offline / failure fallback entry for one repo: real description (or a de-slugged
 *  name when empty), with the canonical link attached. Import always works. */
function fallbackProject(repo: GitHubRepo): GeneratedGitHubProject {
  const description = repo.description?.trim() || deslugRepoName(repo.name);
  return { name: deslugRepoName(repo.name), description, link: repo.htmlUrl };
}

/** Normalize a title/repo name to a match key: de-slug, lowercase, drop every
 *  non-alphanumeric. So "my-cool-app", "My Cool App", and "My_Cool_App" all key
 *  to "mycoolapp" — robust to the model de-slugging or re-spacing the title. */
function projectNameKey(name: string): string {
  return deslugRepoName(name)
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '');
}

/**
 * Turn selected GitHub repos into résumé-ready project entries via the AI provider.
 * Routes through the same streaming pipeline as the other generators (zero
 * per-provider code, NO new IPC) and the untrusted-data fence (a hostile repo
 * description can't steer the model). The model writes the title + bullets only;
 * each repo's `htmlUrl` is re-attached as `link` AFTER parsing — the AI never
 * writes a URL.
 *
 * Resilient by design: if streaming/parsing throws, or yields fewer entries than
 * repos, the missing repos fall back to their raw `description` (or de-slugged
 * name) so import ALWAYS works — even offline or with no provider configured.
 *
 * Each parsed entry is matched back to its repo by de-slugged NAME (case/space/
 * hyphen-insensitive), so a correct bullet always lands on the right repo's link
 * even if the model reorders or renames blocks; only entries with no name match
 * fall back to positional pairing. The `link` is ALWAYS the repo's own `htmlUrl`
 * (never the AI). Output is one entry per repo, in input order.
 */
export async function generateGitHubProjects(params: {
  repos: GitHubRepo[];
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<GeneratedGitHubProject[]> {
  const { repos, model, signal, onToken } = params;
  if (!repos.length) return [];

  let parsed: { name: string; description: string }[] = [];
  try {
    const profile = buildProviderProfile(model);
    const system = buildGitHubProjectsSystemPrompt();
    // Map the IPC repo shape → the prompt's URL-free input (the AI never sees a link).
    const user = buildGitHubProjectsPrompt(
      repos.map((r) => ({
        name: r.name,
        description: r.description,
        language: r.language,
        topics: r.topics,
        stars: r.stars,
        pushedAt: r.pushedAt,
      })),
      profile
    );
    const raw = await streamGenerate(
      model,
      system,
      user,
      onToken ?? (() => {}),
      resolveTemperature('answers', 0.4),
      'en',
      signal
    );
    // Parse the RAW stream, NOT extractPlainText(raw): extractPlainText deletes a
    // whole ```-fenced answer entirely, which a local model often emits — that
    // would silently drop every AI entry to the fallback. The parser strips
    // fences + inline markdown itself.
    parsed = parseGitHubProjects(raw);
  } catch {
    // No provider / offline / aborted-after-partial — fall back for every repo.
    parsed = [];
  }

  // Match each parsed entry to its repo by de-slugged NAME so a correct bullet
  // lands on the right repo's link even if the model reorders/renames blocks.
  // Build a name → entry index so each entry is consumed at most once.
  const byName = new Map<string, number>();
  parsed.forEach((entry, i) => {
    const key = projectNameKey(entry.name);
    if (key && !byName.has(key)) byName.set(key, i);
  });
  const used = new Array<boolean>(parsed.length).fill(false);

  return repos.map((repo, i) => {
    // Prefer a name match; fall back to the positional entry only if it is not
    // already claimed by another repo's name match.
    const nameIdx = byName.get(projectNameKey(repo.name));
    let entry: { name: string; description: string } | undefined;
    if (nameIdx !== undefined && !used[nameIdx]) {
      entry = parsed[nameIdx];
      used[nameIdx] = true;
    } else if (!used[i]) {
      entry = parsed[i];
      if (entry) used[i] = true;
    }

    const description = entry?.description.trim();
    if (description) {
      const name = entry?.name.trim() || deslugRepoName(repo.name);
      // Link is ALWAYS the repo's own URL — never the AI, never the matched entry.
      return { name, description, link: repo.htmlUrl };
    }
    return fallbackProject(repo);
  });
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
  // Referral messages are prose: randomness + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) resist AI-detector fingerprinting.
  const sampling = resolveSampling('referral', 0.7, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling
  );
  return extractPlainText(raw);
}

/**
 * Revise an existing referral draft per a user instruction (F3a improve). Mirrors
 * {@link generateReferral} in every way (provider config, streaming pipeline, no
 * new IPC) but uses {@link buildReferralImprovePrompt} so the revision preserves
 * the same honesty + résumé-grounding contract, channel shape, and the ≤300 hard
 * cap for connection notes.
 *
 * SECURITY: `instruction` MUST be user-originated. Never pass scraped job-ad text,
 * company-research briefs, or any untrusted source as the instruction — it is
 * treated as a live directive by the model. The draft and résumé are fenced.
 */
export async function generateReferralImprove(params: {
  personName: string;
  personRole?: string;
  companyName: string;
  jobTitle: string;
  resume: string;
  draft: string;
  instruction: string;
  format: ReferralFormat;
  charLimit?: number;
  model: string;
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
    draft,
    instruction,
    format,
    charLimit,
    model,
    locale = 'en',
    onToken,
    signal,
  } = params;
  const profile = buildProviderProfile(model);

  const { system, user } = buildReferralImprovePrompt(
    {
      personName,
      personRole,
      companyName,
      jobTitle,
      resume,
      draft,
      instruction,
      format,
      charLimit,
    },
    profile
  );
  // Referral messages are prose: randomness + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) resist AI-detector fingerprinting.
  const sampling = resolveSampling('referral', 0.7, true);
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    locale,
    signal,
    undefined,
    sampling
  );
  return extractPlainText(raw);
}

/**
 * Generate a short application email and stream tokens to the caller.
 * Returns the raw output — the caller splits on the first "Subject: " line
 * (see `buildApplicationEmailPrompt` OUTPUT CONTRACT). Mirrors
 * {@link generateCoverLetter}: same provider config, streaming pipeline, and
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
  const { system, user } = buildApplicationEmailPrompt(
    // The candidate's own résumé doubles as a writing-style reference (their
    // real vocabulary/register) — see buildStyleReferenceBlock.
    { resume, jobAd, meta, recipientName, recipientEmail, companyBrief, styleReference: resume },
    profile
  );
  // Application emails are prose: randomness + the shared detector-resistance
  // penalty set (see PROSE_SAMPLING) resist AI-detector fingerprinting.
  const sampling = resolveSampling('cover', 0.7, true);
  return streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    sampling.temperature,
    meta.targetLanguage ?? 'en',
    signal,
    undefined,
    sampling
  );
}
