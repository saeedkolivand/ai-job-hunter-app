/**
 * EmbeddingsSettings — amber advisory visibility tests.
 *
 * Covers:
 *  - When provider !== 'ollama' the cloud-cost advisory is rendered
 *  - When provider === 'ollama' the advisory is NOT rendered
 *
 * All hooks that reach IPC or trigger side-effects are stubbed so the component
 * renders without a QueryClient / Notification provider tree. Only the
 * provider-selection side-effect (advisory visibility) is exercised here.
 *
 * Dropdown interaction pattern (from packages/ui Dropdown.test.tsx):
 *   click the trigger button → list portals to document.body as plain <button>s
 *   → click the option by text content (no ARIA role="option").
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

import type * as Services from '@/services';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stubs — prevent any IPC / QueryClient dependency ──────────────────

const { mockNotifyError } = vi.hoisted(() => ({ mockNotifyError: vi.fn() }));

// `useSetSemanticScoring` is deliberately NOT stubbed here: the mirror-write
// tests below drive the REAL mutation against a fake AppClient, because the
// question they answer ("does a failed backend write reach the user?") is
// decided by whether the IPC promise rejects — something a stubbed hook that
// calls `onError` by hand cannot observe. The rest are stubbed to keep the
// panel's unrelated IPC out of the render.
vi.mock('@/services', async (importOriginal) => ({
  ...(await importOriginal<typeof Services>()),
  useEmbeddingStatus: () => ({ data: undefined, refetch: vi.fn() }),
  useSetEmbeddingConfig: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useReembedAll: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useJobEvents: (_handler: unknown) => undefined,
}));

// ── useNotification stub (from @ajh/ui) — avoid Notification provider setup ──

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => ({
      open: vi.fn(),
      success: vi.fn(),
      error: mockNotifyError,
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

// ── component under test ──────────────────────────────────────────────────────

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, withProviders } from '@/test-support';

import { EmbeddingsSettings } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

/**
 * Render the panel with the provider tree the (real) `useSetSemanticScoring`
 * mutation needs. `setSemanticScoring` scripts the ONE IPC call these tests
 * care about; everything else on the client resolves `undefined`.
 */
function renderPanel(setSemanticScoring = vi.fn().mockResolvedValue(undefined)) {
  const client = createMockClient({
    'jobPreferences.setSemanticScoring': setSemanticScoring,
  });
  render(<EmbeddingsSettings />, { wrapper: withProviders(client) });
  return { setSemanticScoring };
}

// Unique fragment from the advisory paragraph. The component renders the
// localized string via `t(...)`; the i18n stub returns the key verbatim, so we
// match the advisory's translation key (its text content under the mock).
const ADVISORY_FRAGMENT = /settings\.embeddings\.cloudCostAdvisory/i;

/**
 * Open the Dropdown by clicking the trigger button whose text content matches
 * currentLabel, then click the option button matching optionLabel.
 *
 * The Dropdown portals its list to document.body as plain <button> elements
 * (no ARIA role="option"). After opening, the list items are all in the DOM so
 * getByText resolves them directly.
 */
