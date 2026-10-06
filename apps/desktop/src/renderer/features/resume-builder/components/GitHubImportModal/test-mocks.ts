/**
 * Mock state + factories for the GitHubImportModal tests. `vi.mock` is hoisted per
 * test file, so each file wires these in with
 * `vi.mock('<pkg>', async () => (await import('./test-mocks')).<factory>())`.
 * Kept free of any import of the component so a factory never awaits a module
 * that itself imports the mocked package.
 */
import { type Mock, vi } from 'vitest';

import type { GitHubRepo } from '@ajh/shared';
import type * as AjhUi from '@ajh/ui';

export const mockImportRepos = vi.fn<(input: string) => Promise<GitHubRepo[]>>();
export const mockGenerate =
  vi.fn<
    (params: {
      repos: GitHubRepo[];
      model: string;
    }) => Promise<{ name: string; description: string; link: string }[]>
  >();
export const mockNotify: Record<
  'open' | 'success' | 'error' | 'info' | 'warning' | 'destroy',
  Mock
> = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

let githubProfile: string | undefined = undefined;
export const setGithubProfile = (v: string | undefined) => {
  githubProfile = v;
};

export const translationsMock = () => ({
  useTranslation: () => ({
    t: (k: string, opts?: Record<string, unknown>) => {
      // Interpolate {{ count }} placeholders for addSelected / repoCount assertions.
      if (opts && typeof opts.count === 'number') return `${k}:${String(opts.count)}`;
      return k;
    },
  }),
});

export const uiMock = (actual: typeof AjhUi): Record<string, unknown> => ({
  ...actual,
  useNotification: () => mockNotify,
});

// The model and its provider must agree — 'openai/gpt-4o' can only come
// from the 'openai' provider, never 'ollama' (a local provider never serves
// an OpenAI-namespaced model).
export const modelSelectorMock = () => ({
  useSelectedModel: () => 'openai/gpt-4o',
  useSelectedProvider: () => 'openai',
});

export const contactProfileMock = () => ({
  useContactProfile: () => ({ data: githubProfile ? { github: githubProfile } : undefined }),
});

export const githubImportMock = () => ({
  useGitHubImport: () => ({ mutateAsync: mockImportRepos }),
});

export const generateMock = () => ({ generateGitHubProjects: mockGenerate });

/** Reset the spies + profile (call in `beforeEach`). */
export function resetMocks() {
  setGithubProfile(undefined);
  mockImportRepos.mockReset();
  mockGenerate.mockReset();
  mockNotify.error.mockReset();
  mockNotify.success.mockReset();
}
