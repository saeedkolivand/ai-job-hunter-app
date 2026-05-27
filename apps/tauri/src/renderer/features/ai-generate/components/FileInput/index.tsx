import { Check, ChevronDown, Upload } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';

import { Button, cn, TextArea, transition } from '@ajh/ui';

const ACCEPT_ATTR = '.pdf,.docx,.txt,.md,.markdown';

interface FileInputProps {
  label: string;
  icon: React.ElementType;
  value: string;
  onChange: (v: string) => void;
  uploading: boolean;
  onUpload: (file: File) => void;
  disabled?: boolean;
  t: (key: string) => string;
}

export function FileInput({
  label,
  icon: Icon,
  value,
  onChange,
  uploading,
  onUpload,
  disabled,
  t,
}: FileInputProps) {
  const ref = useRef<HTMLInputElement>(null);
  const [expanded, setExpanded] = useState(true);

  return (
    <div
      className={cn(
        'glass-graphite glass-highlight rounded-xl overflow-hidden transition-colors',
        value ? 'border-brand/20' : ''
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2.5">
        <div className="flex items-center gap-2">
          <Icon size={13} className={value ? 'text-brand-soft' : 'text-foreground/30'} />
          <span className="text-xs font-medium text-foreground/70">{label}</span>
          {value && <Check size={11} className="text-emerald-400" />}
        </div>
        <div className="flex items-center gap-2">
          {!disabled && (
            <>
              <input
                ref={ref}
                type="file"
                accept={ACCEPT_ATTR}
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) onUpload(f);
                  e.target.value = '';
                }}
              />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => ref.current?.click()}
                disabled={uploading || disabled}
                className="flex items-center gap-1 rounded-md bg-white/[0.04] px-2 py-1 text-[10px] text-foreground/50 hover:text-foreground/80 transition-colors h-auto"
              >
                <Upload size={10} className={uploading ? 'animate-pulse' : ''} />
                {uploading ? '…' : t('aiGenerate.upload')}
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setExpanded((e) => !e)}
            className="text-foreground/30 hover:text-foreground/60 transition-colors h-auto p-1"
          >
            <ChevronDown
              size={13}
              className={cn('transition-transform', expanded && 'rotate-180')}
            />
          </Button>
        </div>
      </div>

      {/* Textarea */}
      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            initial={{ height: 0 }}
            animate={{ height: 'auto' }}
            exit={{ height: 0 }}
            transition={transition.normal}
            className="overflow-hidden"
          >
            <TextArea
              value={value}
              onChange={(e) => onChange(e.target.value)}
              disabled={disabled}
              placeholder={t('aiGenerate.placeholder').replace('…', '')}
              className="w-full bg-transparent px-3 py-2.5 text-xs text-foreground/75 placeholder:text-foreground/35 font-mono leading-relaxed disabled:opacity-40"
              style={{ height: '140px' }}
              spellCheck={false}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
