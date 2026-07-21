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

  it('patches and resets the Resume Builder slice (#1 / B9)', () => {
    const { setResumeBuilder, resetResumeBuilder } = useSessionStore.getState();
    setResumeBuilder({
      wizardStep: 2,
      stage: 'done',
      output: '# Jane',
      answers: {
        ...useSessionStore.getState().resumeBuilder.answers,
        fullName: 'Jane Doe',
        skills: ['Python'],
      },
    });
    let rb = useSessionStore.getState().resumeBuilder;
    expect(rb.wizardStep).toBe(2);
    expect(rb.stage).toBe('done');
    expect(rb.answers.fullName).toBe('Jane Doe');
    expect(rb.answers.skills).toEqual(['Python']);

    resetResumeBuilder();
    rb = useSessionStore.getState().resumeBuilder;
    expect(rb.wizardStep).toBe(0);
    expect(rb.stage).toBe('interview');
    expect(rb.output).toBe('');
    expect(rb.answers.fullName).toBe('');
    expect(rb.answers.experience).toEqual([]);
  });

  it('patches jobs, resumes and settings slices', () => {
    useSessionStore.getState().setJobs({ filter: 'react', sortBy: 'company' });
    expect(useSessionStore.getState().jobs).toEqual({
      filter: 'react',
      sortBy: 'company',
      viewMode: 'split',
      selectedId: null,
      listScrollTop: 0,
      hideAgency: false,
    });

    useSessionStore.getState().setResumes({ tab: 'activity' });
    expect(useSessionStore.getState().resumes.tab).toBe('activity');

    useSessionStore.getState().setSettings({ activeSection: 'ai' });
    expect(useSessionStore.getState().settings.activeSection).toBe('ai');
  });

  it('jobs slice defaults to split view with no selection', () => {
    const { jobs } = useSessionStore.getState();
    expect(jobs.viewMode).toBe('split');
    expect(jobs.selectedId).toBeNull();
  });

  it('setJobs selects a job and patches only the supplied fields', () => {
    // Capture an unaffected field before any mutation — it must survive both calls,
    // proving setJobs merges rather than replaces the whole jobs slice.
    const { viewMode } = useSessionStore.getState().jobs;

    useSessionStore.getState().setJobs({ selectedId: 'abc' });
    expect(useSessionStore.getState().jobs.selectedId).toBe('abc');
    expect(useSessionStore.getState().jobs.viewMode).toBe(viewMode);

    useSessionStore.getState().setJobs({ selectedId: null });
    expect(useSessionStore.getState().jobs.selectedId).toBeNull();
    expect(useSessionStore.getState().jobs.viewMode).toBe(viewMode);
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
