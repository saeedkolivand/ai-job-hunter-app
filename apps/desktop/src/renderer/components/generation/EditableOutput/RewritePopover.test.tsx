/**
 * RewritePopover — timeout tests
 *
 * Verifies the client-side safety net added in Fix #8b:
 *  - A stalled provider stream is aborted after REWRITE_TIMEOUT_MS and the
 *    error state is surfaced (not silently swallowed).
 *  - The timeout is cleared when the stream resolves normally — no spurious
 *    error fires after a successful rewrite.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── lib/generate — stall by default (controlled per-test via mockImplementation) ──
vi.mock('@/lib/generate', () => ({
  rewriteSelection: vi.fn(),
}));

// ── motion/react — strip animation props, render plain div ────────────────────
vi.mock('motion/react', () => ({
  motion: {
    div: ({
      initial: _i,
      animate: _a,
      exit: _e,
      transition: _t,
      ...rest
    }: React.HTMLAttributes<HTMLDivElement> & {
      initial?: unknown;
      animate?: unknown;
      exit?: unknown;
      transition?: unknown;
    }) => <div {...rest} />,
  },
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// ── @ajh/ui — minimal stubs ───────────────────────────────────────────────────
vi.mock('@ajh/ui', () => ({
  Button: ({
    children,
    onClick,
    disabled,
    type,
    ...rest
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button type={type ?? 'button'} onClick={onClick} disabled={!!disabled} {...rest}>
      {children}
    </button>
  ),
  Input: ({
    onChange,
    value,
    onKeyDown,
    disabled,
    placeholder,
  }: React.InputHTMLAttributes<HTMLInputElement>) => (
    <input
      onChange={onChange}
      value={value ?? ''}
      onKeyDown={onKeyDown}
      disabled={!!disabled}
      placeholder={placeholder}
    />
  ),
  Tag: {
    CheckableTag: ({
      children,
      onChange,
      disabled,
    }: {
      children?: React.ReactNode;
      onChange?: () => void;
      disabled?: boolean;
    }) => (
      <button type="button" onClick={onChange} disabled={!!disabled}>
        {children}
      </button>
    ),
  },
  transition: { fast: {} },
  // useFocusTrap returns a ref object — the mock div accepts it as a plain prop.
  useFocusTrap: () => ({ current: null }),
  cn: (...classes: unknown[]) => classes.filter(Boolean).join(' '),
}));

// ── component under test (import AFTER mocks so hoisting picks up the stubs) ──
import { rewriteSelection } from '@/lib/generate';

import { RewritePopover } from './RewritePopover';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderPopover() {
  return render(
    <RewritePopover
      target={{ selection: 'some selected text', before: '', after: '' }}
      docType="resume"
      model="test-model"
      onAccept={vi.fn()}
      onClose={vi.fn()}
    />
  );
}

/**
 * Mock that stalls until its AbortSignal fires, then rejects — mirrors a
 * real provider whose connection hangs and is finally aborted by the client.
 */
function mockStall() {
  vi.mocked(rewriteSelection).mockImplementation(
    ({ signal }: { signal?: AbortSignal }) =>
      new Promise<string>((_, reject) => {
        signal?.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')));
      })
  );
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('RewritePopover — timeout', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockStall();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('surfaces aiGenerate.rewrite.failed after REWRITE_TIMEOUT_MS with a stalled stream', async () => {
    renderPopover();

    // Trigger run() via the first preset chip.
    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));

    // Advance the clock past the 60 s client timeout.
    await act(async () => {
      vi.advanceTimersByTime(60_001);
      // Flush the abort-event → rejection → .catch microtask chain.
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.getByText('aiGenerate.rewrite.failed')).toBeTruthy();
  });

  it('does NOT show error and clears streaming when the stream resolves before timeout', async () => {
    // Override for this test: resolves immediately instead of stalling.
    vi.mocked(rewriteSelection).mockResolvedValueOnce('rewritten text');

    renderPopover();

    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));

    // Let the resolved promise flush through .then / .finally.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // Advance past what would have been the timeout — must NOT fire the error
    // since clearTimeout(timeoutId) ran in .finally.
    await act(async () => {
      vi.advanceTimersByTime(60_001);
      await Promise.resolve();
    });

    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();
    // The rewrite result is displayed.
    expect(screen.getByText('rewritten text')).toBeTruthy();
  });
});

