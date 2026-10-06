/**
 * ApplicationDetailPage — job description: Brief tab recovery panel + Documents tab debounced persist
 */

import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ApplicationDetailPage, renderLoaded } from './test-render';
import {
  makeApp,
  mockApp,
  mockImportJobUrlMutate,
  mockUpdateApplicationMutate,
  setLoaded,
  state,
} from './test-support';

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — Brief & answers tab
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — Brief & answers tab', () => {
  beforeEach(() => {
    state.tab = 'brief';
  });

  // The generic briefEmpty EmptyState early-return was removed: an empty
  // brief/answers/JD stub now renders the JD recovery panel (paste/fetch) instead
  // of vanishing — that panel IS the empty experience for a partial import.
  it('renders the JD recovery panel (paste TextArea + notFound prompt) for an empty stub', () => {
    renderLoaded({ brief: '', answers: [], jobDescription: '' });
    // The recovery prompt + paste field are shown; the old EmptyState is gone.
    // useResolveJobUrl mock defaults isFetching=false so jdLoading=false → panel visible.
    expect(screen.getByText('jobUrlImport.notFound')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('applications.detail.jdPlaceholder')).toBeInTheDocument();
    expect(screen.queryByText('applications.detail.briefEmpty')).not.toBeInTheDocument();
  });

  it('shows loading skeleton while auto-resolve is in-flight (jdLoading gate)', () => {
    state.resolveFetching = true;
    renderLoaded({ brief: '', answers: [], jobDescription: '' });
    // While resolve is fetching, the not-found/paste recovery panel must NOT show —
    // that would be a false-empty flash before the description arrives.
    expect(screen.queryByText('jobUrlImport.notFound')).not.toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText('applications.detail.jdPlaceholder')
    ).not.toBeInTheDocument();
    // The loading sentinel is present (aria-busy).
    const busyEl = screen.getByRole('status');
    expect(busyEl).toHaveAttribute('aria-busy', 'true');
  });

  it('typing into the paste TextArea updates it and enables the Save button', async () => {
    const user = userEvent.setup();
    renderLoaded({ brief: '', answers: [], jobDescription: '' });

    const paste = screen.getByPlaceholderText('applications.detail.jdPlaceholder');
    const save = screen.getByRole('button', { name: /applications\.detail\.jdSave/i });
    // Disabled while the draft is empty (fix 3a: value is the draft, so typing sticks).
    expect(save).toBeDisabled();

    await user.type(paste, 'Pasted JD text');

    expect(paste).toHaveValue('Pasted JD text');
    expect(save).toBeEnabled();
  });

  it('clicking Save persists the pasted JD via the update mutation', async () => {
    const user = userEvent.setup();
    renderLoaded({ id: 'app-jd-save', brief: '', answers: [], jobDescription: '' });

    await user.type(screen.getByPlaceholderText('applications.detail.jdPlaceholder'), 'New JD');
    await user.click(screen.getByRole('button', { name: /applications\.detail\.jdSave/i }));

    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith(
      { id: 'app-jd-save', jobDescription: 'New JD' },
      expect.any(Object)
    );
  });

  it('clicking Fetch resolves the description and persists it on success', async () => {
    const user = userEvent.setup();
    // Drive onSuccess synchronously with a posting carrying a description.
    mockImportJobUrlMutate.mockImplementation((_url, opts) => {
      opts?.onSuccess?.({ description: 'Fetched JD body' });
    });
    renderLoaded({ id: 'app-jd-fetch', brief: '', answers: [], jobDescription: '' });

    await user.click(screen.getByRole('button', { name: /applications\.detail\.jdFetch/i }));

    expect(mockImportJobUrlMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jd-fetch',
      jobDescription: 'Fetched JD body',
    });
  });

  it('shows the fetch-failed message when the JD fetch errors', () => {
    state.importIsError = true;
    renderLoaded({ brief: '', answers: [], jobDescription: '' });
    expect(screen.getByText('jobUrlImport.failed')).toBeInTheDocument();
  });

  it('renders the brief text and all answers when both are present', () => {
    renderLoaded({
      brief: 'Great company.',
      answers: [
        { id: 'qa1', question: 'Q1?', answer: 'A1.' },
        { id: 'qa2', question: 'Q2?', answer: 'A2.' },
      ],
    });
    expect(screen.getByText('Great company.')).toBeInTheDocument();
    expect(screen.getByText('Q1?')).toBeInTheDocument();
    expect(screen.getByText('Q2?')).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ApplicationDetailPage — DocumentsTab debounced jobDescription persist
// ─────────────────────────────────────────────────────────────────────────────

describe('ApplicationDetailPage — DocumentsTab debounced jobDescription persist', () => {
  beforeEach(() => {
    state.tab = 'documents';
  });

  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('passes onJobDescChange to TailorFlow on the documents tab', () => {
    renderLoaded({ id: 'app-jdc-1' });
    // The TailorFlow stub captures onJobDescChange — it must be a function.
    expect(typeof state.capturedOnJobDescChange).toBe('function');
  });

  it('calls updateApplication.mutate with jobDescription after the 600ms debounce', () => {
    renderLoaded({ id: 'app-jdc-2' });

    // Simulate a job-ad edit via the captured callback (mirrors what TailorFlow calls).
    act(() => {
      state.capturedOnJobDescChange?.('New pasted job ad text');
    });

    // Before the debounce fires: mutate must NOT have been called yet.
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();

    // Advance past the 600ms debounce window.
    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-2',
      jobDescription: 'New pasted job ad text',
    });
  });

  it('debounce resets on rapid edits — only the last value is persisted', () => {
    renderLoaded({ id: 'app-jdc-3' });

    act(() => {
      state.capturedOnJobDescChange?.('first');
    });
    act(() => {
      vi.advanceTimersByTime(300); // 300ms — debounce not yet fired
    });
    act(() => {
      state.capturedOnJobDescChange?.('second');
    });
    act(() => {
      vi.advanceTimersByTime(300); // another 300ms — debounce for 'first' would have fired, but was reset
    });

    // Debounce not yet complete for 'second' (only 300ms since last edit).
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(300); // total 600ms since 'second' → fires
    });

    // Only one call, with the last value.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-3',
      jobDescription: 'second',
    });
  });

  it('unmount flushes the pending edit immediately — edit is not lost when tab changes before 600ms', () => {
    // Regression guard for the blocking bug: user pastes job ad then switches tabs
    // within 600ms → DocumentsTab unmounts → pending write must still fire.
    const { unmount } = render(
      (() => {
        const app = makeApp({ id: 'app-jdc-flush' });
        setLoaded(app);
        return <ApplicationDetailPage />;
      })()
    );

    // Simulate the user editing the job ad text.
    act(() => {
      state.capturedOnJobDescChange?.('Pasted job ad — not yet saved');
    });

    // The debounce window has NOT elapsed yet — mutate must not have fired.
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalled();

    // User switches tabs: DocumentsTab unmounts before the 600ms window.
    act(() => {
      unmount();
    });

    // The cleanup flush must have fired the mutate with the latest pending value.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-flush',
      jobDescription: 'Pasted job ad — not yet saved',
    });
  });

  it('unmount flush uses the latest edit value when multiple rapid edits preceded the unmount', () => {
    // Edge case: user types 'first', then 'second', then unmounts — only 'second' should flush.
    const { unmount } = render(
      (() => {
        const app = makeApp({ id: 'app-jdc-flush-latest' });
        setLoaded(app);
        return <ApplicationDetailPage />;
      })()
    );

    act(() => {
      state.capturedOnJobDescChange?.('first draft');
    });
    act(() => {
      vi.advanceTimersByTime(200); // still within debounce
    });
    act(() => {
      state.capturedOnJobDescChange?.('second draft — the keeper');
    });

    // Unmount before the debounce completes.
    act(() => {
      unmount();
    });

    // Only one flush call with the most-recent pending value.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-jdc-flush-latest',
      jobDescription: 'second draft — the keeper',
    });
  });

  it('wrong-application regression: flush targets the id captured at edit time, not the current application id', () => {
    // Regression guard for Fix 1 (CodeRabbit MAJOR):
    // User edits Application A's job ad. Before the 600ms fires, the component
    // instance is reused for Application B (same DocumentsTab instance, different
    // `application` prop). The flush must write to A's id, not B's.
    //
    // We simulate the A→B reuse by rendering with app A, calling onJobDescChange,
    // then re-rendering with app B (no unmount), then advancing the timer.
    const appA = makeApp({ id: 'app-a', jobDescription: '' });
    setLoaded(appA);

    const { rerender } = render(<ApplicationDetailPage />);

    // Simulate A's job-ad edit (captures id='app-a' + text).
    act(() => {
      state.capturedOnJobDescChange?.("Application A's job ad");
    });

    // Reuse: re-render with application B before the debounce fires.
    const appB = makeApp({ id: 'app-b', jobDescription: '' });
    mockApp(appB);
    rerender(<ApplicationDetailPage />);

    // Advance past the debounce window — the pending flush from A must fire.
    act(() => {
      vi.advanceTimersByTime(600);
    });

    // Must have been called exactly once with A's id and A's text.
    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenCalledWith({
      id: 'app-a',
      jobDescription: "Application A's job ad",
    });
    // B's id must never appear in the mutate call.
    expect(mockUpdateApplicationMutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ id: 'app-b' })
    );
  });

  it('multiple complete debounce cycles each persist their own value independently', () => {
    // Verifies that after the first debounce fires and clears pendingJd, a second
    // edit round-trips correctly and is not lost or merged with the first.
    renderLoaded({ id: 'app-jdc-cycles' });

    // First edit cycle.
    act(() => {
      state.capturedOnJobDescChange?.('first value');
    });
    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(mockUpdateApplicationMutate).toHaveBeenNthCalledWith(1, {
      id: 'app-jdc-cycles',
      jobDescription: 'first value',
    });

    // Second edit cycle — after the first timer has already fired.
    act(() => {
      state.capturedOnJobDescChange?.('second value');
    });
    act(() => {
      vi.advanceTimersByTime(600);
    });

    expect(mockUpdateApplicationMutate).toHaveBeenCalledTimes(2);
    expect(mockUpdateApplicationMutate).toHaveBeenNthCalledWith(2, {
      id: 'app-jdc-cycles',
      jobDescription: 'second value',
    });
  });
});
