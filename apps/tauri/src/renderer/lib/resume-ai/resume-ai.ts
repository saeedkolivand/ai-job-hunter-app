/**
 * AI pipeline for resume analysis.
 *
 * Pipeline:
 * 1. Build system + analysis prompts
 * 2. Stream LLM response
 * 3. Collect full text
 * 4. Validate JSON
 * 5. Repair if malformed
 * 6. Return typed AnalysisResult
 */

import {
  type AnalysisResult,
  buildAnalysisPrompt,
  buildSystemPrompt,
  type PromptMeta,
  validateAndRepair,
} from '@ajh/prompts/analyze';
import { getModelTier } from '@ajh/prompts/context-manager';
import { detectLanguages } from '@ajh/shared/language-detection';

import { usePreferencesStore } from '@/store/preferences-store';

import { getClient } from '../app-client';
import { createThinkSplitter } from '../generate/think-split';

type ModelTier = 'large' | 'medium' | 'small';

function effectiveTier(model: string): ModelTier {
  const { promptQuality, aiProviderConfig } = usePreferencesStore.getState();
  const provider = aiProviderConfig?.activeProvider ?? 'ollama';
  // Cloud providers always get the full prompt
  if (provider !== 'ollama') return 'large';
  if (promptQuality === 'full') return 'large';
  if (promptQuality === 'compact') return 'small';
  return getModelTier(model);
}

export type { AnalysisResult };

interface RunAnalysisOptions {
  resume: string;
  jobAd: string;
  model: string;
  locale?: string;
  meta?: PromptMeta;
  onToken?: (token: string) => void;
  onThinking?: (token: string) => void;
  onJobId?: (jobId: string) => void;
  signal?: AbortSignal;
}

/**
 * Run the full AI analysis pipeline.
 * Streams tokens to `onToken` while collecting the full response,
 * then validates and repairs the JSON output.
 */
export async function runAnalysis({
  resume,
  jobAd,
  model,
  locale = 'en',
  meta = {},
  onToken,
  onThinking,
  onJobId,
  signal,
}: RunAnalysisOptions): Promise<AnalysisResult> {
  // Detect languages client-side for accurate mismatch detection
  const clientSideDetection = detectLanguages(resume, jobAd);

  const tier = effectiveTier(model);
  const systemPrompt = buildSystemPrompt(tier);
  // Pass detected languages to LLM so it uses correct info during analysis
  const userPrompt = buildAnalysisPrompt(
    resume,
    jobAd,
    {
      ...meta,
      resumeLanguage: clientSideDetection.resumeName,
      jobAdLanguage: clientSideDetection.jobAdName,
    },
    tier
  );

  // Enqueue the generation job
  const validLocales = ['en', 'de', 'fr', 'es', 'it', 'tr', 'pt', 'ru', 'zh', 'ja', 'ko'] as const;
  const safeLocale = (
    validLocales.includes(locale as (typeof validLocales)[number]) ? locale : 'en'
  ) as (typeof validLocales)[number];

  const api = getClient();
  // Route to the active provider's endpoint — without this the backend defaults
  // to Ollama, so cloud analysis would wrongly hit the local Ollama host.
  const providerConfig = usePreferencesStore.getState().aiProviderConfig;
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const providerSettings = providerConfig?.providers?.[activeProvider];
  const res = await api.ai.generate({
    model: providerSettings?.model || model,
    messages: [
      { role: 'system', content: systemPrompt },
      { role: 'user', content: userPrompt },
    ],
    locale: safeLocale,
    temperature: 0.1,
    // Always route through the active provider (no silent Ollama fallback).
    provider: activeProvider,
    baseUrl: providerSettings?.baseUrl,
    // Reasoning effort for CLI agents that support it (e.g. Codex).
    effort: providerSettings?.effort,
  });

  const jobId = res.jobId;
  onJobId?.(jobId);

  // Collect streamed tokens into the full response
  const full = await new Promise<string>((resolve, reject) => {
    let buffer = '';
    // Local models embed reasoning inline as <think>…</think>; the shared splitter
    // keeps it out of the analysis JSON. Cloud providers flag it structurally below.
    const splitter = createThinkSplitter(
      (text) => {
        buffer += text;
        onToken?.(text);
      },
      (text) => onThinking?.(text)
    );
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
        reject(new Error(c.error.message));
        return;
      }
      if (c.delta) {
        if (c.thinking) {
          onThinking?.(c.delta);
        } else {
          splitter.push(c.delta);
        }
      }
      if (c.done) {
        splitter.flush();
        off();
        resolve(buffer);
      }
    });

    // Handle abort signal
    let abortListener: (() => void) | null = null;
    if (signal) {
      abortListener = () => {
        off();
        // Cancel the job on the backend
        void api.jobs.cancel(jobId);
        reject(new Error('Analysis cancelled'));
      };
      signal.addEventListener('abort', abortListener);
    }

    // Timeout safety — local LLMs can be slow
    const timeoutId = setTimeout(
      () => {
        off();
        if (abortListener && signal) signal.removeEventListener('abort', abortListener);
        splitter.flush();
        resolve(buffer);
      },
      5 * 60 * 1000
    ); // 5 min

    // Watch for job completion/failure as a fallback in case the done stream
    // event is missed (e.g. empty final delta).
    const failCheck = setInterval(() => {
      void (async () => {
        try {
          const job = (await api.jobs.get(jobId)) as {
            status: string;
            result?: { text: string };
          } | null;
          if (!job) return;
          if (job.status === 'failed' || job.status === 'cancelled') {
            clearInterval(failCheck);
            clearTimeout(timeoutId);
            off();
            if (abortListener && signal) signal.removeEventListener('abort', abortListener);
            reject(new Error(`Analysis job ${job.status}`));
          } else if (job.status === 'completed') {
            clearInterval(failCheck);
            clearTimeout(timeoutId);
            off();
            if (abortListener && signal) signal.removeEventListener('abort', abortListener);
            splitter.flush();
            // Use the job result text if the stream already buffered everything,
            // otherwise fall back to whatever the job stored.
            resolve(buffer || job.result?.text || '');
          }
        } catch {
          /* noop */
        }
      })();
    }, 2_000);
  });

  // Validate and repair
  const result = validateAndRepair(full);
  if (!result) {
    throw new Error(
      `The AI returned malformed output. Try again — sometimes local models need a retry.\n\nRaw output preview:\n${full.slice(0, 300)}`
    );
  }

  // Override LLM language detection with accurate client-side detection
  result.detectedLanguages = {
    resume: clientSideDetection.resumeName,
    jobAd: clientSideDetection.jobAdName,
    mismatch: clientSideDetection.mismatch,
  };

  return result;
}

/**
 * Score label based on value.
 */
export function scoreLabel(score: number): { label: string; color: string } {
  if (score >= 90) return { label: 'Exceptional', color: 'text-emerald-400' };
  if (score >= 75) return { label: 'Strong', color: 'text-blue-400' };
  if (score >= 60) return { label: 'Moderate', color: 'text-yellow-400' };
  if (score >= 45) return { label: 'Weak', color: 'text-orange-400' };
  return { label: 'Poor Match', color: 'text-red-400' };
}

/**
 * Overall verdict color based on jobMatch score.
 */
export function verdictGradient(score: number): string {
  if (score >= 80) return 'from-emerald-400 to-teal-400';
  if (score >= 65) return 'from-blue-400 to-indigo-400';
  if (score >= 50) return 'from-yellow-400 to-amber-400';
  return 'from-red-400 to-rose-400';
}
