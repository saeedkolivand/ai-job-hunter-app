import { Send, Sparkles, Upload } from 'lucide-react';
import { motion } from 'motion/react';

import { Button, cn, Input, transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';

export function ChatInput({
  value,
  onChange,
  onSend,
  onUpload,
  isInputFocused,
  setIsInputFocused,
  uploading,
  disabled,
  fileInputRef,
}: {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onUpload: (file: File) => void;
  isInputFocused: boolean;
  setIsInputFocused: (focused: boolean) => void;
  uploading: boolean;
  disabled: boolean;
  fileInputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const { t } = useTranslation();

  return (
    <motion.div
      className={cn(
        'glass-elevated mx-auto flex max-w-3xl items-center gap-2 rounded-2xl px-4 py-2.5',
        isInputFocused && 'ring-1 ring-brand/20'
      )}
      animate={{
        boxShadow: isInputFocused ? '0 0 20px rgba(192, 132, 252, 0.3)' : 'none',
      }}
      transition={transition.relaxed}
    >
      <Sparkles
        size={15}
        className={cn(
          'transition-colors',
          isInputFocused ? 'text-brand-soft' : 'text-foreground/40'
        )}
      />
      <input
        ref={fileInputRef}
        type="file"
        accept={ACCEPT_ATTR}
        className="hidden"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void onUpload(file);
          e.target.value = '';
        }}
      />
      <Button
        type="button"
        onClick={() => fileInputRef.current?.click()}
        disabled={uploading}
        className={cn(
          'rounded-lg p-2 text-foreground/70 transition-colors hover:bg-white/10 disabled:opacity-40 h-auto bg-transparent border-transparent',
          uploading && 'cursor-wait'
        )}
        aria-label={t('ai.uploadResume')}
      >
        <Upload size={14} className={uploading ? 'animate-pulse' : ''} />
      </Button>
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            void onSend();
          }
        }}
        onFocus={() => setIsInputFocused(true)}
        onBlur={() => setIsInputFocused(false)}
        placeholder={t('ai.placeholder')}
        disabled={disabled}
        variant="default"
        className="flex-1 bg-transparent"
      />
      <Button
        onClick={() => void onSend()}
        disabled={!value.trim() || disabled}
        className="rounded-lg bg-white/5 p-2 text-foreground/70 transition-colors hover:bg-white/10 disabled:opacity-40 h-auto border-transparent"
        aria-label={t('ai.send')}
      >
        <Send size={14} />
      </Button>
    </motion.div>
  );
}
