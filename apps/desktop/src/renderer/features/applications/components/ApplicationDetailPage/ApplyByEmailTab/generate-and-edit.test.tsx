/**
 * ApplyByEmailTab — standalone generation, subject-only copy, select-to-rewrite
 */

import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import {
  ApplyByEmailTab,
  BODY,
  clickGenerate,
  documentTextMock,
  EMAIL_RAW,
  generateEmailMock,
  makeApp,
  navigateMock,
  NO_GENERATIONS,
  renderTab,
  renderWithDraft,
  resolveJobUrlMock,
  rewriteBodyAndAccept,
  RewritePopoverStub,
  selectSubstring,
  SUBJECT,
} from './test-support';

// ── standalone generation (no prior document generation) ─────────────────────

describe('ApplyByEmailTab — standalone generation', () => {
  /** Render, click Generate, and wait for the streamed draft to settle. */
  async function generate(application: Application) {
    render(<ApplyByEmailTab application={application} matchingGenerations={NO_GENERATIONS} />);
    await clickGenerate();
    await screen.findByText(BODY);
  }

  it('builds meta from the contact profile + application when there is no saved generation', async () => {
    await generate(makeApp());

    expect(generateEmailMock).toHaveBeenCalledWith(
      expect.objectContaining({
        meta: expect.objectContaining({
          candidateName: 'Jane Applicant',
          companyName: 'Acme',
          jobTitle: 'Engineer',
        }),
      })
    );
    // The persisted JD is present, so the URL resolver is disabled.
    expect(resolveJobUrlMock).toHaveBeenCalledWith('https://acme.com/job/1', false);
  });

  it('resolves the job description from the URL when the application has none', async () => {
    const RESOLVED = 'Resolved job description fetched from the posting URL.';
    resolveJobUrlMock.mockReturnValue({ data: { description: RESOLVED }, isFetching: false });

    renderTab({ jobDescription: '' });

    // The resolver is enabled ONLY because the JD is empty.
    expect(resolveJobUrlMock).toHaveBeenCalledWith('https://acme.com/job/1', true);

    const generateBtn = screen.getByRole('button', {
      name: 'applications.detail.email.generate',
    });
    expect(generateBtn).toBeEnabled();

    await act(async () => {
      fireEvent.click(generateBtn);
    });
    await screen.findByText(BODY);

    expect(generateEmailMock).toHaveBeenCalledWith(expect.objectContaining({ jobAd: RESOLVED }));
  });

  it('shows the loading skeleton while the job URL is still resolving', () => {
    // Empty JD + no saved generation → the URL resolver is enabled and in-flight.
    resolveJobUrlMock.mockReturnValue({ data: undefined, isFetching: true });

    const { container } = renderTab({ jobDescription: '' });

    // The loading gate short-circuits to the skeleton only...
    expect(container.querySelector('.animate-skeleton')).not.toBeNull();
    // ...so neither the Generate button nor the needsJob empty state has rendered.
    expect(screen.queryByRole('button', { name: 'applications.detail.email.generate' })).toBeNull();
    expect(screen.queryByText('applications.detail.email.needsJob')).toBeNull();
  });

  it('detects the target language from a German job description (real detectLanguage)', async () => {
    const germanJd =
      'Wir suchen einen erfahrenen Softwareentwickler für unser Team in München. ' +
      'Sie arbeiten an spannenden Projekten und stimmen sich eng mit dem Produktteam ab.';

    await generate(makeApp({ jobDescription: germanJd }));

    expect(generateEmailMock).toHaveBeenCalledWith(
      expect.objectContaining({ meta: expect.objectContaining({ targetLanguage: 'de' }) })
    );
  });

  it('disables Generate and shows the needsResume empty state + CTA when no résumé exists', () => {
    documentTextMock.mockReturnValue({ data: '', isLoading: false });

    renderTab();

    const generateBtn = screen.getByRole('button', {
      name: 'applications.detail.email.generate',
    });
    expect(generateBtn).toBeDisabled();

    expect(screen.getByText('applications.detail.email.needsResume')).toBeTruthy();

    const cta = screen.getByRole('button', { name: 'applications.detail.email.addResume' });
    fireEvent.click(cta);
    expect(navigateMock).toHaveBeenCalledWith({ to: '/documents' });
  });
});

// ── Feature #2 — subject-only copy ────────────────────────────────────────────

