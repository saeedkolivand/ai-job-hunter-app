import { RotateCcw } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { type GenerationMeta, MODES } from '@/lib/generate';

interface Props {
  mode: string;
  meta: GenerationMeta | null;
  onRegenerate: () => void;
}

/** Footer strip of the finished-output panel: mode · language caption + the re-generate action. */
export function RegenerateBar({ mode, meta, onRegenerate }: Props) {
  const { t } = useTranslation();
  return (
    <div className="shrink-0 border-t border-[var(--border-clear)] px-6 py-3 flex items-center justify-between">
      <span className="text-[10px] text-foreground/30">
        {MODES[mode as keyof typeof MODES].label} · {meta?.targetLanguage?.toUpperCase() ?? 'EN'}
        {meta?.mismatch && ` · ${t('aiGenerate.localized')}`}
      </span>
      <Button
        onClick={onRegenerate}
        className="flex items-center gap-1.5 text-[11px] text-foreground/40 hover:text-foreground/70 transition-colors h-auto bg-transparent border-transparent"
      >
        <RotateCcw size={11} /> {t('aiGenerate.regenerate')}
      </Button>
    </div>
  );
}
