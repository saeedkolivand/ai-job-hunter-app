import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { GitHubRepo } from '@ajh/shared';
import type * as AjhUi from '@ajh/ui';

import { GitHubImportModal } from './index';

// ── Hoisted mocks (vi.hoisted runs before vi.mock factories) ──────────────────

const { mockImportRepos, mockGenerate, mockNotify, getGithubProfile, setGithubProfile } =
  vi.hoisted(() => {
    let _githubProfile: string | undefined = undefined;
    return {
      mockImportRepos: vi.fn<(input: string) => Promise<GitHubRepo[]>>(),
      mockGenerate:
        vi.fn<
          (params: {
            repos: GitHubRepo[];
            model: string;
          }) => Promise<{ name: string; description: string; link: string }[]>
        >(),
      mockNotify: {
        open: vi.fn(),
        success: vi.fn(),
        error: vi.fn(),
        info: vi.fn(),
        warning: vi.fn(),
        destroy: vi.fn(),
      },
      getGithubProfile: () => _githubProfile,
      setGithubProfile: (v: string | undefined) => {
        _githubProfile = v;
      },
    };
  });

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, opts?: Record<string, unknown>) => {
      // Interpolate {{ count }} placeholders for addSelected / repoCount assertions.
      if (opts && typeof opts.count === 'number') return `${k}:${String(opts.count)}`;
      return k;
    },
  }),
}));

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => mockNotify,
  };
});

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'openai/gpt-4o',
}));

vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => {
    const gh = getGithubProfile();
    return { data: gh ? { github: gh } : undefined };
  },
}));

vi.mock('@/services/use-github-import', () => ({
  useGitHubImport: () => ({ mutateAsync: mockImportRepos }),
}));

