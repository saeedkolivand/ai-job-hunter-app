import { expect, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { GitHubRepo } from '@ajh/shared';

import { GitHubImportModal } from './index';
import { mockImportRepos } from './test-mocks';

export const REPO_A: GitHubRepo = {
  name: 'my-cool-app',
  description: 'A cool application',
  htmlUrl: 'https://github.com/jane/my-cool-app',
  language: 'TypeScript',
  topics: ['react'],
  stars: 42,
};

export const REPO_B: GitHubRepo = {
  name: 'tiny-parser',
  description: 'Fast parser',
  htmlUrl: 'https://github.com/jane/tiny-parser',
  language: 'Rust',
  topics: [],
  stars: 7,
};

interface RenderOpts {
  onClose?: () => void;
  onAppend?: (entry: { name: string; description: string; link: string }) => void;
}

export function renderModal({ onClose = vi.fn(), onAppend = vi.fn() }: RenderOpts = {}) {
  render(<GitHubImportModal open={true} onClose={onClose} onAppend={onAppend} />);
  return { onClose, onAppend };
}

/** Narrowed textbox accessor — tsc requires explicit cast from HTMLElement. */
export function getTextbox(): HTMLInputElement {
  return screen.getByRole('textbox');
}

/** Narrowed checkbox list — tsc requires explicit cast from HTMLElement[]. */
export function getCheckboxes(): HTMLInputElement[] {
  return screen.getAllByRole('checkbox');
}

/** Type a username, click Fetch, and wait for the mutation to resolve. */
export async function fetchRepos(repos: GitHubRepo[]) {
  mockImportRepos.mockResolvedValue(repos);
  fireEvent.change(getTextbox(), { target: { value: 'jane' } });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /fetchButton/i }));
  });
  await waitFor(() => expect(mockImportRepos).toHaveBeenCalledWith('jane'));
}

/** Click "Add selected" inside `act` so the generation promise chain flushes. */
export async function clickAddSelected() {
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /addSelected/i }));
  });
}
