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
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stubs — prevent any IPC / QueryClient dependency ──────────────────

vi.mock('@/services', () => ({
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
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

// ── component under test ──────────────────────────────────────────────────────

import { EmbeddingsSettings } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

// Unique fragment from the advisory paragraph — matches the component's literal.
const ADVISORY_FRAGMENT = /cloud embedding providers charge per token/i;

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
    render(<EmbeddingsSettings />);
    expect(screen.queryByText(ADVISORY_FRAGMENT)).not.toBeInTheDocument();
  });

  it('advisory paragraph element is not in the DOM when ollama is active', () => {
    render(<EmbeddingsSettings />);
    const advisoryEl = Array.from(document.querySelectorAll('p')).find((p) =>
      ADVISORY_FRAGMENT.test(p.textContent ?? '')
    );
    expect(advisoryEl).toBeUndefined();
  });
});

describe('EmbeddingsSettings — cloud provider selected', () => {
  it('shows advisory after switching from ollama to openai', async () => {
    const user = userEvent.setup();
    render(<EmbeddingsSettings />);

    expect(screen.queryByText(ADVISORY_FRAGMENT)).not.toBeInTheDocument();
    await switchProvider(user, 'Ollama (Local)', 'OpenAI');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();
  });

  it('shows advisory after switching to gemini', async () => {
    const user = userEvent.setup();
    render(<EmbeddingsSettings />);

    await switchProvider(user, 'Ollama (Local)', 'Gemini');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();
  });

  it('shows advisory after switching to openai-compatible', async () => {
    const user = userEvent.setup();
    render(<EmbeddingsSettings />);

    await switchProvider(user, 'Ollama (Local)', 'OpenAI-compatible');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();
  });

  it('advisory disappears when switching back to ollama from a cloud provider', async () => {
    const user = userEvent.setup();
    render(<EmbeddingsSettings />);

    // Switch to openai first — advisory must appear.
    await switchProvider(user, 'Ollama (Local)', 'OpenAI');
    expect(screen.getByText(ADVISORY_FRAGMENT)).toBeInTheDocument();

    // Switch back to ollama — advisory must disappear.
    await switchProvider(user, 'OpenAI', 'Ollama (Local)');
    expect(screen.queryByText(ADVISORY_FRAGMENT)).not.toBeInTheDocument();
  });
});
