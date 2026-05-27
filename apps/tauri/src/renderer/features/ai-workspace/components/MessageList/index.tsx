import { Check, Copy, Sparkles } from 'lucide-react';
import { motion } from 'motion/react';

import { Button, cn, MarkdownMessage, transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import type { Msg } from '../../hooks/useChat';

export function MessageList({
  messages,
  streaming,
  copiedMessageId,
  onCopyMessage,
}: {
  messages: Msg[];
  streaming: boolean;
  copiedMessageId: string | null;
  onCopyMessage: (messageId: string, content: string) => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4">
      {messages.map((m) => (
        <div key={m.id} className="flex flex-col gap-2">
          <div
            className={cn(
              'glass-card rounded-2xl px-4 py-3',
              m.role === 'user'
                ? 'self-end max-w-[80%] ring-1 ring-brand/20 text-sm leading-relaxed'
                : 'self-start max-w-[85%]'
            )}
          >
            {m.role === 'user' ? (
              m.displayedContent || m.content
            ) : (
              <MarkdownMessage content={m.displayedContent || m.content} />
            )}
          </div>
          {m.role === 'assistant' && !streaming && m.content && (
            <div
              className={cn(
                'flex justify-end',
                m.role === 'assistant' ? 'self-start max-w-[85%]' : ''
              )}
            >
              <Button
                onClick={() => void onCopyMessage(m.id, m.content)}
                className="flex items-center gap-1.5 rounded-md bg-white/5 px-2.5 py-1 text-[11px] text-foreground/70 hover:text-foreground transition-colors h-auto"
                aria-label={t('ai.copyMessage')}
              >
                {copiedMessageId === m.id ? (
                  <Check size={12} className="text-emerald-300" />
                ) : (
                  <Copy size={12} />
                )}
                {copiedMessageId === m.id ? t('ai.copied') : t('ai.copy')}
              </Button>
            </div>
          )}
        </div>
      ))}

      {/* Simple Loading State */}
      {streaming && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="flex items-center gap-3 self-start max-w-[85%]"
        >
          <motion.div
            animate={{
              scale: [1, 1.2, 1],
              opacity: [0.5, 1, 0.5],
            }}
            transition={transition.pulse}
            className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-soft/20"
          >
            <Sparkles size={16} className="text-brand-soft" />
          </motion.div>
          <span className="text-xs text-foreground/50">{t('ai.processing')}</span>
        </motion.div>
      )}
    </div>
  );
}
