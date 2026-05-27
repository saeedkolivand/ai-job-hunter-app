import { Sparkles } from 'lucide-react';

import { ModelSelector, useSelectedModel } from '@/components/ui/ModelSelector';
import { useTranslation } from '@/lib/i18n';
import { useGenerateConfig } from '@/services';

import { useChat } from '../hooks/useChat';
import { ChatInput } from './ChatInput';
import { MessageList } from './MessageList';
import { QuickSuggestions } from './QuickSuggestions';
import { ResumeUploadBar } from './ResumeUploadBar';

export function AIWorkspace() {
  const { t } = useTranslation();
  const selectedModel = useSelectedModel();
  const generateConfig = useGenerateConfig();

  const chat = useChat();

  // Set external dependencies
  chat.setSelectedModel(selectedModel);
  chat.setGenerateConfig(generateConfig);

  return (
    <div className="flex h-full flex-col relative">
      {/* Header with model selector */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/5">
        <h2 className="text-sm font-medium text-foreground/70">{t('ai.title')}</h2>
        <ModelSelector className="flex items-center gap-2" />
      </div>

      <div ref={chat.scrollRef} className="flex-1 overflow-y-auto px-8 py-6">
        {chat.messages.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center text-center">
            <div className="glass-elevated mb-5 flex h-14 w-14 items-center justify-center rounded-2xl ring-1 ring-brand/20">
              <Sparkles size={22} className="text-brand-soft" />
            </div>
            <h2 className="text-gradient text-2xl font-semibold tracking-tight">{t('nav.ai')}</h2>
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
