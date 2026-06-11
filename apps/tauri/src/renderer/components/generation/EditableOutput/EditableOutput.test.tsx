import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import type * as Generate from '@/lib/generate';

import { EditableOutput } from './index';

// ── Module mocks ──────────────────────────────────────────────────────────────

// useFocusTrap needs a stable ref-like object — return a callback ref that is
// compatible with the `ref={trapRef as React.RefObject<HTMLDivElement>}` cast.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useFocusTrap: () => ({ current: null }),
  };
});

// Stub the model selector — the component calls this on every render.
vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// rewriteSelection is the only async side-effect we need to control.
const mockRewriteSelection = vi.fn();
vi.mock('@/lib/generate', async (importOriginal) => {
  const actual = await importOriginal<typeof Generate>();
  return {
    ...actual,
    rewriteSelection: (...args: Parameters<typeof mockRewriteSelection>) =>
      mockRewriteSelection(...args),
  };
});

// ── Helpers ───────────────────────────────────────────────────────────────────

const FULL_TEXT = 'Hello world. This is the middle part. Goodbye world.';
// selection covers "This is the middle part." (characters 13–37)
const SEL_START = 13;
const SEL_END = 37;
const REPLACEMENT = 'This is the REPLACED part.';

/** Switch EditableOutput from its default Preview view to Edit view. */
function switchToEdit() {
  fireEvent.click(screen.getByRole('radio', { name: /edit/i }));
}

/**
 * Simulate a text selection in the Edit textarea.
 * jsdom does not fire native selection events from programmatic setSelectionRange,
 * so we set selectionStart/End directly and dispatch the mouseUp event that
 * EditableOutput's `onMouseUp={updateSelection}` handler listens to.
 */
function simulateSelection(start: number, end: number) {
  // The textarea is the only <textarea> element; the popover has an <input>.
  const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
  Object.defineProperty(textarea, 'selectionStart', { writable: true, value: start });
  Object.defineProperty(textarea, 'selectionEnd', { writable: true, value: end });
  fireEvent.mouseUp(textarea);
}

function openRewritePopover() {
  fireEvent.click(screen.getByRole('button', { name: /aiGenerate\.rewrite\.trigger/i }));
}

/**
 * Returns the <textarea> element — there is exactly one in Edit mode before the
 * popover opens. After the popover opens the popover's <input> is also a textbox
 * so use `getAllByRole` + find by tag.
 */
