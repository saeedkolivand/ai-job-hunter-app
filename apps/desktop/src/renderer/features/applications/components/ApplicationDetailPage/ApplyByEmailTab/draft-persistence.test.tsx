/**
 * ApplyByEmailTab — draft persistence + hydration
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import {
  ApplyByEmailTab,
  BODY,
  clickGenerate,
  generateEmailMock,
  makeApp,
  makeSavedGeneration,
  renderTab,
  renderWithDraft,
  rewriteBodyAndAccept,
  type SaveCallbacks,
  saveGenerationMock,
  saveResultMock,
  selectSubstring,
  SUBJECT,
} from './test-support';

// ── persistence + hydration ───────────────────────────────────────────────────

describe('ApplyByEmailTab — draft persistence', () => {
  it('saves the settled draft onto the per-job aggregate with the application meta', async () => {
    await renderWithDraft();

    expect(saveGenerationMock).toHaveBeenCalledTimes(1);
    expect(saveGenerationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        emailSubject: SUBJECT,
        emailBody: BODY,
        // Same dedupe key the tailor flow uses, so the merge-upsert lands on the
        // SAME aggregate row instead of forking a second one.
        jobUrl: 'https://acme.com/job/1',
        board: 'linkedin',
        // This surface owns only the email fields.
        resumeText: '',
        coverLetterText: '',
      }),
      expect.anything()
    );
  });

  // Every field this tab does not itself measure goes out blank, so the backend
  // `pick` merge keeps whatever is stored. `meta`'s fallbacks (en/en, ats) are
  // placeholders and would clobber real values whenever `saved` is undefined
  // while a row exists (orphaned FK, or the query simply not loaded yet).
  it('blanks the language pair, target language and mode when there is no saved record', async () => {
    await renderWithDraft();

    expect(saveGenerationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        resumeLanguage: '',
        jobAdLanguage: '',
        targetLanguage: '',
        mode: '',
        mismatch: false,
      }),
      expect.anything()
    );
  });

  it('never saves for a URL-less application, which would fork a phantom applied application', async () => {
    renderTab({ jobUrl: '' });
    await clickGenerate();
    await screen.findByText(BODY);

    // The draft is on screen (session-only) but nothing was written: an empty
    // jobUrl can match neither the Application nor the generation aggregate.
    expect(saveGenerationMock).not.toHaveBeenCalled();
    // And it must not masquerade as a save failure — it is a deliberate skip.
    expect(screen.queryByText('applications.detail.email.saveFailed')).toBeNull();
  });

  it('warns that the draft is unsaved when the command reports an in-band error', async () => {
    saveResultMock.mockReturnValue({ error: 'disk full' });

    await renderWithDraft();

    // The command resolves with `{ error }` rather than rejecting, so onError
    // never fires — the payload must be inspected or the failure is invisible.
    expect(await screen.findByText('applications.detail.email.saveFailed')).toBeTruthy();
  });

  it('warns that the draft is unsaved when the mutation itself rejects', async () => {
    // The other failure mode: the IPC call throws rather than resolving with
    // `{ error }`, so React Query takes the onError path instead of onSuccess.
    saveGenerationMock.mockImplementationOnce((_req: unknown, cbs?: SaveCallbacks) => {
      cbs?.onError?.();
    });

    await renderWithDraft();

    expect(await screen.findByText('applications.detail.email.saveFailed')).toBeTruthy();
  });

  it('shows no warning when the save succeeds', async () => {
    await renderWithDraft();

    expect(screen.queryByText('applications.detail.email.saveFailed')).toBeNull();
  });

  it('clears a previous save warning when a new generation starts', async () => {
    saveResultMock.mockReturnValue({ error: 'disk full' });
    await renderWithDraft();
    expect(screen.getByText('applications.detail.email.saveFailed')).toBeTruthy();

    // Second run succeeds — the stale warning must not linger.
    saveResultMock.mockReturnValue({ id: 'gen-1', success: true });
    await clickGenerate('regenerate');

    await waitFor(() => {
      expect(screen.queryByText('applications.detail.email.saveFailed')).toBeNull();
    });
  });

  it('echoes the saved language pair + mismatch verdict back, never clobbering it', async () => {
    const saved = makeSavedGeneration({ emailSubject: '', emailBody: '' });
    renderTab({}, [saved]);
    await clickGenerate();
    await screen.findByText(BODY);

    expect(saveGenerationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        resumeLanguage: 'de',
        jobAdLanguage: 'en',
        targetLanguage: 'en',
        mode: 'ats',
        mismatch: true,
      }),
      expect.anything()
    );
  });

  it('persists the spliced result of an accepted rewrite', async () => {
    await renderWithDraft();
    saveGenerationMock.mockClear(); // drop the post-generation save
    selectSubstring(screen.getByText(BODY), BODY, 'interested');

    await rewriteBodyAndAccept();

    expect(saveGenerationMock).toHaveBeenCalledTimes(1);
    expect(saveGenerationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        emailSubject: SUBJECT,
        emailBody: 'Hello, I am REWRITTEN in the role.',
      }),
      expect.anything()
    );
  });

  it('hydrates the draft from the saved record on mount, with no generation', () => {
    const saved = makeSavedGeneration({
      emailSubject: 'Saved subject',
      emailBody: 'Saved body from a previous session.',
    });

    renderTab({}, [saved]);

    expect(screen.getByText('Saved subject')).toBeTruthy();
    expect(screen.getByText('Saved body from a previous session.')).toBeTruthy();
    // A hydrated draft is a draft: the button offers Regenerate, and the empty
    // state is gone. Nothing is re-saved just by mounting.
    expect(
      screen.getByRole('button', { name: 'applications.detail.email.regenerate' })
    ).toBeTruthy();
    expect(screen.queryByText('applications.detail.email.empty')).toBeNull();
    expect(saveGenerationMock).not.toHaveBeenCalled();
  });

  it('lets a hydrated draft be copied and rewritten without re-generating', async () => {
    const savedBody = 'Saved body from a previous session.';
    const saved = makeSavedGeneration({ emailSubject: 'Saved subject', emailBody: savedBody });
    renderTab({}, [saved]);

    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copySubject' }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Saved subject');
    });

    // Rewriting the hydrated draft splices into it (the `email` state is still
    // null at this point — the splice must fall back to the hydrated draft).
    selectSubstring(screen.getByText(savedBody), savedBody, 'previous');
    await rewriteBodyAndAccept();

    expect(screen.getByText('Saved body from a REWRITTEN session.')).toBeTruthy();
    expect(screen.getByText('Saved subject')).toBeTruthy();
    expect(saveGenerationMock).toHaveBeenCalledWith(
      expect.objectContaining({
        emailSubject: 'Saved subject',
        emailBody: 'Saved body from a REWRITTEN session.',
      }),
      expect.anything()
    );
  });

  it('shows the empty state (not a stale draft) when the saved record has no email', () => {
    render(
      <ApplyByEmailTab
        application={makeApp()}
        matchingGenerations={[makeSavedGeneration({ emailSubject: '', emailBody: '' })]}
      />
    );

    expect(screen.getByText('applications.detail.email.empty')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'applications.detail.email.generate' })).toBeTruthy();
  });

  it('falls back to the saved draft when a generation fails mid-stream', async () => {
    const savedBody = 'Saved body from a previous session.';
    const saved = makeSavedGeneration({ emailSubject: 'Saved subject', emailBody: savedBody });
    // Emit a partial draft, then blow up — the pre-fix bug left `streamText`
    // non-empty forever, which pinned the derivation on the truncated fragment
    // and masked the persisted draft until the tab was remounted.
    generateEmailMock.mockImplementation(async (p) => {
      p.onToken?.('Subject: Half-written\n\nThis got cut o');
      throw new Error('stream died');
    });

    renderTab({}, [saved]);
    await clickGenerate('regenerate');

    expect(screen.getByText('applications.detail.email.genError')).toBeTruthy();
    // The saved draft is back on screen; the truncated fragment is gone.
    expect(screen.getByText(savedBody)).toBeTruthy();
    expect(screen.getByText('Saved subject')).toBeTruthy();
    expect(screen.queryByText('This got cut o')).toBeNull();
    expect(screen.queryByText('Half-written')).toBeNull();

    // ...and both copy buttons act on the saved draft, not the abandoned fragment.
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copySubject' }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Saved subject');
    });
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.copy' }));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(savedBody);
    });
  });

  it('replaces the hydrated draft with the live stream while re-generating', async () => {
    const saved = makeSavedGeneration({
      emailSubject: 'Saved subject',
      emailBody: 'Saved body from a previous session.',
    });
    // Never-resolving generation: the tab stays in the "generating, no tokens
    // yet" window, where the stale saved draft must NOT be on screen.
    generateEmailMock.mockImplementation(() => new Promise<string>(() => {}));

    const { container } = renderTab({}, [saved]);
    fireEvent.click(screen.getByRole('button', { name: 'applications.detail.email.regenerate' }));

    await waitFor(() => {
      expect(container.querySelector('.animate-skeleton')).not.toBeNull();
    });
    expect(screen.queryByText('Saved body from a previous session.')).toBeNull();
    expect(screen.queryByText('Saved subject')).toBeNull();
  });
});
