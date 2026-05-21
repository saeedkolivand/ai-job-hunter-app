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

import { getClient } from './app-client';

export type { AnalysisResult };

interface RunAnalysisOptions {
  resume: string;
  jobAd: string;
  model: string;
  locale?: string;
  meta?: PromptMeta;
  onToken?: (token: string) => void;
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
}: RunAnalysisOptions): Promise<AnalysisResult> {
  const systemPrompt = buildSystemPrompt();
  const userPrompt = buildAnalysisPrompt(resume, jobAd, meta);

  // Enqueue the generation job
  const validLocales = ['en', 'de', 'fr', 'es', 'it', 'tr', 'pt', 'ru', 'zh', 'ja', 'ko'] as const;
  const safeLocale = validLocales.includes(locale as (typeof validLocales)[number]) ? locale : 'en';

  const api = getClient();
  const res = (await api.ai.generate({
    model,
    messages: [
      { role: 'system', content: systemPrompt },
      { role: 'user', content: userPrompt },
    ],
    locale: safeLocale,
    temperature: 0.1,
    maxTokens: 3000,
  })) as { jobId: string };

  const jobId = res.jobId;

  // Collect streamed tokens into the full response
  const full = await new Promise<string>((resolve, reject) => {
    let buffer = '';
    const off = api.ai.onStream((chunk: unknown) => {
      const c = chunk as { jobId: string; delta: string; done: boolean };
      if (c.jobId !== jobId) return;
      if (c.delta) {
        buffer += c.delta;
        onToken?.(c.delta);
      }
      if (c.done) {
        off();
        resolve(buffer);
      }
    });

    // Timeout safety — local LLMs can be slow
    setTimeout(
      () => {
        off();
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
            off();
            reject(new Error(`Analysis job ${job.status}`));
          } else if (job.status === 'completed') {
            clearInterval(failCheck);
            off();
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
