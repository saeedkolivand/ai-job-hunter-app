import { forwardRef, useImperativeHandle } from 'react';
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';
import type { RichTextEditorHandle, RichTextEditorProps } from '@ajh/ui';

import type * as Generate from '@/lib/generate';

import { EditableOutput } from './index';

// ── Shared spy functions for the RichTextEditor test-double ───────────────────
//
// Declared before vi.mock('@ajh/ui') so the factory closure can capture them.
// Each describe's beforeEach resets them independently.

const mockReplaceSelection = vi.fn<(text: string) => void>();
const mockGetSelectionText = vi.fn<() => string>(() => '');
const mockGetSelectionContext = vi.fn<() => { selection: string; before: string; after: string }>(
  () => ({ selection: '', before: '', after: '' })
);
const mockEditorFocus = vi.fn<() => void>();

// ── Module mocks ──────────────────────────────────────────────────────────────
//
// The @ajh/ui mock:
//   - replaces RichTextEditor with a test-double (see below)
//   - stubs useFocusTrap (needed by every EditableOutput test)
//   - forwards all other exports unchanged
//
// Test-double contract:
//   (a) renders [data-testid="rich-text-editor"] so tests can assert tab wiring
//   (b) exposes a [data-testid="rte-select-trigger"] button that fires
//       onSelectionChange(true) — simulates the user highlighting text
//   (c) wires the ref to the module-level spy functions above

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();

  const RichTextEditorDouble = forwardRef<RichTextEditorHandle, RichTextEditorProps>(
    function RichTextEditorDouble({ onSelectionChange, value }, ref) {
      useImperativeHandle(
        ref,
        (): RichTextEditorHandle => ({
          getSelectionText: mockGetSelectionText,
          getSelectionContext: mockGetSelectionContext,
          replaceSelection: mockReplaceSelection,
          focus: mockEditorFocus,
        })
      );

      return (
        <div data-testid="rich-text-editor">
          <span data-testid="rte-value">{value}</span>
          <actual.Button data-testid="rte-select-trigger" onClick={() => onSelectionChange?.(true)}>
            simulate selection
          </actual.Button>
          <actual.Button
            data-testid="rte-deselect-trigger"
            onClick={() => onSelectionChange?.(false)}
          >
            deselect
          </actual.Button>
        </div>
      );
    }
  );

  return {
    ...actual,
    useFocusTrap: () => ({ current: null }),
    RichTextEditor: RichTextEditorDouble,
  };
});

// Stub the model selector — the component calls this on every render.
vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// EditableOutput now calls useContactProfile() which reaches for AppClientProvider.
// Stub it so the component mounts in the test environment without a provider tree.
vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => ({ data: undefined }),
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

// ── Constants ─────────────────────────────────────────────────────────────────

const FULL_TEXT = 'Hello world. This is the middle part. Goodbye world.';
// selection covers "This is the middle part." (characters 13–37)
const SEL_START = 13;
const SEL_END = 37;
const REPLACEMENT = 'This is the REPLACED part.';

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Switch EditableOutput to the Source (raw textarea) view.
 * Our translation stub returns keys verbatim → label is 'aiGenerate.source'.
 */
function switchToSource() {
  fireEvent.click(screen.getByRole('radio', { name: /aiGenerate\.source/i }));
}

/**
 * Switch EditableOutput to the WYSIWYG Edit view.
 * Label is 'aiGenerate.edit' from the translation stub.
 */
function switchToWysiwygEdit() {
  fireEvent.click(screen.getByRole('radio', { name: /aiGenerate\.edit/i }));
}

/**
 * Simulate a text selection in the Source textarea.
 * jsdom does not fire native selection events from programmatic setSelectionRange,
 * so we set selectionStart/End directly and dispatch the mouseUp event that
 * EditableOutput's `onMouseUp={updateSelection}` handler listens to.
 */
function simulateSourceSelection(start: number, end: number) {
  const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
  Object.defineProperty(textarea, 'selectionStart', { writable: true, value: start });
  Object.defineProperty(textarea, 'selectionEnd', { writable: true, value: end });
  fireEvent.mouseUp(textarea);
}

function openRewritePopover() {
  fireEvent.click(screen.getByRole('button', { name: /aiGenerate\.rewrite\.trigger/i }));
}

