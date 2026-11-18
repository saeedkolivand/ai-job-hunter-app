// Tailor has no generation id at export time (a pipeline run exposes none), so the
// save boundary must receive the request ONLY — never a job url standing in for an id.
import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { _registerClient } from '@/lib/app-client';
import { createMockClient } from '@/lib/mock-client';

import { useTailorOutputActions } from './useTailorOutputActions';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', () => ({ useNotification: () => ({ error: vi.fn() }) }));

describe('useTailorOutputActions — generation id at the save boundary', () => {
  it.each(['pdf', 'docx'] as const)(
    '%s export calls exportAndSave with the request only',
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
        })
      );
      await act(() => result.current.exportAs(fmt));
      expect(exportAndSave).toHaveBeenCalledTimes(1);
      expect(exportAndSave.mock.calls[0]).toHaveLength(1);
    }
  );
});
