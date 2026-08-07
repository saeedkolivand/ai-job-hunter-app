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
  type AnalysisMode,
  type AnalysisResult,
  buildAnalysisPrompt,
  buildSystemPrompt,
  type PromptMeta,
  validateAndRepair,
} from '@ajh/prompts/analyze';
import { detectLanguages } from '@ajh/shared/language-detection';

import { safeLocale } from '@/lib/generate';

import { getClient } from '../app-client';
import {
  buildProviderProfile,
  resolveActiveProvider,
  resolveTemperatureOverride,
} from '../generate/provider-context';
import { awaitAiStream } from '../generate/stream-promise';

export type { AnalysisMode, AnalysisResult };

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

  const profile = buildProviderProfile(model);
  const systemPrompt = buildSystemPrompt(profile);
  // Pass detected languages to LLM so it uses correct info during analysis
  const userPrompt = buildAnalysisPrompt(
    resume,
    jobAd,
    {
      ...meta,
      resumeLanguage: clientSideDetection.resumeName,
      jobAdLanguage: clientSideDetection.jobAdName,
    },
    profile
  );

  // Enqueue the generation job. Routing (active provider + base_url) is now
  // backend-owned (task #16) — the backend resolves it from its own store and
  // overwrites `model` before streaming, so nothing is threaded here. `effort` (a
  // CLI reasoning knob, not routing) stays renderer-side per RESOLVED-Q1.
  const api = getClient();
  const { activeProvider, providerSettings, activeModel } = resolveActiveProvider(model);
  // This is the MAIN job analysis — bypassed `resolveTemperature` entirely
  // (bare `0.1`) until now, so the Settings "analysis" temperature slider
  // silently never affected the analysis it's named for. `deterministic`:
  // exact, non-creative JSON output — the active provider's own adapter
  // picks its numbers (or none) for that intent (`AiProvider::sampling_profile`,
  // `commands/ai_provider/mod.rs`); the per-model Ollama override, when set,
  // still wins.
  const res = await api.ai.generate({
    model: activeModel || model,
    messages: [
      { role: 'system', content: systemPrompt },
      { role: 'user', content: userPrompt },
    ],
    locale: safeLocale(locale),
    temperature: resolveTemperatureOverride('analysis'),
    intent: 'deterministic',
    effort: providerSettings?.effort,
  });

  const jobId = res.jobId;
  onJobId?.(jobId);

  // Collect streamed tokens into the full response.
  // awaitAiStream enforces the abort-before-register guard (was missing here
  // previously) and unifies poll interval to 3 s (was 2 s here before).
  const full = await awaitAiStream(api, jobId, {
    onToken,
    onThinking,
    signal,
    provider: activeProvider,
    model: activeModel,
    // Same value just sent to the backend above (`effort`) — sizes the
    // renderer-side timeout so it stays the outer bound over the backend's own
    // effort-scaled deadline (`computeStreamTimeoutMs`,
    // apps/desktop/src/renderer/lib/generate/stream-promise.ts).
    effort: providerSettings?.effort,
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
