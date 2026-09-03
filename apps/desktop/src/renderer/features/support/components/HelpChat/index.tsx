import { AlertTriangle, MessagesSquare, RotateCw, Send, Square } from 'lucide-react';
import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';

import { HelpSearchRequestSchema } from '@ajh/shared/schemas';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, MarkdownMessage, SetupHint, StreamingText, TextArea } from '@ajh/ui';

import { AiSetupHint } from '@/components/ui/AiSetupHint';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { ROUTES } from '@/constants/routes/routes';
import { useHelpChat } from '@/features/support/use-help-chat';
import { useSessionStore } from '@/store/session-store';

interface Props {
  /**
   * Put a help entry's question into the page's own search box — what a "Based
   * on" chip does. Owned by `SupportPage` because the box and the filtered
   * sections are its state; the chat only names the entry it used.
   */
  onSearchFor: (query: string) => void;
}

// The wire cap, read off the schema rather than re-typed: a question longer
// than this is refused at the IPC boundary, so the box must not let one be
// typed in the first place. A second copy of the number is how the two drift.
const QUERY_MAX = HelpSearchRequestSchema.shape.query.maxLength ?? 500;

/** Show the remaining-characters hint only once the cap is actually in view. */
const HINT_FROM_REMAINING = 60;

/**
 * The grounded help assistant above the help page's search box (ADR-043).
 *
 * Answers come from the SAME entries rendered below it — the chat is a way into
 * that corpus, never a replacement for it, which is why every answer carries
 * "Based on" chips back to the entries it used and a caption saying the answer
 * is generated. When retrieval falls back to keyword-only, it says so rather
 * than presenting a lexical match as semantic.
 */
