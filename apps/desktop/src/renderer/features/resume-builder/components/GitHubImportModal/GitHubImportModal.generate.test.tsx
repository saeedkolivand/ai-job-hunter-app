import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { GitHubImportModal } from './index';
import {
  mockGenerate,
  mockImportRepos,
  mockNotify,
  resetMocks,
  setGithubProfile,
} from './test-mocks';
import {
  clickAddSelected,
  fetchRepos,
  getCheckboxes,
  getTextbox,
  renderModal,
  REPO_A,
  REPO_B,
} from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-mocks')).uiMock(await importOriginal())
);
vi.mock('@/components/ui/ModelSelector', async () =>
  (await import('./test-mocks')).modelSelectorMock()
);
vi.mock('@/services/use-contact-profile', async () =>
  (await import('./test-mocks')).contactProfileMock()
);
vi.mock('@/services/use-github-import', async () =>
  (await import('./test-mocks')).githubImportMock()
);
vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());

describe('GitHubImportModal — generate, cancel and reopen', () => {
  beforeEach(resetMocks);

  // ── Add selected → onAppend ────────────────────────────────────────────────

  it('calls onAppend for each generated project entry and then onClose', async () => {
    const onAppend = vi.fn();
    const onClose = vi.fn();
    renderModal({ onAppend, onClose });
    await fetchRepos([REPO_A, REPO_B]);

    mockGenerate.mockResolvedValue([
      { name: 'My Cool App', description: 'AI bullet A', link: REPO_A.htmlUrl },
      { name: 'Tiny Parser', description: 'AI bullet B', link: REPO_B.htmlUrl },
    ]);

    await clickAddSelected();

    await waitFor(() => expect(onAppend).toHaveBeenCalledTimes(2));
    expect(onAppend).toHaveBeenNthCalledWith(1, {
      name: 'My Cool App',
      description: 'AI bullet A',
      link: REPO_A.htmlUrl,
    });
    expect(onAppend).toHaveBeenNthCalledWith(2, {
      name: 'Tiny Parser',
      description: 'AI bullet B',
      link: REPO_B.htmlUrl,
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('passes only selected repos to generateGitHubProjects', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);

    // Deselect REPO_B (second checkbox).
    const second = getCheckboxes()[1];
    if (!second) throw new Error('second checkbox not found');
    fireEvent.click(second);

    mockGenerate.mockResolvedValue([
      { name: 'My Cool App', description: 'Bullet', link: REPO_A.htmlUrl },
    ]);

    await clickAddSelected();

    await waitFor(() => expect(mockGenerate).toHaveBeenCalledTimes(1));
    const callArgs = mockGenerate.mock.calls[0]?.[0];
    expect(callArgs?.repos).toHaveLength(1);
    expect(callArgs?.repos[0]?.name).toBe('my-cool-app');
  });

  // ── Generation hard-error path (modal stays open, no partial appends) ──────

  it('keeps modal open and shows inline error when generation throws — does NOT call onAppend or onClose', async () => {
    const onAppend = vi.fn();
    const onClose = vi.fn();
    renderModal({ onAppend, onClose });
    await fetchRepos([REPO_A]);

    mockGenerate.mockRejectedValue(new Error('no provider'));

    await clickAddSelected();

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledTimes(1));
    // Must NOT append partial entries or close — user can retry.
    expect(onAppend).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    // Inline error message visible in the modal body.
    expect(screen.getByText('build.extras.projects.github.generateError')).toBeInTheDocument();
    // Selection is preserved — checkboxes still checked.
    expect(getCheckboxes().every((cb) => cb.checked)).toBe(true);
  });

  // ── item 8: Enter-key fetch ────────────────────────────────────────────────

  it('Enter key on username input triggers the fetch mutation', async () => {
    renderModal();
    mockImportRepos.mockResolvedValue([REPO_A]);
    const input = getTextbox();
    fireEvent.change(input, { target: { value: 'jane' } });
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Enter' });
    });
    await waitFor(() => expect(mockImportRepos).toHaveBeenCalledWith('jane'));
  });

  // ── item 9: Cancel-while-generating aborts + closes (no double-append) ──────

  it('Escape during generation calls onClose exactly once and does NOT append — resolves with non-empty fallback', async () => {
    // Regression: generateGitHubProjects resolves (not throws) with a populated
    // fallback array even on abort. Without the aborted-guard, handleClose would
    // call onClose() once, then the resolved await would loop through onAppend for
    // each entry and call onClose() a second time — silently appending all repos.
    let resolveGenerate!: (v: { name: string; description: string; link: string }[]) => void;
    mockGenerate.mockImplementation(
      () =>
        new Promise<{ name: string; description: string; link: string }[]>((resolve) => {
          resolveGenerate = resolve;
        })
    );

    const onAppend = vi.fn();
    const onClose = vi.fn();
    renderModal({ onAppend, onClose });
    await fetchRepos([REPO_A]);

    // Start generation — the promise is held open.
    await clickAddSelected();

    // Confirm generating state: Cancel button is now disabled.
    await waitFor(() => expect(screen.getByRole('button', { name: /cancel/i })).toBeDisabled());

    // Dismiss via Escape — ModalShell forwards to handleClose → abort + onClose.
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    // handleClose fired onClose exactly once; generation still in flight.
    expect(onClose).toHaveBeenCalledTimes(1);

    // Now settle the in-flight call with a NON-EMPTY fallback array (the real
    // runtime behaviour: generateGitHubProjects returns raw-description entries on
    // abort, it does not throw). Without the aborted-guard this would trigger
    // the append loop and a second onClose call.
    await act(async () => {
      resolveGenerate([
        { name: REPO_A.name, description: REPO_A.description ?? '', link: REPO_A.htmlUrl },
      ]);
    });

    // onClose must still be exactly 1 (no double-close).
    expect(onClose).toHaveBeenCalledTimes(1);
    // onAppend must never have been called — user cancelled.
    expect(onAppend).not.toHaveBeenCalled();
  });

  // ── item 10: Add button disabled when zero repos selected ─────────────────

  it('Add button is disabled when no repos are selected', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);

    // Deselect all repos.
    fireEvent.click(screen.getByRole('button', { name: /deselectAll/i }));

    const addBtn = screen.getByRole('button', { name: /addSelected/i });
    expect(addBtn).toBeDisabled();
  });

  it('deselecting all repos prevents generateGitHubProjects from being called', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);

    fireEvent.click(screen.getByRole('button', { name: /deselectAll/i }));

    // Even if the user somehow clicks the (disabled) Add button, generate must not fire.
    expect(mockGenerate).not.toHaveBeenCalled();
  });

  // ── item 11: toggleAll select-all direction after deselect-all ───────────

  it('select-all re-checks all repos after deselect-all', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);

    // Deselect all.
    fireEvent.click(screen.getByRole('button', { name: /deselectAll/i }));
    expect(getCheckboxes().every((cb) => !cb.checked)).toBe(true);

    // Now the button label should be "selectAll" — click it.
    fireEvent.click(screen.getByRole('button', { name: /selectAll/i }));
    expect(getCheckboxes().every((cb) => cb.checked)).toBe(true);
  });

  // ── item 12: Add button re-enabled after generation error ─────────────────

  it('Add button is re-enabled after a generation error so the user can retry', async () => {
    renderModal();
    await fetchRepos([REPO_A]);

    mockGenerate.mockRejectedValue(new Error('no provider'));

    await clickAddSelected();

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledTimes(1));

    // The Add button must be enabled again (not stuck in disabled/generating state).
    const addBtn = screen.getByRole('button', { name: /addSelected/i });
    expect(addBtn).not.toBeDisabled();
  });

  // ── item 13: async prefill arrival (seededRef regression path) ────────────

  it('populates input when contact profile github resolves after initial render', async () => {
    // Render with no contact profile initially (input stays empty).
    setGithubProfile(undefined);
    const { rerender } = render(
      <GitHubImportModal open={true} onClose={vi.fn()} onAppend={vi.fn()} />
    );

    expect(getTextbox().value).toBe('');

    // Simulate async arrival: set the profile and re-render (mirrors React Query
    // transitioning from undefined → resolved data). The seededRef must catch it.
    setGithubProfile('https://github.com/async-jane');
    await act(async () => {
      rerender(<GitHubImportModal open={true} onClose={vi.fn()} onAppend={vi.fn()} />);
    });

    await waitFor(() => expect(getTextbox().value).toBe('async-jane'));
  });

  // ── item 14: reopen resets state — stale list + duplicate-append regression ──

  it('reopening the modal clears the previous fetch result and selection', async () => {
    const onAppend = vi.fn();
    const { rerender } = render(
      <GitHubImportModal open={true} onClose={vi.fn()} onAppend={onAppend} />
    );

    // First open: fetch and select repos.
    await fetchRepos([REPO_A, REPO_B]);
    expect(getCheckboxes()).toHaveLength(2);
    expect(getCheckboxes().every((cb) => cb.checked)).toBe(true);

    // Close the modal (open → false).
    await act(async () => {
      rerender(<GitHubImportModal open={false} onClose={vi.fn()} onAppend={onAppend} />);
    });

    // Reopen (false → true): state must be reset — no stale repos visible.
    await act(async () => {
      rerender(<GitHubImportModal open={true} onClose={vi.fn()} onAppend={onAppend} />);
    });

    // No checkboxes rendered (repo list cleared).
    expect(screen.queryAllByRole('checkbox')).toHaveLength(0);
    // Add button is disabled (nothing selected, fetchState !== done).
    expect(screen.getByRole('button', { name: /addSelected/i })).toBeDisabled();
    // No appends happened from the mere close/reopen cycle.
    expect(onAppend).not.toHaveBeenCalled();
  });

  // ── item 15: async prefill mid-session must NOT reset fetch state ─────────

  it('a late-arriving contact-profile prefill does NOT wipe a fetched repo list', async () => {
    // Open with no profile — prefill is ''.
    setGithubProfile(undefined);
    const { rerender } = render(
      <GitHubImportModal open={true} onClose={vi.fn()} onAppend={vi.fn()} />
    );

    // Fetch repos — list is now populated.
    await fetchRepos([REPO_A, REPO_B]);
    expect(getCheckboxes()).toHaveLength(2);
    expect(getCheckboxes().every((cb) => cb.checked)).toBe(true);

    // Profile resolves asynchronously while the modal is still open.
    setGithubProfile('https://github.com/late-jane');
    await act(async () => {
      rerender(<GitHubImportModal open={true} onClose={vi.fn()} onAppend={vi.fn()} />);
    });

    // The repo list must be PRESERVED — prefill change must not trigger a reset.
    expect(getCheckboxes()).toHaveLength(2);
    expect(getCheckboxes().every((cb) => cb.checked)).toBe(true);
    // Username field must NOT change (user already has a typed value from fetchRepos).
    expect(getTextbox().value).toBe('jane');
  });
});