async function switchProvider(
  user: ReturnType<typeof userEvent.setup>,
  currentLabel: string,
  optionLabel: string
) {
  const trigger = Array.from(document.querySelectorAll('button')).find((b) =>
    b.textContent?.includes(currentLabel)
  );
  if (!trigger) throw new Error(`Provider trigger with label "${currentLabel}" not found`);
  await user.click(trigger);
  await user.click(screen.getByText(optionLabel));
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('EmbeddingsSettings — ollama provider selected', () => {
  it('advisory is absent on initial render (default provider is ollama)', () => {
    renderPanel();
    expect(screen.queryByText(ADVISORY_FRAGMENT)).not.toBeInTheDocument();
  });

  it('advisory paragraph element is not in the DOM when ollama is active', () => {
    renderPanel();
    const advisoryEl = Array.from(document.querySelectorAll('p')).find((p) =>
      ADVISORY_FRAGMENT.test(p.textContent ?? '')
    );
    expect(advisoryEl).toBeUndefined();
  });
});

describe('EmbeddingsSettings — cloud provider selected', () => {
  it('shows advisory after switching from ollama to openai', async () => {
    const user = userEvent.setup();
    renderPanel();

    expect(screen.queryByText(ADVISORY_FRAGMENT)).not.toBeInTheDocument();
    await switchProvider(user, 'Ollama (Local)', 'OpenAI');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();
  });

  it('shows advisory after switching to gemini', async () => {
    const user = userEvent.setup();
    renderPanel();

    await switchProvider(user, 'Ollama (Local)', 'Gemini');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();
  });

  it('shows advisory after switching to openai-compatible', async () => {
    const user = userEvent.setup();
    renderPanel();

    await switchProvider(user, 'Ollama (Local)', 'OpenAI-compatible');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();
  });

  it('advisory disappears when switching back to ollama from a cloud provider', async () => {
    const user = userEvent.setup();
    renderPanel();

    // Switch to openai first — advisory must appear.
    await switchProvider(user, 'Ollama (Local)', 'OpenAI');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();

    // Switch back to ollama — advisory must disappear.
    await switchProvider(user, 'OpenAI', 'Ollama (Local)');
    expect(screen.queryByText(ADVISORY_FRAGMENT)).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Semantic-scoring toggle → backend mirror (ADR-020 addendum)
//
// The toggle writes to a Zustand store persisted in the WEBVIEW's localStorage,
// which the headless Autopilot scheduler cannot read. Without the write-through
// below, turning semantic scoring on would silently never reach a scheduled run
// (and turning it off would leave the scheduler embedding). The store write
// alone is not evidence of anything — these tests pin the IPC mirror.
// ─────────────────────────────────────────────────────────────────────────────

describe('EmbeddingsSettings — semantic scoring backend mirror', () => {
  beforeEach(() => {
    mockNotifyError.mockClear();
    usePreferencesStore.getState().resetPreferences();
  });

  it('mirrors the toggle to the backend on enable AND on disable', async () => {
    const user = userEvent.setup();
    const { setSemanticScoring } = renderPanel();

    const toggle = screen.getByLabelText('settings.embeddings.semanticScoring');
    await user.click(toggle);
    await waitFor(() => expect(setSemanticScoring).toHaveBeenLastCalledWith(true));
    // The local store moved too — the mirror must not replace it.
    expect(usePreferencesStore.getState().semanticScoring).toBe(true);

    await user.click(toggle);
    await waitFor(() => expect(setSemanticScoring).toHaveBeenLastCalledWith(false));
    expect(setSemanticScoring).toHaveBeenCalledTimes(2);
    expect(usePreferencesStore.getState().semanticScoring).toBe(false);
  });

  it('does NOT mirror when an unrelated toggle on the same panel changes', async () => {
    const user = userEvent.setup();
    const { setSemanticScoring } = renderPanel();

    await user.click(screen.getByLabelText('settings.embeddings.autoIndex'));
    expect(setSemanticScoring).not.toHaveBeenCalled();
  });

  // A silently-failed mirror write is the worst of both worlds: the in-app
  // score follows the new value while a scheduled run keeps using the old one,
  // with nothing on screen to say so.
  //
  // Driven through the REAL mutation with a REJECTED IPC promise, which is the
  // only thing that reaches `onError`. The backend command therefore has to
  // return `AppResult<()>`: while it returned a `Value` holding `{"error": …}`
  // the invoke RESOLVED, the mutation counted the write as a success, and this
  // notification could never fire in production no matter how it was wired.
  it('surfaces a failed mirror write instead of diverging silently', async () => {
    const user = userEvent.setup();
    renderPanel(vi.fn().mockRejectedValue(new Error('job_preferences: db is locked')));

    await user.click(screen.getByLabelText('settings.embeddings.semanticScoring'));

    await waitFor(() =>
      expect(mockNotifyError).toHaveBeenCalledWith({
        message: 'settings.embeddings.semanticScoringSyncFailed',
      })
    );
  });

  it('says nothing when the mirror write succeeds', async () => {
    const user = userEvent.setup();
    const { setSemanticScoring } = renderPanel();

    await user.click(screen.getByLabelText('settings.embeddings.semanticScoring'));

    await waitFor(() => expect(setSemanticScoring).toHaveBeenCalledTimes(1));
    expect(mockNotifyError).not.toHaveBeenCalled();
  });
});
