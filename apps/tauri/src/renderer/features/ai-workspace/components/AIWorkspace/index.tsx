import { AlertTriangle, Sparkles, Zap } from 'lucide-react';

import { cn } from '@ajh/ui';

import { ModelSelector, useSelectedModel } from '@/components/ui/ModelSelector';
import { useTranslation } from '@/lib/i18n';
import { useGenerateConfig } from '@/services';
import type { PromptQuality } from '@/store/preferences-schema';
import { usePreferencesStore, usePromptQuality } from '@/store/preferences-store';

import { useChat } from '../../hooks/useChat';
import { ChatInput } from '../ChatInput';
import { MessageList } from '../MessageList';
import { QuickSuggestions } from '../QuickSuggestions';
import { ResumeUploadBar } from '../ResumeUploadBar';

export function AIWorkspace() {
  const { t } = useTranslation();
  const selectedModel = useSelectedModel();
  const generateConfig = useGenerateConfig();
  const promptQuality = usePromptQuality();
  const setPromptQuality = usePreferencesStore((s) => s.setPromptQuality);

  const chat = useChat();

  // Set external dependencies
  chat.setSelectedModel(selectedModel);
  chat.setGenerateConfig(generateConfig);

  return (
    <div className="flex h-full flex-col relative">
      {/* Header */}
      <div className="flex items-center px-4 py-3 border-b border-white/5">
        <h2 className="text-sm font-medium text-foreground/70">{t('ai.title')}</h2>
      </div>

      {/* Model + prompt quality */}
      <div className="px-4 pt-3 pb-2 border-b border-white/5">
        <div className="mb-3">
          <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
            {t('ai.model')}
          </div>
          <ModelSelector className="w-full" />
        </div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
          {t('ai.promptQuality')}
        </div>
        <div className="grid grid-cols-3 gap-1.5">
          {(
            [
              { id: 'full' as PromptQuality, label: 'Full' },
              { id: 'auto' as PromptQuality, label: 'Auto' },
              { id: 'compact' as PromptQuality, label: 'Fast' },
            ] as const
          ).map(({ id, label }) => (
            <button
              key={id}
              type="button"
              onClick={() => setPromptQuality(id)}
              className={cn(
                'flex items-center justify-center gap-1 rounded-lg border py-1.5 text-[11px] font-medium transition-all',
                promptQuality === id
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] bg-white/[0.02] text-foreground/45 hover:border-white/10 hover:text-foreground/70'
              )}
            >
              {id === 'compact' && <Zap size={11} />}
              {label}
            </button>
          ))}
        </div>
        {promptQuality === 'compact' && (
          <div className="mt-2 flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2">
            <Zap size={11} className="text-amber-400 mt-0.5 shrink-0" />
            <p className="text-[10px] text-amber-400/80 leading-relaxed">
              Fast mode — rewrites and detailed suggestions are reduced for speed.
            </p>
          </div>
        )}
        {promptQuality === 'full' && (
          <div className="mt-2 flex items-start gap-2 rounded-lg border border-orange-500/20 bg-orange-500/5 px-3 py-2">
            <AlertTriangle size={11} className="text-orange-400 mt-0.5 shrink-0" />
            <p className="text-[10px] text-orange-400/80 leading-relaxed">
              Full mode on a small model may produce incomplete or noisy output.
            </p>
          </div>
        )}
      </div>

      <div ref={chat.scrollRef} className="flex-1 overflow-y-auto px-8 py-6">
        {chat.messages.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center text-center">
            <div className="glass-elevated mb-5 flex h-14 w-14 items-center justify-center rounded-2xl ring-1 ring-brand/20">
              <Sparkles size={22} className="text-brand-soft" />
            </div>
            <h2 className="text-gradient text-3xl font-bold tracking-tight">{t('nav.ai')}</h2>
            <QuickSuggestions onSelect={chat.setInput} />
          </div>
        ) : (
          <MessageList
            messages={chat.messages}
            streaming={chat.streaming}
            copiedMessageId={chat.copiedMessageId}
            onCopyMessage={chat.copyMessage}
          />
        )}
      </div>

      <div className="border-t border-white/5 p-4">
        {chat.resumeFileName && (
          <ResumeUploadBar fileName={chat.resumeFileName} onRemove={chat.removeResume} />
        )}
        <ChatInput
          value={chat.input}
          onChange={chat.setInput}
          onSend={chat.send}
          onUpload={chat.handleResumeUpload}
          isInputFocused={chat.isInputFocused}
          setIsInputFocused={chat.setIsInputFocused}
          uploading={chat.uploading}
          disabled={chat.streaming}
          fileInputRef={chat.fileInputRef}
        />
      </div>
    </div>
  );
}
