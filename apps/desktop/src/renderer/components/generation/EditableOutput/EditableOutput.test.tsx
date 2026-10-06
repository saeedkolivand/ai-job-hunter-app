import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import { EditableOutput } from './index';
import {
  mockEditorFocus,
  mockGetSelectionContext,
  mockGetSelectionText,
  mockReplaceSelection,
  mockRewriteSelection,
} from './test-mocks';

vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-mocks')).uiMock(await importOriginal())
);

// Stub the model selector — the component calls this on every render.
vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useSelectedProvider: () => 'ollama',
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// EditableOutput now calls useContactProfile() which reaches for AppClientProvider.
// Stub it so the component mounts in the test environment without a provider tree.
vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => ({ data: undefined }),
}));

vi.mock('@/lib/generate', async (importOriginal) =>
  (await import('./test-mocks')).generateMock(await importOriginal())
);

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

/** Mock the rewrite stream: emits `text` as one token, then resolves with it. */
function mockStream(text: string) {
  mockRewriteSelection.mockImplementation(
    async ({ onToken }: { onToken: (tok: string) => void }) => {
      onToken(text);
      return text;
    }
  );
}

/** Click the "shorten" preset chip (starts a rewrite) inside `act`. */
async function runShortenPreset() {
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /aiGenerate\.rewrite\.presets\.shorten/i }));
  });
}

/** Wait for the streamed result to become acceptable, then click Accept. */
async function acceptRewrite() {
  const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
  await waitFor(() => expect(acceptBtn).not.toBeDisabled());
  fireEvent.click(acceptBtn);
}

/** Cancel the open popover without starting a rewrite. */
function cancelRewrite() {
  const [firstCancel] = screen.getAllByRole('button', { name: /aiGenerate\.rewrite\.cancel/i });
  if (!firstCancel) throw new Error('no cancel button rendered');
  fireEvent.click(firstCancel);
}

/** The single onChange argument, failing loudly when onChange never fired. */
function firstOnChangeArg(onChange: Mock<(value: string) => void>): string {
  const [firstCall] = onChange.mock.calls;
  if (!firstCall) throw new Error('onChange was not called');
  return firstCall[0];
}

/** Source view with `start..end` selected and the rewrite popover open. */
function openSourceRewrite(onChange: (v: string) => void, start: number, end: number) {
  const view = render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);
  switchToSource();
  simulateSourceSelection(start, end);
  openRewritePopover();
  return view;
}

/** WYSIWYG view with a selection made inside the editor and the popover open. */
function openEditorRewrite(onChange: (v: string) => void) {
  render(<EditableOutput value={FULL_TEXT} onChange={onChange} docType="resume" />);
  switchToWysiwygEdit();
  fireEvent.click(screen.getByTestId(TEST_IDS.generation.rteSelectTrigger));
  openRewritePopover();
}

// ── Source-path rewrite tests ─────────────────────────────────────────────────