function getTextarea(): HTMLTextAreaElement {
  const el = screen
    .getAllByRole('textbox')
    .find((e): e is HTMLTextAreaElement => e.tagName === 'TEXTAREA');
  if (!el) throw new Error('No <textarea> found in the rendered output');
  return el;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('EditableOutput — F4 inline rewrite splice', () => {
  let onChange: Mock<(value: string) => void>;

  beforeEach(() => {
    onChange = vi.fn<(value: string) => void>();
    mockRewriteSelection.mockReset();
  });

  it('selecting a range + accepting a rewrite splices exactly [start,end) with the replacement', async () => {
    // Arrange: rewriteSelection calls onToken once then resolves the full result.
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken(REPLACEMENT);
        return REPLACEMENT;
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(SEL_START, SEL_END);
    openRewritePopover();

    // Click a preset to start the rewrite.
    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    // Wait for streaming to finish and Accept to be enabled.
    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    // onChange must receive the splice: text before + replacement + text after.
    const expected = FULL_TEXT.slice(0, SEL_START) + REPLACEMENT + FULL_TEXT.slice(SEL_END);
    expect(onChange).toHaveBeenCalledWith(expected);

    // Surrounding context is intact.
    const [firstCall] = onChange.mock.calls;
    if (!firstCall) throw new Error('onChange was not called');
    const result = firstCall[0];
    expect(result.startsWith('Hello world. ')).toBe(true);
    expect(result.endsWith(' Goodbye world.')).toBe(true);
  });

  it('splice at offset 0 (selection starts at the very beginning of text)', async () => {
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken('PREFIX');
        return 'PREFIX';
      }
    );

    // Selection covers the first 5 characters: "Hello"
    const START_BOUNDARY = 0;
    const END_BOUNDARY = 5;

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(START_BOUNDARY, END_BOUNDARY);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    // Expected: '' + 'PREFIX' + FULL_TEXT.slice(5)
    const expected = '' + 'PREFIX' + FULL_TEXT.slice(END_BOUNDARY);
    expect(onChange).toHaveBeenCalledWith(expected);
    // Sanity: result starts with the replacement, not original text.
    const [firstCall] = onChange.mock.calls;
    if (!firstCall) throw new Error('onChange was not called');
    expect(firstCall[0].startsWith('PREFIX')).toBe(true);
  });

  it('splice ending at text.length (selection ends at the very end of text)', async () => {
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken('SUFFIX');
        return 'SUFFIX';
      }
    );

    // Selection covers the last 14 characters: "Goodbye world."
    const END_BOUNDARY = FULL_TEXT.length;
    const START_BOUNDARY = END_BOUNDARY - 14;

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(START_BOUNDARY, END_BOUNDARY);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    // Expected: FULL_TEXT.slice(0, START_BOUNDARY) + 'SUFFIX' + '' (empty after-slice)
    const expected = FULL_TEXT.slice(0, START_BOUNDARY) + 'SUFFIX';
    expect(onChange).toHaveBeenCalledWith(expected);
    const [firstCall] = onChange.mock.calls;
    if (!firstCall) throw new Error('onChange was not called');
    expect(firstCall[0].endsWith('SUFFIX')).toBe(true);
  });

  it('Cancel leaves onChange uncalled and text unchanged', () => {
    mockRewriteSelection.mockImplementation(async () => REPLACEMENT);

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(SEL_START, SEL_END);
    openRewritePopover();

    // Two Cancel controls exist: the X icon in the header and the text button in
    // the footer — both call onClose. Click the first (header X).
    const cancelBtns = screen.getAllByRole('button', { name: /aiGenerate\.rewrite\.cancel/i });
    const [firstCancel] = cancelBtns;
    if (!firstCancel) throw new Error('no cancel button rendered');
    fireEvent.click(firstCancel);

    expect(onChange).not.toHaveBeenCalled();
  });

  it('stream rejection: onChange is NOT called, error is surfaced, Accept is disabled', async () => {
    // Make rewriteSelection reject outright — simulates a network or provider failure.
    mockRewriteSelection.mockImplementation(() => Promise.reject(new Error('provider error')));

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    // The popover's .catch() at ~line 119 sets an error string — it must appear in the DOM.
    // (The rendered error text comes from the 'aiGenerate.rewrite.failed' i18n key, which
    // our stub returns verbatim as the key itself.)
    await waitFor(() => {
      expect(screen.getByText('aiGenerate.rewrite.failed')).toBeInTheDocument();
    });

    // Accept must be disabled — canAccept = !streaming && !!result.trim() && !error.
    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    expect(acceptBtn).toBeDisabled();

    // onChange must never have been called — the selection text must not be deleted.
    expect(onChange).not.toHaveBeenCalled();
  });

  it('empty/whitespace result: onChange is NOT called, error is surfaced, Accept is disabled', async () => {
    // Resolve with only whitespace — the popover trims and checks !cleaned in the .then().
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken('   ');
        return '   ';
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    // The .then() guard: `if (!cleaned) setError(t('aiGenerate.rewrite.empty'))`.
    await waitFor(() => {
      expect(screen.getByText('aiGenerate.rewrite.empty')).toBeInTheDocument();
    });

    // Accept must be disabled — prevents silently deleting the selected text.
    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    expect(acceptBtn).toBeDisabled();

    // onChange must never have been called.
    expect(onChange).not.toHaveBeenCalled();
  });

  it('textarea is readOnly (not disabled) while a rewrite streams', async () => {
    // Keep the stream unresolved so we can inspect the locked state mid-flight.
    let resolveStream!: (v: string) => void;
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken('partial…');
        return new Promise<string>((res) => {
          resolveStream = res;
        });
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToEdit();
    simulateSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    // The textarea must be readOnly (accessible) rather than disabled (hidden from
    // screen readers) while the popover is showing.
    const textarea = getTextarea();
    expect(textarea).toHaveAttribute('readonly');
    expect(textarea).not.toBeDisabled();

    // Resolve so the component can clean up without act() warnings.
    await act(async () => {
      resolveStream(REPLACEMENT);
    });
  });
});

describe('EditableOutput — preview surface (#24)', () => {
  it('renders the prettified-markdown preview by default (no previewSlot)', () => {
    render(<EditableOutput value="Led **payments** work." onChange={vi.fn()} docType="resume" />);
    // **markers** render as bold — the markdown fallback is active.
    expect(screen.getByText('payments').tagName).toBe('STRONG');
  });

  it('renders a custom previewSlot instead of markdown when provided', () => {
    render(
      <EditableOutput
        value="Led **payments** work."
        onChange={vi.fn()}
        docType="resume"
        previewSlot={<div data-testid="custom-preview">PDF</div>}
      />
    );
    expect(screen.getByTestId('custom-preview')).toBeInTheDocument();
    // The markdown fallback must NOT also render when a slot is supplied.
    expect(screen.queryByText('payments')).toBeNull();
  });
});
