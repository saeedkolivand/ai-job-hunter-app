import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

// `t` returns the key, so every assertion below names the i18n key rather than
// a copy string that a wording change would silently break.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: vi.fn() }),
}));

const canUseAI = vi.fn(() => ({ canUse: true, reason: undefined as string | undefined }));
vi.mock('@/components/ui/ModelSelector', () => ({
  useCanUseAI: () => canUseAI(),
  useSelectedModel: () => 'llama3:70b',
}));

const chat = vi.fn();
vi.mock('@/features/support/use-help-chat', () => ({
  useHelpChat: () => chat(),
}));

import { HelpChat } from './index';

type Turn = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  sources?: Array<{ id: string; title: string }>;
  mode?: 'hybrid' | 'keyword';
  dense?: 'ran' | 'skipped' | 'unavailable';
};

const send = vi.fn<(q: string) => Promise<boolean>>();
const retry = vi.fn<() => Promise<boolean>>();
const stop = vi.fn();

function state(
  over: Partial<{ turns: Turn[]; answer: string; streaming: boolean; error: string }>
) {
  chat.mockReturnValue({
    turns: over.turns ?? [],
    answer: over.answer ?? '',
    streaming: over.streaming ?? false,
    error: over.error ?? null,
    send,
    retry,
    stop,
  });
}

/** The single always-mounted live region this card owns. */
const announcement = (container: HTMLElement) =>
  container.querySelector('span.sr-only')?.textContent;

const ANSWERED: Turn = {
  id: 'a-1',
  role: 'assistant',
  content: 'Open the document and click Export.',
  mode: 'hybrid',
  dense: 'ran',
};

