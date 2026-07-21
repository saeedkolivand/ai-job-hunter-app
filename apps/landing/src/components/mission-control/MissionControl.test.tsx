// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { MC_CONFIG } from '@/lib/mission-control/config';
import { saveToken } from '@/lib/mission-control/pat';

import { MissionControl } from './MissionControl';

const TOKEN = 'github_pat_11ABCDEFG_secret000';

// A stale, unanswered issue whose title carries a script-injection attempt — it
// must render as inert text (React escaping), never as live DOM.
const HOSTILE_TITLE = '<img src=x onerror=alert(1)>pwn';
const staleIssue = {
  number: 9,
  title: HOSTILE_TITLE,
  html_url: 'https://github.com/x/y/issues/9',
  state: 'open',
  created_at: new Date(Date.now() - 40 * 24 * 3600 * 1000).toISOString(),
  updated_at: new Date(Date.now() - 40 * 24 * 3600 * 1000).toISOString(),
  closed_at: null,
  comments: 0,
  labels: [],
  user: { login: 'saeed' },
};

interface FakeInit {
  ok?: boolean;
  status?: number;
  headers?: Record<string, string>;
  body?: unknown;
}
function makeRes(init: FakeInit = {}) {
  const map = new Map(Object.entries(init.headers ?? {}));
  return {
    ok: init.ok ?? true,
    status: init.status ?? 200,
    headers: { get: (key: string) => map.get(key) ?? null },
    json: () => Promise.resolve(init.body ?? {}),
  };
}

function routeGet(url: string): unknown {
  // Match a substring shared by the live REST path and the snapshot filename
  // (e.g. '/issues?state=open' and '/metrics/issues.json' both contain 'issues')
  // so these fixtures serve both data-source modes.
  if (url.includes('meta.json')) return { generatedAt: new Date().toISOString() };
  if (url.includes('issues')) return [staleIssue];
  if (url.includes('runs')) return { workflow_runs: [] };
  if (url.includes('releases')) return [];
  if (url.includes('commits')) return [];
  if (url.includes('pulls')) return [];
  // The primary repo call (live '' → apiBase exactly; snapshot '/metrics/repo.json').
  return { stargazers_count: 12, forks_count: 3, subscribers_count: 4, open_issues_count: 1 };
}

let fetchMock: ReturnType<typeof vi.fn>;

function installFetch() {
  fetchMock = vi.fn((url: string, init?: RequestInit) => {
    const method = init?.method ?? 'GET';
    if (method === 'GET') return Promise.resolve(makeRes({ body: routeGet(url) }));
    return Promise.resolve(makeRes({ ok: true, status: 200 })); // writes
  });
  vi.stubGlobal('fetch', fetchMock);
}

function writeCalls() {
  return fetchMock.mock.calls.filter(
    (call) => ((call[1] as RequestInit | undefined)?.method ?? 'GET') !== 'GET'
  );
}

beforeEach(() => {
  localStorage.clear();
  installFetch();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('MissionControl', () => {
  it('renders the whole-repo verdict from the live API', async () => {
    render(<MissionControl />);
    await waitFor(() => expect(screen.getByLabelText('Repository verdict')).toBeTruthy());
    expect(screen.getByText(/the verdict/i)).toBeTruthy();
  });

  it('escapes an untrusted issue title (no dangerouslySetInnerHTML anywhere)', async () => {
    saveToken(TOKEN);
    render(<MissionControl />);
    await screen.findByText((t) => t.includes('pwn'));
    // The hostile markup rendered as inert text, not a live <img>.
    expect(document.querySelector('img')).toBeNull();
  });

  it('gates a write behind the confirm dialog and never sends the token in the URL', async () => {
    saveToken(TOKEN);
    render(<MissionControl />);

    const closeBtn = await screen.findByRole('button', { name: 'Close' });
    fireEvent.click(closeBtn);

    // Dialog opens; NO write request has fired yet.
    const dialog = await screen.findByRole('alertdialog');
    expect(dialog).toBeTruthy();
    expect(writeCalls()).toHaveLength(0);

    // Cancelling makes no request.
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
    expect(writeCalls()).toHaveLength(0);

    // Re-open and confirm → exactly one PATCH, auth in header, token not in URL.
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    await screen.findByRole('alertdialog');
    fireEvent.click(screen.getByRole('button', { name: 'Yes, do it' }));

    await waitFor(() => expect(writeCalls()).toHaveLength(1));
    const [url, init] = writeCalls()[0] as [string, RequestInit];
    expect(url).toBe(`${MC_CONFIG.apiBase}/issues/9`);
    expect(url).not.toContain(TOKEN);
    expect(init.method).toBe('PATCH');
    expect((init.headers as Record<string, string>).Authorization).toBe(`Bearer ${TOKEN}`);
  });

  it('shows no write controls at all when signed out', async () => {
    render(<MissionControl />);
    await waitFor(() => expect(screen.getByLabelText('Repository verdict')).toBeTruthy());
    expect(screen.queryByRole('button', { name: 'Close' })).toBeNull();
    expect(screen.queryByRole('button', { name: /Dispatch release/i })).toBeNull();
  });
});