describe('ApplyByEmailTab — subject-only copy', () => {
  it('writes JUST the subject to the clipboard (no "Subject:" prefix, no body)', async () => {
    await renderWithDraft();

    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copySubject' }));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(SUBJECT);
    });
    // It must NOT have written the whole "Subject: …\n\n<body>" blob.
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(EMAIL_RAW);
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(
      `Subject: ${SUBJECT}\n\n${BODY}`
    );
  });

  it('the body Copy button writes JUST the body (no "Subject: …" prefix)', async () => {
    await renderWithDraft();

    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copy' }));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(BODY);
    });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(
      `Subject: ${SUBJECT}\n\n${BODY}`
    );
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(EMAIL_RAW);
  });
});

// ── Feature #5 — select-to-rewrite ────────────────────────────────────────────

describe('ApplyByEmailTab — select-to-rewrite', () => {
  it('opens the popover with docType="email" and the full body when nothing is selected', async () => {
    await renderWithDraft();

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );

    const popover = screen.getByTestId('rewrite-popover');
    expect(popover.getAttribute('data-doc-type')).toBe('email');
    expect(popover.getAttribute('data-selection')).toBe(BODY);
  });

  it('targets only the selected substring of the body', async () => {
    await renderWithDraft();
    selectSubstring(screen.getByText(BODY), BODY, 'interested');

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );

    expect(screen.getByTestId('rewrite-popover').getAttribute('data-selection')).toBe('interested');
  });

  it('accepting a rewrite of a selected substring splices it back into the body', async () => {
    await renderWithDraft();
    selectSubstring(screen.getByText(BODY), BODY, 'interested');

    await rewriteBodyAndAccept();

    // 'interested' → 'REWRITTEN', rest of the body untouched.
    expect(screen.getByText('Hello, I am REWRITTEN in the role.')).toBeTruthy();
    // Popover closes after accept.
    expect(screen.queryByTestId('rewrite-popover')).toBeNull();
  });

  it('accepting a subject rewrite splices into the subject, leaving the body untouched', async () => {
    await renderWithDraft();
    selectSubstring(screen.getByText(SUBJECT), SUBJECT, 'Senior');

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteSubjectAriaLabel' })
    );
    await act(async () => {
      fireEvent.click(screen.getByTestId('popover-accept'));
    });

    // 'Senior' → 'REWRITTEN' in the subject; body unchanged.
    expect(screen.getByText('REWRITTEN Engineer application')).toBeTruthy();
    expect(screen.getByText(BODY)).toBeTruthy();
  });

  it('closing the popover leaves the draft unchanged', async () => {
    await renderWithDraft();

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );
    expect(screen.getByTestId('rewrite-popover')).toBeTruthy();

    fireEvent.click(screen.getByTestId('popover-close'));

    expect(screen.queryByTestId('rewrite-popover')).toBeNull();
    expect(screen.getByText(BODY)).toBeTruthy();
  });

  it('passes the document model + locale to the popover', async () => {
    await renderWithDraft();

    fireEvent.click(
      screen.getByRole('button', { name: 'applications.detail.email.rewriteBodyAriaLabel' })
    );

    expect(RewritePopoverStub).toHaveBeenCalledWith(
      expect.objectContaining({ model: 'test-model', docType: 'email', locale: 'en' })
    );
  });

  // Both outputs read the mutable post-rewrite draft — not the frozen generation.
  it('the whole-email Copy and mailto reflect a rewrite of the draft', async () => {
    const openSpy = vi.spyOn(window, 'open').mockReturnValue(null);
    renderTab({ contactEmail: 'hr@acme.com' });
    await clickGenerate();
    await screen.findByText(BODY);

    // Rewrite 'interested' → 'REWRITTEN', mutating the draft body.
    selectSubstring(screen.getByText(BODY), BODY, 'interested');
    await rewriteBodyAndAccept();

    const rewrittenBody = 'Hello, I am REWRITTEN in the role.';

    // Body Copy writes the post-rewrite body (not the original BODY, and no subject).
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copy' }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(rewrittenBody);
    });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalledWith(BODY);

    // mailto encodes the post-rewrite body too.
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.openMailto' }));
    expect(openSpy).toHaveBeenCalledWith(
      expect.stringContaining(encodeURIComponent(rewrittenBody)),
      '_blank'
    );
    openSpy.mockRestore();
  });
});
