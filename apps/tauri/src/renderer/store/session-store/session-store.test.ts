import { beforeEach, describe, expect, it } from 'vitest';

import { useSessionStore } from './session-store';

const initial = useSessionStore.getState();

beforeEach(() => {
  useSessionStore.setState(initial, true);
});

describe('useSessionStore', () => {
  it('patches the AI Generate slice and resets it', () => {
    const { setAIGenerate, resetAIGenerate } = useSessionStore.getState();
    setAIGenerate({ resume: 'my resume', stage: 'generating' });
    expect(useSessionStore.getState().aiGenerate.resume).toBe('my resume');
    expect(useSessionStore.getState().aiGenerate.stage).toBe('generating');

    resetAIGenerate();
    expect(useSessionStore.getState().aiGenerate.resume).toBe('');
    expect(useSessionStore.getState().aiGenerate.stage).toBe('idle');
  });

  it('patches and resets the Analyze slice', () => {
    useSessionStore.getState().setAnalyze({ jobAd: 'ad', stage: 'running' });
    expect(useSessionStore.getState().analyze.stage).toBe('running');
    useSessionStore.getState().resetAnalyze();
    expect(useSessionStore.getState().analyze.stage).toBe('idle');
  });

  it('patches jobs, resumes and settings slices', () => {
    useSessionStore.getState().setJobs({ filter: 'react', sortBy: 'company' });
    expect(useSessionStore.getState().jobs).toEqual({ filter: 'react', sortBy: 'company' });

    useSessionStore.getState().setResumes({ tab: 'generated' });
    expect(useSessionStore.getState().resumes.tab).toBe('generated');

    useSessionStore.getState().setSettings({ activeSection: 'ai' });
    expect(useSessionStore.getState().settings.activeSection).toBe('ai');
  });

  it('patches the autopilot slice and resets the wizard', () => {
    useSessionStore.getState().setAutopilot({ creating: true, wizardStep: 3, editingId: 'a1' });
    expect(useSessionStore.getState().autopilot.wizardStep).toBe(3);

    useSessionStore.getState().resetAutopilotWizard();
    const ap = useSessionStore.getState().autopilot;
    expect(ap.creating).toBe(false);
    expect(ap.editingId).toBeNull();
    expect(ap.wizardStep).toBe(0);
    expect(ap.wizardForm).toBeNull();
  });
});
