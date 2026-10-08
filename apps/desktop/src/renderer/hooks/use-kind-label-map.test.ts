import { describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import en from '../../../../../packages/translations/src/locales/en/translation.json';
import { useKindLabelMap } from './use-kind-label-map';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

describe('useKindLabelMap', () => {
  it('labels every backend job kind with an existing translation key', () => {
    const { result } = renderHook(() => useKindLabelMap());
    for (const kind of ['pipeline.generate', 'ai.indexStale', 'ai.reembed', 'ai.pull_model']) {
      const key = result.current[kind];
      expect(key, kind).toMatch(/^monitoring\.jobKinds\./);
      expect(
        en.monitoring.jobKinds[key?.split('.').pop() as keyof typeof en.monitoring.jobKinds]
      ).toBeTruthy();
    }
  });
});
