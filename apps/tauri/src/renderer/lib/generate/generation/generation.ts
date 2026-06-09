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

import { getModelTier } from '@ajh/prompts/context-manager';
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
import { createThinkSplitter } from '../think-split';

type ModelTier = 'large' | 'medium' | 'small';

function effectiveTier(model: string, provider: string): ModelTier {
  const { promptQuality } = usePreferencesStore.getState();
  // Cloud providers always get the full prompt
  if (provider !== 'ollama') return 'large';
  if (promptQuality === 'full') return 'large';
  if (promptQuality === 'compact') return 'small';
  return getModelTier(model);
}

export type { GenerationMeta, GenerationMode };
export { MODES } from '@ajh/prompts/generate';

// ─── LLM helpers ─────────────────────────────────────────────────────────────

const VALID_LOCALES = ['en', 'de', 'fr', 'es', 'it', 'tr', 'pt', 'ru', 'zh', 'ja', 'ko'] as const;
type SupportedLocale = (typeof VALID_LOCALES)[number];

function safeLocale(lng: string): SupportedLocale {
  return VALID_LOCALES.includes(lng as SupportedLocale) ? (lng as SupportedLocale) : 'en';
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
  const storeState = usePreferencesStore.getState();
  const providerConfig = storeState.aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const providerSettings = providerConfig?.providers?.[activeProvider];
  const activeModel = providerSettings?.model || model;
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

  const jobId = res.jobId;
  let buffer = '';

  // Local reasoning models embed <think>…</think> inline; the shared splitter
  // separates that reasoning from the answer. Cloud providers instead flag
  // reasoning structurally (the `thinking` chunk flag handled below).
  const splitter = createThinkSplitter(
    (text) => {
      buffer += text;
      onToken(text);
    },
    (text) => onThinking?.(text)
  );

  return new Promise((resolve, reject) => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let poll: ReturnType<typeof setInterval> | null = null;
    let abortListener: (() => void) | null = null;

    const cleanup = () => {
      if (timeoutId !== null) clearTimeout(timeoutId);
      if (poll !== null) clearInterval(poll);
      if (abortListener && signal) signal.removeEventListener('abort', abortListener);
    };

    const off = api.ai.onStream((chunk: unknown) => {
      const c = chunk as {
        jobId: string;
        delta: string;
        done: boolean;
        error?: { code: string; message: string };
        thinking?: boolean;
      };
      if (c.jobId !== jobId) return;
      if (c.error) {
        off();
        cleanup();
        reject(new Error(c.error.message));
        return;
      }
      if (c.delta) {
        if (c.thinking) {
          // Provider-flagged reasoning (Anthropic, and now OpenAI/Gemini/Ollama
          // via the normalized `thinking` chunk flag).
          onThinking?.(c.delta);
        } else {
          // Local models embed reasoning inline as <think>…</think>.
          splitter.push(c.delta);
        }
      }
      if (c.done) {
        splitter.flush();
        off();
        cleanup();
        resolve(buffer);
      }
    });

    // Handle abort signal
    if (signal) {
      abortListener = () => {
        off();
        void api.jobs.cancel(jobId);
        cleanup();
        reject(new Error('Generation cancelled'));
      };
      signal.addEventListener('abort', abortListener);
    }

    timeoutId = setTimeout(
      () => {
        off();
        cleanup();
        resolve(buffer);
      },
      5 * 60 * 1000
    );

    poll = setInterval(() => {
      void (async () => {
        const job = (await api.jobs.get(jobId).catch(() => null)) as {
          status: string;
        } | null;
        if (job?.status === 'failed' || job?.status === 'cancelled') {
          off();
          cleanup();
          reject(new Error(`Generation ${job.status}. Please try again.`));
        }
        if (job?.status === 'completed') {
          off();
          cleanup();
          resolve(buffer);
        }
      })();
    }, 3_000);
  });
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

  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

  const { system, user } = buildMetadataPrompt(resume, jobAd, tier);
  try {
    const raw = await streamGenerate(model, system, user, () => {}, 0.1, locale);
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
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

  const system = buildResumeSystemPrompt(mode, tier);
  const user = buildResumePrompt(resume, jobAd, meta, mode, tier);
  const raw = await streamGenerate(model, system, user, onToken, 0.25, locale, signal, onThinking);
  // Contact links go on the header line; body links (projects/publications, #18)
  // are re-attached to their own items anywhere in the body.
  return injectLinksIntoGeneratedText(
    extractPlainText(raw),
    getLinkMap(resume),
    getBodyLinkMap(resume)
  );
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
    const providerConfig = usePreferencesStore.getState().aiProviderConfig;
    const activeProvider = providerConfig?.activeProvider ?? 'ollama';
    const providerSettings = providerConfig?.providers?.[activeProvider];
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
): Promise<string> {
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

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

  const system = buildCoverLetterSystemPrompt(mode, tier);
  const user = buildCoverLetterPrompt(
    resume,
    jobAd,
    meta,
    mode,
    tier,
    companyBrief,
    market,
    applicant
  );
  // Cover letters are prose: a little more temperature loosens the phrasing so it
  // reads human, not mechanical. Small models stay lower to limit drift.
  const temperature = tier === 'small' ? 0.4 : 0.55;
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
  return injectLinksIntoGeneratedText(extractPlainText(raw), getLinkMap(resume));
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
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

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
    target: tier,
    market,
    applicant,
  });
  const raw = await streamGenerate(
    model,
    system,
    user,
    onToken ?? (() => {}),
    0.3,
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
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

  const { system, user } = buildRewritePrompt(
    { selection, instruction, before, after, docType },
    tier
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
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

  const { system, user } = buildReferralPrompt(
    { personName, personRole, companyName, jobTitle, resume, format, charLimit },
    tier
  );
  const raw = await streamGenerate(model, system, user, onToken ?? (() => {}), 0.4, locale, signal);
  return extractPlainText(raw);
}