describe('HelpChat', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    canUseAI.mockReturnValue({ canUse: true, reason: undefined });
    send.mockResolvedValue(true);
    retry.mockResolvedValue(true);
    // jsdom implements no scrolling; a spy also lets the auto-scroll assertion
    // below be about behaviour rather than about a thrown TypeError.
    Element.prototype.scrollIntoView = vi.fn();
    state({});
  });

  it('renders the card with the setup nudge and a disabled box when AI is unusable', () => {
    canUseAI.mockReturnValue({ canUse: false, reason: 'addApiKey' });
    render(<HelpChat onSearchFor={vi.fn()} />);

    // The card itself is ALWAYS rendered — the nudge replaces the answer, not
    // the surface, so the user can see what they would be able to ask.
    expect(screen.getByTestId(TEST_IDS.support.chatCard)).toBeInTheDocument();
    expect(screen.getByText('aiSetup.addApiKey')).toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.support.chatInput)).toBeDisabled();
    expect(screen.getByTestId(TEST_IDS.support.chatAsk)).toBeDisabled();
  });

  it('caps a "Based on" chip so it reads as a chip, keeping the full title reachable', () => {
    const title = 'How do I export a tailored resume as a PDF or a DOCX file from this app?';
    state({ turns: [{ ...ANSWERED, sources: [{ id: 'exportDoc', title }] }] });
    const onSearchFor = vi.fn();
    render(<HelpChat onSearchFor={onSearchFor} />);

    const chip = screen.getByTestId(TEST_IDS.support.chatSource);
    // `truncate` on the Button itself cannot ellipsize: it is an inline-flex
    // box, so the label clipped mid-word. The ellipsis lives on a block child,
    // and the label starts at the left edge rather than being centred.
    expect(chip.className).toContain('justify-start');
    expect(chip.querySelector('span')?.className).toContain('truncate');
    expect(chip.querySelector('span')?.className).toContain('block');
    // Capped visually, but the whole question stays reachable both ways.
    expect(chip).toHaveAttribute('title', title);
    expect(chip).toHaveAccessibleName(`support.chat.sourceHint: ${title}`);

    fireEvent.click(chip);
    expect(onSearchFor).toHaveBeenCalledWith(title);
  });

  it('announces the pending state while streaming, and never scroll-hijacks the page', () => {
    state({ streaming: true, answer: 'Open the ' });
    const { container } = render(<HelpChat onSearchFor={vi.fn()} />);

    expect(screen.getByTestId(TEST_IDS.support.chatAnswer)).toBeInTheDocument();
    expect(announcement(container)).toBe('support.chat.thinking');
    // The card sits at the TOP of a long help page: scrolling to the answer's
    // tail on every token drags the page out from under the reader.
    expect(Element.prototype.scrollIntoView).not.toHaveBeenCalled();
  });

  it('announces the finished answer once, from the same region', () => {
    state({ turns: [ANSWERED] });
    const { container } = render(<HelpChat onSearchFor={vi.fn()} />);
    expect(announcement(container)).toBe(ANSWERED.content);
  });

  it('announces the FAILURE — not the previous answer — and shows one live region only', () => {
    state({ turns: [ANSWERED], error: 'help_search failed' });
    const { container } = render(<HelpChat onSearchFor={vi.fn()} />);

    expect(announcement(container)).toBe('support.chat.error');
    // Re-announcing the previous answer would present a stale reply as the one
    // for the question that just failed.
    expect(announcement(container)).not.toContain(ANSWERED.content);

    // The visible row is decoration: a second live region, mounted together
    // with its content, is either dropped or double-spoken.
    const row = screen.getByTestId(TEST_IDS.support.chatError);
    expect(row).not.toHaveAttribute('role');
    expect(row).not.toHaveAttribute('aria-live');
  });

  it('keeps the typed question on failure and clears it only once answered', async () => {
    send.mockResolvedValue(false);
    render(<HelpChat onSearchFor={vi.fn()} />);

    const box = screen.getByTestId(TEST_IDS.support.chatInput);
    fireEvent.change(box, { target: { value: 'how do i export a pdf' } });
    fireEvent.click(screen.getByTestId(TEST_IDS.support.chatAsk));

    await waitFor(() => expect(send).toHaveBeenCalledWith('how do i export a pdf'));
    // A failed question must not have to be retyped from memory.
    expect(box).toHaveValue('how do i export a pdf');

    send.mockResolvedValue(true);
    fireEvent.click(screen.getByTestId(TEST_IDS.support.chatAsk));
    await waitFor(() => expect(box).toHaveValue(''));
  });

  it('does not eat a draft typed while the previous answer was still streaming', async () => {
    // The box is editable during a stream, so the user can start the follow-up
    // before the answer lands. Clearing unconditionally on success wiped it.
    let resolveSend: (answered: boolean) => void = () => {};
    send.mockImplementation(() => new Promise<boolean>((res) => (resolveSend = res)));
    render(<HelpChat onSearchFor={vi.fn()} />);

    const box = screen.getByTestId(TEST_IDS.support.chatInput);
    fireEvent.change(box, { target: { value: 'how do i export a pdf' } });
    fireEvent.click(screen.getByTestId(TEST_IDS.support.chatAsk));
    await waitFor(() => expect(send).toHaveBeenCalledWith('how do i export a pdf'));

    // ...user starts typing the next question while the first one streams.
    fireEvent.change(box, { target: { value: 'and how do i export a docx' } });
    await act(async () => {
      resolveSend(true);
    });

    // The answer landed, but what is in the box was never sent — it survives.
    expect(box).toHaveValue('and how do i export a docx');
  });

  it('offers a retry on the error row that re-sends the last question', async () => {
    state({
      turns: [{ id: 'q-1', role: 'user', content: 'how do i export a pdf' }],
      error: 'boom',
    });
    render(<HelpChat onSearchFor={vi.fn()} />);

    fireEvent.click(screen.getByTestId(TEST_IDS.support.chatRetry));
    await waitFor(() => expect(retry).toHaveBeenCalledTimes(1));
    // Retry re-answers the turn already on screen — it does not re-submit the
    // box, which would ask the same question twice.
    expect(send).not.toHaveBeenCalled();
  });

  it('says the semantic arm is OFF only when the user opted out', () => {
    state({ turns: [{ ...ANSWERED, mode: 'keyword', dense: 'skipped' }] });
    render(<HelpChat onSearchFor={vi.fn()} />);

    expect(screen.getByTestId(TEST_IDS.support.chatKeywordNotice)).toBeInTheDocument();
    expect(screen.getByText('support.chat.keywordNotice')).toBeInTheDocument();
    // The fix is one click away, so the notice carries the deep link.
    expect(screen.getByText('support.chat.keywordAction')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.support.chatDenseNotice)).toBeNull();
  });

  it('says something DIFFERENT when the preference is on and the embedding failed', () => {
    state({ turns: [{ ...ANSWERED, mode: 'keyword', dense: 'unavailable' }] });
    render(<HelpChat onSearchFor={vi.fn()} />);

    // "Semantic scoring is off" would be a lie here: it is on, and it failed.
    expect(screen.getByTestId(TEST_IDS.support.chatDenseNotice)).toBeInTheDocument();
    expect(screen.getByText('support.chat.denseUnavailable')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.support.chatKeywordNotice)).toBeNull();
    // Nothing for the user to switch, so no Settings deep link.
    expect(screen.queryByText('support.chat.keywordAction')).toBeNull();
  });

  it('shows no retrieval notice at all when both arms ran', () => {
    state({ turns: [ANSWERED] });
    render(<HelpChat onSearchFor={vi.fn()} />);
    expect(screen.queryByTestId(TEST_IDS.support.chatKeywordNotice)).toBeNull();
    expect(screen.queryByTestId(TEST_IDS.support.chatDenseNotice)).toBeNull();
  });

  it('caps the question at the wire limit and warns before the box goes silent', () => {
    render(<HelpChat onSearchFor={vi.fn()} />);
    const box = screen.getByTestId(TEST_IDS.support.chatInput);
    // The cap is the one `HelpSearchRequestSchema` states; a longer question is
    // refused at the IPC boundary, so the box must not accept one.
    expect(box).toHaveAttribute('maxlength', '500');

    // Silent while the limit is far away…
    expect(screen.queryByText('support.chat.charsLeft')).toBeNull();
    fireEvent.change(box, { target: { value: 'x'.repeat(480) } });
    expect(screen.getByText('support.chat.charsLeft')).toBeInTheDocument();
  });

  it('Enter sends, Shift+Enter does not, and an IME confirm keypress never submits', () => {
    render(<HelpChat onSearchFor={vi.fn()} />);
    const box = screen.getByTestId(TEST_IDS.support.chatInput);
    fireEvent.change(box, { target: { value: 'how do i export a pdf' } });

    fireEvent.keyDown(box, { key: 'Enter', shiftKey: true });
    expect(send).not.toHaveBeenCalled();

    // Confirming a candidate in a Japanese/Korean/Chinese IME fires Enter with
    // `isComposing` set — submitting there sends a half-typed word.
    fireEvent.keyDown(box, { key: 'Enter', isComposing: true });
    expect(send).not.toHaveBeenCalled();

    fireEvent.keyDown(box, { key: 'Enter' });
    expect(send).toHaveBeenCalledWith('how do i export a pdf');
  });

  it('swaps Ask for Stop while a stream is in flight', () => {
    state({ streaming: true, answer: 'Open the ' });
    render(<HelpChat onSearchFor={vi.fn()} />);

    expect(screen.queryByTestId(TEST_IDS.support.chatAsk)).toBeNull();
    fireEvent.click(screen.getByTestId(TEST_IDS.support.chatStop));
    expect(stop).toHaveBeenCalledTimes(1);
  });
});
