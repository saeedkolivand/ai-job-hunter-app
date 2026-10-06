import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, screen, waitFor } from '@testing-library/react';

import { mockImportRepos, resetMocks, setGithubProfile } from './test-mocks';
import { fetchRepos, getCheckboxes, getTextbox, renderModal, REPO_A, REPO_B } from './test-support';

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

describe('GitHubImportModal', () => {
  beforeEach(resetMocks);

  // ── Rendering + a11y ───────────────────────────────────────────────────────

  it('renders the fetch input and fetch button', () => {
    renderModal();
    expect(screen.getByRole('textbox')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /fetchButton/i })).toBeInTheDocument();
  });

  it('dialog element is labelled by the modal title via ariaLabelledby', () => {
    renderModal();
    const dialog = screen.getByRole('dialog');
    const labelledBy = dialog.getAttribute('aria-labelledby');
    expect(labelledBy).toBe('github-import-modal-title');
    // The element the dialog references must be present in the DOM.
    expect(document.getElementById('github-import-modal-title')).toBeInTheDocument();
  });

  // ── Prefill (useEffect seed — the trap fix) ────────────────────────────────

  it('prefills username from contact profile github URL', () => {
    setGithubProfile('https://github.com/jane');
    renderModal();
    expect(getTextbox().value).toBe('jane');
  });

  it('prefills bare username from contact profile when no URL', () => {
    setGithubProfile('jane');
    renderModal();
    expect(getTextbox().value).toBe('jane');
  });

  it('user can clear a prefilled username (no snap-back)', () => {
    // Regression for the resolvedUsername = username || prefill trap: once
    // prefill is set, clearing the field must yield '' — not re-snap to prefill.
    setGithubProfile('https://github.com/jane');
    renderModal();
    // Confirm seeded first.
    expect(getTextbox().value).toBe('jane');
    // Clear and confirm it stays empty.
    fireEvent.change(getTextbox(), { target: { value: '' } });
    expect(getTextbox().value).toBe('');
    // Typing a new name must work.
    fireEvent.change(getTextbox(), { target: { value: 'otherperson' } });
    expect(getTextbox().value).toBe('otherperson');
  });

  // ── Fetch → list ───────────────────────────────────────────────────────────

  it('shows repo list with name, language, and description after fetch', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);

    expect(screen.getByText('my-cool-app')).toBeInTheDocument();
    expect(screen.getByText('TypeScript')).toBeInTheDocument();
    expect(screen.getByText('A cool application')).toBeInTheDocument();
    expect(screen.getByText('tiny-parser')).toBeInTheDocument();
  });

  it('shows EmptyState when fetch returns empty list', async () => {
    renderModal();
    await fetchRepos([]);
    expect(screen.getByText('build.extras.projects.github.noRepos')).toBeInTheDocument();
  });

  it('shows error title from thrown Error when fetch fails', async () => {
    renderModal();
    mockImportRepos.mockRejectedValue(new Error('GitHub user not found'));
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'nobody' } });
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /fetchButton/i }));
    });
    await waitFor(() => expect(screen.getByText('GitHub user not found')).toBeInTheDocument());
  });

  // ── Multi-select ───────────────────────────────────────────────────────────

  it('selects all repos by default after fetch', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);
    expect(getCheckboxes().every((cb) => cb.checked)).toBe(true);
  });

  it('toggles individual repo on checkbox click', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);
    const [first] = getCheckboxes();
    if (!first) throw new Error('checkbox not found');
    fireEvent.click(first);
    expect(first.checked).toBe(false);
  });

  it('deselects all when select-all is toggled while all are selected', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);
    fireEvent.click(screen.getByRole('button', { name: /deselectAll/i }));
    expect(getCheckboxes().every((cb) => !cb.checked)).toBe(true);
  });
});
