import { useRef, useState } from 'react';
import { useTranslation } from '@/lib/i18n';
import { type AnalysisResult, runAnalysis } from '@/lib/resume-ai';
import { useOutputTone, usePromptQuality } from '@/store/preferences-store';
import type { Stage } from '../constants';

export function useAnalysisRun(
  resume: string,
  jobAd: string,
  selectedModel: string,
  canUseAI: boolean,
  i18n: any,
  setStage: (v: Stage) => void,
  setResult: (v: AnalysisResult | null) => void,
  t: (key: string) => string
) {
  const [error, setError] = useState<string | null>(null);
  const [stream, setStream] = useState('');
  const [thinkingBuffer, setThinkingBuffer] = useState('');
  const [runId, setRunId] = useState(0);
  const [modelLoading, setModelLoading] = useState(false);
  const [tokenCount, setTokenCount] = useState(0);
  const tokenStartRef = useRef<number | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  const outputTone = useOutputTone();
  const promptQuality = usePromptQuality();

  const run = async () => {
    if (!canUseAI) return;
    setRunId((n) => n + 1);
    setStage('running');
    setError(null);
    setResult(null);
    setStream('');
    setThinkingBuffer('');
    setModelLoading(true);
    setTokenCount(0);
    tokenStartRef.current = null;

    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      const analysis = await runAnalysis({
        resume,
        jobAd,
        model: selectedModel,
        locale: i18n.language,
        meta: { targetLocale: i18n.language, outputTone: outputTone ?? 'professional' },
        onToken: (tok) => {
          if (!tokenStartRef.current) {
            tokenStartRef.current = Date.now();
          }
          setModelLoading(false);
          setTokenCount((c) => c + 1);
          setStream((p) => (p + tok).slice(-2000));
        },
        onThinking: (tok) => {
          setModelLoading(false);
          setThinkingBuffer((p) => p + tok);
        },
        signal: controller.signal,
      });
      setResult(analysis);
      setStage('done');
    } catch (err) {
      setError(err instanceof Error ? err.message : t('analyze.errorBody'));
      setStage('idle');
    } finally {
      setStream('');
      abortControllerRef.current = null;
    }
  };

  const reset = async () => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    setError(null);
    setStream('');
    setThinkingBuffer('');
    setRunId(0);
  };

  return {
    error,
    stream,
    thinkingBuffer,
    runId,
    modelLoading,
    tokenCount,
    tokenStartRef,
    abortControllerRef,
    run,
    reset,
    outputTone,
    promptQuality,
  };
}
