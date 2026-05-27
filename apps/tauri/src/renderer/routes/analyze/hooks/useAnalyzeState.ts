import { useCallback } from 'react';
import { useSessionStore } from '@/store/session-store';
import type { AnalysisResult } from '@/lib/resume-ai';
import type { Stage } from '../constants';

export function useAnalyzeState() {
  const { analyze, setAnalyze } = useSessionStore();
  const { resume, jobAd, stage, result } = analyze;
  const setResume = useCallback((v: string) => setAnalyze({ resume: v }), [setAnalyze]);
  const setJobAd = (v: string) => setAnalyze({ jobAd: v });
  const setStage = (v: Stage) => setAnalyze({ stage: v });
  const setResult = (v: AnalysisResult | null) => setAnalyze({ result: v });

  return {
    resume,
    jobAd,
    stage,
    result,
    setResume,
    setJobAd,
    setStage,
    setResult,
  };
}