describe('EditableOutput — F4 inline rewrite splice (Source path)', () => {
  let onChange: Mock<(value: string) => void>;

  beforeEach(() => {
    onChange = vi.fn<(value: string) => void>();
    mockRewriteSelection.mockReset();
  });

  it('selecting a range + accepting a rewrite splices exactly [start,end) with the replacement', async () => {
    mockStream(REPLACEMENT);
    openSourceRewrite(onChange, SEL_START, SEL_END);
    await runShortenPreset();
    await acceptRewrite();

    const expected = FULL_TEXT.slice(0, SEL_START) + REPLACEMENT + FULL_TEXT.slice(SEL_END);
    expect(onChange).toHaveBeenCalledWith(expected);

    const result = firstOnChangeArg(onChange);
    expect(result.startsWith('Hello world. ')).toBe(true);
    expect(result.endsWith(' Goodbye world.')).toBe(true);
  });

  it.each([
    {
      name: 'splice at offset 0 (selection starts at the very beginning of text)',
      token: 'PREFIX',
      start: 0,
      end: 5,
      expected: 'PREFIX' + FULL_TEXT.slice(5),
      edge: (r: string) => r.startsWith('PREFIX'),
    },
    {
      name: 'splice ending at text.length (selection ends at the very end of text)',
      token: 'SUFFIX',
      start: FULL_TEXT.length - 14,
      end: FULL_TEXT.length,
      expected: FULL_TEXT.slice(0, FULL_TEXT.length - 14) + 'SUFFIX',
      edge: (r: string) => r.endsWith('SUFFIX'),
    },
  ])('$name', async ({ token, start, end, expected, edge }) => {
    mockStream(token);
    openSourceRewrite(onChange, start, end);
    await runShortenPreset();
    await acceptRewrite();

    expect(onChange).toHaveBeenCalledWith(expected);
    expect(edge(firstOnChangeArg(onChange))).toBe(true);
  });

  it('opens the popover in PORTAL mode, outside every overflow-hidden ancestor', () => {
    // The measured defect: rendered inline, the popover sat inside this
    // component's own root AND the caller's fixed-height card, both
    // overflow-hidden, so the result and the entire Cancel / Regenerate /
    // Accept footer were clipped and Accept could not be clicked. Passing
    // `anchorEl` (the always-mounted toolbar row) is what takes the portal
    // branch; drop that prop and the dialog lands back inside `container`.
    mockRewriteSelection.mockImplementation(async () => REPLACEMENT);

    const { container } = openSourceRewrite(onChange, SEL_START, SEL_END);

    const dialog = screen.getByRole('dialog');
    expect(container.contains(dialog)).toBe(false);
    expect(dialog.className).toContain('z-toast');
  });

  it('Cancel leaves onChange uncalled and text unchanged', () => {
    mockRewriteSelection.mockImplementation(async () => REPLACEMENT);
    openSourceRewrite(onChange, SEL_START, SEL_END);

    cancelRewrite();

    expect(onChange).not.toHaveBeenCalled();
  });

  it('stream rejection: onChange is NOT called, error is surfaced, Accept is disabled', async () => {
    mockRewriteSelection.mockImplementation(() => Promise.reject(new Error('provider error')));
    openSourceRewrite(onChange, SEL_START, SEL_END);
    await runShortenPreset();

    await waitFor(() => {
      expect(screen.getByText('aiGenerate.rewrite.failed')).toBeInTheDocument();
    });

    const acceptBtn = screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
    expect(acceptBtn).toBeDisabled();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('empty/whitespace result: onChange is NOT called, error is surfaced, Accept is disabled', async () => {
    mockStream('   ');
    openSourceRewrite(onChange, SEL_START, SEL_END);
    await runShortenPreset();

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
    openSourceRewrite(onChange, SEL_START, SEL_END);
    await runShortenPreset();

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

    expect(screen.getByTestId(TEST_IDS.generation.richTextEditor)).toBeInTheDocument();
    // No raw <textarea> in WYSIWYG Edit view.
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('Source tab renders a <textarea> and not the RichTextEditor', () => {
    render(<EditableOutput value="Some **text**." onChange={vi.fn()} docType="resume" />);

    switchToSource();

    expect(screen.getByRole('textbox')).toBeInTheDocument();
    expect(screen.getByRole<HTMLTextAreaElement>('textbox').tagName).toBe('TEXTAREA');
    expect(screen.queryByTestId(TEST_IDS.generation.richTextEditor)).toBeNull();
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
    mockStream(AI_RESULT);
    openEditorRewrite(onChange);
    await runShortenPreset();
    await acceptRewrite();

    // The editor's replaceSelection must be called exactly once with the result.
    expect(mockReplaceSelection).toHaveBeenCalledTimes(1);
    expect(mockReplaceSelection).toHaveBeenCalledWith(AI_RESULT);

    // On the editor path the component does NOT call onChange directly —
    // replaceSelection drives it internally. Spy mock doesn't emit onChange,
    // so onChange should not have been called.
    expect(onChange).not.toHaveBeenCalled();
  });

  it('editor rewrite cancel: replaceSelection is never called', () => {
    openEditorRewrite(onChange);

    // Cancel without starting a rewrite.
    cancelRewrite();

    expect(mockReplaceSelection).not.toHaveBeenCalled();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('editor rewrite: getSelectionContext called when trigger fires', async () => {
    mockStream('result');
    openEditorRewrite(onChange);

    // getSelectionContext must have been invoked when the popover opened.
    expect(mockGetSelectionContext).toHaveBeenCalled();

    // Accept to clean up state.
    await runShortenPreset();
    await acceptRewrite();
  });

  it('editor rewrite stream rejection: replaceSelection and onChange uncalled', async () => {
    mockRewriteSelection.mockImplementation(() => Promise.reject(new Error('network error')));
    openEditorRewrite(onChange);
    await runShortenPreset();

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
        previewSlot={<div data-testid={TEST_IDS.generation.customPreview}>PDF</div>}
      />
    );
    expect(screen.getByTestId(TEST_IDS.generation.customPreview)).toBeInTheDocument();
    // The markdown fallback must NOT also render when a slot is supplied.
    expect(screen.queryByText('payments')).toBeNull();
  });
});
