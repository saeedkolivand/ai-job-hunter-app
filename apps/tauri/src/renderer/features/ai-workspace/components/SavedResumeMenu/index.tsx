import { FileText, Sparkles } from 'lucide-react';
import { createPortal } from 'react-dom';

import type { DocumentRecord } from '@ajh/shared';
import { cn } from '@ajh/ui';

interface Props {
  show: boolean;
  docs: DocumentRecord[];
  menuPos: { top: number; right: number };
  onSelect: (doc: DocumentRecord) => void;
}

export function SavedResumeMenu({ show, docs, menuPos, onSelect }: Props) {
  if (!show) return null;

  return createPortal(
    <div
      style={{
        position: 'fixed',
        top: menuPos.top,
        right: menuPos.right,
        zIndex: 9999,
      }}
      className="min-w-[200px] rounded-xl glass-elevated shadow-2xl overflow-hidden"
    >
      <div className="px-2 py-1.5 space-y-0.5 max-h-48 overflow-y-auto">
        {docs.map((doc) => (
          <button
            key={doc.id}
            onClick={() => void onSelect(doc)}
            className={cn(
              'flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-xs transition-colors',
              doc.isDefault
                ? 'bg-brand/15 text-brand-soft'
                : 'text-foreground/65 hover:bg-white/[0.05] hover:text-foreground/90'
            )}
          >
            <FileText size={11} className="shrink-0" />
            <span className="truncate flex-1">{doc.title}</span>
            {doc.isDefault && <Sparkles size={9} />}
          </button>
        ))}
      </div>
    </div>,
    document.body
  );
}
