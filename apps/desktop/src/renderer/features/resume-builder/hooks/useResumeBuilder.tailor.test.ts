import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type * as Ui from '@ajh/ui';

import { runAbortRef, useSessionStore } from '@/store/session-store';

import { useResumeBuilder } from './useResumeBuilder';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', async (orig) => ({
  ...(await orig<typeof Ui>()),
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));
vi.mock('@/components/ui/ModelSelector', () => ({
  useCanUseAI: () => ({ canUse: true }),
  useSelectedModel: () => 'm',
}));
vi.mock('@/services/use-ai-generations', () => ({
  useSaveAiGeneration: () => ({ mutate: vi.fn() }),
}));
vi.mock('@/services/use-contact-profile', () => ({ useContactProfile: () => ({ data: null }) }));

describe('useResumeBuilder.tailorToJob', () => {
  it('aborts a live AI Generate run before seeding the résumé', () => {
    const live = new AbortController();
    runAbortRef.current = live;
    useSessionStore.getState().setResumeBuilder({ output: 'built resume' });

    const { result } = renderHook(() => useResumeBuilder());
    act(() => result.current.tailorToJob());

    expect(live.signal.aborted).toBe(true);
    expect(runAbortRef.current).toBeNull();
    expect(useSessionStore.getState().aiGenerate.resume).toBe('built resume');
  });
});
