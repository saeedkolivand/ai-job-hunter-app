import { afterEach, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import * as mod from './use-match';

const { useJobAdTextMatchScore } = mod;

describe('use-match services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

// ── useJobAdTextMatchScore — the Score tab's semantic gate ─────────────────────
//
// `match:text` used to hardcode semantic scoring OFF; it now reads the SAME
// `semanticScoring` app preference the Jobs page's `useJobMatchScore` does.
// These pin the two things a preference-gated IPC call can get wrong: the
// flag reaching the actual request, and the query key changing WITH it (so a
// keyword-only result cached under one preference is never served back under
// the other — see `keys.match.textScore`'s doc).

afterEach(() => {
  usePreferencesStore.setState({ semanticScoring: false });
  vi.restoreAllMocks();
});

describe('useJobAdTextMatchScore — semantic preference threading', () => {
  it('threads semanticScoringEnabled: true through to the IPC call when the preference is on', async () => {
    usePreferencesStore.setState({ semanticScoring: true });
    const text = vi.fn().mockResolvedValue({
      resumeId: 'resume-1',
      jobId: 'job-ad-text:x',
      ats: 60,
      semantic: 80,
      combined: 74,
      gaps: [],
      recommendations: [],
      scoreSource: 'combined',
    });
    const client = createMockClient({ 'match.text': text });

    renderHookWithClient(() => useJobAdTextMatchScore('resume-1', 'a real job posting'), {
      client,
    });

    await waitFor(() => expect(text).toHaveBeenCalledTimes(1));
    expect(text).toHaveBeenCalledWith(
      expect.objectContaining({ resumeId: 'resume-1', semanticScoringEnabled: true })
    );
  });

  it('threads semanticScoringEnabled: false through when the preference is off (the default)', async () => {
    usePreferencesStore.setState({ semanticScoring: false });
    const text = vi.fn().mockResolvedValue({
      resumeId: 'resume-1',
      jobId: 'job-ad-text:x',
      ats: 60,
      semantic: 0,
      combined: 60,
      gaps: [],
      recommendations: [],
      scoreSource: 'keyword',
    });
    const client = createMockClient({ 'match.text': text });

    renderHookWithClient(() => useJobAdTextMatchScore('resume-1', 'a real job posting'), {
      client,
    });

    await waitFor(() => expect(text).toHaveBeenCalledTimes(1));
    expect(text).toHaveBeenCalledWith(expect.objectContaining({ semanticScoringEnabled: false }));
  });

  it('re-fires the IPC call when the preference flips — never serves a stale cache entry from the other preference', async () => {
    // Same resumeId/jobText both times: if the query key did not include the
    // preference, the second render would hit the FIRST call's cached result
    // instead of firing a fresh (correctly-flagged) request.
    const text = vi.fn().mockResolvedValue({
      resumeId: 'resume-1',
      jobId: 'job-ad-text:x',
      ats: 60,
      semantic: 0,
      combined: 60,
      gaps: [],
      recommendations: [],
      scoreSource: 'keyword',
    });
    const client = createMockClient({ 'match.text': text });

    usePreferencesStore.setState({ semanticScoring: false });
    const { rerender } = renderHookWithClient(
      () => useJobAdTextMatchScore('resume-1', 'a real job posting'),
      { client }
    );
    await waitFor(() => expect(text).toHaveBeenCalledTimes(1));

    usePreferencesStore.setState({ semanticScoring: true });
    rerender();

    await waitFor(() => expect(text).toHaveBeenCalledTimes(2));
    expect(text).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ semanticScoringEnabled: true })
    );
  });
});
