import { GENERATION_STAGES } from '@/features/ai-generate/constants';
import { runStageTimer } from '@/store/session-store';

export function useStageRotation(
  setStageLabel: (label: string) => void,
  t: (key: string) => string
) {
  const stageIdxRef = { current: 0 };
  // Module-level timer: stoppable from any mount or by `resetAIGenerateAll`.
  const stageTimerRef = runStageTimer;

  const startStageRotation = () => {
    if (stageTimerRef.current) clearInterval(stageTimerRef.current);
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