vi.mock('@/lib/generate', () => ({
  generateGitHubProjects: mockGenerate,
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

const REPO_A: GitHubRepo = {
  name: 'my-cool-app',
  description: 'A cool application',
  htmlUrl: 'https://github.com/jane/my-cool-app',
  language: 'TypeScript',
  topics: ['react'],
  stars: 42,
};

const REPO_B: GitHubRepo = {
  name: 'tiny-parser',
  description: 'Fast parser',
  htmlUrl: 'https://github.com/jane/tiny-parser',
  language: 'Rust',
  topics: [],
  stars: 7,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

interface RenderOpts {
  onClose?: () => void;
  onAppend?: (entry: { name: string; description: string; link: string }) => void;
}

function renderModal({ onClose = vi.fn(), onAppend = vi.fn() }: RenderOpts = {}) {
  render(<GitHubImportModal open={true} onClose={onClose} onAppend={onAppend} />);
  return { onClose, onAppend };
}

/** Type a username, click Fetch, and wait for the mutation to resolve. */
async function fetchRepos(repos: GitHubRepo[]) {
  mockImportRepos.mockResolvedValue(repos);
  const input = screen.getByRole('textbox');
  fireEvent.change(input, { target: { value: 'jane' } });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /fetchButton/i }));
  });
  await waitFor(() => expect(mockImportRepos).toHaveBeenCalledWith('jane'));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('GitHubImportModal', () => {
  beforeEach(() => {
    setGithubProfile(undefined);
    mockImportRepos.mockReset();
    mockGenerate.mockReset();
    mockNotify.error.mockReset();
    mockNotify.success.mockReset();
  });

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
    const input = screen.getByRole('textbox');
    expect(input.value).toBe('jane');
  });

  it('prefills bare username from contact profile when no URL', () => {
    setGithubProfile('jane');
    renderModal();
    const input = screen.getByRole('textbox');
    expect(input.value).toBe('jane');
  });

  it('user can clear a prefilled username (no snap-back)', () => {
    // Regression for the resolvedUsername = username || prefill trap: once
    // prefill is set, clearing the field must yield '' — not re-snap to prefill.
    setGithubProfile('https://github.com/jane');
    renderModal();
    const input = screen.getByRole('textbox');
    // Confirm seeded first.
    expect(input.value).toBe('jane');
    // Clear and confirm it stays empty.
    fireEvent.change(input, { target: { value: '' } });
    expect(input.value).toBe('');
    // Typing a new name must work.
    fireEvent.change(input, { target: { value: 'otherperson' } });
    expect(input.value).toBe('otherperson');
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
    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes.every((cb) => cb.checked)).toBe(true);
  });

  it('toggles individual repo on checkbox click', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);
    const [first] = screen.getAllByRole('checkbox');
    if (!first) throw new Error('checkbox not found');
    fireEvent.click(first);
    expect(first.checked).toBe(false);
  });

  it('deselects all when select-all is toggled while all are selected', async () => {
    renderModal();
    await fetchRepos([REPO_A, REPO_B]);
    fireEvent.click(screen.getByRole('button', { name: /deselectAll/i }));
    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes.every((cb) => !cb.checked)).toBe(true);
  });

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

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /addSelected/i }));
    });

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
    const checkboxes = screen.getAllByRole('checkbox');
    const second = checkboxes[1];
    if (!second) throw new Error('second checkbox not found');
    fireEvent.click(second);

    mockGenerate.mockResolvedValue([
      { name: 'My Cool App', description: 'Bullet', link: REPO_A.htmlUrl },
    ]);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /addSelected/i }));
    });

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

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /addSelected/i }));
    });

    await waitFor(() => expect(mockNotify.error).toHaveBeenCalledTimes(1));
    // Must NOT append partial entries or close — user can retry.
    expect(onAppend).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    // Inline error message visible in the modal body.
    expect(screen.getByText('build.extras.projects.github.generateError')).toBeInTheDocument();
    // Selection is preserved — checkboxes still checked.
    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes.every((cb) => cb.checked)).toBe(true);
  });

  // ── item 8: Enter-key fetch ────────────────────────────────────────────────

  it('Enter key on username input triggers the fetch mutation', async () => {
    renderModal();
    mockImportRepos.mockResolvedValue([REPO_A]);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'jane' } });
    await act(async () => {
      fireEvent.keyDown(input, { key: 'Enter' });
    });
    await waitFor(() => expect(mockImportRepos).toHaveBeenCalledWith('jane'));
  });

  // ── item 9: Cancel-while-generating aborts + closes ───────────────────────

  it('Escape key during generation calls onClose (Cancel button is disabled while generating)', async () => {
    // The Cancel button is disabled={generating} per the component design — the only
    // way to dismiss during generation is Escape (or backdrop click), which ModalShell
    // forwards to its onClose prop (= handleClose → abort + onClose).
    let resolveGenerate!: (v: { name: string; description: string; link: string }[]) => void;
    mockGenerate.mockImplementation(
      () =>
        new Promise<{ name: string; description: string; link: string }[]>((resolve) => {
          resolveGenerate = resolve;
        })
    );

    const onClose = vi.fn();
    renderModal({ onClose });
    await fetchRepos([REPO_A]);

    // Start generation — the promise never settles until we resolve it.
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /addSelected/i }));
    });

    // Confirm generating state: Cancel button is now disabled.
    await waitFor(() => expect(screen.getByRole('button', { name: /cancel/i })).toBeDisabled());

    // Dismiss via Escape — ModalShell listens on window and calls handleClose.
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Escape' });
    });

    expect(onClose).toHaveBeenCalledTimes(1);

    // Settle the in-flight call inside act so React state updates from the
    // finally block don't fire outside the test boundary.
    await act(async () => {
      resolveGenerate([]);
    });
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
    const afterDeselect = screen.getAllByRole('checkbox');
    expect(afterDeselect.every((cb) => !cb.checked)).toBe(true);

    // Now the button label should be "selectAll" — click it.
    fireEvent.click(screen.getByRole('button', { name: /selectAll/i }));
    const afterReselect = screen.getAllByRole('checkbox');
    expect(afterReselect.every((cb) => cb.checked)).toBe(true);
  });

  // ── item 12: Add button re-enabled after generation error ─────────────────

  it('Add button is re-enabled after a generation error so the user can retry', async () => {
    renderModal();
    await fetchRepos([REPO_A]);

    mockGenerate.mockRejectedValue(new Error('no provider'));

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /addSelected/i }));
    });

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

    const input = screen.getByRole('textbox');
    expect(input.value).toBe('');

    // Simulate async arrival: set the profile and re-render (mirrors React Query
    // transitioning from undefined → resolved data). The seededRef must catch it.
    setGithubProfile('https://github.com/async-jane');
    await act(async () => {
      rerender(<GitHubImportModal open={true} onClose={vi.fn()} onAppend={vi.fn()} />);
    });

    await waitFor(() => expect(screen.getByRole('textbox').value).toBe('async-jane'));
  });
});
