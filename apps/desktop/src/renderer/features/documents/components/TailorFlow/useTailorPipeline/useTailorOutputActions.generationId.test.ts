// The save boundary gets the run detail's generation id (never a job url standing in
// for one); an old run with no id passes the request only.
import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { _registerClient } from '@/lib/app-client';
import { createMockClient } from '@/lib/mock-client';

import { useTailorOutputActions } from './useTailorOutputActions';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', () => ({ useNotification: () => ({ error: vi.fn() }) }));

describe('useTailorOutputActions — generation id at the save boundary', () => {
  it.each(['pdf', 'docx'] as const)(
    '%s export passes the generation id to exportAndSave',
    async (fmt) => {
      const exportAndSave = vi.fn().mockResolvedValue(`/out/file.${fmt}`);
      _registerClient(createMockClient({ documents: { exportAndSave } }));
      const { result } = renderHook(() =>
        useTailorOutputActions({
          output: 'text',
          activeOut: 'resume',
          meta: null,
          market: 'intl',
          templateId: 'classic',
          atsMode: false,
          generationId: 'gen-1',
        })
      );
      await act(() => result.current.exportAs(fmt));
      expect(exportAndSave).toHaveBeenCalledTimes(1);
      expect(exportAndSave.mock.calls[0]?.[1]).toBe('gen-1');
    }
  );

  it('passes the request only when the run has no generation id', async () => {
    const exportAndSave = vi.fn().mockResolvedValue('/out/file.pdf');
    _registerClient(createMockClient({ documents: { exportAndSave } }));
    const { result } = renderHook(() =>
      useTailorOutputActions({
        output: 'text',
        activeOut: 'resume',
        meta: null,
        market: 'intl',
        templateId: 'classic',
        atsMode: false,
      })
    );
    await act(() => result.current.exportAs('pdf'));
    expect(exportAndSave.mock.calls[0]).toHaveLength(1);
  });
});
