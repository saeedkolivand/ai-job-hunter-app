import { FileCheck, FileText, Sparkles } from 'lucide-react';
import { motion } from 'motion/react';

import { Button } from '@ajh/ui';

import { cn } from '@/lib/cn';
import { type GenerationMode, MODES, type TemplateId, TEMPLATES } from '@/lib/generate-ai';
import { useTranslation } from '@/lib/i18n';

interface GenerationConfigProps {
  stage: string;
  mode: GenerationMode;
  target: 'resume' | 'cover' | 'both';
  templateId: TemplateId;
  atsMode: boolean;
  onModeChange: (mode: GenerationMode) => void;
  onTargetChange: (target: 'resume' | 'cover' | 'both') => void;
  onTemplateChange: (templateId: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
  onGenerate: () => void;
  isGenerating: boolean;
}

export function GenerationConfig({
  stage,
  mode,
  target,
  templateId,
  atsMode,
  onModeChange,
  onTargetChange,
  onTemplateChange,
  onAtsModeChange,
  onGenerate,
  isGenerating,
}: GenerationConfigProps) {
  const { t } = useTranslation();

  if (stage !== 'configuring' && stage !== 'done') return null;

  return (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="px-6 pb-4 space-y-4">
      {/* What to generate */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30">
          {t('aiGenerate.generate')}
        </div>
        <div className="grid grid-cols-3 gap-1.5">
          {(
            [
              { id: 'resume' as const, icon: FileText, label: t('aiGenerate.resume') },
              { id: 'cover' as const, icon: FileCheck, label: t('aiGenerate.coverLetter') },
              { id: 'both' as const, icon: Sparkles, label: t('aiGenerate.both') },
            ] as const
          ).map(({ id, icon: Icon, label }) => (
            <Button
              key={id}
              onClick={() => onTargetChange(id)}
              className={cn(
                'flex flex-col items-center gap-1 rounded-lg border py-2.5 text-[11px] font-medium transition-all h-auto',
                target === id
                  ? 'border-brand/40 bg-brand/10 text-brand-soft'
                  : 'border-white/[0.06] bg-white/[0.02] text-foreground/45 hover:border-white/10 hover:text-foreground/70'
              )}
            >
              <Icon size={14} />
              {label}
            </Button>
          ))}
        </div>
      </div>

      {/* Mode */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30">
          {t('aiGenerate.style')}
        </div>
        <div className="space-y-1">
          {(Object.entries(MODES) as [GenerationMode, (typeof MODES)[GenerationMode]][]).map(
            ([id, m]) => (
              <Button
                key={id}
                onClick={() => onModeChange(id)}
                className={cn(
                  'w-full flex items-center gap-3 rounded-lg border px-3 py-2 text-left transition-all h-auto',
                  mode === id
                    ? 'border-brand/35 bg-brand/8 text-foreground/90'
                    : 'border-white/[0.05] bg-transparent text-foreground/50 hover:border-white/[0.08] hover:text-foreground/75'
                )}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] font-medium">{m.label}</div>
                  <div className="text-[10px] text-foreground/35 truncate">{m.description}</div>
                </div>
                {mode === id && <div className="h-1.5 w-1.5 rounded-full bg-brand shrink-0" />}
              </Button>
            )
          )}
        </div>
      </div>

      {/* Template */}
      <div>
        <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30">
          Template
        </div>
        <div className="grid grid-cols-3 gap-1.5">
          {Object.values(TEMPLATES).map((tpl) => (
            <Button
              key={tpl.id}
              onClick={() => onTemplateChange(tpl.id)}
              className={cn(
                'flex flex-col items-center gap-1 rounded-lg border px-2 py-2 text-center transition-all h-auto',
                templateId === tpl.id
                  ? 'border-brand/35 bg-brand/8 text-foreground/90'
                  : 'border-white/[0.05] bg-transparent text-foreground/50 hover:border-white/[0.08] hover:text-foreground/75'
              )}
            >
              <span className="text-[10px] font-medium leading-tight">{tpl.name}</span>
            </Button>
          ))}
        </div>
      </div>

      {/* ATS safe mode — only for two-column template */}
      {templateId === 'two-column' && (
        <button
          type="button"
          onClick={() => onAtsModeChange(!atsMode)}
          className={cn(
            'w-full flex items-center justify-between rounded-lg border px-3 py-2.5 transition-all text-left',
            atsMode
              ? 'border-brand/35 bg-brand/8'
              : 'border-white/[0.05] bg-transparent hover:border-white/[0.08]'
          )}
        >
          <div>
            <div
              className={cn(
                'text-[11px] font-medium',
                atsMode ? 'text-foreground/90' : 'text-foreground/55'
              )}
            >
              {t('aiGenerate.atsMode')}
            </div>
            <div className="text-[10px] text-foreground/35 mt-0.5">
              {t('aiGenerate.atsModeHint')}
            </div>
          </div>
          <div
            className={cn(
              'h-4 w-7 rounded-full transition-colors shrink-0 ml-3 relative',
              atsMode ? 'bg-brand' : 'bg-white/10'
            )}
          >
            <div
              className={cn(
                'absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-transform',
                atsMode ? 'translate-x-3.5' : 'translate-x-0.5'
              )}
            />
          </div>
        </button>
      )}

      <Button
        size="md"
        variant="glass"
        onClick={onGenerate}
        disabled={isGenerating}
        loading={isGenerating}
        className="w-full justify-center transition-all duration-150 ease-out"
      >
        {!isGenerating && <Sparkles size={14} />}
        {isGenerating ? t('aiGenerate.generating') : t('aiGenerate.generate')}
      </Button>
    </motion.div>
  );
}
