/**
 * ModelSelector — cloud model-list health for the ACTIVE provider (live-model-lists
 * PR — the curated cloud fallback is gone; catalogues are fetched live with a
 * last-good local cache):
 *  - Not connected (no stored key) → a neutral "add a key" note, not an error.
 *  - Live fetch failed with a cached list available → the dropdown still lists the
 *    cached models, plus a "showing cached list" note.
 *  - Live fetch failed with NO cache → the real failure message.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { PROVIDER_ORDER, PROVIDERS } from '@/lib/ai-providers/provider-meta';

import { ModelSelector } from './index';
import { resetStub, stub } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-support')).translationsMock());
vi.mock('@/providers/AppClientProvider', async () =>
  (await import('./test-support')).appClientMock()
);
vi.mock('@/services', async () => (await import('./test-support')).servicesMock());
vi.mock('@tanstack/react-query', async (importOriginal) =>
  (await import('./test-support')).reactQueryMock(await importOriginal())
);

// A real cloud provider id, derived from the registry rather than hardcoded.
// Cloud providers carry no curated `models` list any more (live-model-lists PR) —
// every model in these tests comes from a stubbed `provider-models` query result.
const cloudProviderId = PROVIDER_ORDER.find((p) => PROVIDERS[p].kind === 'cloud');
if (!cloudProviderId) throw new Error('expected at least one cloud provider in the registry');

const renderSelector = () => render(<ModelSelector />);

beforeEach(resetStub);

describe('ModelSelector — active cloud provider, no key stored', () => {
  it('shows the neutral "add a key" note as a polite status region, not an error', () => {
    // No curated fallback exists any more (live-model-lists PR removed it) —
    // an un-keyed cloud provider must read as "add a key", never as a failure.
    stub.activeProvider = cloudProviderId;
    stub.activeProviderModel = '';
    stub.cloudKeyQueries = { [cloudProviderId]: { data: { has: false }, isLoading: false } };

    renderSelector();

    const note = screen.getByText('models.cloud.addKeyToLoad');
    expect(note).toBeInTheDocument();
    expect(note).toHaveAttribute('role', 'status');
    expect(note).toHaveAttribute('aria-live', 'polite');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
    expect(screen.queryByText('settings.aiModel.loading')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — no key stored, but a stale cache-served result lingers (CodeRabbit #936 MEDIUM)', () => {
  it('shows ONLY "add a key" — never alongside the cached-list note', () => {
    // The exact reported bug: the key was removed (or never granted) AFTER a
    // prior successful cache-served fetch, so the model query's `data` can
    // still carry `cached: true` even though `activeCloudNeedsKey` is now
    // true. `activeCloudCached` used to check only `!modelsLoading &&
    // !activeCloudErrorMessage` — not `!activeCloudNeedsKey` — so both notes
    // rendered together.
    stub.activeProvider = cloudProviderId;
    stub.activeProviderModel = '';
    stub.cloudKeyQueries = { [cloudProviderId]: { data: { has: false }, isLoading: false } };
    stub.cloudModelQueries = {
      [cloudProviderId]: {
        data: { models: [{ name: 'stale-model' }], cached: true },
        isLoading: false,
        isError: false,
      },
    };

    renderSelector();

    expect(screen.getByText('models.cloud.addKeyToLoad')).toBeInTheDocument();
    // None of the other three cloud-status hints — a single ordered chain
    // (`activeCloudStatus`) makes them structurally exclusive, not just this
    // one pairing: a future fifth state can't co-render with an earlier one
    // either.
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByText('settings.aiModel.loading')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — connected cloud provider, model list still loading', () => {
  it('shows a polite loading status — the state none of the other three hints cover', () => {
    stub.activeProvider = cloudProviderId;
    stub.activeProviderModel = '';
    stub.cloudKeyQueries = { [cloudProviderId]: { data: { has: true }, isLoading: false } };
    stub.cloudModelQueries = {
      [cloudProviderId]: { data: undefined, isLoading: true, isError: false },
    };

    renderSelector();

    const loading = screen.getByText('settings.aiModel.loading');
    expect(loading).toBeInTheDocument();
    expect(loading.closest('[role="status"]')).not.toBeNull();
    expect(screen.queryByText('models.cloud.addKeyToLoad')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — connected cloud provider, live fetch failed, cache available', () => {
  it('lists the cached models and notes the list is cached, as a polite status (not an error)', async () => {
    const cachedModel = 'cached-model-x';
    stub.activeProvider = cloudProviderId;
    stub.activeProviderModel = cachedModel;
    stub.cloudKeyQueries = { [cloudProviderId]: { data: { has: true }, isLoading: false } };
    // The fetch-with-cache wrapper RESOLVES (not an error) when a cached list
    // is available — only rejects when both the live fetch AND the cache miss.
    stub.cloudModelQueries = {
      [cloudProviderId]: {
        data: { models: [{ name: cachedModel }], cached: true },
        isLoading: false,
        isError: false,
      },
    };

    renderSelector();

    const note = screen.getByText('models.cloud.cachedList');
    expect(note).toBeInTheDocument();
    expect(note).toHaveAttribute('role', 'status');
    expect(screen.queryByText('models.cloud.fetchFailed')).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button'));
    expect(screen.getByRole('option', { name: cachedModel })).toBeInTheDocument();
  });
});

describe('ModelSelector — key-required cloud provider, key query still loading (CodeRabbit #936 Minor)', () => {
  it('shows the loading hint, not "add a key" — the key query has not settled yet', () => {
    // `cloudProviderId` is the first cloud provider in registry order
    // (key-required, not the keyless-exempt openai-compatible).
    stub.activeProvider = cloudProviderId;
    stub.activeProviderModel = '';
    // The key query itself is still in flight — `data` is not yet known.
    stub.cloudKeyQueries = { [cloudProviderId]: { data: undefined, isLoading: true } };

    renderSelector();

    // Before the fix, `canFetchModels` read the not-yet-loaded `false`
    // default and concluded "no key" immediately, telling a user who may
    // already have a key to add one.
    expect(screen.queryByText('models.cloud.addKeyToLoad')).not.toBeInTheDocument();
    const loading = screen.getByText('settings.aiModel.loading');
    expect(loading.closest('[role="status"]')).not.toBeNull();
  });
});

describe('ModelSelector — openai-compatible, key query loading (irrelevant to a keyless-but-configured provider)', () => {
  it('ignores the key query entirely — model-query state alone decides readiness', () => {
    stub.activeProvider = 'openai-compatible';
    stub.activeProviderModel = '';
    // Configured via a stored base URL (not a key) — the case #936 exists
    // for. Without it, "needs configuration" would correctly win instead.
    stub.openAiCompatibleBaseUrl = 'http://localhost:1234/v1';
    // The key query is still loading, but openai-compatible doesn't need a
    // key at all — this must not gate anything for it.
    stub.cloudKeyQueries = { 'openai-compatible': { data: undefined, isLoading: true } };
    stub.cloudModelQueries = {
      'openai-compatible': {
        data: { models: [{ name: 'local-model' }], cached: false },
        isLoading: false,
        isError: false,
      },
    };

    renderSelector();

    expect(screen.queryByText('settings.aiModel.loading')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.addKeyToLoad')).not.toBeInTheDocument();
  });
});

describe('ModelSelector — openai-compatible never configured (PR #937 finding 1)', () => {
  it('tells the user to add a base URL, not an API key — this is the default state for the population #936 exists for', () => {
    stub.activeProvider = 'openai-compatible';
    stub.activeProviderModel = '';
    // Neither a base URL nor a key — the default state for a user who has
    // never touched this provider (e.g. a fully-local Ollama user).
    stub.openAiCompatibleBaseUrl = undefined;
    stub.cloudKeyQueries = { 'openai-compatible': { data: { has: false }, isLoading: false } };

    renderSelector();

    // "Add an API key" is the one instruction that can never help here — the
    // provider only ever needs a base URL, or a key it doesn't have shown.
    expect(screen.queryByText('models.cloud.addKeyToLoad')).not.toBeInTheDocument();
    const note = screen.getByText('models.cloud.addUrlToLoad');
    expect(note).toBeInTheDocument();
    expect(note).toHaveAttribute('role', 'status');
  });
});

describe('ModelSelector — openai-compatible authenticated by a KEY (no base URL), key query loading (PR #937 finding 2)', () => {
  it('shows the loading hint, not "add a key/URL" — a stored key makes the key query relevant again', () => {
    stub.activeProvider = 'openai-compatible';
    stub.activeProviderModel = '';
    // No base URL configured — this user authenticates with a stored KEY
    // instead, so (unlike the keyless-via-base-URL case above) its readiness
    // DOES depend on the key query settling.
    stub.openAiCompatibleBaseUrl = undefined;
    stub.cloudKeyQueries = { 'openai-compatible': { data: undefined, isLoading: true } };

    renderSelector();

    // Before the fix, `activeKeyRequired` excluded openai-compatible
    // unconditionally, so `activeKeyLoading` stayed `false` while this query
    // was still in flight — `canFetchModels` read the not-yet-loaded `false`
    // default and told a user who has a key to add one (or, after finding 1's
    // fix, to add a base URL — equally wrong for someone authenticated by key).
    expect(screen.queryByText('models.cloud.addKeyToLoad')).not.toBeInTheDocument();
    expect(screen.queryByText('models.cloud.addUrlToLoad')).not.toBeInTheDocument();
    const loading = screen.getByText('settings.aiModel.loading');
    expect(loading.closest('[role="status"]')).not.toBeNull();
  });
});

describe('ModelSelector — connected cloud provider, live fetch failed, no cache', () => {
  it('shows the real failure message as role="alert"', () => {
    stub.activeProvider = cloudProviderId;
    stub.activeProviderModel = '';
    stub.cloudKeyQueries = { [cloudProviderId]: { data: { has: true }, isLoading: false } };
    stub.cloudModelQueries = {
      [cloudProviderId]: {
        data: undefined,
        isLoading: false,
        isError: true,
        error: new Error('invalid or unauthorized API key'),
      },
    };

    renderSelector();

    // `t` is stubbed to the identity function and drops interpolation params,
    // so the key itself is what renders — the real message reaches the DOM
    // via the component's own `t(key, { message })` call, which is the
    // behaviour under test (PR 1's classified error finally has a reader).
    const failure = screen.getByText('models.cloud.fetchFailed');
    expect(failure).toBeInTheDocument();
    expect(failure.closest('[role="alert"]')).not.toBeNull();
    expect(screen.queryByText('models.cloud.cachedList')).not.toBeInTheDocument();
  });
});
