import { Check, FileText } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button, cn } from '@ajh/ui';

import { isTwoColumnTemplate, type TemplateId, TEMPLATES } from '@/lib/generate';

import { COVER_TEMPLATE_PREVIEWS, TEMPLATE_CAPTIONS, TEMPLATE_PREVIEWS } from '../../../samples';

interface StepTemplateProps {
  templateId: TemplateId;
  atsMode: boolean;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
  target?: 'resume' | 'cover' | 'both';
}

export function StepTemplate({
  templateId,
  atsMode,
  onTemplateChange,
  onAtsModeChange,
  target = 'resume',
}: StepTemplateProps) {
  const isCover = target === 'cover';
  const { t } = useTranslation();

  const handleTemplateSelect = (id: TemplateId) => {
    onTemplateChange(id);
    // Single-column templates must never have ATS mode on — reset it.
    // Cover letters have no ATS toggle, so never touch atsMode there.
    if (!isCover && !isTwoColumnTemplate(id)) {
      onAtsModeChange(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Template thumbnail gallery */}
      <div className="grid grid-cols-3 gap-3">
        {Object.values(TEMPLATES).map((tpl) => {
          // 'both' (and 'resume') intentionally use the résumé thumbnails — the template is shared
          // and the résumé is the primary document; only cover-only swaps to cover-letter style previews.
          const image = isCover ? COVER_TEMPLATE_PREVIEWS[tpl.id] : TEMPLATE_PREVIEWS[tpl.id];
          const caption = TEMPLATE_CAPTIONS[tpl.id];
          const selected = templateId === tpl.id;

          return (
            <Button
              key={tpl.id}
              onClick={() => handleTemplateSelect(tpl.id)}
              className={cn(
                'flex flex-col items-start gap-1.5 rounded-xl border p-2 text-left transition-all h-auto',
                selected
                  ? 'border-brand/50 bg-brand/10 ring-2 ring-brand/40'
                  : 'border-white/[0.06] bg-white/[0.02] hover:border-white/10'
              )}
            >
              {/* Thumbnail */}
              <div className="relative w-full rounded-lg overflow-hidden bg-white/[0.03] aspect-[3/4] flex items-center justify-center">
                {image ? (
                  <img
                    src={image}
                    alt={tpl.name}
                    className="w-full h-full object-cover rounded-lg"
                  />
                ) : (
                  <FileText size={20} className="text-foreground/20" />
                )}
                {/* #13 — selected state reads clearly with a check badge. */}
                {selected && (
                  <span className="absolute right-1.5 top-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-brand text-white shadow-sm">
                    <Check size={12} strokeWidth={3} />
                  </span>
                )}
              </div>

              {/* Template name */}
              <span
                className={cn(
                  'text-[10px] font-medium leading-tight w-full text-center',
                  selected ? 'text-foreground/90' : 'text-foreground/50'
                )}
              >
                {tpl.name}
              </span>

              {caption && (
                <span className="text-[9px] text-foreground/30 leading-tight w-full text-center line-clamp-1">
                  {caption}
                </span>
              )}
            </Button>
          );
        })}
      </div>

      {/* ATS safe mode toggle — only relevant for two-column résumé templates */}
      {!isCover && isTwoColumnTemplate(templateId) && (
        <Button
          variant="unstyled"
          type="button"
          role="switch"
          aria-checked={atsMode}
          onClick={() => onAtsModeChange(!atsMode)}
          className={cn(
            'w-full flex items-center justify-between rounded-xl border px-3 py-2.5 transition-all text-left',
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
        </Button>
      )}
    </div>
  );
}
