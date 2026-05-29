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
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildMetadataPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  type GenerationMode,
  getLinkMap,
  injectLinksIntoGeneratedText,
  validateMetadata,
} from '@ajh/prompts/generate';
import { detectLanguages } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from '../../app-client';

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
  // Resume + cover-letter generation runs through the backend orchestration
  // pipeline (a composable Pipeline of stages), not the raw generate command.
  // Same streaming contract: emits `ai:stream` deltas under the returned jobId.
  const res = (await api.ai.generatePipeline({
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
  } as Parameters<typeof api.ai.generatePipeline>[0])) as { jobId: string };

  const jobId = res.jobId;
  let buffer = '';

  // Tracks whether we're inside an inline <think>...</think> block emitted
  // token-by-token by local reasoning models (DeepSeek, Qwen, etc.)
  let inThinkBlock = false;
  let thinkAccum = '';

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
          // Anthropic-style separate thinking flag
          onThinking?.(c.delta);
        } else {
          // Accumulate to detect inline <think> tags from local models
          thinkAccum += c.delta;

          // Flush any complete non-thinking content from the accumulator
          let out = '';
          let remaining = thinkAccum;

          while (remaining.length > 0) {
            if (inThinkBlock) {
              const closeIdx = remaining.indexOf('</think>');
              if (closeIdx !== -1) {
                onThinking?.(remaining.slice(0, closeIdx));
                inThinkBlock = false;
                remaining = remaining.slice(closeIdx + 8);
              } else {
                // Still inside think block — forward to thinking handler, keep nothing
                onThinking?.(remaining);
                remaining = '';
              }
            } else {
              const openIdx = remaining.indexOf('<think>');
              if (openIdx !== -1) {
                out += remaining.slice(0, openIdx);
                inThinkBlock = true;
                remaining = remaining.slice(openIdx + 7);
              } else {
                // No open tag — but it might be a partial tag at the end, hold back 7 chars
                const holdBack = 7;
                if (remaining.length > holdBack) {
                  out += remaining.slice(0, remaining.length - holdBack);
                  remaining = remaining.slice(remaining.length - holdBack);
                }
                break;
              }
            }
          }

          thinkAccum = remaining;

          if (out) {
            buffer += out;
            onToken(out);
          }
        }
      }
      if (c.done) {
        // Flush whatever remains — even if a think block never closed, discard it;
        // flush any trailing non-think content so the buffer is complete.
        if (thinkAccum && !inThinkBlock) {
          buffer += thinkAccum;
          onToken(thinkAccum);
        }
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
  return injectLinksIntoGeneratedText(extractPlainText(raw), getLinkMap(resume));
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
  onThinking?: (tok: string) => void
): Promise<string> {
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const activeModel = providerConfig?.providers?.[activeProvider]?.model || model;
  const tier = effectiveTier(activeModel, activeProvider);

  const system = buildCoverLetterSystemPrompt(mode, tier);
  const user = buildCoverLetterPrompt(resume, jobAd, meta, mode, tier);
  // Lower temperature for small models to reduce hallucination noise
  const temperature = tier === 'small' ? 0.3 : 0.4;
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
