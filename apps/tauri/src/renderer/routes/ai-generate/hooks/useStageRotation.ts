import { GENERATION_STAGES } from '../constants';

export function useStageRotation(
  setStageLabel: (label: string) => void,
  t: (key: string) => string
) {
  const stageIdxRef = { current: 0 };
  const stageTimerRef = { current: null as ReturnType<typeof setInterval> | null };

  const startStageRotation = () => {
    stageIdxRef.current = 0;
    setStageLabel(t(GENERATION_STAGES[0] ?? ''));
    stageTimerRef.current = setInterval(() => {
      stageIdxRef.current = (stageIdxRef.current + 1) % GENERATION_STAGES.length;
      setStageLabel(t(GENERATION_STAGES[stageIdxRef.current] ?? ''));
    }, 2800);
  };

  const stopStageRotation = () => {
    if (stageTimerRef.current) {
      clearInterval(stageTimerRef.current);
      stageTimerRef.current = null;
    }
  };

  return { stageIdxRef, stageTimerRef, startStageRotation, stopStageRotation };
}
