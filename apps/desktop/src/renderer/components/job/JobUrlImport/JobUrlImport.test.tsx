/**
 * JobUrlImport (ADR-031) — surfaces the resolved posting's text + provenance,
 * and stays a never-a-dead-end on failure.
 *
 * Covers, against a real service layer (createMockClient):
 *   - happy path: resolveUrl resolves → onImport(text, { url, board }); Enter key works
 *   - failure path: resolveUrl resolves null (unreachable / rate-limited — the
 *     command returns json!(null) for every failure) → inline error, no import,
 *     the URL field stays enabled so manual paste remains usable
 */
import { describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { JobUrlImport } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Minimal JobPosting the resolver returns (only the fields JobUrlImport reads).
const POSTING = {
  title: 'Staff Engineer',
  company: 'Acme',
  description: 'We are hiring a staff engineer to build reliable systems.',
  url: 'https://boards.greenhouse.io/acme/jobs/1',
  source: 'greenhouse',
};

function renderImport(
  onImport: (text: string, provenance: { url: string; board?: string }) => void,
  overrides: Record<string, (...args: never[]) => unknown> = {}
) {
  const client = createMockClient(overrides);
  const queryClient = makeQueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <AppClientProvider client={client}>
        <JobUrlImport onImport={onImport} />
      </AppClientProvider>
    </QueryClientProvider>
  );
}

describe('JobUrlImport', () => {
  it('imports on Enter and surfaces the text plus provenance (url + board)', async () => {
    const onImport = vi.fn();
    const resolveUrl = vi.fn().mockResolvedValue(POSTING);
    renderImport(onImport, { 'scrape.resolveUrl': resolveUrl });

    const input = screen.getByPlaceholderText('jobUrlImport.placeholder');
    await userEvent.type(input, 'https://boards.greenhouse.io/acme/jobs/1');
    fireEvent.keyDown(input, { key: 'Enter' }); // keyboard-only flow

    await waitFor(() => expect(onImport).toHaveBeenCalled());
    // Composed header ("title — company") + description, and the provenance the
    // persist path needs (canonical posting url + board).
    expect(onImport).toHaveBeenCalledWith(expect.stringContaining('Staff Engineer — Acme'), {
      url: 'https://boards.greenhouse.io/acme/jobs/1',
      board: 'greenhouse',
    });
    expect(onImport).toHaveBeenCalledWith(
      expect.stringContaining('build reliable systems'),
      expect.objectContaining({ board: 'greenhouse' })
    );
  });

  it('shows an inline error without importing when the url is unresolvable (paste stays usable)', async () => {
    const onImport = vi.fn();
    // The Rust command returns json!(null) for unreachable/unparseable/rate-limited
    // — there is no distinct error union to surface, so this is the failure path.
    const resolveUrl = vi.fn().mockResolvedValue(null);
    renderImport(onImport, { 'scrape.resolveUrl': resolveUrl });

    const input = screen.getByPlaceholderText('jobUrlImport.placeholder');
    await userEvent.type(input, 'https://bad.example/nope');
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(await screen.findByText('jobUrlImport.notFound')).toBeInTheDocument();
    expect(onImport).not.toHaveBeenCalled();
    // Never a dead end: the field remains enabled for a retry / manual paste.
    expect(input).not.toBeDisabled();
  });
});
