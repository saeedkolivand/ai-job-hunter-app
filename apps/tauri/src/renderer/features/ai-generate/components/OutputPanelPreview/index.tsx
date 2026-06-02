import { FileText, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { MarkdownMessage, transition, variants } from '@ajh/ui';

import { type GenerationMode, MODES, type TemplateId, TEMPLATES } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import type { PromptQuality } from '@/store/preferences-schema';

import {
  type GenTarget,
  type PreviewFocus,
  QUALITY_SAMPLES,
  STYLE_SAMPLES,
  TARGET_SAMPLES,
  TEMPLATE_CAPTIONS,
  TEMPLATE_PREVIEWS,
} from '../../samples';

interface Props {
  focus: PreviewFocus;
}

const QUALITY_LABELS: Record<PromptQuality, string> = {
  full: 'Full',
  auto: 'Auto',
  compact: 'Fast',
};

/**
 * Result-panel preview shown while configuring. Renders a generic, illustrative
 * sample of the end result for whichever option was last clicked — a rendered
 * page image for templates, sample wording for styles/target/quality. Never the
 * user's own data.
 *
 * Motion: the whole panel slides+fades in/out as it enters/leaves the result
 * area (root `fadeSlideUp`, keyed `"preview"` by the parent `AnimatePresence`),
 * and each option click slides the previous sample up-and-out, then the next
 * up-and-in (inner `AnimatePresence`, keyed by focus). All on design tokens.
 */
export function OutputPanelPreview({ focus }: Props) {
  const { t } = useTranslation();

  const targetLabels: Record<GenTarget, string> = {
    resume: t('aiGenerate.resume'),
    cover: t('aiGenerate.coverLetter'),
    both: t('aiGenerate.both'),
  };

  let kind: 'image' | 'text' = 'text';
  let label = focus.id;
  let body = '';
  let image: string | undefined;
  let caption: string | undefined;

  switch (focus.group) {
    case 'template': {
      const id = focus.id as TemplateId;
      kind = 'image';
      label = TEMPLATES[id]?.name ?? focus.id;
      image = TEMPLATE_PREVIEWS[id];
      caption = TEMPLATE_CAPTIONS[id];
      break;
    }
    case 'style': {
      const id = focus.id as GenerationMode;
      label = MODES[id]?.label ?? focus.id;
      body = STYLE_SAMPLES[id] ?? '';
      break;
    }
    case 'target': {
      const id = focus.id as GenTarget;
      label = targetLabels[id] ?? focus.id;
      body = TARGET_SAMPLES[id] ?? '';
      break;
    }
    case 'quality': {
      const id = focus.id as PromptQuality;
      label = QUALITY_LABELS[id] ?? focus.id;
      body = QUALITY_SAMPLES[id] ?? '';
      break;
    }
  }

  return (
    <motion.div
      key="preview"
      {...variants.fadeSlideUp}
      transition={transition.relaxed}
      className="flex flex-1 flex-col overflow-hidden"
    >
      {/* Stable anchor — the one constant header while options swap below it */}
      <div className="shrink-0 px-8 pt-8 pb-3">
        <div className="mb-2 inline-flex items-center gap-1 rounded-full bg-brand/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-brand-soft ring-1 ring-brand/20">
          <Sparkles size={11} /> {t('aiGenerate.previewPanel.tag')}
        </div>
        <div className="text-xs text-foreground/35">{t('aiGenerate.previewPanel.disclaimer')}</div>
      </div>

      {/* Per-option block — slides up out, the next slides up in, on each click */}
      <div className="relative flex-1 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.div
            key={`${focus.group}-${focus.id}`}
            {...variants.fadeSlideUp}
            transition={transition.relaxed}
            className="absolute inset-0 flex flex-col overflow-y-auto px-8 pb-8"
          >
            <div className="shrink-0 pb-4">
              <div className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/30">
                {t(`aiGenerate.previewPanel.groups.${focus.group}`)}
              </div>
              <div className="mt-1 text-lg font-semibold text-foreground/80">{label}</div>
            </div>

            {kind === 'image' ? (
              image ? (
                <figure className="mx-auto w-full max-w-[460px]">
                  <img
                    src={image}
                    alt={label}
                    className="w-full rounded-lg ring-1 ring-white/10 shadow-2xl"
                  />
                  {caption && (
                    <figcaption className="mt-3 text-center text-xs text-foreground/45">
                      {caption}
                    </figcaption>
                  )}
                </figure>
              ) : (
                <div className="mx-auto flex w-full max-w-[460px] flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-white/10 bg-white/[0.02] px-6 py-16 text-center">
                  <FileText size={28} className="text-foreground/25" />
                  <div className="text-sm font-medium text-foreground/55">{label}</div>
                  {caption && <div className="text-xs text-foreground/35">{caption}</div>}
                  <div className="text-[11px] text-foreground/25">
                    {t('aiGenerate.previewPanel.imagePending')}
                  </div>
                </div>
              )
            ) : (
              <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] px-5 py-4">
                <MarkdownMessage content={body} />
              </div>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
