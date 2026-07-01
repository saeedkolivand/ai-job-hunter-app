import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { GitHubRepo } from '@ajh/shared/ipc';

// Mock the Tauri transport so the envelope-unwrap logic can be exercised in node.
const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { github } from './github';

beforeEach(() => {
  invoke.mockReset();
});

const repo = (name: string): GitHubRepo => ({
  name,
  htmlUrl: `https://github.com/u/${name}`,
  topics: [],
  stars: 1,
});

describe('github.importRepos', () => {
  it('unwraps the { repos } envelope on success', async () => {
    invoke.mockResolvedValue({ repos: [repo('a'), repo('b')] });
    await expect(github.importRepos('octocat')).resolves.toEqual([repo('a'), repo('b')]);
    expect(invoke).toHaveBeenCalledWith('github_import_repos', { input: 'octocat' });
  });

  it('throws the backend error from an { error } envelope', async () => {
    invoke.mockResolvedValue({ error: 'GitHub user not found' });
    await expect(github.importRepos('nope')).rejects.toThrow('GitHub user not found');
  });

  it('throws on an undefined result instead of masking it as an empty list', async () => {
    // A missing command / capability denial / serialization failure resolves
    // undefined — that must surface as an error, never a silent "no repos".
    invoke.mockResolvedValue(undefined);
    await expect(github.importRepos('octocat')).rejects.toThrow(/unexpected response/);
  });

  it('throws on a malformed result (no repos array)', async () => {
    invoke.mockResolvedValue({ unexpected: true });
    await expect(github.importRepos('octocat')).rejects.toThrow(/unexpected response/);
  });

  it('throws a clean error (not a raw TypeError) on a primitive result', async () => {
    // `'error' in "oops"` would throw a TypeError if the `in` check ran on a
    // primitive; the object guard must catch a bare string first.
    invoke.mockResolvedValue('oops');
    await expect(github.importRepos('octocat')).rejects.toThrow(/unexpected response/);
  });
});
