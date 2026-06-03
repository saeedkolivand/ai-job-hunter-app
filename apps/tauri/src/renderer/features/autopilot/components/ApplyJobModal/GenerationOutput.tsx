import { Check, Copy, Download } from 'lucide-react';

import { Button, cn } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import type { TailorTarget } from './useTailorGeneration';

interface Props {
  target: TailorTarget;
  activeOut: 'resume' | 'cover';
  setActiveOut: (o: 'resume' | 'cover') => void;
  output: string;
  copied: boolean;
  onCopy: () => void;
  exportOpen: boolean;
  setExportOpen: React.Dispatch<React.SetStateAction<boolean>>;
  onExport: (fmt: 'pdf' | 'docx' | 'txt') => void;
}

export function GenerationOutput({
  target,
  activeOut,
  setActiveOut,
  output,
  copied,
  onCopy,
  exportOpen,
  setExportOpen,
  onExport,
}: Props) {
  const { t } = useTranslation();

  return (
    <div className="rounded-lg border border-white/[0.06] bg-white/[0.02]">
      <div className="flex items-center justify-between border-b border-white/[0.06] px-3 py-2">
        {target === 'both' ? (
          <div className="flex items-center gap-1">
            {(['resume', 'cover'] as const).map((o) => (
              <Button
                key={o}
                variant="unstyled"
                type="button"
                onClick={() => setActiveOut(o)}
                className={cn(
                  'rounded px-2 py-0.5 text-[10px] font-medium transition-colors',
                  activeOut === o
                    ? 'bg-brand/15 text-brand-soft'
                    : 'text-foreground/40 hover:text-foreground/70'
                )}
              >
                {o === 'resume'
                  ? t('autopilot.apply.target.resume')
                  : t('autopilot.apply.target.cover')}
              </Button>
            ))}
          </div>
        ) : (
          <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
            {activeOut === 'resume'
              ? t('autopilot.apply.target.resume')
              : t('autopilot.apply.target.cover')}
          </span>
        )}
        <div className="flex items-center gap-3">
          <Button
            onClick={() => void onCopy()}
            disabled={!output}
            className="flex h-auto items-center gap-1 border-transparent bg-transparent p-0 text-[10px] text-foreground/40 hover:text-foreground/70"
          >
            {copied ? <Check size={11} /> : <Copy size={11} />}
            {copied ? t('autopilot.apply.copied') : t('autopilot.apply.copy')}
          </Button>
          <div className="relative">
            <Button
              onClick={() => setExportOpen((o) => !o)}
              disabled={!output}
              className="flex h-auto items-center gap-1 border-transparent bg-transparent p-0 text-[10px] text-brand-soft hover:text-brand-soft/80"
            >
              <Download size={11} />
              {t('aiGenerate.export')}
            </Button>
            {exportOpen && (
              <>
                <div className="fixed inset-0 z-[650]" onClick={() => setExportOpen(false)} />
                <div className="absolute right-0 top-full z-[700] mt-1.5 w-32 overflow-hidden rounded-lg border border-white/10 bg-secondary shadow-2xl">
                  {(['pdf', 'docx', 'txt'] as const).map((fmt) => (
                    <Button
                      key={fmt}
                      variant="unstyled"
                      type="button"
                      onClick={() => void onExport(fmt)}
                      className="flex w-full items-center gap-2 px-3 py-2 text-[11px] text-foreground/65 transition-colors hover:bg-white/[0.05] hover:text-foreground"
                    >
                      <Download size={10} />
                      {t('aiGenerate.download', { fmt: fmt.toUpperCase() })}
                    </Button>
                  ))}
                </div>
              </>
            )}
          </div>
        </div>
      </div>
      <div className="max-h-56 overflow-y-auto whitespace-pre-wrap px-3 py-2 text-[11px] leading-relaxed text-foreground/75">
        {output || '…'}
      </div>
    </div>
  );
}
