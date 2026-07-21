/**
 * JobAdField (ADR-031) — the URL-import provenance seam end-to-end.
 *
 * A harness mirrors the AIGeneratePage wiring: `onChange` (manual edit) clears
 * provenance, `onImport` records it. This asserts JobAdField routes a URL import
 * to `onImport` (with provenance) and a manual paste-over to `onChange` — so a
 * since-replaced job's url can't persist (the stale-url foot-gun).
 */
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { JobAdProvenance } from '@/components/job/JobUrlImport';
import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { JobAdField } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const POSTING = {
  title: 'Staff Engineer',
  company: 'Acme',
  description: 'Build reliable systems here.',
  url: 'https://boards.greenhouse.io/acme/jobs/1',
  source: 'greenhouse',
};

/** Mirrors AIGeneratePage: manual onChange clears provenance, onImport sets it. */
function Harness() {
  const [state, setState] = useState<{ jobAd: string; jobUrl?: string; board?: string }>({
    jobAd: '',
  });
  return (
    <>
      <JobAdField
        label="Job ad"
        value={state.jobAd}
        onChange={(v) => setState({ jobAd: v })}
        onImport={(text, prov: JobAdProvenance) =>
          setState({ jobAd: text, jobUrl: prov.url, board: prov.board })
        }
        uploading={false}
        onUpload={() => {}}
        placeholder="Paste the job ad"
        uploadText="Upload"
      />
      <output data-testid="prov">
        {JSON.stringify({ jobUrl: state.jobUrl ?? null, board: state.board ?? null })}
      </output>
    </>
  );
}

function renderField(overrides: Record<string, (...args: never[]) => unknown> = {}) {
  const client = createMockClient(overrides);
  const queryClient = makeQueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <AppClientProvider client={client}>
        <Harness />
      </AppClientProvider>
    </QueryClientProvider>
  );
}

describe('JobAdField — URL-import provenance (ADR-031)', () => {
  it('records provenance on import, then clears it on a manual paste-over', async () => {
    const resolveUrl = vi.fn().mockResolvedValue(POSTING);
    renderField({ 'scrape.resolveUrl': resolveUrl });

    const prov = () => JSON.parse(screen.getByTestId('prov').textContent ?? '{}');
    expect(prov()).toEqual({ jobUrl: null, board: null });

    // Reveal the URL importer and import a posting.
    await userEvent.click(screen.getByRole('button', { name: 'jobUrlImport.link' }));
    const urlInput = screen.getByPlaceholderText('jobUrlImport.placeholder');
    await userEvent.type(urlInput, 'https://boards.greenhouse.io/acme/jobs/1');
    fireEvent.keyDown(urlInput, { key: 'Enter' });

    await waitFor(() =>
      expect(prov()).toEqual({
        jobUrl: 'https://boards.greenhouse.io/acme/jobs/1',
        board: 'greenhouse',
      })
    );

    // Manual paste-over into the job-ad textarea → provenance cleared.
    fireEvent.change(screen.getByPlaceholderText('Paste the job ad'), {
      target: { value: 'a completely different pasted job posting' },
    });
    expect(prov()).toEqual({ jobUrl: null, board: null });
  });
});