describe('RewritePopover — anchored portal (anchorEl)', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('renders inline (inside its own container) when anchorEl is not set', () => {
    const { container } = renderPopover();
    expect(container.contains(screen.getByRole('dialog'))).toBe(true);
  });

  it('portals to document.body, fixed-positions off the trigger rect, and lifts z-toast', () => {
    const anchorEl = document.createElement('button');
    anchorEl.getBoundingClientRect = () =>
      ({ top: 100, bottom: 120, left: 200, right: 300, width: 100, height: 20 }) as DOMRect;
    document.body.appendChild(anchorEl);

    const { container } = render(
      <RewritePopover
        target={{ selection: 'some selected text', before: '', after: '' }}
        docType="resume"
        model="test-model"
        onAccept={vi.fn()}
        onClose={vi.fn()}
        anchorEl={anchorEl}
      />
    );

    const dialog = screen.getByRole('dialog');
    // Portaled OUT of the render container (a document.body sibling), not inline.
    expect(container.contains(dialog)).toBe(false);
    expect(dialog.className).toContain('z-toast');
    expect(dialog.style.position).toBe('fixed');
    expect(dialog.style.top).toBe('124px'); // rect.bottom (120) + 4px gap
    expect(dialog.style.left).toBe('8px'); // clamped — rect.right (300) - panel width would go negative

    document.body.removeChild(anchorEl);
  });

  it('flips the popover upward when it would not fit below the trigger', () => {
    // jsdom performs no layout, so every element's real getBoundingClientRect()
    // reads as zeros. Stub it globally to a realistic popover panel height
    // (~380px, matching header + selection echo + presets + input + footer);
    // the trigger below overrides its own instance method (takes precedence
    // over this prototype stub) to report its own, separately-controlled rect.
    const originalGetBoundingClientRect = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = () =>
      ({ top: 0, left: 0, right: 352, bottom: 380, width: 352, height: 380 }) as DOMRect;

    const anchorEl = document.createElement('button');
    // Trigger sits near the bottom of jsdom's default 768px-tall viewport —
    // opening below (720 + 380 + 4 = 1104) would push the footer far offscreen.
    anchorEl.getBoundingClientRect = () =>
      ({ top: 700, bottom: 720, left: 200, right: 300, width: 100, height: 20 }) as DOMRect;
    document.body.appendChild(anchorEl);

    render(
      <RewritePopover
        target={{ selection: 'some selected text', before: '', after: '' }}
        docType="resume"
        model="test-model"
        onAccept={vi.fn()}
        onClose={vi.fn()}
        anchorEl={anchorEl}
      />
    );

    const dialog = screen.getByRole('dialog');
    // Flipped above: anchorRect.top (700) - panelHeight (380) - 4px gap = 316.
    expect(dialog.style.top).toBe('316px');
    expect(Number(dialog.style.top.replace('px', ''))).toBeGreaterThanOrEqual(8);

    document.body.removeChild(anchorEl);
    Element.prototype.getBoundingClientRect = originalGetBoundingClientRect;
  });

  it('clamps the flipped-upward position to never go above the 8px viewport margin', () => {
    // A trigger near the very TOP of the viewport with a panel taller than the
    // whole viewport: it doesn't fit below (forcing a flip), and flipping
    // naively (anchorRect.top - panelHeight - 4) would go negative — must
    // clamp to 8px instead of running off the top edge.
    const originalGetBoundingClientRect = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = () =>
      ({ top: 0, left: 0, right: 352, bottom: 750, width: 352, height: 750 }) as DOMRect;

    const anchorEl = document.createElement('button');
    anchorEl.getBoundingClientRect = () =>
      ({ top: 20, bottom: 40, left: 200, right: 300, width: 100, height: 20 }) as DOMRect;
    document.body.appendChild(anchorEl);

    render(
      <RewritePopover
        target={{ selection: 'some selected text', before: '', after: '' }}
        docType="resume"
        model="test-model"
        onAccept={vi.fn()}
        onClose={vi.fn()}
        anchorEl={anchorEl}
      />
    );

    const dialog = screen.getByRole('dialog');
    expect(dialog.style.top).toBe('8px');

    document.body.removeChild(anchorEl);
    Element.prototype.getBoundingClientRect = originalGetBoundingClientRect;
  });
});
