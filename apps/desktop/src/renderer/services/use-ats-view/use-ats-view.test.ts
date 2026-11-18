import { describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import type { AtsView } from '@ajh/shared/ipc';

import { createMockClient, exerciseServiceHooks, renderHookWithClient } from '@/test-support';

import { keys } from '../query-client';
import * as mod from './use-ats-view';
import { useAtsView } from './use-ats-view';

const VIEW: AtsView = {
  contact: {
    name: { status: 'found', value: 'Jane Doe' },
    email: { status: 'found', value: 'jane@example.com' },
    phone: { status: 'missing', reason: 'notFound' },
    location: { status: 'found', value: 'Berlin, Germany' },
  },
  sections: [{ kind: 'experience', heading: 'WORK EXPERIENCE', status: 'found' }],
  roles: [],
  readingOrder: { status: 'found' },
  summary: { found: 4, suspicious: 0, missing: 1, total: 5 },
};

describe('use-ats-view services', () => {
  it('renders every exported hook without crashing', async () => {
    await exerciseServiceHooks(mod);
  });
});

describe('useAtsView', () => {
  it('reads the view for a path and caches it under that path', async () => {
    const atsView = vi.fn().mockResolvedValue(VIEW);
    const client = createMockClient({ 'export.atsView': atsView });
    const { result, queryClient } = renderHookWithClient(() => useAtsView('C:/cv/jane.pdf'), {
      client,
    });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(VIEW);
    expect(atsView).toHaveBeenCalledWith('C:/cv/jane.pdf');
    expect(queryClient.getQueryData(keys.export.atsView('C:/cv/jane.pdf'))).toEqual(VIEW);
  });

  it('does not call the backend without a path', async () => {
    const atsView = vi.fn().mockResolvedValue(VIEW);
    const client = createMockClient({ 'export.atsView': atsView });
    const { result } = renderHookWithClient(() => useAtsView(null), { client });
    expect(result.current.fetchStatus).toBe('idle');
    expect(atsView).not.toHaveBeenCalled();
  });
});
