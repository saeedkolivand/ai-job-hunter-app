/**
 * RewritePopover — timeout, unchanged-result, limit enforcement and portal mode.
 *
 * Verifies the client-side safety net added in Fix #8b:
 *  - A stalled provider stream is aborted at the EFFORT-SCALED bound the shared
 *    stream helper uses (never a hardcoded 60 s, which sat below the backend's
 *    own deadline for the same request) and the error state is surfaced.
 *  - The timeout is cleared when the stream resolves normally — no spurious
 *    error fires after a successful rewrite.
 * Plus the three measured defects: a result identical to the selection is a
 * neutral "unchanged" state and not an Accept-able success, a numeric limit in
 * the instruction is verified by code with exactly ONE re-ask, and a long
 * reasoning pass says it is still working instead of looking dead.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// The bound `resolveRewriteTimeoutMs` hands back in these tests — deliberately
// far above the deleted 60 s constant, so a test advancing past 60 s can prove
// the popover no longer aborts there.
const { RESOLVED_TIMEOUT_MS } = vi.hoisted(() => ({ RESOLVED_TIMEOUT_MS: 300_000 }));

// ── lib/generate — stall by default (controlled per-test via mockImplementation) ──
vi.mock('@/lib/generate', () => ({
  rewriteSelection: vi.fn(),
  resolveRewriteTimeoutMs: () => RESOLVED_TIMEOUT_MS,
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

const SELECTION = 'some selected text';

function renderPopover() {
  return render(
    <RewritePopover
      target={{ selection: SELECTION, before: '', after: '' }}
      docType="resume"
      model="test-model"
      onAccept={vi.fn()}
      onClose={vi.fn()}
    />
  );
}

/** The Accept button, whose enabled state is the honesty contract under test. */
function acceptButton(): HTMLButtonElement {
  return screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
}

/** Run the free-instruction path with `text`, then flush the promise chain. */
async function runInstruction(text: string) {
  fireEvent.change(screen.getByPlaceholderText('aiGenerate.rewrite.instructionPlaceholder'), {
    target: { value: text },
  });
  fireEvent.click(screen.getByRole('button', { name: 'aiGenerate.rewrite.submit' }));
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
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

  it('does NOT abort at the deleted 60 s constant, and surfaces aiGenerate.rewrite.failed at the resolved bound', async () => {
    renderPopover();

    // Trigger run() via the first preset chip.
    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));

    // 60 s used to kill the stream here while the backend was still streaming
    // (its own deadline is 300 s scaled by effort). Nothing may happen yet.
    await act(async () => {
      vi.advanceTimersByTime(60_001);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();

    // Past the bound `resolveRewriteTimeoutMs` actually returned: abort + error.
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS);
      // Flush the abort-event → rejection → .catch microtask chain.
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(screen.getByText('aiGenerate.rewrite.failed')).toBeTruthy();
  });

  it('shows the still-working line once a stream passes ~20 s, and drops it when the stream lands', async () => {
    renderPopover();

    fireEvent.click(screen.getByText('aiGenerate.rewrite.presets.shorten'));
    expect(screen.queryByText('aiGenerate.rewrite.stillWorking')).toBeNull();

    await act(async () => {
      vi.advanceTimersByTime(20_001);
      await Promise.resolve();
    });
    expect(screen.getByText('aiGenerate.rewrite.stillWorking')).toBeTruthy();

    // The stall mock rejects on abort; abort via the resolved bound and the
    // line must go away with the stream.
    await act(async () => {
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.queryByText('aiGenerate.rewrite.stillWorking')).toBeNull();
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
      vi.advanceTimersByTime(RESOLVED_TIMEOUT_MS + 1);
      await Promise.resolve();
    });

    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();
    // The rewrite result is displayed.
    expect(screen.getByText('rewritten text')).toBeTruthy();
  });
});

describe('RewritePopover — unchanged result (C2)', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows the neutral unchanged notice and DISABLES Accept when the result echoes the selection', async () => {
    // The measured no-op: the result differed from the input by one comma.
    vi.mocked(rewriteSelection).mockResolvedValue(`${SELECTION},`);
    renderPopover();

    await runInstruction('tighten this');

    expect(screen.getByText('aiGenerate.rewrite.unchanged')).toBeTruthy();
    // Neutral, NOT an error.
    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();
    expect(acceptButton().disabled).toBe(true);
    // Regenerate stays live so the user can ask again.
    expect(screen.getByRole('button', { name: 'aiGenerate.rewrite.regenerate' })).toBeTruthy();
  });

  it('leaves Accept enabled and shows no notice for a genuinely different result', async () => {
    vi.mocked(rewriteSelection).mockResolvedValue('a completely different sentence');
    renderPopover();

    await runInstruction('tighten this');

    expect(screen.queryByText('aiGenerate.rewrite.unchanged')).toBeNull();
    expect(acceptButton().disabled).toBe(false);
  });
});

describe('RewritePopover — code-enforced length limit (C4)', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('re-asks exactly ONCE with the measured overshoot when the result breaks the parsed limit', async () => {
    const over = 'x'.repeat(30);
    const inside = 'y'.repeat(10);
    vi.mocked(rewriteSelection)
      .mockResolvedValueOnce(over)
      .mockResolvedValueOnce(inside)
      .mockResolvedValue('never reached');
    renderPopover();

    await runInstruction('rewrite this under 20 characters');

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(2);
    const secondInstruction = vi.mocked(rewriteSelection).mock.calls[1]?.[0].instruction as string;
    expect(secondInstruction).toContain('rewrite this under 20 characters');
    expect(secondInstruction).toContain('30 characters');
    expect(secondInstruction).toContain('cut at least 10 characters');
    // Inside the limit on the retry → no count line, Accept live.
    expect(screen.queryByText('aiGenerate.rewrite.overLimit.chars')).toBeNull();
    expect(acceptButton().disabled).toBe(false);
  });

  it('never asks a third time, and shows the count next to a still-enabled Accept', async () => {
    vi.mocked(rewriteSelection).mockResolvedValue('x'.repeat(30));
    renderPopover();

    await runInstruction('rewrite this under 20 characters');

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(2);
    expect(screen.getByText('aiGenerate.rewrite.overLimit.chars')).toBeTruthy();
    // Advisory, not a block: the user decides.
    expect(acceptButton().disabled).toBe(false);
  });

  it('does not re-ask at all when the instruction carries no numeric limit', async () => {
    vi.mocked(rewriteSelection).mockResolvedValue('x'.repeat(400));
    renderPopover();

    await runInstruction('make this punchier');

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('aiGenerate.rewrite.overLimit.chars')).toBeNull();
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
