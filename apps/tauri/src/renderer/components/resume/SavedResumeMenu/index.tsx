import { Check, FileText, Sparkles } from 'lucide-react';
import { createPortal } from 'react-dom';

import type { DocumentRecord } from '@ajh/shared';
import { Button, cn } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  show: boolean;
  docs: DocumentRecord[];
  menuPos: { top: number; right: number };
  /** Id of the resume currently loaded into the editor, if any. */
  selectedId: string | null;
  onSelect: (doc: DocumentRecord) => void;
  onSetDefault: (doc: DocumentRecord) => void;
  menuRef?: React.RefObject<HTMLDivElement | null>;
}

export function SavedResumeMenu({
  show,
  docs,
  menuPos,
  selectedId,
  onSelect,
  onSetDefault,
  menuRef,
}: Props) {
  const { t } = useTranslation();

  if (!show) return null;

  return createPortal(
    <div
      ref={menuRef}
      style={{
        position: 'fixed',
        top: menuPos.top,
        right: menuPos.right,
        zIndex: 9999,
      }}
      className="min-w-[230px] rounded-xl glass-elevated shadow-2xl overflow-hidden"
    >
      <div className="px-2 py-1.5 space-y-0.5 max-h-48 overflow-y-auto">
        {docs.map((doc) => {
          const isSelected = doc.id === selectedId;
          return (
            <div
              key={doc.id}
              className={cn(
                'flex items-center gap-1 rounded-lg pr-1 transition-colors',
                isSelected ? 'bg-brand/15' : 'hover:bg-white/[0.05]'
              )}
            >
              <Button
                variant="unstyled"
                type="button"
                onClick={() => onSelect(doc)}
                className={cn(
                  'flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2.5 py-2 text-left text-xs transition-colors',
                  isSelected ? 'text-brand-soft' : 'text-foreground/65 hover:text-foreground/90'
                )}
              >
                <FileText size={11} className="shrink-0" />
                <span className="truncate flex-1">{doc.title}</span>
                {isSelected && <Check size={11} className="shrink-0 text-brand-soft" />}
              </Button>

              {doc.isDefault ? (
                <span
                  title={t('resumeInput.default')}
                  className="flex h-6 w-6 shrink-0 items-center justify-center text-amber-400"
                >
                  <Sparkles size={11} />
                </span>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => onSetDefault(doc)}
                  title={t('resumeInput.setDefault')}
                  aria-label={t('resumeInput.setDefault')}
                  className="h-6 w-6 shrink-0 p-0 text-foreground/25 hover:text-amber-400"
                >
                  <Sparkles size={11} />
                </Button>
              )}
            </div>
          );
        })}
      </div>
    </div>,
    document.body
  );
}