/**
 * Returns the <textarea> element in Source mode. After the popover opens the
 * popover's <input> is also a textbox, so filter by tag name.
 */
function getSourceTextarea(): HTMLTextAreaElement {
  const el = screen
    .getAllByRole('textbox')
    .find((e): e is HTMLTextAreaElement => e.tagName === 'TEXTAREA');
  if (!el) throw new Error('No <textarea> found in the rendered output');
  return el;
}

// ── Source-path rewrite tests ─────────────────────────────────────────────────

describe('EditableOutput — F4 inline rewrite splice (Source path)', () => {
  let onChange: Mock<(value: string) => void>;

  beforeEach(() => {
    onChange = vi.fn<(value: string) => void>();
    mockRewriteSelection.mockReset();
  });

  it('selecting a range + accepting a rewrite splices exactly [start,end) with the replacement', async () => {
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken(REPLACEMENT);
        return REPLACEMENT;
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToSource();
    simulateSourceSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    const expected = FULL_TEXT.slice(0, SEL_START) + REPLACEMENT + FULL_TEXT.slice(SEL_END);
    expect(onChange).toHaveBeenCalledWith(expected);

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

    const START_BOUNDARY = 0;
    const END_BOUNDARY = 5;

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToSource();
    simulateSourceSelection(START_BOUNDARY, END_BOUNDARY);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    const expected = 'PREFIX' + FULL_TEXT.slice(END_BOUNDARY);
    expect(onChange).toHaveBeenCalledWith(expected);
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

    const END_BOUNDARY = FULL_TEXT.length;
    const START_BOUNDARY = END_BOUNDARY - 14;

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToSource();
    simulateSourceSelection(START_BOUNDARY, END_BOUNDARY);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    const expected = FULL_TEXT.slice(0, START_BOUNDARY) + 'SUFFIX';
    expect(onChange).toHaveBeenCalledWith(expected);
    const [firstCall] = onChange.mock.calls;
    if (!firstCall) throw new Error('onChange was not called');
    expect(firstCall[0].endsWith('SUFFIX')).toBe(true);
  });

  it('Cancel leaves onChange uncalled and text unchanged', () => {
    mockRewriteSelection.mockImplementation(async () => REPLACEMENT);

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToSource();
    simulateSourceSelection(SEL_START, SEL_END);
    openRewritePopover();

    const cancelBtns = screen.getAllByRole('button', { name: /aiGenerate\.rewrite\.cancel/i });
    const [firstCancel] = cancelBtns;
    if (!firstCancel) throw new Error('no cancel button rendered');
    fireEvent.click(firstCancel);

    expect(onChange).not.toHaveBeenCalled();
  });

  it('stream rejection: onChange is NOT called, error is surfaced, Accept is disabled', async () => {
    mockRewriteSelection.mockImplementation(() => Promise.reject(new Error('provider error')));

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToSource();
    simulateSourceSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    await waitFor(() => {
      expect(screen.getByText('aiGenerate.rewrite.failed')).toBeInTheDocument();
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    expect(acceptBtn).toBeDisabled();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('empty/whitespace result: onChange is NOT called, error is surfaced, Accept is disabled', async () => {
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken('   ');
        return '   ';
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToSource();
    simulateSourceSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    await waitFor(() => {
      expect(screen.getByText('aiGenerate.rewrite.empty')).toBeInTheDocument();
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    expect(acceptBtn).toBeDisabled();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('textarea is readOnly (not disabled) while a rewrite streams', async () => {
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

    switchToSource();
    simulateSourceSelection(SEL_START, SEL_END);
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const textarea = getSourceTextarea();
    expect(textarea).toHaveAttribute('readonly');
    expect(textarea).not.toBeDisabled();

    await act(async () => {
      resolveStream(REPLACEMENT);
    });
  });
});

// ── Tab-wiring smoke tests ────────────────────────────────────────────────────

describe('EditableOutput — tab wiring', () => {
  it('Edit tab renders the (mocked) RichTextEditor, not a textarea', () => {
    render(<EditableOutput value="Some **text**." onChange={vi.fn()} docType="resume" />);

    switchToWysiwygEdit();

    expect(screen.getByTestId('rich-text-editor')).toBeInTheDocument();
    // No raw <textarea> in WYSIWYG Edit view.
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('Source tab renders a <textarea> and not the RichTextEditor', () => {
    render(<EditableOutput value="Some **text**." onChange={vi.fn()} docType="resume" />);

    switchToSource();

    expect(screen.getByRole('textbox')).toBeInTheDocument();
    expect(screen.getByRole<HTMLTextAreaElement>('textbox').tagName).toBe('TEXTAREA');
    expect(screen.queryByTestId('rich-text-editor')).toBeNull();
  });
});

// ── Editor-path rewrite (WYSIWYG / frozen.mode === 'editor') ─────────────────

describe('EditableOutput — F4 inline rewrite splice (Editor/WYSIWYG path)', () => {
  let onChange: Mock<(value: string) => void>;

  beforeEach(() => {
    onChange = vi.fn<(value: string) => void>();
    mockRewriteSelection.mockReset();
    mockReplaceSelection.mockReset();
    mockGetSelectionContext.mockReset();
    mockGetSelectionText.mockReset();
    mockEditorFocus.mockReset();

    // Default: a meaningful non-empty selection so openEditorRewrite() proceeds.
    mockGetSelectionContext.mockReturnValue({
      selection: 'This is the middle part.',
      before: 'Hello world. ',
      after: ' Goodbye world.',
    });
    mockGetSelectionText.mockReturnValue('This is the middle part.');
  });

  it('editor rewrite accept: replaceSelection called once with the AI result', async () => {
    const AI_RESULT = 'This is the REPLACED part.';

    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken(AI_RESULT);
        return AI_RESULT;
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    // Switch to WYSIWYG Edit view.
    switchToWysiwygEdit();

    // Simulate the user making a selection inside the editor.
    fireEvent.click(screen.getByTestId('rte-select-trigger'));

    // Rewrite trigger should now be visible.
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());

    fireEvent.click(acceptBtn);

    // The editor's replaceSelection must be called exactly once with the result.
    expect(mockReplaceSelection).toHaveBeenCalledTimes(1);
    expect(mockReplaceSelection).toHaveBeenCalledWith(AI_RESULT);

    // On the editor path the component does NOT call onChange directly —
    // replaceSelection drives it internally. Spy mock doesn't emit onChange,
    // so onChange should not have been called.
    expect(onChange).not.toHaveBeenCalled();
  });

  it('editor rewrite cancel: replaceSelection is never called', () => {
    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToWysiwygEdit();
    fireEvent.click(screen.getByTestId('rte-select-trigger'));
    openRewritePopover();

    // Cancel without starting a rewrite.
    const cancelBtns = screen.getAllByRole('button', { name: /aiGenerate\.rewrite\.cancel/i });
    const [firstCancel] = cancelBtns;
    if (!firstCancel) throw new Error('no cancel button rendered');
    fireEvent.click(firstCancel);

    expect(mockReplaceSelection).not.toHaveBeenCalled();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('editor rewrite: getSelectionContext called when trigger fires', async () => {
    mockRewriteSelection.mockImplementation(
      async ({ onToken }: { onToken: (tok: string) => void }) => {
        onToken('result');
        return 'result';
      }
    );

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToWysiwygEdit();
    fireEvent.click(screen.getByTestId('rte-select-trigger'));
    openRewritePopover();

    // getSelectionContext must have been invoked when the popover opened.
    expect(mockGetSelectionContext).toHaveBeenCalled();

    // Accept to clean up state.
    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });
    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    await waitFor(() => expect(acceptBtn).not.toBeDisabled());
    fireEvent.click(acceptBtn);
  });

  it('editor rewrite stream rejection: replaceSelection and onChange uncalled', async () => {
    mockRewriteSelection.mockImplementation(() => Promise.reject(new Error('network error')));

    render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);

    switchToWysiwygEdit();
    fireEvent.click(screen.getByTestId('rte-select-trigger'));
    openRewritePopover();

    await act(async () => {
      fireEvent.click(
        screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i })
      );
    });

    await waitFor(() => {
      expect(screen.getByText('aiGenerate.rewrite.failed')).toBeInTheDocument();
    });

    expect(mockReplaceSelection).not.toHaveBeenCalled();
    expect(onChange).not.toHaveBeenCalled();
  });
});

// ── Preview surface (#24) ─────────────────────────────────────────────────────

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
