import {
  type AIGenerateRunStoreState,
  useAIGenerateRunStore,
  useSessionStore,
} from '@/store/session-store';

type Updater<T> = T | ((prev: T) => T);

const run = () => useAIGenerateRunStore.getState();

const setter =
  <K extends keyof AIGenerateRunStoreState>(key: K) =>
  (v: Updater<AIGenerateRunStoreState[K]>) =>
    useAIGenerateRunStore.setState({
      [key]: typeof v === 'function' ? v(run()[key]) : v,
    } as Partial<AIGenerateRunStoreState>);

/** A streamed document: functional (per-token) updates stay in the run store; a plain value is the committed text. */
const doc = (live: 'liveResume' | 'liveCover', committed: 'resumeOut' | 'coverOut') => {
  const setLive = setter(live);
  return (v: Updater<string>) => {
    if (typeof v === 'function') return setLive(v);
    setLive('');
    useSessionStore.getState().setAIGenerate({ [committed]: v });
  };
};

/**
 * Setters for the in-flight run state, backed by module-level stores instead of
 * component state: the stream callbacks keep writing after the page unmounts and
 * the next mount reads the live values. Functional updates read the CURRENT value
 * (never a render-time closure), which stays correct across a remount.
 */
export const runSetters = {
  setResumeOut: doc('liveResume', 'resumeOut'),
  setCoverOut: doc('liveCover', 'coverOut'),
  setIsGenerating: setter('isGenerating'),
  setStageLabel: setter('stageLabel'),
  setStreamBuffer: setter('streamBuffer'),
  setThinkingBuffer: setter('thinkingBuffer'),
  setModelLoading: setter('modelLoading'),
  setTokenCount: setter('tokenCount'),
  setGenStep: setter('genStep'),
  setError: setter('error'),
};
