import { describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';

import { createMockClient } from '@/lib/mock-client';

import { AppClientProvider } from './AppClientProvider';
import { PerformanceModeProvider } from './PerformanceModeProvider';

describe('PerformanceModeProvider', () => {
  it('reflects the performance mode on <html> and forwards it to the backend', async () => {
    const setPerformanceMode = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ system: { setPerformanceMode } });

    render(
      <AppClientProvider client={client}>
        <PerformanceModeProvider>
          <span>child</span>
        </PerformanceModeProvider>
      </AppClientProvider>
    );

    await waitFor(() => expect(setPerformanceMode).toHaveBeenCalled());
    // Default preference is 'balanced'.
    expect(document.documentElement.getAttribute('data-performance-mode')).toBe('balanced');
    expect(setPerformanceMode).toHaveBeenCalledWith('balanced');
  });

  it('swallows backend errors without crashing', async () => {
    const setPerformanceMode = vi.fn().mockRejectedValue(new Error('not ready'));
    const client = createMockClient({ system: { setPerformanceMode } });

    const { getByText } = render(
      <AppClientProvider client={client}>
        <PerformanceModeProvider>
          <span>ok</span>
        </PerformanceModeProvider>
      </AppClientProvider>
    );
    await waitFor(() => expect(setPerformanceMode).toHaveBeenCalled());
    expect(getByText('ok')).toBeInTheDocument();
  });
});
