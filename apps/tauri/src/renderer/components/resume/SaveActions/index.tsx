import { Loader2, Save, Sparkles } from 'lucide-react';

import { Button } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  fileName: string;
  saving: boolean;
  onSaveToLibrary: () => void;
  onSetDefault: () => void;
}

export function SaveActions({ fileName, saving, onSaveToLibrary, onSetDefault }: Props) {
  const { t } = useTranslation();

  if (saving) {
    return (
      <div className="flex items-center gap-2 border-t border-white/[0.05] px-3 py-2 text-[11px] text-foreground/40">
        <Loader2 size={10} className="animate-spin" /> {t('resumeInput.saving')}
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 border-t border-white/[0.05] px-3 py-2">
      <span className="flex-1 truncate text-[11px] text-foreground/40">{fileName}</span>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => void onSaveToLibrary()}
        className="gap-1 text-[11px] text-foreground/45 hover:text-foreground/80 h-6 px-2"
      >
        <Save size={10} /> {t('resumeInput.saveToLibrary')}
      </Button>
      <Button
        variant="glass"
        size="sm"
        onClick={() => void onSetDefault()}
        className="gap-1 text-[11px] h-6 px-2 ring-1 ring-brand/20"
      >
        <Sparkles size={10} /> {t('resumeInput.setDefault')}
      </Button>
    </div>
  );
}
