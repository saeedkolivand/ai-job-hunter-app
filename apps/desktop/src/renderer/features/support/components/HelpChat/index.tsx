import { AlertTriangle, MessagesSquare, Send, Square } from 'lucide-react';
import { useState } from 'react';
import { useRouter } from '@tanstack/react-router';

import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, GlassCard, MarkdownMessage, SetupHint, StreamingText, TextArea } from '@ajh/ui';

import { AiSetupHint } from '@/components/ui/AiSetupHint';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { ROUTES } from '@/constants/routes/routes';
import { useHelpChat } from '@/hooks/use-help-chat';
import { useSessionStore } from '@/store/session-store';

interface Props {
  /**
   * Put a help entry's question into the page's own search box — what a "Based
   * on" chip does. Owned by `SupportPage` because the box and the filtered
   * sections are its state; the chat only names the entry it used.
   */
  onSearchFor: (query: string) => void;
}

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
  const { turns, answer, streaming, error, send, stop } = useHelpChat({ model, canUse });
  const [question, setQuestion] = useState('');

  // Same one-click jump `AiSetupHint` performs: the fix for "semantic ranking
  // is off" lives in the AI settings section, so take the user straight there.
  const openAiSettings = () => {
    setSettings({ activeSection: 'ai' });
    void router.navigate({ to: ROUTES.SETTINGS });
  };

  const submit = () => {
    if (!question.trim() || streaming) return;
    void send(question);
    setQuestion('');
  };

  // The most recent answer is the one whose retrieval mode is worth reporting;
  // an older keyword turn is history, not a live caveat.
  const lastAssistant = [...turns].reverse().find((turn) => turn.role === 'assistant');
  const keywordOnly = !streaming && lastAssistant?.mode === 'keyword';

  return (
    <GlassCard className="mb-6 p-5" aria-busy={streaming} data-testid={TEST_IDS.support.chatCard}>
      <div className="mb-1 flex items-center gap-2">
        <MessagesSquare size={14} className="text-brand-soft" />
        <h2 className="text-sm font-semibold text-foreground">{t('support.chat.title')}</h2>
      </div>
      <p className="mb-4 text-xs text-foreground/55">{t('support.chat.subtitle')}</p>

      <AiSetupHint show={!canUse} reason={reason} />

      {turns.length > 0 && (
        <div className="mb-4 space-y-4">
          {turns.map((turn) => (
            <div key={turn.id} data-testid={TEST_IDS.support.chatTurn}>
              <span className="mb-1 block text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
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
                  <span className="text-[11px] text-foreground/45">
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
                      onClick={() => onSearchFor(source.title)}
                      data-testid={TEST_IDS.support.chatSource}
                      className="max-w-full truncate text-[11px]"
                    >
                      {source.title}
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
          <span className="mb-1 block text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
            {t('support.chat.assistant')}
          </span>
          {answer ? (
            <StreamingText text={answer} isStreaming />
          ) : (
            <p className="text-sm text-foreground/55">{t('support.chat.thinking')}</p>
          )}
        </div>
      )}

      {/*
        Streaming is silent for a screen reader: text simply appears. The region
        is always mounted and only its TEXT is conditional — a live region
        inserted together with its first message is unreliably announced
        (same treatment as `SupportPage`'s result-count region). It carries the
        pending state while streaming and the finished answer once, rather than
        every token, which would be unusable.
      */}
      <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {streaming ? t('support.chat.thinking') : (lastAssistant?.content ?? '')}
      </span>

      {error && (
        <div
          role="status"
          aria-live="polite"
          className="mb-3 flex items-start gap-2 text-xs text-red-400"
          data-testid={TEST_IDS.support.chatError}
        >
          <AlertTriangle size={13} className="mt-0.5 shrink-0" />
          <span>{t('support.chat.error')}</span>
        </div>
      )}

      {keywordOnly && (
        <div data-testid={TEST_IDS.support.chatKeywordNotice}>
          <SetupHint
            tone="amber"
            message={t('support.chat.keywordNotice')}
            actionLabel={t('support.chat.keywordAction')}
            onAction={openAiSettings}
          />
        </div>
      )}

      <div className="flex items-end gap-2">
        <TextArea
          rows={2}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          // Enter sends; Shift+Enter is a newline — the convention every chat
          // box in this app uses. The guard matches `submit`'s own so the
          // keyboard path can never do something the button would refuse.
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
          placeholder={t('support.chat.placeholder')}
          aria-label={t('support.chat.ariaLabel')}
          disabled={!canUse}
          data-testid={TEST_IDS.support.chatInput}
        />
        {streaming ? (
          <Button variant="default" onClick={stop} data-testid={TEST_IDS.support.chatStop}>
            <Square size={13} />
            {t('support.chat.stop')}
          </Button>
        ) : (
          <Button
            variant="primary"
            onClick={submit}
            disabled={!canUse || !question.trim()}
            data-testid={TEST_IDS.support.chatAsk}
          >
            <Send size={13} />
            {t('support.chat.ask')}
          </Button>
        )}
      </div>

      <p className="mt-2 text-[11px] text-foreground/45">{t('support.chat.caption')}</p>
    </GlassCard>
  );
}
