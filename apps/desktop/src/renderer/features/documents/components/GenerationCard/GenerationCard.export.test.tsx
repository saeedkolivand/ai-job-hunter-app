import { beforeEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { exportAndSave, GEN, GenerationCard, mockNotify } from './test-render';

// ── Cover-letter export locale (fix/letter-export-contract) ──────────────────
//
// GenerationCard used to pass `locale: undefined` to the cover-letter export.
// The backend resolves a missing locale to "intl" and its `complete_letter_text`
// synthesizes an English salutation/sign-off onto a body-only letter — so a
// German generation re-exported here silently got English furniture. This
// guards the fix: the resolved market must reach the mocked export call, not
// just "was called".

describe('GenerationCard — cover-letter export locale', () => {
  beforeEach(() => {
    exportAndSave.mockReset();
    exportAndSave.mockResolvedValue('/out/letter.pdf');
  });

  /** Export lives in the 3-dots overflow menu → the composed ExportPicker's
   *  own action buttons (default format is pdf). */
  function openExportModal() {
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'resumes.generated.export' }));
  }

  it('resolves a German generation to the German market, not undefined', async () => {
    render(<GenerationCard gen={{ ...GEN, targetLanguage: 'de' }} />);
    openExportModal();
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportCoverLetter' }));

    await waitFor(() => expect(exportAndSave).toHaveBeenCalledTimes(1));
    // The resolved market reaches the request; the generation's id (never its job
    // url) is the second argument, so the backend records the export template on it.
    const [request, generationId] = exportAndSave.mock.calls[0] ?? [];
    expect(request).toMatchObject({ locale: 'de', documentType: 'cover-letter', format: 'pdf' });
    expect(generationId).toBe(GEN.id);
  });

  it('shows the ATS view for a saved résumé but not for a cover letter', async () => {
    render(<GenerationCard gen={GEN} />);
    openExportModal();
    expect(screen.queryByTestId('ats-panel')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportCoverLetter' }));
    await waitFor(() => expect(exportAndSave).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId('ats-panel')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportResume' }));
    expect(await screen.findByTestId('ats-panel')).toHaveTextContent('/out/letter.pdf');
  });

  it('keeps the résumé panel when a cover letter is exported afterwards', async () => {
    exportAndSave.mockResolvedValueOnce('/out/cv.pdf').mockResolvedValueOnce('/out/letter.pdf');
    render(<GenerationCard gen={GEN} />);
    openExportModal();
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportResume' }));
    expect(await screen.findByTestId('ats-panel')).toHaveTextContent('/out/cv.pdf');

    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportCoverLetter' }));
    await waitFor(() => expect(exportAndSave).toHaveBeenCalledTimes(2));
    expect(screen.getByTestId('ats-panel')).toHaveTextContent('/out/cv.pdf');
  });

  it('leaves no panel when a résumé re-export is rejected', async () => {
    exportAndSave.mockResolvedValueOnce('/out/cv.pdf').mockRejectedValueOnce(new Error('blocked'));
    render(<GenerationCard gen={GEN} />);
    openExportModal();
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportResume' }));
    expect(await screen.findByTestId('ats-panel')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportResume' }));
    await waitFor(() => expect(exportAndSave).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByTestId('ats-panel')).not.toBeInTheDocument());
  });

  it('leaves the résumé export locale unset (unchanged behavior)', async () => {
    render(<GenerationCard gen={{ ...GEN, targetLanguage: 'de' }} />);
    openExportModal();
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportResume' }));

    await waitFor(() => expect(exportAndSave).toHaveBeenCalledTimes(1));
    expect(exportAndSave.mock.calls[0]?.[0].locale).toBeUndefined();
    expect(exportAndSave.mock.calls[0]?.[1]).toBe(GEN.id);
  });
});

// ── Export failure surfaces a toast (was a silent unhandled rejection) ────────

describe('GenerationCard — export failure', () => {
  beforeEach(() => {
    mockNotify.error.mockClear();
    exportAndSave.mockReset();
  });

  it('shows a toast instead of dropping a rejected export on the floor', async () => {
    exportAndSave.mockRejectedValueOnce(new Error('Export blocked: too many pages.'));
    render(<GenerationCard gen={GEN} />);

    // Export lives in the 3-dots overflow menu, then the composed ExportPicker's
    // own "Export Resume" action button (default format is pdf).
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'resumes.generated.export' }));
    fireEvent.click(screen.getByRole('button', { name: 'resumes.generated.exportResume' }));

    await waitFor(() =>
      expect(mockNotify.error).toHaveBeenCalledWith({
        message: 'Failed to export PDF: Export blocked: too many pages.',
      })
    );
  });
});
