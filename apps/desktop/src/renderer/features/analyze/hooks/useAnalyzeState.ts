import { useCallback } from 'react';

import type { Stage } from '@/features/analyze/constants';
import type { AnalysisMode, AnalysisResult } from '@/lib/resume-ai';
import { useSessionStore } from '@/store/session-store';

export function useAnalyzeState() {
  const { analyze, setAnalyze } = useSessionStore();
  const { resume, jobAd, stage, result, analysisMode } = analyze;
  const setResume = useCallback((v: string) => setAnalyze({ resume: v }), [setAnalyze]);
  const setJobAd = (v: string) => setAnalyze({ jobAd: v });
  const setStage = (v: Stage) => setAnalyze({ stage: v });
  const setResult = (v: AnalysisResult | null) => setAnalyze({ result: v });
  const setAnalysisMode = (v: AnalysisMode) => setAnalyze({ analysisMode: v });

  return {
    resume,
    jobAd,
    stage,
    result,
    analysisMode,
    setResume,
    setJobAd,
    setStage,
    setResult,
    setAnalysisMode,
  };
}