export function HelpChat({ onSearchFor }: Props) {
  const { t } = useTranslation();
  const router = useRouter();
  const setSettings = useSessionStore((s) => s.setSettings);
  const model = useSelectedModel();
  const { canUse, reason } = useCanUseAI();
  const { turns, answer, streaming, error, send, retry, stop } = useHelpChat({ model, canUse });
  const [question, setQuestion] = useState('');

  // Same one-click jump `AiSetupHint` performs: the fix for "semantic scoring
  // is off" lives in the AI settings section, so take the user straight there.
  const openAiSettings = () => {
    setSettings({ activeSection: 'ai' });
    void router.navigate({ to: ROUTES.SETTINGS });
  };

  // The box is cleared only once an answer has actually landed. Clearing it on
  // submit meant a failed question had to be retyped from memory — the one
  // moment the text is hardest to reproduce and most needed.
  //
  // Which makes WHAT is cleared a question of its own: the box stays editable
  // while the answer streams (drafting the follow-up during a slow local model
  // is the point), so by the time this resolves the box may hold a question
  // that was never sent. Clear the submitted text, not "whatever is in the box
  // now" — if the user has typed since, their draft is the newer truth and an
  // unconditional `setQuestion('')` silently eats it.
  const clearIfUntouched = (submitted: string) => (current: string) =>
    current === submitted ? '' : current;

  const submit = () => {
    const submitted = question;
    if (!submitted.trim() || streaming) return;
    void send(submitted).then((answered) => {
      if (answered) setQuestion(clearIfUntouched(submitted));
    });
  };

  // The error row is only on screen when nothing is in flight (starting a run
  // clears `error`), so this needs no streaming guard of its own — `retry`
  // refuses one anyway. Same snapshot rule: a retry re-answers the turn already
  // in the transcript, and the box is free the whole time it runs.
  const retryLast = () => {
    const submitted = question;
    void retry().then((answered) => {
      if (answered) setQuestion(clearIfUntouched(submitted));
    });
  };

  // The most recent answer is the one whose retrieval mode is worth reporting;
  // an older keyword turn is history, not a live caveat.
  const lastAssistant = [...turns].reverse().find((turn) => turn.role === 'assistant');
  // `skipped` is the user's own opt-out and has a fix one click away;
  // `unavailable` means the preference is already ON and the embedding failed,
  // where a "switch it on" link would send the user to a switch already set.
  const denseState = streaming ? undefined : lastAssistant?.dense;
  const semanticOff = denseState === 'skipped';
  const semanticFailed = denseState === 'unavailable';

  const errorText = t('support.chat.error');
  const remaining = QUERY_MAX - question.length;
  const showCounter = remaining <= HINT_FROM_REMAINING;

  // ONE announcement value for the single always-mounted live region. Deriving
  // it here (rather than letting the region fall through to the last answer)
  // is what stops a failed question from re-announcing the PREVIOUS answer as
  // if it were the reply — the region's text changed, so the reader speaks it.
  const announcement = streaming
    ? t('support.chat.thinking')
    : error
      ? errorText
      : (lastAssistant?.content ?? '');

  return (
    <GlassCard className="mb-6 p-5" aria-busy={streaming} data-testid={TEST_IDS.support.chatCard}>
      <div className="mb-1 flex items-center gap-2">
        <MessagesSquare size={14} className="text-brand-soft" />
        <h2 className="text-sm font-semibold text-foreground">{t('support.chat.title')}</h2>
      </div>
      <p className="mb-4 text-xs text-foreground/70">{t('support.chat.subtitle')}</p>

      <AiSetupHint show={!canUse} reason={reason} />

      {turns.length > 0 && (
        <div className="mb-4 space-y-4">
          {turns.map((turn) => (
            <div key={turn.id} data-testid={TEST_IDS.support.chatTurn}>
              <span className="mb-1 block text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/70">
                {turn.role === 'user' ? t('support.chat.you') : t('support.chat.assistant')}
              </span>
              {turn.role === 'user' ? (
                <p className="whitespace-pre-wrap break-words text-sm text-foreground/85">
                  {turn.content}
                </p>
              ) : (
                <MarkdownMessage content={turn.content} />
              )}

              {turn.sources && turn.sources.length > 0 && (
                <div className="mt-2 flex flex-wrap items-center gap-1.5">
                  <span className="text-[11px] text-foreground/70">
                    {t('support.chat.basedOn')}
                  </span>
                  {turn.sources.map((source) => (
                    <Button
                      key={source.id}
                      variant="glass"
                      size="sm"
                      // The chip is a shortcut into the list below, so its
                      // accessible name must say what it DOES, not just repeat
                      // the title it is labelled with.
                      aria-label={`${t('support.chat.sourceHint')}: ${source.title}`}
                      // The visible label is capped, so the full question stays
                      // reachable on hover for a sighted user too.
                      title={source.title}
                      onClick={() => onSearchFor(source.title)}
                      data-testid={TEST_IDS.support.chatSource}
                      // A chip must not outweigh the answer: cap the width, and
                      // start the (truncated) label at the left edge. `truncate`
                      // cannot ellipsize the Button itself — it is an inline-flex
                      // box, so the text simply clipped mid-word; the ellipsis
                      // has to live on a block-level child.
                      className="max-w-[16rem] justify-start text-[11px]"
                    >
                      <span className="block truncate">{source.title}</span>
                    </Button>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {streaming && (
        <div className="mb-4" data-testid={TEST_IDS.support.chatAnswer}>
          <span className="mb-1 block text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/70">
            {t('support.chat.assistant')}
          </span>
          {answer ? (
            // No auto-scroll: this card sits at the TOP of a long scrollable
            // help page, so scrolling to the answer's tail on every token drags
            // the whole page out from under whatever the user was reading — and
            // it is a `smooth` scroll, which no `prefers-reduced-motion` branch
            // here could opt out of. The answer is a few lines; it stays in view.
            <StreamingText text={answer} isStreaming autoScroll={false} />
          ) : (
            <p className="text-sm text-foreground/70">{t('support.chat.thinking')}</p>
          )}
        </div>
      )}

      {/*
        Streaming is silent for a screen reader: text simply appears. The region
        is always mounted and only its TEXT is conditional — a live region
        inserted together with its first message is unreliably announced
        (same treatment as `SupportPage`'s result-count region). Because it is
        the ONLY live region in this card, its value must be stated explicitly
        for every state: pending while streaming, the failure when there is one,
        and otherwise the finished answer once — never every token.
      */}
      <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {announcement}
      </span>

      {error && (
        // Purely visual: the announcement above already carries this text, and
        // a second live region mounted together with its content would either
        // be dropped or double-spoken.
        <div
          className="mb-3 flex items-start gap-2 text-xs text-red-400"
          data-testid={TEST_IDS.support.chatError}
        >
          <AlertTriangle size={13} className="mt-0.5 shrink-0" />
          <span>{errorText}</span>
          <Button
            variant="unstyled"
            onClick={retryLast}
            data-testid={TEST_IDS.support.chatRetry}
            className="ml-auto inline-flex shrink-0 items-center gap-1 rounded px-1 py-0.5 text-brand-soft underline-offset-2 hover:underline"
          >
            <RotateCw size={12} />
            {t('support.chat.retry')}
          </Button>
        </div>
      )}

      {semanticOff && (
        <div data-testid={TEST_IDS.support.chatKeywordNotice}>
          <SetupHint
            tone="amber"
            message={t('support.chat.keywordNotice')}
            actionLabel={t('support.chat.keywordAction')}
            onAction={openAiSettings}
          />
        </div>
      )}

      {semanticFailed && (
        <div data-testid={TEST_IDS.support.chatDenseNotice}>
          {/* No Settings link: the preference is already on, so there is
              nothing there to change — the embedding call simply failed. */}
          <SetupHint tone="amber" message={t('support.chat.denseUnavailable')} />
        </div>
      )}

      <div className="flex items-end gap-2">
        <TextArea
          rows={2}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          maxLength={QUERY_MAX}
          // Enter sends; Shift+Enter is a newline — the convention every chat
          // box in this app uses. The guard matches `submit`'s own so the
          // keyboard path can never do something the button would refuse.
          // `isComposing` keeps an IME's confirm-candidate Enter (Japanese,
          // Korean, Chinese, and Vietnamese telex input) from submitting a
          // half-typed word.
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit();
            }
          }}
          placeholder={t('support.chat.placeholder')}
          aria-label={t('support.chat.ariaLabel')}
          disabled={!canUse}
          data-testid={TEST_IDS.support.chatInput}
          // `min-w-0` so the box is the thing that gives way: a flex item's
          // automatic minimum size is content-based, and a textarea's is its
          // default column count — without this the row can only balance by
          // squeezing the button beside it.
          className="min-w-0"
        />
        {/* `shrink-0 whitespace-nowrap` on both: the label is one word in
            English and two in German ("Frage senden"), and a shrinkable button
            broke it across two lines inside the gradient. The button keeps its
            natural width and the box above yields the difference. */}
        {streaming ? (
          <Button
            variant="default"
            onClick={stop}
            data-testid={TEST_IDS.support.chatStop}
            className="shrink-0 whitespace-nowrap"
          >
            <Square size={13} />
            {t('support.chat.stop')}
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={submit}
            disabled={!canUse || !question.trim()}
            data-testid={TEST_IDS.support.chatAsk}
            className="shrink-0 whitespace-nowrap"
          >
            <Send size={13} />
            {t('support.chat.ask')}
          </Button>
        )}
      </div>

      {/*
        The remaining-characters hint. Always mounted, text conditional: it is a
        live region, and one inserted together with its first message is
        unreliably announced — the same reason the answer region above is
        mounted empty. Silent until the cap is actually in view, because a
        permanent counter on a two-line box is noise.
      */}
      <p className="mt-1 text-[11px] text-foreground/70" aria-live="polite">
        {showCounter ? t('support.chat.charsLeft', { count: remaining }) : ''}
      </p>

      <p className="mt-2 text-[11px] text-foreground/70">{t('support.chat.caption')}</p>
    </GlassCard>
  );
}
